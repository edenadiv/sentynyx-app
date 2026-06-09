use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::PathBuf;

use crate::audit::{AuditEntry, make as make_audit};
use crate::vendetta::Span;

pub struct Store { pub(crate) conn: Connection }

#[derive(Serialize, Clone)]
pub struct ConversationRow {
    pub id: String,
    pub title: String,
    pub model_id: String,
    pub created_at: String,
    pub shielded: bool,
}

#[derive(Serialize, Clone)]
pub struct MessageRow {
    pub id: String,
    pub conv_id: String,
    pub role: String,
    pub text_raw: String,
    pub text_aliased: String,
    pub spans: Vec<Span>,
    pub created_at: String,
}

#[derive(Serialize, Clone)]
pub struct AuditMetrics {
    pub redactions_total: i64,
    pub blocks_total: i64,
    pub classes: i64,
    pub redactions_24h: i64,
    pub redactions_7d: i64,
    pub blocks_7d: i64,
}

impl Store {
    pub fn open_default() -> anyhow::Result<Self, rusqlite::Error> {
        let path = default_db_path();
        if let Some(dir) = path.parent() { let _ = std::fs::create_dir_all(dir); }
        let conn = Connection::open(&path)?;
        let s = Store { conn };
        s.migrate()?;
        Ok(s)
    }

    fn migrate(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS conversations (
              id TEXT PRIMARY KEY,
              title TEXT NOT NULL,
              model_id TEXT NOT NULL,
              alias_map_json TEXT NOT NULL DEFAULT '{}',
              counters_json TEXT NOT NULL DEFAULT '{}',
              created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS messages (
              id TEXT PRIMARY KEY,
              conv_id TEXT NOT NULL,
              role TEXT NOT NULL,
              text_raw TEXT NOT NULL,
              text_aliased TEXT NOT NULL,
              spans_json TEXT NOT NULL DEFAULT '[]',
              created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_messages_conv ON messages(conv_id, created_at);
            CREATE TABLE IF NOT EXISTS audit (
              id TEXT PRIMARY KEY,
              ts TEXT NOT NULL,
              kind TEXT NOT NULL,
              raw_hash TEXT NOT NULL,
              alias TEXT NOT NULL,
              action TEXT NOT NULL,
              prev_hash TEXT NOT NULL,
              sig TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_audit_ts ON audit(ts DESC);
            CREATE TABLE IF NOT EXISTS settings (
              key TEXT PRIMARY KEY,
              value TEXT NOT NULL
            );
        "#)?;

        let has_source: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('audit') WHERE name='source'",
            [], |r| r.get(0)
        ).unwrap_or(0);
        if has_source == 0 {
            self.conn.execute(
                "ALTER TABLE audit ADD COLUMN source TEXT NOT NULL DEFAULT 'regex'",
                []
            )?;
        }

        // `uploaded_at` — unix seconds when this row was successfully
        // shipped to the team-tier CF Worker. NULL means "not yet sent".
        // The sync task queries on IS NULL + ORDER BY ts. Idempotent ALTER
        // so upgrades from v0.3 builds don't lose their audit chain.
        let has_uploaded_at: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('audit') WHERE name='uploaded_at'",
            [], |r| r.get(0)
        ).unwrap_or(0);
        if has_uploaded_at == 0 {
            self.conn.execute(
                "ALTER TABLE audit ADD COLUMN uploaded_at INTEGER",
                []
            )?;
            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_audit_uploaded_at ON audit(uploaded_at) WHERE uploaded_at IS NULL",
                []
            )?;
        }
        Ok(())
    }

