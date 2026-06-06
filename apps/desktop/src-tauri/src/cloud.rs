//! Team-tier audit sync — client side.
//!
//! Reads unsent rows from the local `audit` table, batches them into an
//! Ed25519-signed envelope, POSTs to the CF Worker at `/audit`, and marks
//! sent rows as uploaded. Runs on an interval from a tokio task spawned
//! in `lib.rs::run()`.
//!
//! The wire format mirrors `apps/api/src/index.ts::AuditPostBody`: two
//! base64 fields, `envelope` (the raw bytes the client signed) and
//! `signature` (detached Ed25519 over those bytes). This deliberately
//! sidesteps JSON-canonicalization pain — server verifies against the
//! literal bytes we sent, not a re-stringified object.
//!
//! Privacy posture: the server never receives raw user text. The client-
//! side `audit.raw_hash` is already SHA-256(raw) and that's all we ship;
//! local-only columns (`prev_hash`, `sig`, the chain proof) stay on the
//! laptop for tamper-evidence.

use std::sync::Arc;
use tokio::sync::Mutex;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};

use crate::store::{Store, UnuploadedAuditRow};

/// Settings-table keys used to persist team config. Values are strings
/// (the settings table is JSON/raw-string keyed) so each one is plain
/// UTF-8 — no nested JSON blobs.
pub const KEY_ENABLED: &str = "team_audit_enabled";
pub const KEY_TEAM_ID: &str = "team_id";
pub const KEY_MEMBER_EMAIL: &str = "team_member_email";
pub const KEY_ENDPOINT: &str = "team_audit_endpoint";
pub const KEY_LAST_UPLOAD_AT: &str = "team_audit_last_upload_at";
/// Keychain service string for the Ed25519 signing key. The private key
/// NEVER touches the SQLite `settings` table — same defense-in-depth as
/// provider API keys. 32 bytes base64-encoded.
pub const KEYCHAIN_SERVICE: &str = "sentynyx";
pub const KEYCHAIN_KEY_NAME: &str = "team_audit_signing_key";

pub const DEFAULT_ENDPOINT: &str = "https://api.sentynyx.com/audit";
/// How many events per POST. Keeps request bodies under ~50 KB at the
/// typical ~300-byte-per-event ceiling and keeps D1 batch inserts fast.
pub const BATCH_SIZE: i64 = 200;
/// Background sync cadence. Short enough that a pilot user gets close to
/// real-time dashboard updates; long enough to not saturate CF Pages.
pub const SYNC_INTERVAL_SEC: u64 = 300;

#[derive(Serialize, Deserialize)]
struct EnvelopeEvent {
    ts: i64,
    kind: String,
    source: String,
    action: String,
    raw_hash: String,
    alias: String,
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    team_id: String,
    member_email: String,
    nonce: String,
    ts: i64,
    events: Vec<EnvelopeEvent>,
}

#[derive(Serialize)]
struct PostBody<'a> {
    envelope: &'a str,
    signature: &'a str,
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("team sync disabled")]
    Disabled,
    #[error("missing team config: {0}")]
    MissingConfig(&'static str),
    #[error("signing key missing or unreadable: {0}")]
    NoSigningKey(String),
    #[error("signing key invalid (expected 32 bytes base64)")]
    InvalidSigningKey,
    #[error("db: {0}")]
    Db(String),
    #[error("http: {0}")]
    Http(String),
    #[error("server rejected batch: {status} {body}")]
    ServerRejected { status: u16, body: String },
}

/// Single outcome of a sync tick. Used both by the periodic task (for
/// logging) and by `team_upload_now` IPC (returned to the frontend so
/// Settings can show "✓ synced 47 events" feedback).
#[derive(Debug, Clone, Serialize)]
pub struct SyncOutcome {
    pub attempted: usize,
    pub uploaded: usize,
    pub skipped_replay: usize,
    pub error: Option<String>,
}

