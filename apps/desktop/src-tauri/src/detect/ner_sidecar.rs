//! `NerSidecarDetector` — runs NER in an isolated child process.
//!
//! Why: `spm_precompiled` (transitive tokenizer dep) occasionally panics on
//! certain inputs with an index-out-of-bounds. The panic happens inside a
//! thread that llama-cpp/tokenizers own, and escapes our `catch_unwind`
//! wrappers. In-process, that takes down the tokio worker mid-send.
//!
//! Out-of-process, a panic only kills *one child*. We notice via EOF on
//! stdout, mark the child dead, respawn on the next call. The in-flight
//! request falls back to empty-spans (the existing `commands::send` path
//! already degrades cleanly to regex-only when NER errors).
//!
//! Protocol: line-framed JSON over the child's stdin/stdout. See the
//! `sentynyx-ner` binary for the matching responder.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child as TokioChild, ChildStdin, Command as TokioCommand};
use tokio::sync::{oneshot, Mutex};
use tokio::time::{timeout, Duration};

use crate::vendetta::Span;

use super::{DetectError, Detector, Source};

/// How long to wait for a single NER response at the IPC layer before
/// assuming the sidecar is hung. This is deliberately generous — the 500 ms
/// latency budget in `commands::send` is enforced one layer up via a
/// `tokio::time::timeout` wrapping the whole `detect()` call. We only want
/// this inner timeout to fire on real hangs (child blocked on a syscall,
/// not just on slow inference).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Max number of respawn attempts inside a single `detect` call before we give
/// up and return `Err`. One extra respawn covers the "child died between
/// requests" case; a second death within one call means something's seriously
/// wrong and we should let the caller fall back to regex-only.
const MAX_RESPAWN_PER_CALL: u32 = 1;

/// Discovers the `sentynyx-ner` binary. Tries a few well-known locations in
/// order: an explicit env var, the directory of the current executable, then
/// the conventional Tauri bundle layout.
fn resolve_sidecar_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("SENTYNYX_NER_BIN") {
        return Some(PathBuf::from(p));
    }
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;

    // Release bundle (macOS): Sentynyx.app/Contents/MacOS/sentynyx-ner
    // Dev / cargo run: target/debug/sentynyx-ner  or  target/release/sentynyx-ner
    let name = if cfg!(windows) { "sentynyx-ner.exe" } else { "sentynyx-ner" };
    let here = exe_dir.join(name);
    if here.exists() {
        return Some(here);
    }

    // Fallback when main exe lives in target/debug/deps/ (cargo test)
    let up = exe_dir.parent()?.join(name);
    if up.exists() {
        return Some(up);
    }
    None
}

/// A response we're waiting on: a `oneshot` sender the reader task will
/// complete with the sidecar's decoded answer.
type PendingMap = HashMap<u64, oneshot::Sender<Result<Vec<Span>, String>>>;

/// One running sidecar instance. Owns the child's stdin/stdout and the
/// request/response correlation table.
struct SidecarConn {
    child: TokioChild,
    stdin: ChildStdin,
    pending: Arc<Mutex<PendingMap>>,
    next_id: AtomicU64,
}

impl SidecarConn {
    async fn spawn(path: &std::path::Path) -> Result<Self, DetectError> {
        let mut child = TokioCommand::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| DetectError::Inference(format!("spawn sidecar: {e}")))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| DetectError::Inference("sidecar missing stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| DetectError::Inference("sidecar missing stdout".into()))?;

        let pending: Arc<Mutex<PendingMap>> = Arc::new(Mutex::new(HashMap::new()));
        let pending_reader = Arc::clone(&pending);

        // Reader task: demultiplexes responses by id back to the pending waiters.
        // On EOF or parse error we drain the pending map with errors so callers
        // see "sidecar died" rather than hanging.
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break, // child exited
                    Ok(_) => {}
                    Err(_) => break,
                }
                if line.trim().is_empty() {
                    continue;
                }
                let parsed: serde_json::Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => continue, // corrupt line; keep reading
                };
                let Some(id) = parsed.get("id").and_then(|v| v.as_u64()) else { continue };
                let result = if let Some(err) = parsed.get("error").and_then(|v| v.as_str()) {
                    Err(err.to_string())
                } else if let Some(spans) = parsed.get("spans") {
                    match serde_json::from_value::<Vec<Span>>(spans.clone()) {
                        Ok(s) => Ok(s),
                        Err(e) => Err(format!("decode spans: {e}")),
                    }
                } else {
                    Err("response missing spans or error".into())
                };
                if let Some(tx) = pending_reader.lock().await.remove(&id) {
                    let _ = tx.send(result);
                }
            }
            // Reader exited — drain all pending with an error so in-flight
            // detect() calls unblock.
            let mut map = pending_reader.lock().await;
            for (_, tx) in map.drain() {
                let _ = tx.send(Err("sidecar reader exited".into()));
            }
        });

        Ok(Self {
            child,
            stdin,
            pending,
            next_id: AtomicU64::new(1),
        })
    }

    async fn request(&mut self, text: &str) -> Result<Vec<Span>, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let line = format!(
            "{{\"id\":{id},\"text\":{}}}\n",
            serde_json::to_string(text)
                .unwrap_or_else(|_| "\"\"".to_string())
        );
        if let Err(e) = self.stdin.write_all(line.as_bytes()).await {
            self.pending.lock().await.remove(&id);
            return Err(format!("sidecar write: {e}"));
        }
        if let Err(e) = self.stdin.flush().await {
            self.pending.lock().await.remove(&id);
            return Err(format!("sidecar flush: {e}"));
        }

        match timeout(REQUEST_TIMEOUT, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("sidecar response channel closed".into()),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err("sidecar response timeout".into())
            }
        }
    }

    /// Best-effort health check: has the child exited?
    fn is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,
            _ => false,
        }
    }
}