    pub fn new_conversation(&mut self, id: &str, title: &str, model_id: &str) -> rusqlite::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO conversations(id,title,model_id,alias_map_json,counters_json,created_at) VALUES(?,?,?,'{}','{}',?)",
            params![id, title, model_id, now]
        )?;
        Ok(())
    }

    pub fn load_alias_state(&self, conv_id: &str) -> rusqlite::Result<(crate::vendetta::AliasMap, std::collections::HashMap<String, usize>)> {
        let (amj, cmj): (String, String) = self.conn.query_row(
            "SELECT alias_map_json, counters_json FROM conversations WHERE id=?",
            params![conv_id],
            |r| Ok((r.get(0)?, r.get(1)?))
        ).unwrap_or(("{}".to_string(), "{}".to_string()));
        let am: crate::vendetta::AliasMap = serde_json::from_str(&amj).unwrap_or_default();
        let cm: std::collections::HashMap<String, usize> = serde_json::from_str(&cmj).unwrap_or_default();
        Ok((am, cm))
    }

    pub fn save_alias_state(&mut self, conv_id: &str, map: &crate::vendetta::AliasMap, counters: &std::collections::HashMap<String, usize>) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE conversations SET alias_map_json=?, counters_json=? WHERE id=?",
            params![serde_json::to_string(map).unwrap(), serde_json::to_string(counters).unwrap(), conv_id]
        )?;
        Ok(())
    }

    pub fn list_conversations(&self) -> rusqlite::Result<Vec<ConversationRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.title, c.model_id, c.created_at,
             (SELECT COUNT(1) FROM messages m WHERE m.conv_id=c.id AND m.spans_json<>'[]') > 0
             FROM conversations c ORDER BY c.created_at DESC LIMIT 200"
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(ConversationRow {
                id: r.get(0)?, title: r.get(1)?, model_id: r.get(2)?, created_at: r.get(3)?,
                shielded: r.get::<_, i64>(4)? != 0,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn insert_message(&mut self, m: &MessageRow) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO messages(id,conv_id,role,text_raw,text_aliased,spans_json,created_at) VALUES(?,?,?,?,?,?,?)",
            params![m.id, m.conv_id, m.role, m.text_raw, m.text_aliased,
                serde_json::to_string(&m.spans).unwrap(), m.created_at]
        )?;
        Ok(())
    }

    pub fn load_messages(&self, conv_id: &str) -> rusqlite::Result<Vec<MessageRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,conv_id,role,text_raw,text_aliased,spans_json,created_at FROM messages WHERE conv_id=? ORDER BY created_at ASC"
        )?;
        let rows = stmt.query_map(params![conv_id], |r| {
            let spans_s: String = r.get(5)?;
            Ok(MessageRow {
                id: r.get(0)?, conv_id: r.get(1)?, role: r.get(2)?,
                text_raw: r.get(3)?, text_aliased: r.get(4)?,
                spans: serde_json::from_str(&spans_s).unwrap_or_default(),
                created_at: r.get(6)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn append_audit_for_spans(&mut self, spans: &[Span], action: &str, source: &str) -> rusqlite::Result<Vec<AuditEntry>> {
        let prev: String = self.conn.query_row(
            "SELECT sig FROM audit ORDER BY ts DESC LIMIT 1",
            [], |r| r.get(0)
        ).unwrap_or_else(|_| "genesis".to_string());

        let mut last = prev;
        let mut entries = Vec::new();
        for s in spans {
            let e = make_audit(&last, s.kind.as_str(), &s.raw, &s.alias, action);
            self.conn.execute(
                "INSERT INTO audit(id,ts,kind,raw_hash,alias,action,prev_hash,sig,source) VALUES(?,?,?,?,?,?,?,?,?)",
                params![e.id, e.ts, e.kind, e.raw_hash, e.alias, e.action, e.prev_hash, e.sig, source]
            )?;
            last = e.sig.clone();
            entries.push(e);
        }
        Ok(entries)
    }

    pub fn list_audit(&self, limit: i64) -> rusqlite::Result<Vec<AuditEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,ts,kind,raw_hash,alias,action,prev_hash,sig FROM audit ORDER BY ts DESC LIMIT ?"
        )?;
        let rows = stmt.query_map(params![limit], |r| {
            Ok(AuditEntry {
                id: r.get(0)?, ts: r.get(1)?, kind: r.get(2)?, raw_hash: r.get(3)?,
                alias: r.get(4)?, action: r.get(5)?, prev_hash: r.get(6)?, sig: r.get(7)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn audit_metrics(&self) -> rusqlite::Result<AuditMetrics> {
        let redactions_total: i64 = self.conn.query_row("SELECT COUNT(*) FROM audit WHERE action='ALIAS'", [], |r| r.get(0)).unwrap_or(0);
        let blocks_total: i64 = self.conn.query_row("SELECT COUNT(*) FROM audit WHERE action='BLOCK'", [], |r| r.get(0)).unwrap_or(0);
        let classes: i64 = self.conn.query_row("SELECT COUNT(DISTINCT kind) FROM audit", [], |r| r.get(0)).unwrap_or(0);
        // `ts` is RFC3339 UTC text, so lexicographic >= against another RFC3339
        // UTC cutoff is chronological — no parsing needed in SQL.
        let day_ago = (chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339();
        let week_ago = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
        let windowed = |action: &str, cutoff: &str| -> i64 {
            self.conn.query_row(
                "SELECT COUNT(*) FROM audit WHERE action=? AND ts >= ?",
                params![action, cutoff], |r| r.get(0),
            ).unwrap_or(0)
        };
        Ok(AuditMetrics {
            redactions_total,
            blocks_total,
            classes,
            redactions_24h: windowed("ALIAS", &day_ago),
            redactions_7d: windowed("ALIAS", &week_ago),
            blocks_7d: windowed("BLOCK", &week_ago),
        })
    }

    /// Fetches up to `limit` audit rows that have not yet been shipped to
    /// the team-tier backend (i.e. `uploaded_at IS NULL`). Returned in
    /// insertion order so the chain's `prev_hash` sequence stays consistent
    /// across batches if the server ever needs to verify chain integrity.
    /// Used by the background audit-sync task.
    pub fn list_unuploaded_audit(&self, limit: i64) -> rusqlite::Result<Vec<UnuploadedAuditRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, ts, kind, raw_hash, alias, action, source \
             FROM audit WHERE uploaded_at IS NULL \
             ORDER BY ts ASC LIMIT ?"
        )?;
        let rows = stmt.query_map(params![limit], |r| {
            Ok(UnuploadedAuditRow {
                id: r.get(0)?,
                ts: r.get(1)?,
                kind: r.get(2)?,
                raw_hash: r.get(3)?,
                alias: r.get(4)?,
                action: r.get(5)?,
                source: r.get(6)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Marks a batch of audit rows as successfully uploaded at `now_sec`.
    /// Called after the CF Worker returns 200 on `/audit`. Uses a single
    /// UPDATE with an IN clause rather than one per row to keep latency
    /// bounded even for very large catch-up batches.
    pub fn mark_audit_uploaded(&self, ids: &[String], now_sec: i64) -> rusqlite::Result<usize> {
        if ids.is_empty() { return Ok(0); }
        // Build placeholder list — rusqlite doesn't support array-binding
        // natively with its positional params, so we construct `?,?,?` and
        // bind N+1 params (uploaded_at + the ids).
        let placeholders = std::iter::repeat("?").take(ids.len())
            .collect::<Vec<_>>().join(",");
        let sql = format!(
            "UPDATE audit SET uploaded_at = ? WHERE id IN ({})",
            placeholders
        );
        let mut bind: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(ids.len() + 1);
        bind.push(&now_sec);
        for id in ids { bind.push(id); }
        let rows = self.conn.execute(&sql, rusqlite::params_from_iter(bind.iter().copied()))?;
        Ok(rows)
    }

    pub fn count_unuploaded_audit(&self) -> rusqlite::Result<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM audit WHERE uploaded_at IS NULL",
            [], |r| r.get(0)
        )
    }
}

/// Minimal projection of an `audit` row for the cloud uploader. The server
/// only needs the fields it stores — the chain-signature columns
/// (`prev_hash`, `sig`) stay client-side for local tamper-evidence and
/// aren't shipped (no reason to let the server know them).
#[derive(Debug, Clone)]
pub struct UnuploadedAuditRow {
    pub id: String,
    pub ts: String,       // RFC3339 text (we convert to unix sec in the uploader)
    pub kind: String,
    pub raw_hash: String,
    pub alias: String,
    pub action: String,
    pub source: String,
}

fn default_db_path() -> PathBuf {
    let base = dirs_dir();
    base.join("sentynyx.db")
}

fn dirs_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("SENTYNYX_DATA_DIR") { return PathBuf::from(d); }
    #[cfg(target_os = "macos")]
    { if let Some(h) = std::env::var_os("HOME") { return PathBuf::from(h).join("Library/Application Support/Sentynyx"); } }
    #[cfg(target_os = "windows")]
    { if let Some(a) = std::env::var_os("APPDATA") { return PathBuf::from(a).join("Sentynyx"); } }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    { if let Some(h) = std::env::var_os("HOME") { return PathBuf::from(h).join(".local/share/sentynyx"); } }
    PathBuf::from(".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn open_in(dir: &std::path::Path) -> Store {
        let path = dir.join("test.db");
        let conn = Connection::open(&path).unwrap();
        let s = Store { conn };
        s.migrate().unwrap();
        s
    }

    #[test]
    fn audit_has_source_column() {
        let dir = tempdir().unwrap();
        let s = open_in(dir.path());
        let mut stmt = s.conn.prepare("PRAGMA table_info(audit)").unwrap();
        let cols: Vec<String> = stmt.query_map([], |r| r.get::<_, String>(1))
            .unwrap().collect::<Result<_, _>>().unwrap();
        assert!(cols.contains(&"source".to_string()), "audit.source missing: {:?}", cols);
    }

    #[test]
    fn audit_metrics_windows_by_timestamp() {
        let dir = tempdir().unwrap();
        let s = open_in(dir.path());
        let insert = |ts: String, action: &str| {
            s.conn.execute(
                "INSERT INTO audit (id, ts, kind, raw_hash, alias, action, prev_hash, sig, source) \
                 VALUES (?, ?, 'EMAIL', 'h', 'a', ?, 'p', 's', 'regex')",
                params![uuid::Uuid::new_v4().to_string(), ts, action],
            ).unwrap();
        };
        let now = chrono::Utc::now();
        insert(now.to_rfc3339(), "ALIAS");                                  // in 24h + 7d
        insert((now - chrono::Duration::days(3)).to_rfc3339(), "ALIAS");    // in 7d only
        insert((now - chrono::Duration::days(30)).to_rfc3339(), "ALIAS");   // lifetime only
        insert((now - chrono::Duration::days(2)).to_rfc3339(), "BLOCK");    // block in 7d
        let m = s.audit_metrics().unwrap();
        assert_eq!(m.redactions_total, 3);
        assert_eq!(m.redactions_24h, 1);
        assert_eq!(m.redactions_7d, 2);
        assert_eq!(m.blocks_total, 1);
        assert_eq!(m.blocks_7d, 1);
    }

    #[test]
    fn settings_table_exists() {
        let dir = tempdir().unwrap();
        let s = open_in(dir.path());
        let mut stmt = s.conn.prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='settings'"
        ).unwrap();
        let found: Result<String, _> = stmt.query_row([], |r| r.get(0));
        assert!(found.is_ok(), "settings table missing");
    }

    #[test]
    fn audit_source_backfills_existing_rows() {
        let dir = tempdir().unwrap();
        let s = open_in(dir.path());
        s.conn.execute(
            "INSERT INTO audit(id,ts,kind,raw_hash,alias,action,prev_hash,sig) VALUES('x','t','EMAIL','h','a','ALIAS','p','s')",
            []
        ).unwrap();
        let source: String = s.conn.query_row("SELECT source FROM audit WHERE id='x'", [], |r| r.get(0)).unwrap();
        assert_eq!(source, "regex");
    }
}