/// Run one sync tick. Returns Ok even when there's nothing to send — only
/// Errs on unrecoverable state (missing config, signing key problems). The
/// periodic task should log and retry; it should NOT panic or exit.
pub async fn sync_once(store: &Arc<Mutex<Store>>) -> Result<SyncOutcome, SyncError> {
    let cfg = load_config(store).await?;
    if !cfg.enabled { return Err(SyncError::Disabled); }

    // Load unsent batch.
    let rows = {
        let s = store.lock().await;
        s.list_unuploaded_audit(BATCH_SIZE)
            .map_err(|e| SyncError::Db(e.to_string()))?
    };
    if rows.is_empty() {
        return Ok(SyncOutcome {
            attempted: 0, uploaded: 0, skipped_replay: 0, error: None,
        });
    }

    // Build + sign the envelope.
    let signing = load_signing_key(&cfg)?;
    let now_sec = now_unix_sec();
    let envelope_bytes = build_envelope(&cfg, &rows, now_sec);
    let signature = signing.sign(&envelope_bytes);
    let post = PostBody {
        envelope: &B64.encode(&envelope_bytes),
        signature: &B64.encode(signature.to_bytes()),
    };

    // Post.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build().map_err(|e| SyncError::Http(e.to_string()))?;
    let res = client.post(&cfg.endpoint)
        .json(&post)
        .send().await
        .map_err(|e| SyncError::Http(e.to_string()))?;
    let status = res.status();
    let body = res.text().await.unwrap_or_default();

    // Handle server outcomes.
    if status.is_success() {
        let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
        let n = {
            let s = store.lock().await;
            s.mark_audit_uploaded(&ids, now_sec)
                .map_err(|e| SyncError::Db(e.to_string()))?
        };
        persist_last_upload_at(store, now_sec).await;
        return Ok(SyncOutcome {
            attempted: rows.len(), uploaded: n, skipped_replay: 0, error: None,
        });
    }
    // 409 = server already has this nonce (replay). Mark the batch as
    // uploaded anyway — the server's dedup means our local state drifted,
    // not that the rows were never shipped. This is the right move to
    // stop re-sending the same batch on every tick forever.
    if status.as_u16() == 409 {
        let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
        let n = {
            let s = store.lock().await;
            s.mark_audit_uploaded(&ids, now_sec)
                .map_err(|e| SyncError::Db(e.to_string()))?
        };
        return Ok(SyncOutcome {
            attempted: rows.len(), uploaded: 0, skipped_replay: n, error: Some("server deduped".into()),
        });
    }

    Err(SyncError::ServerRejected {
        status: status.as_u16(),
        body: body.chars().take(400).collect(),
    })
}