/// Parent-side handle. Holds one lazy-spawned sidecar. Recovers by respawning
/// on the next call after a crash.
pub struct NerSidecarDetector {
    conn: Mutex<Option<SidecarConn>>,
    sidecar_path: Option<PathBuf>,
    /// Seconds-since-UNIX-epoch of the last successful detect call. Written on
    /// every request, read by the idle-unload supervisor task.
    last_use_at: AtomicU64,
}

impl NerSidecarDetector {
    pub fn new() -> Self {
        Self {
            conn: Mutex::new(None),
            sidecar_path: resolve_sidecar_path(),
            last_use_at: AtomicU64::new(0),
        }
    }

    async fn ensure_conn(&self) -> Result<(), DetectError> {
        let mut guard = self.conn.lock().await;
        if let Some(conn) = guard.as_mut() {
            if conn.is_alive() {
                return Ok(());
            }
            *guard = None;
        }
        let path = self
            .sidecar_path
            .as_deref()
            .ok_or_else(|| DetectError::ModelNotLoaded("ner sidecar binary not found".into()))?;
        let conn = SidecarConn::spawn(path).await?;
        *guard = Some(conn);
        Ok(())
    }

    /// Returns the seconds-since-epoch of the last successful detect call, or
    /// 0 if the detector has never served a request. Used by the supervisor
    /// task in `lib.rs` to decide whether to unload the sidecar after idle.
    pub fn last_use_at(&self) -> u64 {
        self.last_use_at.load(Ordering::Relaxed)
    }

    /// Explicitly drop the live sidecar child process. Next `detect()` call
    /// spawns a fresh one. Safe to call even if no conn exists.
    pub async fn unload(&self) {
        let mut guard = self.conn.lock().await;
        *guard = None; // Drop -> kill_on_drop(true) SIGTERMs the child.
    }
}

impl Default for NerSidecarDetector {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl Detector for NerSidecarDetector {
    fn source(&self) -> Source { Source::Ner }

    async fn detect(&self, text: &str) -> Result<Vec<Span>, DetectError> {
        for attempt in 0..=MAX_RESPAWN_PER_CALL {
            self.ensure_conn().await?;
            let mut guard = self.conn.lock().await;
            let conn = match guard.as_mut() {
                Some(c) => c,
                None => continue,
            };
            match conn.request(text).await {
                Ok(spans) => {
                    self.last_use_at.store(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0),
                        Ordering::Relaxed,
                    );
                    return Ok(spans);
                }
                Err(e) => {
                    // On the last attempt, surface the error so caller can
                    // degrade to regex-only. Otherwise drop the dead conn
                    // and try once more.
                    *guard = None;
                    if attempt == MAX_RESPAWN_PER_CALL {
                        return Err(DetectError::Inference(e));
                    }
                }
            }
        }
        Err(DetectError::Inference("sidecar respawn loop exhausted".into()))
    }
}

// Sidecar cleanup: `TokioChild` with `kill_on_drop(true)` handles SIGTERMing
// the child when a `SidecarConn` is dropped, so we don't need a custom Drop.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_sidecar_respects_env_var() {
        let _g = crate::models::tests::ENV_GUARD.lock().unwrap();
        let sentinel = "/tmp/not-a-real-binary-for-sentynyx-test";
        std::env::set_var("SENTYNYX_NER_BIN", sentinel);
        let resolved = resolve_sidecar_path();
        std::env::remove_var("SENTYNYX_NER_BIN");
        assert_eq!(resolved.as_deref().and_then(|p| p.to_str()), Some(sentinel));
    }

    #[tokio::test]
    async fn detect_errors_cleanly_when_sidecar_binary_is_missing() {
        let _g = crate::models::tests::ENV_GUARD.lock().unwrap();
        std::env::set_var("SENTYNYX_NER_BIN", "/definitely/does/not/exist/sentynyx-ner");
        let det = NerSidecarDetector::new();
        let result = det.detect("hello").await;
        std::env::remove_var("SENTYNYX_NER_BIN");
        assert!(result.is_err(), "expected failure when sidecar path is bogus");
    }
}