/// Periodic task — runs forever at `SYNC_INTERVAL_SEC`. Spawned from
/// `lib.rs::run()`'s setup block. Logs outcome to stderr; does not panic.
/// Returning Err(Disabled) is expected every tick for users who haven't
/// configured team mode — we just sleep and check again.
pub async fn run_periodic(store: Arc<Mutex<Store>>) {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(SYNC_INTERVAL_SEC));
    tick.tick().await; // first tick fires immediately; skip.
    loop {
        tick.tick().await;
        match sync_once(&store).await {
            Ok(out) if out.attempted > 0 => {
                eprintln!("[cloud] sync: attempted={} uploaded={} skipped_replay={} err={:?}",
                    out.attempted, out.uploaded, out.skipped_replay, out.error);
            }
            Ok(_) => {} // nothing to send — silent.
            Err(SyncError::Disabled) => {} // opt-in off — silent.
            Err(e) => eprintln!("[cloud] sync error: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub struct TeamConfig {
    pub enabled: bool,
    pub team_id: String,
    pub member_email: String,
    pub endpoint: String,
}

pub async fn load_config(store: &Arc<Mutex<Store>>) -> Result<TeamConfig, SyncError> {
    let s = store.lock().await;
    let enabled = read_setting(&s, KEY_ENABLED).as_deref() == Some("1");
    if !enabled {
        return Ok(TeamConfig { enabled: false, team_id: String::new(), member_email: String::new(), endpoint: String::new() });
    }
    let team_id = read_setting(&s, KEY_TEAM_ID)
        .ok_or(SyncError::MissingConfig("team_id"))?;
    let member_email = read_setting(&s, KEY_MEMBER_EMAIL)
        .ok_or(SyncError::MissingConfig("member_email"))?;
    let endpoint = read_setting(&s, KEY_ENDPOINT)
        .unwrap_or_else(|| DEFAULT_ENDPOINT.to_string());
    Ok(TeamConfig { enabled: true, team_id, member_email, endpoint })
}

fn read_setting(s: &Store, key: &str) -> Option<String> {
    s.conn.query_row(
        "SELECT value FROM settings WHERE key=?",
        rusqlite::params![key],
        |r| r.get::<_, String>(0),
    ).ok()
}

async fn persist_last_upload_at(store: &Arc<Mutex<Store>>, now_sec: i64) {
    let s = store.lock().await;
    let _ = s.conn.execute(
        "INSERT INTO settings(key, value) VALUES(?, ?) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![KEY_LAST_UPLOAD_AT, now_sec.to_string()],
    );
}

fn load_signing_key(_cfg: &TeamConfig) -> Result<SigningKey, SyncError> {
    // Read from OS keychain (same backend as provider API keys). For dev /
    // unsigned builds the keyring crate may silently succeed without
    // actually persisting; the `keys::get` helper in keys.rs has a file
    // fallback, but here we only accept the keychain path — team audit
    // signing is a release-build feature.
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_KEY_NAME)
        .map_err(|e| SyncError::NoSigningKey(e.to_string()))?;
    let b64 = entry.get_password()
        .map_err(|e| SyncError::NoSigningKey(e.to_string()))?;
    let bytes = B64.decode(b64.trim()).map_err(|_| SyncError::InvalidSigningKey)?;
    let arr: [u8; 32] = bytes.as_slice().try_into()
        .map_err(|_| SyncError::InvalidSigningKey)?;
    Ok(SigningKey::from_bytes(&arr))
}

fn build_envelope(cfg: &TeamConfig, rows: &[UnuploadedAuditRow], now_sec: i64) -> Vec<u8> {
    use rand::RngCore;
    let mut nonce_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = hex::encode(nonce_bytes);

    let events: Vec<EnvelopeEvent> = rows.iter().map(|r| EnvelopeEvent {
        ts: rfc3339_to_unix(&r.ts),
        kind: r.kind.clone(),
        source: r.source.clone(),
        action: r.action.clone(),
        raw_hash: r.raw_hash.clone(),
        alias: r.alias.clone(),
    }).collect();

    let env = Envelope {
        team_id: cfg.team_id.clone(),
        member_email: cfg.member_email.clone(),
        nonce,
        ts: now_sec,
        events,
    };
    // serde_json::to_vec uses V8-compatible compact encoding. The server
    // verifies the signature against exactly these bytes — it does NOT
    // JSON.stringify anything, so the encoder's quirks are irrelevant.
    serde_json::to_vec(&env).expect("envelope serde must succeed")
}

fn now_unix_sec() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn rfc3339_to_unix(ts: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(ts)
        .map(|dt| dt.timestamp())
        .unwrap_or(0)
}

/// Generate + persist a fresh Ed25519 signing key. Returns the base64
/// public key so the admin can paste it into the server's
/// `POST /admin/teams` request (the server stores the pubkey; the
/// private key never leaves this machine).
pub fn generate_and_persist_signing_key() -> Result<String, SyncError> {
    use rand::rngs::OsRng;
    let sk = SigningKey::generate(&mut OsRng);
    let pk = sk.verifying_key();
    let priv_b64 = B64.encode(sk.to_bytes());
    let pub_b64 = B64.encode(pk.to_bytes());

    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_KEY_NAME)
        .map_err(|e| SyncError::NoSigningKey(e.to_string()))?;
    entry.set_password(&priv_b64)
        .map_err(|e| SyncError::NoSigningKey(e.to_string()))?;
    Ok(pub_b64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_round_trip() {
        let ts = "2026-04-21T20:00:00Z";
        let unix = rfc3339_to_unix(ts);
        assert!(unix > 1_700_000_000 && unix < 2_100_000_000);
    }

    #[test]
    fn rfc3339_invalid_returns_zero() {
        assert_eq!(rfc3339_to_unix("not-a-date"), 0);
    }

    #[test]
    fn build_envelope_shape() {
        let cfg = TeamConfig {
            enabled: true,
            team_id: "t_abc".into(),
            member_email: "alice@halcyon.io".into(),
            endpoint: DEFAULT_ENDPOINT.into(),
        };
        let rows = vec![UnuploadedAuditRow {
            id: "r1".into(),
            ts: "2026-04-21T20:00:00Z".into(),
            kind: "EMAIL".into(),
            raw_hash: "h".into(),
            alias: "⟦email_01⟧".into(),
            action: "ALIAS".into(),
            source: "regex".into(),
        }];
        let bytes = build_envelope(&cfg, &rows, 1712345678);
        let parsed: Envelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed.team_id, "t_abc");
        assert_eq!(parsed.events.len(), 1);
        assert_eq!(parsed.events[0].kind, "EMAIL");
        assert!(!parsed.nonce.is_empty());
    }
}
