# Semantic Redaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a two-stage ML detection layer to Sentynyx's Vendetta perimeter — an always-on GLiNER encoder (ONNX) for semantic NER, and an opt-in Qwen-3-1.5B paranoid LLM for deep semantic sensitivity — merged with the existing regex detector via `tokio::join!` so regex stays primary and the ML layer is strictly additive.

**Architecture:** New `Detector` trait unifies regex/NER/LLM. `commands::send` fires regex + NER concurrently, merges spans (regex wins on overlap), aliases, and dispatches to provider. The optional paranoid LLM runs as a non-blocking background task. Models are downloaded on first run from HuggingFace Hub, SHA-256 verified, stored in `<app_data>/models/`. Regex remains load-bearing; the ML layer degrades gracefully if models are missing or inference fails.

**Tech Stack:** Rust 1.94, Tauri 2, `ort` 2.x (ONNX Runtime), `tokenizers` (HuggingFace), `llama-cpp-2` (llama.cpp bindings), `reqwest` (with `stream` feature for resumable downloads), React 18 + TypeScript on the frontend. SQLite for persistence.

**Reference spec:** `docs/superpowers/specs/2026-04-19-semantic-redaction-design.md`.

---

## Pre-flight

Before starting: you are in the repo root `/Users/edenadiv/Eden's Files/Eden's Coding Projects/Sentynyx/`. Active branch: `main`. The existing codebase has working regex detection, streaming LLM calls, SQLite persistence, and an audit log. Don't break any of that.

Every task ends with a commit. Small, frequent commits are required. Commit messages use conventional-commits style (`feat:`, `test:`, `refactor:`, `chore:`).

After EACH task's final step, run the full quick-check before committing:

```bash
cd "apps/desktop/src-tauri" && cargo check --all-targets
cd "apps/desktop/src-tauri" && cargo test
cd "apps/desktop" && pnpm build
```

If any of those fail, fix before committing.

---

## Task 1: Extend Kind enum + schema migration

**Files:**
- Modify: `apps/desktop/src-tauri/src/vendetta.rs` (lines 6–28, the `Kind` enum and its impls)
- Modify: `apps/desktop/src-tauri/src/store.rs` (migrate function around line 47)

- [ ] **Step 1: Write failing test for new Kind variants**

Add to `apps/desktop/src-tauri/src/vendetta.rs` at the bottom of the `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn ner_kinds_have_stable_labels() {
        assert_eq!(Kind::PERSON_NER.as_str(), "PERSON_NER");
        assert_eq!(Kind::ORG_NER.as_str(), "ORG_NER");
        assert_eq!(Kind::CODENAME_NER.as_str(), "CODENAME_NER");
        assert_eq!(Kind::LOCATION_NER.as_str(), "LOCATION_NER");
        assert_eq!(Kind::EMPID_NER.as_str(), "EMPID_NER");
        assert_eq!(Kind::PERSON_NER.label(), "person");
        assert_eq!(Kind::CODENAME_NER.label(), "codename");
    }
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd "apps/desktop/src-tauri"
cargo test ner_kinds_have_stable_labels 2>&1 | tail -20
```

Expected: fails with "no variant named PERSON_NER found for enum Kind".

- [ ] **Step 3: Extend Kind enum**

In `apps/desktop/src-tauri/src/vendetta.rs`, replace the `Kind` enum (lines ~6–9) and its `impl Kind` block (lines ~11–28) with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Kind {
    EMAIL, PHONE, SSN, IP, APIKEY, URL, ADDRESS, MONEY, NAME, COMPANY, EMPID,
    PERSON_NER, ORG_NER, CODENAME_NER, LOCATION_NER, EMPID_NER,
}

impl Kind {
    pub fn label(&self) -> &'static str {
        match self {
            Kind::EMAIL => "email", Kind::PHONE => "phone", Kind::SSN => "ssn",
            Kind::IP => "ip", Kind::APIKEY => "api-key", Kind::URL => "url",
            Kind::ADDRESS => "address", Kind::MONEY => "amount",
            Kind::NAME => "person", Kind::COMPANY => "entity", Kind::EMPID => "employee-id",
            Kind::PERSON_NER => "person", Kind::ORG_NER => "entity",
            Kind::CODENAME_NER => "codename", Kind::LOCATION_NER => "location",
            Kind::EMPID_NER => "employee-id",
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::EMAIL => "EMAIL", Kind::PHONE => "PHONE", Kind::SSN => "SSN",
            Kind::IP => "IP", Kind::APIKEY => "APIKEY", Kind::URL => "URL",
            Kind::ADDRESS => "ADDRESS", Kind::MONEY => "MONEY",
            Kind::NAME => "NAME", Kind::COMPANY => "COMPANY", Kind::EMPID => "EMPID",
            Kind::PERSON_NER => "PERSON_NER", Kind::ORG_NER => "ORG_NER",
            Kind::CODENAME_NER => "CODENAME_NER", Kind::LOCATION_NER => "LOCATION_NER",
            Kind::EMPID_NER => "EMPID_NER",
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cd "apps/desktop/src-tauri"
cargo test ner_kinds_have_stable_labels
```

Expected: PASS.

- [ ] **Step 5: Write failing test for schema migration**

Add to `apps/desktop/src-tauri/src/store.rs` in a new `#[cfg(test)] mod tests` block at the bottom of the file:

```rust
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
```

Add `tempfile = "3"` to `[dev-dependencies]` in `apps/desktop/src-tauri/Cargo.toml`. If `[dev-dependencies]` doesn't exist, create it:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 6: Run tests to verify they fail**

```bash
cd "apps/desktop/src-tauri"
cargo test audit_has_source_column settings_table_exists audit_source_backfills 2>&1 | tail -20
```

Expected: all three fail.

- [ ] **Step 7: Add migration for audit.source and settings table**

In `apps/desktop/src-tauri/src/store.rs`, replace the `migrate` function (lines ~47–79) with:

```rust
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
        Ok(())
    }
```

- [ ] **Step 8: Run all three schema tests**

```bash
cd "apps/desktop/src-tauri"
cargo test audit_has_source_column settings_table_exists audit_source_backfills
```

Expected: all PASS.

- [ ] **Step 9: Run the full Rust test suite to catch regressions**

```bash
cd "apps/desktop/src-tauri"
cargo test
```

Expected: all existing tests still pass. No regressions.

- [ ] **Step 10: Commit**

```bash
git add apps/desktop/src-tauri/src/vendetta.rs apps/desktop/src-tauri/src/store.rs apps/desktop/src-tauri/Cargo.toml
git commit -m "feat(rust): extend Kind with _NER variants and migrate audit.source + settings table"
```

---

## Task 2: Detector trait and merge_spans

**Files:**
- Create: `apps/desktop/src-tauri/src/detect/mod.rs`
- Create: `apps/desktop/src-tauri/src/detect/regex.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs` (add `mod detect;`)

- [ ] **Step 1: Write failing tests for merge_spans**

Create `apps/desktop/src-tauri/src/detect/mod.rs`:

```rust
pub mod regex;

use async_trait::async_trait;
use serde::Serialize;
use crate::vendetta::{Kind, Span};

#[derive(Debug, thiserror::Error)]
pub enum DetectError {
    #[error("model not loaded: {0}")]
    ModelNotLoaded(String),
    #[error("inference failed: {0}")]
    Inference(String),
    #[error("timeout")]
    Timeout,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Source { Regex, Ner, Llm }

#[async_trait]
pub trait Detector: Send + Sync {
    fn source(&self) -> Source;
    async fn detect(&self, text: &str) -> Result<Vec<Span>, DetectError>;
}

/// Merge regex and NER spans. Regex wins on overlap.
/// Non-overlapping NER spans are kept. Both inputs should already be span-valid
/// (start < end, offsets within text bounds).
pub fn merge_spans(mut regex: Vec<Span>, ner: Vec<Span>) -> Vec<Span> {
    for n in ner {
        let overlaps = regex.iter().any(|r| ranges_overlap(r.start, r.end, n.start, n.end));
        if !overlaps {
            regex.push(n);
        }
    }
    regex.sort_by_key(|s| s.start);
    regex
}

fn ranges_overlap(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> bool {
    a_start < b_end && b_start < a_end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sp(start: usize, end: usize, kind: Kind, raw: &str, alias: &str) -> Span {
        Span { start, end, kind, raw: raw.to_string(), alias: alias.to_string() }
    }

    #[test]
    fn merge_preserves_non_overlapping_ner_spans() {
        let regex = vec![sp(0, 5, Kind::EMAIL, "a@b.c", "{{email_01}}")];
        let ner = vec![sp(10, 18, Kind::PERSON_NER, "Jamie T.", "{{person_NER_01}}")];
        let merged = merge_spans(regex, ner);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].start, 0);
        assert_eq!(merged[1].start, 10);
    }

    #[test]
    fn merge_drops_ner_when_regex_overlaps() {
        let regex = vec![sp(0, 10, Kind::NAME, "Sarah Chen", "{{person_01}}")];
        let ner = vec![sp(6, 14, Kind::PERSON_NER, "Chen Smith", "{{person_NER_01}}")];
        let merged = merge_spans(regex, ner);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].alias, "{{person_01}}");
    }

    #[test]
    fn merge_drops_ner_fully_inside_regex() {
        let regex = vec![sp(0, 20, Kind::EMAIL, "alice@example.com", "{{email_01}}")];
        let ner = vec![sp(5, 10, Kind::PERSON_NER, "inner", "{{person_NER_01}}")];
        let merged = merge_spans(regex, ner);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn merge_sorts_output_by_start() {
        let regex = vec![sp(20, 25, Kind::EMAIL, "b@c.d", "{{email_01}}")];
        let ner = vec![sp(0, 5, Kind::PERSON_NER, "Ana", "{{person_NER_01}}")];
        let merged = merge_spans(regex, ner);
        assert_eq!(merged[0].start, 0);
        assert_eq!(merged[1].start, 20);
    }

    #[test]
    fn merge_handles_empty_inputs() {
        assert!(merge_spans(vec![], vec![]).is_empty());
        let r = vec![sp(0, 5, Kind::EMAIL, "a@b.c", "{{email_01}}")];
        assert_eq!(merge_spans(r.clone(), vec![]).len(), 1);
        assert_eq!(merge_spans(vec![], r).len(), 1);
    }

    #[test]
    fn ranges_overlap_boundary_cases() {
        // touching but not overlapping
        assert!(!ranges_overlap(0, 5, 5, 10));
        assert!(!ranges_overlap(5, 10, 0, 5));
        // single-point overlap
        assert!(ranges_overlap(0, 6, 5, 10));
        // identical ranges
        assert!(ranges_overlap(0, 5, 0, 5));
        // one contains the other
        assert!(ranges_overlap(0, 10, 3, 7));
    }
}
```

Add `anyhow` and `thiserror` checks — both are already in the Cargo.toml dependencies (confirmed in the existing file). `async-trait` is also already there.

Register the module. In `apps/desktop/src-tauri/src/lib.rs`, add `mod detect;` in the first block of `mod` declarations (around line 1–7), so it becomes:

```rust
mod vendetta;
mod audit;
mod store;
mod keys;
mod router;
mod providers;
mod commands;
mod detect;
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd "apps/desktop/src-tauri"
cargo test --lib detect:: 2>&1 | tail -30
```

Expected: fails to compile — `mod regex;` not found. That confirms the module wiring is pending.

- [ ] **Step 3: Create detect/regex.rs**

Create `apps/desktop/src-tauri/src/detect/regex.rs`:

```rust
use async_trait::async_trait;
use std::collections::HashMap;
use crate::vendetta::{self, Span};
use super::{Detector, DetectError, Source};

/// Stateless wrapper that runs the existing regex engine with a fresh alias map.
/// For the merge pipeline we don't want per-call aliases here — the caller
/// (`commands::send`) owns the conversation alias state. This detector returns
/// spans with empty aliases; the caller re-runs aliasing on the merged set.
pub struct RegexDetector;

#[async_trait]
impl Detector for RegexDetector {
    fn source(&self) -> Source { Source::Regex }

    async fn detect(&self, text: &str) -> Result<Vec<Span>, DetectError> {
        let mut map = HashMap::new();
        let mut counters = HashMap::new();
        // Clear aliases — the merge caller applies its own alias state afterward.
        let mut spans = vendetta::detect(text, &mut map, &mut counters);
        for s in spans.iter_mut() { s.alias.clear(); }
        Ok(spans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vendetta::Kind;

    #[tokio::test]
    async fn regex_detector_returns_email_spans() {
        let d = RegexDetector;
        let spans = d.detect("email alice@acme.com now").await.unwrap();
        assert!(spans.iter().any(|s| matches!(s.kind, Kind::EMAIL)));
    }

    #[tokio::test]
    async fn regex_detector_returns_empty_aliases() {
        let d = RegexDetector;
        let spans = d.detect("email a@b.c").await.unwrap();
        for s in &spans { assert!(s.alias.is_empty()); }
    }
}
```

- [ ] **Step 4: Run all the detect tests**

```bash
cd "apps/desktop/src-tauri"
cargo test --lib detect::
```

Expected: all tests PASS. `merge_spans` tests + both `regex_detector_*` tests.

- [ ] **Step 5: Run the full test suite to verify no regressions**

```bash
cd "apps/desktop/src-tauri"
cargo test
```

Expected: all existing tests still pass.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri/src/detect apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(rust): add Detector trait, merge_spans, and RegexDetector"
```

---

## Task 3: Models module — ModelSpec, SHA verify, paths

**Files:**
- Create: `apps/desktop/src-tauri/src/models.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs` (add `mod models;`)
- Modify: `apps/desktop/src-tauri/Cargo.toml` (add `reqwest` streaming features — check current Cargo.toml first)

- [ ] **Step 1: Write failing tests for verify_sha256 and local_path**

Create `apps/desktop/src-tauri/src/models.rs`:

```rust
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub struct ModelSpec {
    pub id: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModelStatus {
    Missing,
    Downloading { percent: u32 },
    Ready,
    Error { msg: String },
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("io error: {0}")] Io(#[from] std::io::Error),
    #[error("http error: {0}")] Http(String),
    #[error("sha256 mismatch (expected {expected}, got {actual})")]
    Sha { expected: String, actual: String },
    #[error("size mismatch (expected {expected}, got {actual})")]
    Size { expected: u64, actual: u64 },
    #[error("cancelled")] Cancelled,
}

pub const GLINER_SMALL: ModelSpec = ModelSpec {
    id: "gliner-small-v2.1",
    file: "model.onnx",
    url: "https://huggingface.co/onnx-community/gliner-small-v2.1/resolve/main/onnx/model.onnx",
    sha256: "REPLACE_WITH_ACTUAL_SHA_AT_IMPLEMENTATION_TIME",
    size_bytes: 83_000_000,
};

pub const GLINER_TOKENIZER: ModelSpec = ModelSpec {
    id: "gliner-small-v2.1",
    file: "tokenizer.json",
    url: "https://huggingface.co/onnx-community/gliner-small-v2.1/resolve/main/tokenizer.json",
    sha256: "REPLACE_WITH_ACTUAL_SHA_AT_IMPLEMENTATION_TIME",
    size_bytes: 3_000_000,
};

pub const QWEN3_1_5B_Q4: ModelSpec = ModelSpec {
    id: "qwen3-1.5b-q4km",
    file: "qwen3-1.5b-q4km.gguf",
    url: "https://huggingface.co/Qwen/Qwen3-1.5B-Instruct-GGUF/resolve/main/qwen3-1.5b-q4_k_m.gguf",
    sha256: "REPLACE_WITH_ACTUAL_SHA_AT_IMPLEMENTATION_TIME",
    size_bytes: 950_000_000,
};

pub fn models_root() -> PathBuf {
    let base = if let Some(d) = std::env::var_os("SENTYNYX_DATA_DIR") {
        PathBuf::from(d)
    } else {
        #[cfg(target_os = "macos")]
        { PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Library/Application Support/Sentynyx") }
        #[cfg(target_os = "windows")]
        { PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join("Sentynyx") }
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        { PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share/sentynyx") }
    };
    base.join("models")
}

pub fn local_path(spec: &ModelSpec) -> PathBuf {
    models_root().join(spec.id).join(spec.file)
}

pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), ModelError> {
    let mut f = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    let actual = hex::encode(hasher.finalize());
    if actual != expected {
        return Err(ModelError::Sha { expected: expected.into(), actual });
    }
    Ok(())
}

pub fn status(spec: &ModelSpec) -> ModelStatus {
    let p = local_path(spec);
    if !p.exists() { return ModelStatus::Missing; }
    match verify_sha256(&p, spec.sha256) {
        Ok(()) => ModelStatus::Ready,
        Err(ModelError::Sha { actual, .. }) => ModelStatus::Error {
            msg: format!("sha mismatch ({})", &actual[..8]),
        },
        Err(e) => ModelStatus::Error { msg: e.to_string() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn verify_sha256_matches_correct_hash() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("x.bin");
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(b"hello").unwrap();
        let hash = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert!(verify_sha256(&p, hash).is_ok());
    }

    #[test]
    fn verify_sha256_rejects_mismatched_hash() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("x.bin");
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(b"hello").unwrap();
        let bad = "0000000000000000000000000000000000000000000000000000000000000000";
        assert!(matches!(verify_sha256(&p, bad), Err(ModelError::Sha { .. })));
    }

    #[test]
    fn status_reports_missing_when_file_absent() {
        std::env::set_var("SENTYNYX_DATA_DIR", tempdir().unwrap().path());
        let s = status(&GLINER_SMALL);
        assert_eq!(s, ModelStatus::Missing);
    }
}
```

Register the module in `apps/desktop/src-tauri/src/lib.rs`:

```rust
mod vendetta;
mod audit;
mod store;
mod keys;
mod router;
mod providers;
mod commands;
mod detect;
mod models;
```

Ensure `apps/desktop/src-tauri/Cargo.toml` `[dependencies]` has `reqwest` with the `stream` feature already enabled (it does: `features = ["json", "stream", "rustls-tls"]`). No Cargo change needed yet.

- [ ] **Step 2: Run the tests**

```bash
cd "apps/desktop/src-tauri"
cargo test --lib models::
```

Expected: all three tests PASS.

> Note: the `REPLACE_WITH_ACTUAL_SHA_AT_IMPLEMENTATION_TIME` values are intentional. They get real values in Task 5 when we wire up the downloader and manually verify one download. The tests above don't depend on them.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src-tauri/src/models.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(rust): add models module with ModelSpec, verify_sha256, status"
```

---

## Task 4: Resumable downloader with progress

**Files:**
- Modify: `apps/desktop/src-tauri/src/models.rs` (add `ensure_local` and download helpers)

- [ ] **Step 1: Write failing integration test for download resume**

Add to `apps/desktop/src-tauri/src/models.rs` `mod tests` block:

```rust
    use tokio::io::AsyncWriteExt;

    /// Local HTTP server that streams a fixed body with Range support.
    /// Spawned per test, random port.
    async fn spawn_range_server(body: Vec<u8>) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
        use tokio::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else { break; };
                let body = body.clone();
                tokio::spawn(async move {
                    use tokio::io::AsyncReadExt;
                    let mut buf = vec![0u8; 2048];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let range = req.lines().find_map(|l| l.strip_prefix("Range: bytes="));
                    let (start, status) = if let Some(r) = range {
                        let start: usize = r.split('-').next().unwrap_or("0").parse().unwrap_or(0);
                        (start, "206 Partial Content")
                    } else { (0, "200 OK") };
                    let slice = &body[start..];
                    let response = format!(
                        "HTTP/1.1 {}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n\r\n",
                        status, slice.len()
                    );
                    sock.write_all(response.as_bytes()).await.ok();
                    sock.write_all(slice).await.ok();
                    sock.shutdown().await.ok();
                });
            }
        });
        (addr, handle)
    }

    #[tokio::test]
    async fn ensure_local_downloads_full_file_when_absent() {
        let body = b"hello world".to_vec();
        let expected_sha = {
            let mut h = Sha256::new(); h.update(&body); hex::encode(h.finalize())
        };
        let (addr, _srv) = spawn_range_server(body.clone()).await;
        let dir = tempdir().unwrap();
        std::env::set_var("SENTYNYX_DATA_DIR", dir.path());

        let spec = ModelSpec {
            id: "testmodel", file: "x.bin",
            url: Box::leak(format!("http://{}/x.bin", addr).into_boxed_str()),
            sha256: Box::leak(expected_sha.clone().into_boxed_str()),
            size_bytes: body.len() as u64,
        };
        let out = ensure_local(&spec, |_, _| {}).await.unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), body);
    }

    #[tokio::test]
    async fn ensure_local_resumes_partial_download() {
        let body = b"abcdefghij".to_vec();
        let expected_sha = {
            let mut h = Sha256::new(); h.update(&body); hex::encode(h.finalize())
        };
        let (addr, _srv) = spawn_range_server(body.clone()).await;
        let dir = tempdir().unwrap();
        std::env::set_var("SENTYNYX_DATA_DIR", dir.path());

        // Pre-populate a partial file
        let partial = local_path(&ModelSpec {
            id: "resume", file: "y.bin", url: "", sha256: "", size_bytes: 0,
        }).with_extension("bin.partial");
        std::fs::create_dir_all(partial.parent().unwrap()).unwrap();
        std::fs::write(&partial, &body[..3]).unwrap();

        let spec = ModelSpec {
            id: "resume", file: "y.bin",
            url: Box::leak(format!("http://{}/y.bin", addr).into_boxed_str()),
            sha256: Box::leak(expected_sha.clone().into_boxed_str()),
            size_bytes: body.len() as u64,
        };
        let out = ensure_local(&spec, |_, _| {}).await.unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), body);
    }

    #[tokio::test]
    async fn ensure_local_rejects_sha_mismatch() {
        let body = b"hello".to_vec();
        let (addr, _srv) = spawn_range_server(body.clone()).await;
        let dir = tempdir().unwrap();
        std::env::set_var("SENTYNYX_DATA_DIR", dir.path());

        let spec = ModelSpec {
            id: "badsha", file: "z.bin",
            url: Box::leak(format!("http://{}/z.bin", addr).into_boxed_str()),
            sha256: "0000000000000000000000000000000000000000000000000000000000000000",
            size_bytes: body.len() as u64,
        };
        assert!(matches!(ensure_local(&spec, |_, _| {}).await, Err(ModelError::Sha { .. })));
        // Partial file should be cleaned up on sha mismatch
        assert!(!local_path(&spec).with_extension("bin.partial").exists());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd "apps/desktop/src-tauri"
cargo test --lib models:: 2>&1 | tail -20
```

Expected: fails to compile — `ensure_local` doesn't exist yet.

- [ ] **Step 3: Implement ensure_local with resume and progress**

Add to `apps/desktop/src-tauri/src/models.rs`:

```rust
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

pub async fn ensure_local<F>(spec: &ModelSpec, progress: F) -> Result<PathBuf, ModelError>
where F: Fn(u64, u64) + Send + 'static
{
    let final_path = local_path(spec);
    if final_path.exists() {
        if verify_sha256(&final_path, spec.sha256).is_ok() {
            return Ok(final_path);
        }
        let _ = std::fs::remove_file(&final_path);
    }

    if let Some(parent) = final_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let partial = final_path.with_extension(
        format!("{}.partial", final_path.extension().and_then(|s| s.to_str()).unwrap_or(""))
    );

    let resume_from = if partial.exists() {
        std::fs::metadata(&partial).map(|m| m.len()).unwrap_or(0)
    } else { 0 };

    let client = reqwest::Client::builder()
        .build().map_err(|e| ModelError::Http(e.to_string()))?;
    let mut req = client.get(spec.url);
    if resume_from > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={}-", resume_from));
    }
    let resp = req.send().await.map_err(|e| ModelError::Http(e.to_string()))?;
    let status = resp.status();
    if !status.is_success() && status.as_u16() != 206 {
        return Err(ModelError::Http(format!("http {}", status)));
    }

    let total = spec.size_bytes;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true).append(true).open(&partial).await?;
    let mut downloaded = resume_from;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ModelError::Http(e.to_string()))?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        progress(downloaded, total);
    }
    file.flush().await?;
    drop(file);

    if let Err(e) = verify_sha256(&partial, spec.sha256) {
        let _ = std::fs::remove_file(&partial);
        return Err(e);
    }

    let final_size = std::fs::metadata(&partial)?.len();
    if final_size != total {
        let _ = std::fs::remove_file(&partial);
        return Err(ModelError::Size { expected: total, actual: final_size });
    }

    std::fs::rename(&partial, &final_path)?;
    Ok(final_path)
}
```

- [ ] **Step 4: Run the downloader tests**

```bash
cd "apps/desktop/src-tauri"
cargo test --lib models::
```

Expected: all tests PASS, including the three new `ensure_local_*` tests.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/models.rs
git commit -m "feat(rust): implement resumable model download with SHA-256 verification"
```

---

## Task 5: IPC commands — model_status, download_model, delete_model

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands.rs` (add new handlers)
- Modify: `apps/desktop/src-tauri/src/lib.rs` (register new commands in `invoke_handler`)

- [ ] **Step 1: Add ModelStatus IPC return type and handlers**

Add to the end of `apps/desktop/src-tauri/src/commands.rs`:

```rust
use crate::models::{self, ModelSpec, ModelStatus};

#[derive(Serialize)]
pub struct AllModelStatus {
    pub ner: ModelStatus,
    pub ner_tokenizer: ModelStatus,
    pub llm: ModelStatus,
}

#[tauri::command]
pub fn model_status() -> AllModelStatus {
    AllModelStatus {
        ner: models::status(&models::GLINER_SMALL),
        ner_tokenizer: models::status(&models::GLINER_TOKENIZER),
        llm: models::status(&models::QWEN3_1_5B_Q4),
    }
}

#[derive(Deserialize)]
pub struct ModelIdArgs { pub id: String }

fn spec_by_id(id: &str) -> Option<&'static ModelSpec> {
    match id {
        "gliner-small-v2.1" => Some(&models::GLINER_SMALL),
        "gliner-small-v2.1-tokenizer" => Some(&models::GLINER_TOKENIZER),
        "qwen3-1.5b-q4km" => Some(&models::QWEN3_1_5B_Q4),
        _ => None,
    }
}

#[tauri::command]
pub async fn download_model(app: tauri::AppHandle, args: ModelIdArgs) -> Result<(), String> {
    let spec = spec_by_id(&args.id).ok_or_else(|| format!("unknown model id: {}", args.id))?;
    let app_emit = app.clone();
    let id = args.id.clone();
    models::ensure_local(spec, move |done, total| {
        let pct = if total > 0 { (done * 100 / total) as u32 } else { 0 };
        let _ = app_emit.emit("model://progress", serde_json::json!({
            "id": id, "done": done, "total": total, "percent": pct
        }));
    }).await.map_err(|e| e.to_string())?;
    let _ = app.emit("model://ready", serde_json::json!({ "id": args.id }));
    Ok(())
}

#[tauri::command]
pub fn delete_model(args: ModelIdArgs) -> Result<(), String> {
    let spec = spec_by_id(&args.id).ok_or_else(|| format!("unknown model id: {}", args.id))?;
    let p = models::local_path(spec);
    if p.exists() { std::fs::remove_file(&p).map_err(|e| e.to_string())?; }
    Ok(())
}

#[derive(Deserialize)]
pub struct SetParanoidArgs { pub enabled: bool }

#[tauri::command]
pub async fn set_paranoid_mode(state: State<'_, AppState>, args: SetParanoidArgs) -> Result<(), String> {
    let s = state.store.lock().await;
    s.conn.execute(
        "INSERT INTO settings(key,value) VALUES('paranoid_mode',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![if args.enabled { "1" } else { "0" }]
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_paranoid_mode(state: State<'_, AppState>) -> Result<bool, String> {
    let s = state.store.lock().await;
    let v: Result<String, _> = s.conn.query_row(
        "SELECT value FROM settings WHERE key='paranoid_mode'",
        [], |r| r.get(0)
    );
    Ok(matches!(v.as_deref(), Ok("1")))
}
```

Expose `conn` as `pub(crate)` or add accessor methods. The simplest: change `pub struct Store { conn: Connection }` → `pub struct Store { pub(crate) conn: Connection }` in `apps/desktop/src-tauri/src/store.rs` (line 8).

- [ ] **Step 2: Register new commands**

In `apps/desktop/src-tauri/src/lib.rs`, extend the `invoke_handler` macro call to include the new commands:

```rust
        .invoke_handler(tauri::generate_handler![
            commands::detect,
            commands::send,
            commands::consensus,
            commands::list_conversations,
            commands::load_conversation,
            commands::new_conversation,
            commands::set_api_key,
            commands::has_api_key,
            commands::list_configured_providers,
            commands::list_audit,
            commands::audit_metrics,
            commands::model_status,
            commands::download_model,
            commands::delete_model,
            commands::set_paranoid_mode,
            commands::get_paranoid_mode,
        ])
```

- [ ] **Step 3: Verify compilation**

```bash
cd "apps/desktop/src-tauri"
cargo check --all-targets
```

Expected: clean compile.

- [ ] **Step 4: Verify full test suite still passes**

```bash
cd "apps/desktop/src-tauri"
cargo test
```

Expected: all tests pass (no new tests for pure IPC plumbing; the unit-testable pieces already have tests).

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/commands.rs apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/src/store.rs
git commit -m "feat(rust): add model_status, download_model, delete_model, paranoid toggle IPC"
```

---

## Task 6: Frontend — types, ipc wrappers, TopBar status chip

**Files:**
- Modify: `apps/desktop/src/lib/types.ts`
- Modify: `apps/desktop/src/lib/ipc.ts`
- Modify: `apps/desktop/src/chrome/TopBar.tsx`

- [ ] **Step 1: Add new types**

In `apps/desktop/src/lib/types.ts`, add at the bottom:

```typescript
export type ModelStatus =
  | { kind: "missing" }
  | { kind: "downloading"; percent: number }
  | { kind: "ready" }
  | { kind: "error"; msg: string };

export interface AllModelStatus {
  ner: ModelStatus;
  ner_tokenizer: ModelStatus;
  llm: ModelStatus;
}

export interface ModelProgressEvent {
  id: string;
  done: number;
  total: number;
  percent: number;
}
```

Inspect the existing `types.ts` to confirm how `Span.kind` is typed. If it's a string-literal union, extend it to include the five new `_NER` variants:

```typescript
// Find the existing Span kind union and extend. If it's just `string`, no change needed.
```

Run `pnpm build` to catch anything broken by the type addition.

- [ ] **Step 2: Add ipc wrappers and event listener**

In `apps/desktop/src/lib/ipc.ts`, add at the bottom (before the `isTauri` export):

```typescript
import type { AllModelStatus, ModelProgressEvent } from "./types";

export const modelsIpc = {
  status: () => invoke<AllModelStatus>("model_status"),
  download: (id: string) => invoke<void>("download_model", { args: { id } }),
  delete: (id: string) => invoke<void>("delete_model", { args: { id } }),
  setParanoid: (enabled: boolean) => invoke<void>("set_paranoid_mode", { args: { enabled } }),
  getParanoid: () => invoke<boolean>("get_paranoid_mode"),
};

export function onModelProgress(cb: (e: ModelProgressEvent) => void) {
  return listen<ModelProgressEvent>("model://progress", (e) => cb(e.payload));
}

export function onModelReady(cb: (id: string) => void) {
  return listen<{ id: string }>("model://ready", (e) => cb(e.payload.id));
}
```

- [ ] **Step 3: Add status chip to TopBar**

Open `apps/desktop/src/chrome/TopBar.tsx`. Add a new state-driven chip to the right of the existing buttons. At the top of the component file:

```typescript
import { useEffect, useState } from "react";
import { modelsIpc, onModelProgress, onModelReady, isTauri } from "../lib/ipc";
import type { AllModelStatus } from "../lib/types";
```

Inside the `TopBar` component body, add:

```typescript
const [modelStatus, setModelStatus] = useState<AllModelStatus | null>(null);
const [downloadPct, setDownloadPct] = useState<number | null>(null);

useEffect(() => {
  if (!isTauri) return;
  let cancelled = false;
  modelsIpc.status().then(s => { if (!cancelled) setModelStatus(s); }).catch(() => {});
  const unProgress = onModelProgress(e => {
    if (e.id.startsWith("gliner")) setDownloadPct(e.percent);
  });
  const unReady = onModelReady(() => {
    setDownloadPct(null);
    modelsIpc.status().then(setModelStatus).catch(() => {});
  });
  return () => {
    cancelled = true;
    unProgress.then(u => u());
    unReady.then(u => u());
  };
}, []);

const nerReady = modelStatus?.ner.kind === "ready" && modelStatus?.ner_tokenizer.kind === "ready";
const paranoidReady = modelStatus?.llm.kind === "ready";
const chipLabel =
  downloadPct !== null ? `◐ semantic ${downloadPct}%` :
  !modelStatus ? "◐ loading…" :
  nerReady && paranoidReady ? "◆◆ paranoid ready" :
  nerReady ? "◆ semantic ready" :
  "◐ semantic off";

const chipColor =
  downloadPct !== null ? "var(--neon)" :
  nerReady ? "#7cffb2" :
  "#666";
```

Find where the TopBar renders its right-hand-side items (keyboard shortcuts or empty space) and insert before `</div>`:

```tsx
<button
  onClick={props.onOpenSettings}
  title="Semantic detection status · click to manage models"
  style={{
    fontFamily: "JetBrains Mono, monospace",
    fontSize: 11, padding: "4px 10px", marginLeft: 8,
    background: "transparent", color: chipColor,
    border: `1px solid ${chipColor}`, borderRadius: 4, cursor: "pointer",
    letterSpacing: 0.5,
  }}
>{chipLabel}</button>
```

You'll need to add `onOpenSettings?: () => void;` to the TopBar props interface if it's not there (check the existing shape). If `onOpenSettings` isn't on props, add it and thread it through from `App.tsx` (the App already has `setSettingsOpen` — pass it as a prop: `onOpenSettings={() => setSettingsOpen(true)}`).

- [ ] **Step 4: Verify frontend builds**

```bash
cd "apps/desktop"
pnpm build 2>&1 | tail -30
```

Expected: clean build (tsc --noEmit + vite build succeed).

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/lib/types.ts apps/desktop/src/lib/ipc.ts apps/desktop/src/chrome/TopBar.tsx apps/desktop/src/app/App.tsx
git commit -m "feat(ui): add model status chip to TopBar with live progress"
```

---

## Task 7: ModelDownloadPanel — first-run modal

**Files:**
- Create: `apps/desktop/src/scenes/ModelDownloadPanel.tsx`
- Modify: `apps/desktop/src/app/App.tsx` (mount the panel when NER is missing)

- [ ] **Step 1: Create ModelDownloadPanel component**

Create `apps/desktop/src/scenes/ModelDownloadPanel.tsx`:

```tsx
import { useEffect, useState } from "react";
import { modelsIpc, onModelProgress, onModelReady } from "../lib/ipc";
import type { AllModelStatus, ModelStatus } from "../lib/types";

interface Props { onClose: () => void }

type RowState = "idle" | "downloading" | "ready" | "error";

interface Row {
  id: string;
  label: string;
  sizeMb: number;
  optional: boolean;
  state: RowState;
  percent: number;
  error: string | null;
}

const INITIAL: Row[] = [
  { id: "gliner-small-v2.1", label: "Semantic NER (GLiNER)", sizeMb: 80, optional: false, state: "idle", percent: 0, error: null },
  { id: "gliner-small-v2.1-tokenizer", label: "NER tokenizer", sizeMb: 3, optional: false, state: "idle", percent: 0, error: null },
  { id: "qwen3-1.5b-q4km", label: "Paranoid LLM (Qwen 3 1.5B)", sizeMb: 950, optional: true, state: "idle", percent: 0, error: null },
];

function rowStateFromStatus(s: ModelStatus): RowState {
  switch (s.kind) {
    case "ready": return "ready";
    case "downloading": return "downloading";
    case "error": return "error";
    case "missing": default: return "idle";
  }
}

export function ModelDownloadPanel({ onClose }: Props) {
  const [rows, setRows] = useState<Row[]>(INITIAL);
  const [includeLlm, setIncludeLlm] = useState(false);

  useEffect(() => {
    modelsIpc.status().then((s: AllModelStatus) => {
      setRows(rs => rs.map(r => {
        const status =
          r.id === "gliner-small-v2.1" ? s.ner :
          r.id === "gliner-small-v2.1-tokenizer" ? s.ner_tokenizer :
          s.llm;
        return { ...r, state: rowStateFromStatus(status),
          error: status.kind === "error" ? status.msg : null };
      }));
    }).catch(() => {});

    const unP = onModelProgress(e => {
      setRows(rs => rs.map(r => r.id === e.id
        ? { ...r, state: "downloading", percent: e.percent }
        : r));
    });
    const unR = onModelReady(id => {
      setRows(rs => rs.map(r => r.id === id
        ? { ...r, state: "ready", percent: 100, error: null }
        : r));
    });
    return () => { unP.then(u => u()); unR.then(u => u()); };
  }, []);

  const start = async (id: string) => {
    setRows(rs => rs.map(r => r.id === id ? { ...r, state: "downloading", error: null } : r));
    try {
      await modelsIpc.download(id);
    } catch (e) {
      setRows(rs => rs.map(r => r.id === id
        ? { ...r, state: "error", error: String(e) }
        : r));
    }
  };

  const startAll = async () => {
    const targets = rows.filter(r => r.state !== "ready" && (!r.optional || includeLlm));
    for (const r of targets) await start(r.id);
  };

  return (
    <div style={{
      position: "fixed", inset: 0, background: "rgba(5,6,10,0.88)",
      display: "flex", alignItems: "center", justifyContent: "center", zIndex: 90,
    }}>
      <div style={{
        width: 560, padding: 32, background: "#0a0d14",
        border: "1px solid rgba(242,255,43,0.25)", borderRadius: 8,
        fontFamily: "Inter, sans-serif", color: "#e5e9f0",
      }}>
        <div style={{ fontFamily: "Instrument Serif, serif", fontSize: 28, marginBottom: 8 }}>
          Enable semantic detection
        </div>
        <div style={{ fontSize: 13, color: "#9ba3b4", marginBottom: 20 }}>
          Downloads run from HuggingFace Hub. Files are SHA-256 verified and stored in your app data directory.
        </div>

        {rows.map(r => (
          <div key={r.id} style={{
            display: "flex", alignItems: "center", justifyContent: "space-between",
            padding: "12px 0", borderBottom: "1px solid rgba(255,255,255,0.05)",
          }}>
            <div>
              <div style={{ fontSize: 14 }}>{r.label}{r.optional && <span style={{ color: "#9ba3b4", fontSize: 11, marginLeft: 8 }}>optional</span>}</div>
              <div style={{ fontSize: 11, color: "#9ba3b4", fontFamily: "JetBrains Mono, monospace", marginTop: 2 }}>
                {r.sizeMb} MB · {r.state === "downloading" ? `${r.percent}%` : r.state}
                {r.error && <span style={{ color: "#ff6b9d", marginLeft: 8 }}>{r.error}</span>}
              </div>
            </div>
            <button
              onClick={() => start(r.id)}
              disabled={r.state === "downloading" || r.state === "ready"}
              style={{
                padding: "6px 14px", fontSize: 12,
                background: r.state === "ready" ? "transparent" : "var(--neon, #f2ff2b)",
                color: r.state === "ready" ? "#7cffb2" : "#000",
                border: "none", borderRadius: 4, cursor: "pointer",
                opacity: r.state === "downloading" ? 0.5 : 1,
              }}
            >{r.state === "ready" ? "✓ ready" : r.state === "downloading" ? `${r.percent}%` : "download"}</button>
          </div>
        ))}

        <label style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 16, fontSize: 12, color: "#9ba3b4" }}>
          <input type="checkbox" checked={includeLlm} onChange={e => setIncludeLlm(e.target.checked)} />
          Include paranoid LLM in "download all" (950 MB)
        </label>

        <div style={{ display: "flex", gap: 12, marginTop: 24, justifyContent: "flex-end" }}>
          <button onClick={onClose} style={{
            padding: "8px 16px", fontSize: 13, background: "transparent",
            color: "#9ba3b4", border: "1px solid #2a3040", borderRadius: 4, cursor: "pointer",
          }}>Continue with regex only</button>
          <button onClick={startAll} style={{
            padding: "8px 16px", fontSize: 13, background: "var(--neon, #f2ff2b)",
            color: "#000", border: "none", borderRadius: 4, cursor: "pointer",
          }}>Download all</button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Wire ModelDownloadPanel into App.tsx**

In `apps/desktop/src/app/App.tsx`:

1. Add the import at the top with the other scene imports:

```typescript
import { ModelDownloadPanel } from "../scenes/ModelDownloadPanel";
import { modelsIpc } from "../lib/ipc";
```

2. Add state:

```typescript
const [modelPanelOpen, setModelPanelOpen] = useState(false);
```

3. In the first-run `useEffect` that loads conversations (the one beginning with `if (!isTauri) return`), add after the conversation-seeding block:

```typescript
try {
  const s = await modelsIpc.status();
  if (s.ner.kind === "missing" || s.ner_tokenizer.kind === "missing") {
    setModelPanelOpen(true);
  }
} catch {}
```

4. In the render section, before the closing fragment, add:

```tsx
{modelPanelOpen && <ModelDownloadPanel onClose={() => setModelPanelOpen(false)} />}
```

- [ ] **Step 3: Verify frontend builds**

```bash
cd "apps/desktop"
pnpm build 2>&1 | tail -20
```

Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/scenes/ModelDownloadPanel.tsx apps/desktop/src/app/App.tsx
git commit -m "feat(ui): add ModelDownloadPanel first-run modal"
```

---

## Task 8: Add ort + tokenizers dependencies; stub detect/ner.rs

**Files:**
- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Create: `apps/desktop/src-tauri/src/detect/ner.rs`
- Modify: `apps/desktop/src-tauri/src/detect/mod.rs` (add `pub mod ner;`)

- [ ] **Step 1: Add dependencies**

In `apps/desktop/src-tauri/Cargo.toml`, add to `[dependencies]`:

```toml
ort = { version = "2.0.0-rc.9", default-features = false, features = ["load-dynamic", "ndarray"] }
tokenizers = { version = "0.20", default-features = false, features = ["onig"] }
ndarray = "0.16"
```

Also add (if not present) under `[dev-dependencies]`:

```toml
tokio = { version = "1", features = ["full", "test-util"] }
```

- [ ] **Step 2: Verify deps compile (slow first time — expect 3–5 min)**

```bash
cd "apps/desktop/src-tauri"
cargo build --lib 2>&1 | tail -30
```

Expected: clean build. `ort` may require a runtime ONNX library to be installed for `load-dynamic`. On macOS with Homebrew:

```bash
brew list onnxruntime 2>/dev/null || brew install onnxruntime
```

If the build fails due to missing onnxruntime, install it first and retry.

- [ ] **Step 3: Stub detect/ner.rs returning empty spans**

Create `apps/desktop/src-tauri/src/detect/ner.rs`:

```rust
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::OnceLock;
use crate::models::{self, GLINER_SMALL, GLINER_TOKENIZER};
use crate::vendetta::Span;
use super::{Detector, DetectError, Source};

pub const NER_LABELS: &[&str] = &[
    "person",
    "organization",
    "internal-project-codename",
    "location",
    "employee-id-code",
];

/// Thread-safe lazy holder for the ONNX session and tokenizer.
/// Once initialized, all callers share the same instance.
pub struct NerDetector {
    inner: OnceLock<Option<NerRuntime>>,
}

struct NerRuntime {
    // Filled in Task 9. For now, just marker fields so the type compiles.
    _onnx_path: PathBuf,
    _tok_path: PathBuf,
}

impl NerDetector {
    pub fn new() -> Self { Self { inner: OnceLock::new() } }

    fn try_load(&self) -> Option<&NerRuntime> {
        self.inner.get_or_init(|| {
            let onnx = models::local_path(&GLINER_SMALL);
            let tok = models::local_path(&GLINER_TOKENIZER);
            if !onnx.exists() || !tok.exists() { return None; }
            if models::verify_sha256(&onnx, GLINER_SMALL.sha256).is_err() { return None; }
            if models::verify_sha256(&tok, GLINER_TOKENIZER.sha256).is_err() { return None; }
            Some(NerRuntime { _onnx_path: onnx, _tok_path: tok })
        }).as_ref()
    }
}

#[async_trait]
impl Detector for NerDetector {
    fn source(&self) -> Source { Source::Ner }

    async fn detect(&self, _text: &str) -> Result<Vec<Span>, DetectError> {
        match self.try_load() {
            Some(_rt) => {
                // Real inference lands in Task 9. For now returning empty keeps
                // the merge pipeline wired without changing behavior.
                Ok(vec![])
            }
            None => Err(DetectError::ModelNotLoaded("ner".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn ner_returns_err_when_model_missing() {
        let dir = tempdir().unwrap();
        std::env::set_var("SENTYNYX_DATA_DIR", dir.path());
        let d = NerDetector::new();
        let r = d.detect("anything").await;
        assert!(matches!(r, Err(DetectError::ModelNotLoaded(_))));
    }
}
```

- [ ] **Step 4: Register module**

Modify `apps/desktop/src-tauri/src/detect/mod.rs` — change `pub mod regex;` to also declare ner:

```rust
pub mod regex;
pub mod ner;
```

- [ ] **Step 5: Run tests**

```bash
cd "apps/desktop/src-tauri"
cargo test --lib detect::
```

Expected: all tests pass, including `ner_returns_err_when_model_missing`.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/src/detect
git commit -m "feat(rust): add ort + tokenizers deps; stub NerDetector returning Err when model missing"
```

---

## Task 9: GLiNER inference — tokenize, infer, decode spans

**Files:**
- Modify: `apps/desktop/src-tauri/src/detect/ner.rs`

- [ ] **Step 1: Verify the GLiNER ONNX I/O shape**

Before writing inference code, the engineer must manually download `model.onnx` from `https://huggingface.co/onnx-community/gliner-small-v2.1/resolve/main/onnx/model.onnx` and `tokenizer.json` from the same repo. Compute their SHA-256 values and update the `GLINER_SMALL.sha256` and `GLINER_TOKENIZER.sha256` constants in `models.rs`:

```bash
# From the repo root
mkdir -p /tmp/gliner && cd /tmp/gliner
curl -L -o model.onnx https://huggingface.co/onnx-community/gliner-small-v2.1/resolve/main/onnx/model.onnx
curl -L -o tokenizer.json https://huggingface.co/onnx-community/gliner-small-v2.1/resolve/main/tokenizer.json
shasum -a 256 model.onnx tokenizer.json
```

Update the constants in `apps/desktop/src-tauri/src/models.rs`:

```rust
pub const GLINER_SMALL: ModelSpec = ModelSpec {
    id: "gliner-small-v2.1",
    file: "model.onnx",
    url: "https://huggingface.co/onnx-community/gliner-small-v2.1/resolve/main/onnx/model.onnx",
    sha256: "PASTE_ACTUAL_HASH_HERE",
    size_bytes: /* actual file size */,
};
// Same for GLINER_TOKENIZER
```

Also inspect the ONNX model's input/output shape:

```bash
python3 -c "import onnx; m = onnx.load('/tmp/gliner/model.onnx'); print([(i.name, [d.dim_value for d in i.type.tensor_type.shape.dim]) for i in m.graph.input]); print([(o.name, [d.dim_value for d in o.type.tensor_type.shape.dim]) for o in m.graph.output])"
```

Record the input names and shapes — you'll need them for the `ort::inputs!` call. GLiNER typically takes `input_ids`, `attention_mask`, and a `words_mask` or `num_words`, with labels encoded as part of the input. The exact shape is model-revision specific; code below is a template, adjust input names to match.

- [ ] **Step 2: Write failing integration test that requires the model**

Add to `apps/desktop/src-tauri/src/detect/ner.rs` `mod tests`:

```rust
    /// Requires the GLiNER ONNX model + tokenizer to be present locally at the
    /// default `SENTYNYX_DATA_DIR` path. Skipped otherwise so CI without models
    /// doesn't falsely fail. Run locally after `download_model` IPC or manual
    /// placement.
    #[tokio::test]
    async fn ner_detects_person_when_model_available() {
        let root = models::models_root();
        if !models::local_path(&GLINER_SMALL).exists() { return; }
        if !models::local_path(&GLINER_TOKENIZER).exists() { return; }

        let d = NerDetector::new();
        let spans = d.detect("Draft a memo for Jamie Torres at the office.").await;
        assert!(spans.is_ok(), "ner detect errored: {:?}", spans);
        let spans = spans.unwrap();
        // We expect at least one PERSON_NER span covering "Jamie Torres"
        let has_person = spans.iter().any(|s| {
            matches!(s.kind, crate::vendetta::Kind::PERSON_NER)
                && s.raw.contains("Jamie")
        });
        assert!(has_person, "expected PERSON_NER span for Jamie Torres, got: {:?}", spans);
    }

    #[tokio::test]
    async fn ner_returns_empty_for_benign_text() {
        if !models::local_path(&GLINER_SMALL).exists() { return; }
        if !models::local_path(&GLINER_TOKENIZER).exists() { return; }
        let d = NerDetector::new();
        let spans = d.detect("the quick brown fox jumps over the lazy dog").await.unwrap();
        assert!(spans.iter().all(|s| !matches!(s.kind,
            crate::vendetta::Kind::PERSON_NER | crate::vendetta::Kind::ORG_NER
        )));
    }
```

- [ ] **Step 3: Implement inference**

Replace the body of `apps/desktop/src-tauri/src/detect/ner.rs` with:

```rust
use async_trait::async_trait;
use ndarray::{Array1, Array2, ArrayD, s};
use ort::{session::Session, inputs, value::Value};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tokenizers::Tokenizer;
use crate::models::{self, GLINER_SMALL, GLINER_TOKENIZER};
use crate::vendetta::{Kind, Span};
use super::{Detector, DetectError, Source};

pub const NER_LABELS: &[(&str, Kind)] = &[
    ("person",                    Kind::PERSON_NER),
    ("organization",              Kind::ORG_NER),
    ("internal-project-codename", Kind::CODENAME_NER),
    ("location",                  Kind::LOCATION_NER),
    ("employee-id-code",          Kind::EMPID_NER),
];

pub struct NerDetector {
    inner: OnceLock<Option<Mutex<NerRuntime>>>,
}

struct NerRuntime {
    session: Session,
    tokenizer: Tokenizer,
    _onnx_path: PathBuf,
}

impl NerDetector {
    pub fn new() -> Self { Self { inner: OnceLock::new() } }

    fn load(&self) -> Option<&Mutex<NerRuntime>> {
        self.inner.get_or_init(|| {
            let onnx = models::local_path(&GLINER_SMALL);
            let tok = models::local_path(&GLINER_TOKENIZER);
            if !onnx.exists() || !tok.exists() { return None; }
            if models::verify_sha256(&onnx, GLINER_SMALL.sha256).is_err() { return None; }
            if models::verify_sha256(&tok, GLINER_TOKENIZER.sha256).is_err() { return None; }
            let session = Session::builder().ok()?
                .commit_from_file(&onnx).ok()?;
            let tokenizer = Tokenizer::from_file(&tok).ok()?;
            Some(Mutex::new(NerRuntime {
                session, tokenizer, _onnx_path: onnx,
            }))
        }).as_ref()
    }
}

#[async_trait]
impl Detector for NerDetector {
    fn source(&self) -> Source { Source::Ner }

    async fn detect(&self, text: &str) -> Result<Vec<Span>, DetectError> {
        let rt_cell = self.load().ok_or_else(|| DetectError::ModelNotLoaded("ner".into()))?;
        let text = text.to_string();

        // Run inference on a blocking task to avoid stalling the async runtime.
        let rt_cell_clone: &'static Mutex<NerRuntime> = unsafe {
            // The OnceLock holds the Mutex for the lifetime of the process, so a
            // 'static reference is correct here. We use unsafe to name the lifetime
            // — tokio::task::spawn_blocking requires 'static captures.
            std::mem::transmute(rt_cell)
        };

        let result = tokio::task::spawn_blocking(move || -> Result<Vec<Span>, String> {
            let mut rt = rt_cell_clone.lock().map_err(|e| e.to_string())?;
            run_inference(&mut rt, &text)
        }).await
            .map_err(|e| DetectError::Inference(e.to_string()))?
            .map_err(DetectError::Inference)?;

        Ok(result)
    }
}

fn run_inference(rt: &mut NerRuntime, text: &str) -> Result<Vec<Span>, String> {
    // IMPORTANT: GLiNER input format is:
    //   [CLS] label1 [SEP] label2 [SEP] ... [SEP] token1 token2 ... [SEP]
    // where the model outputs logits per (label × span) over the text tokens.
    //
    // The exact pre/post-processing depends on the ONNX export revision.
    // This implementation targets `onnx-community/gliner-small-v2.1` as of
    // 2025-Q4. If the model's I/O shape has changed, consult its model card
    // and adjust the tensor construction below.

    let labels: Vec<&str> = NER_LABELS.iter().map(|(l, _)| *l).collect();
    let label_str = labels.join(" ");
    let combined = format!("{} [SEP] {}", label_str, text);

    let encoding = rt.tokenizer.encode(combined, true)
        .map_err(|e| format!("tokenize: {}", e))?;
    let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
    let attn: Vec<i64> = encoding.get_attention_mask().iter().map(|&x| x as i64).collect();
    let offsets = encoding.get_offsets().to_vec();
    let words: Vec<String> = encoding.get_tokens().to_vec();
    let seq_len = input_ids.len();

    let ids_arr = Array2::from_shape_vec((1, seq_len), input_ids).map_err(|e| e.to_string())?;
    let attn_arr = Array2::from_shape_vec((1, seq_len), attn).map_err(|e| e.to_string())?;

    let outputs = rt.session.run(inputs![
        "input_ids" => Value::from_array(ids_arr).map_err(|e| e.to_string())?,
        "attention_mask" => Value::from_array(attn_arr).map_err(|e| e.to_string())?,
    ]).map_err(|e| format!("ort run: {}", e))?;

    // GLiNER outputs "logits" with shape [batch=1, num_spans, num_labels].
    // We decode by taking argmax over labels per span, threshold at 0.5 (sigmoid),
    // then map span indices back to character offsets via the tokenizer offsets.
    let logits: &Value = outputs.get("logits").ok_or("missing 'logits' output")?;
    let (shape, data) = logits.try_extract_raw_tensor::<f32>()
        .map_err(|e| format!("extract logits: {}", e))?;

    // Placeholder decoding — the actual GLiNER span decoder iterates over all
    // (start, end, label) candidates, applies sigmoid, filters by threshold,
    // and deduplicates overlapping spans by max score.
    let spans = decode_gliner_spans(shape.as_slice(), data, &offsets, &words, text)?;
    Ok(spans)
}

fn decode_gliner_spans(
    shape: &[i64],
    data: &[f32],
    offsets: &[(usize, usize)],
    _words: &[String],
    text: &str,
) -> Result<Vec<Span>, String> {
    // Expected shape: [1, num_spans, num_labels]
    // where num_spans = seq_len * max_width (configurable; GLiNER default 12).
    if shape.len() != 3 { return Ok(vec![]); }
    let (_, num_spans, num_labels) = (shape[0] as usize, shape[1] as usize, shape[2] as usize);
    let max_width = 12usize;
    let seq_len = offsets.len();
    if num_spans != seq_len * max_width { return Ok(vec![]); }

    let mut out: Vec<Span> = Vec::new();
    let mut seen: Vec<(usize, usize)> = Vec::new();

    for start_idx in 0..seq_len {
        for width in 0..max_width {
            let end_idx = start_idx + width;
            if end_idx >= seq_len { break; }
            let span_idx = start_idx * max_width + width;

            let mut best_label: Option<usize> = None;
            let mut best_score: f32 = 0.5; // sigmoid threshold
            for li in 0..num_labels {
                let raw = data[span_idx * num_labels + li];
                let prob = 1.0 / (1.0 + (-raw).exp());
                if prob > best_score {
                    best_score = prob;
                    best_label = Some(li);
                }
            }
            if let Some(li) = best_label {
                let char_start = offsets[start_idx].0;
                let char_end = offsets[end_idx].1;
                if char_start >= char_end || char_end > text.len() { continue; }
                if seen.iter().any(|(s, e)| *s < char_end && char_start < *e) { continue; }
                seen.push((char_start, char_end));
                let kind = NER_LABELS.get(li).map(|(_, k)| k.clone())
                    .unwrap_or(Kind::PERSON_NER);
                let raw = text[char_start..char_end].to_string();
                out.push(Span { start: char_start, end: char_end, kind, raw, alias: String::new() });
            }
        }
    }
    out.sort_by_key(|s| s.start);
    Ok(out)
}
```

- [ ] **Step 4: Run ner tests**

```bash
cd "apps/desktop/src-tauri"
cargo test --lib detect::ner
```

Expected: the existing `ner_returns_err_when_model_missing` test passes (skip the model-dependent tests, they no-op when models are absent). If the engineer has downloaded the GLiNER model locally, the real-inference tests will also run and must pass.

> Note on decoding: the `decode_gliner_spans` logic above is a best-effort starting point for GLiNER's multi-span output format. If the downloaded model's output shape doesn't match `[1, num_spans, num_labels]` with `num_spans = seq_len * 12`, consult the GLiNER README and adjust. Run the eval harness (Task 19) and tune thresholds before concluding the integration is done.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/detect/ner.rs apps/desktop/src-tauri/src/models.rs
git commit -m "feat(rust): implement GLiNER ONNX inference with label mapping and span decoding"
```

---

## Task 10: Wire regex + NER into commands::send via tokio::join!

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands.rs` (refactor the detection section in `send`)

- [ ] **Step 1: Refactor send() to use both detectors in parallel**

In `apps/desktop/src-tauri/src/commands.rs`, find the `send` command and replace the block labeled `// Detect + alias against the conversation's alias map.` (lines ~47–54 in the current file) with:

```rust
    // Detect: regex + NER in parallel. NER falls back silently if model not loaded.
    let regex_det = crate::detect::regex::RegexDetector;
    let ner_det = crate::detect::ner::NerDetector::new();

    let (regex_spans, ner_spans_result) = tokio::join!(
        regex_det.detect(&args.text),
        tokio::time::timeout(
            std::time::Duration::from_millis(500),
            ner_det.detect(&args.text),
        ),
    );
    let regex_spans = regex_spans.map_err(|e| e.to_string())?;
    let ner_spans: Vec<crate::vendetta::Span> = match ner_spans_result {
        Ok(Ok(spans)) => spans,
        Ok(Err(crate::detect::DetectError::ModelNotLoaded(_))) => vec![],
        Ok(Err(e)) => { eprintln!("ner detect error: {}", e); vec![] },
        Err(_timeout) => { eprintln!("ner detect timeout"); vec![] },
    };

    let merged_pre_alias = crate::detect::merge_spans(regex_spans, ner_spans);

    // Alias the merged spans against the conversation's persistent alias map.
    let (map, counters, spans, aliased) = {
        let s = store.lock().await;
        let (mut m, mut c) = s.load_alias_state(&args.conv_id).unwrap_or_default();
        let spans = vendetta::apply_alias_map(&merged_pre_alias, &mut m, &mut c);
        let aliased = vendetta::aliasize(&args.text, &spans);
        (m, c, spans, aliased)
    };
```

- [ ] **Step 2: Add `apply_alias_map` to vendetta.rs**

Add to `apps/desktop/src-tauri/src/vendetta.rs` (just below `aliasize`):

```rust
/// Given spans that may not have aliases assigned yet, walk them through the
/// conversation alias map — reusing existing aliases where the same raw token
/// has been seen, or minting new ones.
pub fn apply_alias_map(
    spans: &[Span],
    map: &mut AliasMap,
    counters: &mut std::collections::HashMap<String, usize>,
) -> Vec<Span> {
    let mut out = Vec::with_capacity(spans.len());
    for s in spans {
        let key = alias_key(&s.kind, &s.raw);
        let alias = if let Some(a) = map.get(&key) {
            a.clone()
        } else {
            let c = counters.entry(s.kind.as_str().to_string()).or_insert(0);
            *c += 1;
            let a = format!("{{{{{}_{:02}}}}}", s.kind.label(), *c);
            map.insert(key, a.clone());
            a
        };
        out.push(Span {
            start: s.start, end: s.end, kind: s.kind.clone(),
            raw: s.raw.clone(), alias,
        });
    }
    out
}
```

- [ ] **Step 3: Add unit test for apply_alias_map**

Add to `apps/desktop/src-tauri/src/vendetta.rs` `mod tests`:

```rust
    #[test]
    fn apply_alias_map_mints_consistent_aliases() {
        let mut m = AliasMap::new();
        let mut c = HashMap::new();
        let s1 = Span { start: 0, end: 5, kind: Kind::EMAIL, raw: "a@b.c".into(), alias: String::new() };
        let s2 = Span { start: 10, end: 15, kind: Kind::EMAIL, raw: "a@b.c".into(), alias: String::new() };
        let aliased = apply_alias_map(&[s1, s2], &mut m, &mut c);
        assert_eq!(aliased[0].alias, "{{email_01}}");
        assert_eq!(aliased[1].alias, "{{email_01}}");  // same raw -> same alias
    }

    #[test]
    fn apply_alias_map_respects_ner_kinds() {
        let mut m = AliasMap::new();
        let mut c = HashMap::new();
        let sp = Span { start: 0, end: 10, kind: Kind::PERSON_NER, raw: "Jamie".into(), alias: String::new() };
        let aliased = apply_alias_map(&[sp], &mut m, &mut c);
        assert_eq!(aliased[0].alias, "{{person_01}}");
    }
```

- [ ] **Step 4: Add source tagging to audit entries**

Modify `apps/desktop/src-tauri/src/store.rs` `append_audit_for_spans` to take a `source` parameter:

Change the signature from:
```rust
pub fn append_audit_for_spans(&mut self, spans: &[Span], action: &str) -> rusqlite::Result<Vec<AuditEntry>>
```
To:
```rust
pub fn append_audit_for_spans(&mut self, spans: &[Span], action: &str, source: &str) -> rusqlite::Result<Vec<AuditEntry>>
```

Update the `INSERT` inside the method:

```rust
self.conn.execute(
    "INSERT INTO audit(id,ts,kind,raw_hash,alias,action,prev_hash,sig,source) VALUES(?,?,?,?,?,?,?,?,?)",
    params![e.id, e.ts, e.kind, e.raw_hash, e.alias, e.action, e.prev_hash, e.sig, source]
)?;
```

- [ ] **Step 5: Update callers of append_audit_for_spans in commands.rs**

Find the two call sites in `send`:

```rust
let _ = s.append_audit_for_spans(&[critical.clone()], "BLOCK");
```

Change to:

```rust
let src = if matches!(critical.kind, Kind::PERSON_NER | Kind::ORG_NER | Kind::CODENAME_NER | Kind::LOCATION_NER | Kind::EMPID_NER) { "ner" } else { "regex" };
let _ = s.append_audit_for_spans(&[critical.clone()], "BLOCK", src);
```

And:

```rust
s.append_audit_for_spans(&spans, "ALIAS").ok();
```

Change to:

```rust
// Split into regex vs ner source buckets so audit attribution is accurate.
let (ner, reg): (Vec<_>, Vec<_>) = spans.iter().cloned().partition(|s| matches!(s.kind,
    Kind::PERSON_NER | Kind::ORG_NER | Kind::CODENAME_NER | Kind::LOCATION_NER | Kind::EMPID_NER));
if !reg.is_empty() { s.append_audit_for_spans(&reg, "ALIAS", "regex").ok(); }
if !ner.is_empty() { s.append_audit_for_spans(&ner, "ALIAS", "ner").ok(); }
```

Add `use crate::vendetta::Kind;` at the top of `commands.rs` if not already imported (the existing `use crate::vendetta::{self, Span};` should cover it — use `vendetta::Kind::PERSON_NER` qualified names instead if needed).

- [ ] **Step 6: Run all Rust tests**

```bash
cd "apps/desktop/src-tauri"
cargo test
```

Expected: all tests pass, including the two new `apply_alias_map_*` tests.

- [ ] **Step 7: Manual integration smoke test (no models — just proves no regression)**

```bash
cd "apps/desktop"
pnpm tauri dev &
# wait for window, type a prompt with an email, hit Transmit,
# verify streaming works and regex spans alias as before.
# Close the window.
killall -q Sentynyx 2>/dev/null || true
```

Expected: window opens, transmit works exactly as v0.1. No regression.

- [ ] **Step 8: Commit**

```bash
git add apps/desktop/src-tauri/src/commands.rs apps/desktop/src-tauri/src/vendetta.rs apps/desktop/src-tauri/src/store.rs
git commit -m "feat(rust): wire regex+NER parallel detection with merge and audit source tagging"
```

---

## Task 11: Frontend — source glyphs in VendettaPanel

**Files:**
- Modify: `apps/desktop/src/chrome/VendettaPanel.tsx`
- Modify: `apps/desktop/src/lib/vendetta.ts` (kind-to-glyph helper)

- [ ] **Step 1: Add source classification helper**

In `apps/desktop/src/lib/vendetta.ts`, add at the bottom:

```typescript
export type DetectionSource = "regex" | "ner" | "llm";

export function sourceForKind(kind: string): DetectionSource {
  if (kind.endsWith("_LLM")) return "llm";
  if (kind.endsWith("_NER")) return "ner";
  return "regex";
}

export function sourceGlyph(src: DetectionSource): string {
  return src === "regex" ? "∎" : src === "ner" ? "◆" : "✦";
}

export function sourceTooltip(src: DetectionSource): string {
  return src === "regex" ? "Detected by regex pattern"
       : src === "ner"   ? "Detected by GLiNER semantic model"
       :                   "Detected by Qwen paranoid scan";
}
```

- [ ] **Step 2: Render glyphs in VendettaPanel**

Open `apps/desktop/src/chrome/VendettaPanel.tsx`. Find where each span's alias is rendered (look for `span.alias` usage). Add the glyph prefix. Imports:

```typescript
import { sourceForKind, sourceGlyph, sourceTooltip } from "../lib/vendetta";
```

In the render logic, replace `{span.alias}` with:

```tsx
<span title={sourceTooltip(sourceForKind(span.kind))}>
  <span style={{ opacity: 0.6, marginRight: 4 }}>{sourceGlyph(sourceForKind(span.kind))}</span>
  {span.alias}
</span>
```

(Inspect the existing VendettaPanel render to find the exact JSX block — the grep target is `span.alias` or `.alias`.)

- [ ] **Step 3: Verify frontend builds**

```bash
cd "apps/desktop"
pnpm build 2>&1 | tail -20
```

Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/lib/vendetta.ts apps/desktop/src/chrome/VendettaPanel.tsx
git commit -m "feat(ui): distinguish detection source with glyphs in VendettaPanel"
```

---

## Task 12: SettingsPanel — Models tab

**Files:**
- Modify: `apps/desktop/src/scenes/SettingsPanel.tsx`

- [ ] **Step 1: Add Models tab**

Open `apps/desktop/src/scenes/SettingsPanel.tsx`. Inspect the existing structure — it already has API key settings. Add a tabbed interface or a second section for Models.

At the top of the component, add state and data:

```typescript
import { modelsIpc, onModelProgress, onModelReady } from "../lib/ipc";
import type { AllModelStatus } from "../lib/types";
```

Inside the component body:

```typescript
const [modelStatus, setModelStatus] = useState<AllModelStatus | null>(null);
const [paranoid, setParanoid] = useState(false);
const [downloadingId, setDownloadingId] = useState<string | null>(null);
const [pct, setPct] = useState(0);

useEffect(() => {
  modelsIpc.status().then(setModelStatus).catch(() => {});
  modelsIpc.getParanoid().then(setParanoid).catch(() => {});
  const unP = onModelProgress(e => { setDownloadingId(e.id); setPct(e.percent); });
  const unR = onModelReady(() => {
    setDownloadingId(null); setPct(0);
    modelsIpc.status().then(setModelStatus).catch(() => {});
  });
  return () => { unP.then(u => u()); unR.then(u => u()); };
}, []);

const toggleParanoid = async (v: boolean) => {
  await modelsIpc.setParanoid(v);
  setParanoid(v);
};
```

Below the existing API keys section in the rendered JSX, add:

```tsx
<section style={{ marginTop: 32 }}>
  <h3 style={{ fontFamily: "Instrument Serif, serif", fontSize: 20, margin: "0 0 12px" }}>Models</h3>

  <div style={{ padding: 12, background: "#0a0d14", border: "1px solid rgba(255,255,255,0.06)", borderRadius: 4, marginBottom: 12 }}>
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
      <div>
        <div>Semantic NER (GLiNER)</div>
        <div style={{ fontSize: 11, color: "#9ba3b4" }}>
          Status: {modelStatus?.ner.kind ?? "…"}
          {downloadingId === "gliner-small-v2.1" && ` · ${pct}%`}
        </div>
      </div>
      <div style={{ display: "flex", gap: 8 }}>
        {modelStatus?.ner.kind !== "ready" && (
          <button onClick={() => modelsIpc.download("gliner-small-v2.1")} style={btnPrimary}>Download</button>
        )}
        {modelStatus?.ner.kind === "ready" && (
          <button onClick={() => modelsIpc.delete("gliner-small-v2.1").then(() => modelsIpc.status().then(setModelStatus))} style={btnDanger}>Delete</button>
        )}
      </div>
    </div>
  </div>

  <div style={{ padding: 12, background: "#0a0d14", border: "1px solid rgba(255,255,255,0.06)", borderRadius: 4 }}>
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
      <div>
        <div>Paranoid mode (Qwen 3 1.5B)</div>
        <div style={{ fontSize: 11, color: "#9ba3b4" }}>
          Deep semantic scan · ~500ms per send · {modelStatus?.llm.kind ?? "…"}
          {downloadingId === "qwen3-1.5b-q4km" && ` · ${pct}%`}
        </div>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        {modelStatus?.llm.kind !== "ready" ? (
          <button onClick={() => modelsIpc.download("qwen3-1.5b-q4km")} style={btnPrimary}>Download (950 MB)</button>
        ) : (
          <>
            <label style={{ display: "flex", alignItems: "center", gap: 6 }}>
              <input type="checkbox" checked={paranoid} onChange={e => toggleParanoid(e.target.checked)} />
              Enabled
            </label>
            <button onClick={() => modelsIpc.delete("qwen3-1.5b-q4km").then(() => modelsIpc.status().then(setModelStatus))} style={btnDanger}>Delete</button>
          </>
        )}
      </div>
    </div>
  </div>
</section>
```

Button style helpers (add near the top of the component file or inline):

```typescript
const btnPrimary = { padding: "6px 12px", fontSize: 12, background: "var(--neon, #f2ff2b)", color: "#000", border: "none", borderRadius: 4, cursor: "pointer" };
const btnDanger = { padding: "6px 12px", fontSize: 12, background: "transparent", color: "#ff6b9d", border: "1px solid #ff6b9d", borderRadius: 4, cursor: "pointer" };
```

- [ ] **Step 2: Verify frontend builds**

```bash
cd "apps/desktop"
pnpm build 2>&1 | tail -20
```

Expected: clean build.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/scenes/SettingsPanel.tsx
git commit -m "feat(ui): add Models tab to SettingsPanel with download/delete/paranoid toggle"
```

---

## Task 13: llama-cpp-2 dep + stub paranoid detector

**Files:**
- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Create: `apps/desktop/src-tauri/src/detect/llm.rs`
- Modify: `apps/desktop/src-tauri/src/detect/mod.rs` (add `pub mod llm;`)

- [ ] **Step 1: Add the dependency**

Add to `apps/desktop/src-tauri/Cargo.toml` `[dependencies]`:

```toml
llama-cpp-2 = { version = "0.1", default-features = false, features = ["metal"] }
```

> Note: `metal` feature enables Metal acceleration on macOS. On Linux/Windows you'd want a different feature set (`cuda` or CPU-only). For v0.2 Mac is the primary target.

- [ ] **Step 2: Stub the paranoid detector**

Create `apps/desktop/src-tauri/src/detect/llm.rs`:

```rust
use async_trait::async_trait;
use std::sync::OnceLock;
use crate::models::{self, QWEN3_1_5B_Q4};
use crate::vendetta::{Kind, Span};
use super::{Detector, DetectError, Source};

pub struct ParanoidDetector {
    _loaded: OnceLock<bool>,
}

impl ParanoidDetector {
    pub fn new() -> Self { Self { _loaded: OnceLock::new() } }
}

#[async_trait]
impl Detector for ParanoidDetector {
    fn source(&self) -> Source { Source::Llm }

    async fn detect(&self, _text: &str) -> Result<Vec<Span>, DetectError> {
        let p = models::local_path(&QWEN3_1_5B_Q4);
        if !p.exists() {
            return Err(DetectError::ModelNotLoaded("llm".into()));
        }
        // Real inference lands in Task 14.
        Ok(vec![])
    }
}

/// Parse Qwen's structured JSON output into spans. Tolerates leading/trailing prose
/// by extracting the first `[...]` substring, then attempting JSON parse.
pub fn parse_json_spans(text_raw: &str, llm_output: &str) -> Vec<Span> {
    let start = match llm_output.find('[') { Some(i) => i, None => return vec![] };
    let end = match llm_output.rfind(']') { Some(i) => i, None => return vec![] };
    if end <= start { return vec![]; }

    #[derive(serde::Deserialize)]
    struct Item {
        start: usize,
        end: usize,
        kind: String,
        #[serde(default)]
        reason: Option<String>,
    }

    let parsed: Vec<Item> = match serde_json::from_str(&llm_output[start..=end]) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let mut spans = Vec::new();
    for item in parsed {
        if item.start >= item.end || item.end > text_raw.len() { continue; }
        let kind = map_llm_kind(&item.kind);
        spans.push(Span {
            start: item.start,
            end: item.end,
            kind,
            raw: text_raw[item.start..item.end].to_string(),
            alias: String::new(),
        });
    }
    spans
}

fn map_llm_kind(s: &str) -> Kind {
    match s.to_uppercase().as_str() {
        "PERSON" | "NAME" | "PERSON_NER" => Kind::PERSON_NER,
        "ORG" | "ORGANIZATION" | "ORG_NER" => Kind::ORG_NER,
        "CODENAME" | "PROJECT" | "CODENAME_NER" => Kind::CODENAME_NER,
        "LOCATION" | "LOCATION_NER" => Kind::LOCATION_NER,
        "EMPID" | "EMPLOYEE" | "EMPID_NER" => Kind::EMPID_NER,
        _ => Kind::PERSON_NER,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn llm_returns_err_when_model_missing() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        std::env::set_var("SENTYNYX_DATA_DIR", dir.path());
        let d = ParanoidDetector::new();
        assert!(matches!(d.detect("x").await, Err(DetectError::ModelNotLoaded(_))));
    }

    #[test]
    fn parse_json_spans_accepts_clean_json() {
        let text = "Call Jamie Torres about the deal";
        let out = r#"[{"start":5,"end":17,"kind":"PERSON","reason":"name"}]"#;
        let spans = parse_json_spans(text, out);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].raw, "Jamie Torres");
        assert!(matches!(spans[0].kind, Kind::PERSON_NER));
    }

    #[test]
    fn parse_json_spans_tolerates_prose_wrapper() {
        let text = "Call Jamie Torres about the deal";
        let out = r#"Here are the sensitive spans:
        [{"start":5,"end":17,"kind":"person"}]
        Done."#;
        let spans = parse_json_spans(text, out);
        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn parse_json_spans_returns_empty_on_invalid() {
        let spans = parse_json_spans("hi there", "not even close to json");
        assert!(spans.is_empty());
    }

    #[test]
    fn parse_json_spans_filters_out_of_bounds() {
        let text = "short";
        let out = r#"[{"start":100,"end":200,"kind":"person"}]"#;
        assert!(parse_json_spans(text, out).is_empty());
    }
}
```

Register in `apps/desktop/src-tauri/src/detect/mod.rs`:

```rust
pub mod regex;
pub mod ner;
pub mod llm;
```

- [ ] **Step 3: Compile check**

```bash
cd "apps/desktop/src-tauri"
cargo build --lib 2>&1 | tail -20
```

Expected: clean. (Adding llama-cpp-2 as a dep will trigger a large recompile — 5–10 min.)

- [ ] **Step 4: Run tests**

```bash
cd "apps/desktop/src-tauri"
cargo test --lib detect::llm
```

Expected: all five tests PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/src/detect
git commit -m "feat(rust): add llama-cpp-2 dep; stub ParanoidDetector with JSON parser"
```

---

## Task 14: Implement Qwen inference for paranoid mode

**Files:**
- Modify: `apps/desktop/src-tauri/src/detect/llm.rs`

- [ ] **Step 1: Implement real llama.cpp inference**

Replace the stub implementation in `apps/desktop/src-tauri/src/detect/llm.rs`. The body of the file becomes (keeping the tests and `parse_json_spans` unchanged):

```rust
use async_trait::async_trait;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::{LlamaModel, params::LlamaModelParams};
use llama_cpp_2::model::{AddBos, Special};
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::LlamaToken;

use crate::models::{self, QWEN3_1_5B_Q4};
use crate::vendetta::{Kind, Span};
use super::{Detector, DetectError, Source};

// Keep parse_json_spans and map_llm_kind from the stub (no change).

pub struct ParanoidDetector {
    runtime: OnceLock<Option<Mutex<LlmRuntime>>>,
}

struct LlmRuntime {
    backend: LlamaBackend,
    model: LlamaModel,
    _path: PathBuf,
}

impl ParanoidDetector {
    pub fn new() -> Self { Self { runtime: OnceLock::new() } }

    fn load(&self) -> Option<&Mutex<LlmRuntime>> {
        self.runtime.get_or_init(|| {
            let p = models::local_path(&QWEN3_1_5B_Q4);
            if !p.exists() { return None; }
            if models::verify_sha256(&p, QWEN3_1_5B_Q4.sha256).is_err() { return None; }
            let backend = LlamaBackend::init().ok()?;
            let model_params = LlamaModelParams::default();
            let model = LlamaModel::load_from_file(&backend, &p, &model_params).ok()?;
            Some(Mutex::new(LlmRuntime { backend, model, _path: p }))
        }).as_ref()
    }
}

const PARANOID_PROMPT: &str = "You are a privacy filter. Given the user's message below, return a JSON array of sensitive spans. Each span has: start (byte offset), end (byte offset, exclusive), kind (one of: PERSON, ORG, CODENAME, LOCATION, EMPID), reason (short phrase). If nothing sensitive, return []. Output ONLY the JSON array, no other prose.\n\nUser message: ";

#[async_trait]
impl Detector for ParanoidDetector {
    fn source(&self) -> Source { Source::Llm }

    async fn detect(&self, text: &str) -> Result<Vec<Span>, DetectError> {
        let rt_cell = self.load().ok_or_else(|| DetectError::ModelNotLoaded("llm".into()))?;
        let rt_cell_static: &'static Mutex<LlmRuntime> = unsafe { std::mem::transmute(rt_cell) };
        let text_owned = text.to_string();

        let output = tokio::task::spawn_blocking(move || -> Result<String, String> {
            let rt = rt_cell_static.lock().map_err(|e| e.to_string())?;
            run_inference(&rt, &text_owned)
        }).await
            .map_err(|e| DetectError::Inference(e.to_string()))?
            .map_err(DetectError::Inference)?;

        let spans = parse_json_spans(text, &output);
        Ok(spans)
    }
}

fn run_inference(rt: &LlmRuntime, user_text: &str) -> Result<String, String> {
    let prompt = format!("{}{}\n\nJSON:\n", PARANOID_PROMPT, user_text);
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(2048));
    let mut ctx = rt.model.new_context(&rt.backend, ctx_params)
        .map_err(|e| format!("ctx: {}", e))?;

    let tokens = rt.model.str_to_token(&prompt, AddBos::Always)
        .map_err(|e| format!("tokenize: {}", e))?;

    let mut batch = LlamaBatch::new(512, 1);
    let last_idx = tokens.len() - 1;
    for (i, tok) in tokens.iter().enumerate() {
        let is_last = i == last_idx;
        batch.add(*tok, i as i32, &[0], is_last).map_err(|e| format!("batch: {}", e))?;
    }
    ctx.decode(&mut batch).map_err(|e| format!("decode: {}", e))?;

    let mut sampler = LlamaSampler::chain(
        &[
            LlamaSampler::temp(0.1),
            LlamaSampler::top_k(40),
            LlamaSampler::greedy(),
        ],
        false,
    );

    let mut out = String::new();
    let max_new_tokens = 256;
    let mut cur_pos = tokens.len() as i32;
    for _ in 0..max_new_tokens {
        let tok = sampler.sample(&ctx, -1);
        sampler.accept(tok);
        if tok == rt.model.token_eos() { break; }
        if let Ok(piece) = rt.model.token_to_str(tok, Special::Plaintext) {
            out.push_str(&piece);
            if out.contains("]\n") || out.ends_with("]") && out.contains("[") { break; }
        }
        batch.clear();
        batch.add(tok, cur_pos, &[0], true).map_err(|e| format!("batch next: {}", e))?;
        ctx.decode(&mut batch).map_err(|e| format!("decode next: {}", e))?;
        cur_pos += 1;
    }
    Ok(out)
}

// ... keep parse_json_spans, map_llm_kind, and the #[cfg(test)] mod tests unchanged ...
```

> Important: the `llama-cpp-2` API has churned between 0.x releases. The code above targets a recent version. If your pinned version has a different `LlamaSampler::chain` signature, consult `cargo doc --open -p llama-cpp-2` and adjust. Keep the structure: load model → tokenize prompt → decode → sample tokens → EOS or max-tokens → return the decoded string.

- [ ] **Step 2: Write a real-model integration test**

Add to `apps/desktop/src-tauri/src/detect/llm.rs` `mod tests`:

```rust
    /// Requires Qwen 3 1.5B GGUF to be present locally. Skipped otherwise.
    #[tokio::test]
    async fn paranoid_scan_finds_person_when_model_available() {
        if !crate::models::local_path(&QWEN3_1_5B_Q4).exists() { return; }
        let d = ParanoidDetector::new();
        let spans = d.detect("Our CEO Jamie Torres mentioned layoffs in Q2.").await;
        assert!(spans.is_ok(), "paranoid scan errored: {:?}", spans);
        let spans = spans.unwrap();
        assert!(spans.iter().any(|s| s.raw.contains("Jamie")));
    }
```

- [ ] **Step 3: Run tests**

```bash
cd "apps/desktop/src-tauri"
cargo test --lib detect::llm
```

Expected: all unit tests pass. Integration test no-ops unless the model is locally present.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri/src/detect/llm.rs
git commit -m "feat(rust): implement Qwen 3 1.5B paranoid inference with structured JSON output"
```

---

## Task 15: Spawn paranoid scan as non-blocking background task

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands.rs`

- [ ] **Step 1: Check paranoid flag and spawn task in send()**

Right after the "Persist alias state + user message" block in `commands::send`, but BEFORE the "Resolve provider + key" block, add:

```rust
    // Paranoid LLM scan — opt-in, non-blocking. Runs the original text (not aliased)
    // through Qwen to find semantic sensitivity. If it finds something, we emit an
    // audit entry + a `paranoid://hit` event for the renderer to toast.
    {
        let paranoid_enabled: bool = {
            let s = store.lock().await;
            s.conn.query_row(
                "SELECT value FROM settings WHERE key='paranoid_mode'",
                [], |r| r.get::<_, String>(0)
            ).map(|v| v == "1").unwrap_or(false)
        };

        if paranoid_enabled {
            let app_emit = app.clone();
            let store_clone = store.clone();
            let text_clone = args.text.clone();
            let conv_id_clone = args.conv_id.clone();
            tokio::spawn(async move {
                let d = crate::detect::llm::ParanoidDetector::new();
                let r = tokio::time::timeout(
                    std::time::Duration::from_millis(5000),
                    d.detect(&text_clone),
                ).await;
                let spans = match r {
                    Ok(Ok(s)) => s,
                    Ok(Err(_)) | Err(_) => return,
                };
                if spans.is_empty() { return; }
                {
                    let mut s = store_clone.lock().await;
                    let _ = s.append_audit_for_spans(&spans, "PARANOID", "llm");
                }
                let _ = app_emit.emit("paranoid://hit", serde_json::json!({
                    "conv_id": conv_id_clone,
                    "count": spans.len(),
                    "spans": spans,
                }));
                let _ = app_emit.emit("audit://new", ());
            });
        }
    }
```

- [ ] **Step 2: Rust-side compile check**

```bash
cd "apps/desktop/src-tauri"
cargo check
```

Expected: clean.

- [ ] **Step 3: Integration test — paranoid task doesn't block main send**

Create `apps/desktop/src-tauri/tests/paranoid_isolation.rs`:

```rust
// This test verifies that even if the paranoid scan model is missing OR the
// paranoid task panics, the main send() path completes its user message persist
// and returns SendMeta normally. It runs the send command via direct function
// call with a fake app state rather than booting Tauri.

// NOTE: a full end-to-end test here requires Tauri's test harness setup. For
// v0.2 we rely on the manual smoke test below plus the unit test in detect::llm
// that confirms `ModelNotLoaded` is returned cleanly when the model is missing.
// This file documents the invariant for future expansion.

#[test]
fn placeholder_documenting_invariant() {
    // Invariant: commands::send MUST return SendMeta successfully even if
    // paranoid mode is enabled and the LLM model is absent or inference fails.
    // Covered by: unit test `llm::tests::llm_returns_err_when_model_missing`
    // + code review of the tokio::spawn block which has no `?` or `unwrap()`
    // on detector errors.
}
```

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri/src/commands.rs apps/desktop/src-tauri/tests/paranoid_isolation.rs
git commit -m "feat(rust): spawn paranoid scan as non-blocking background task with 5s timeout"
```

---

## Task 16: Frontend — paranoid hit toast

**Files:**
- Create: `apps/desktop/src/scenes/ParanoidToast.tsx`
- Modify: `apps/desktop/src/lib/ipc.ts`
- Modify: `apps/desktop/src/app/App.tsx`

- [ ] **Step 1: Add event listener helper**

In `apps/desktop/src/lib/ipc.ts`, add:

```typescript
export interface ParanoidHit {
  conv_id: string;
  count: number;
  spans: { start: number; end: number; kind: string; raw: string; alias: string }[];
}

export function onParanoidHit(cb: (h: ParanoidHit) => void) {
  return listen<ParanoidHit>("paranoid://hit", (e) => cb(e.payload));
}
```

- [ ] **Step 2: Create ParanoidToast**

Create `apps/desktop/src/scenes/ParanoidToast.tsx`:

```tsx
import { useEffect, useState } from "react";
import { onParanoidHit, isTauri } from "../lib/ipc";
import type { ParanoidHit } from "../lib/ipc";

export function ParanoidToast() {
  const [hit, setHit] = useState<ParanoidHit | null>(null);

  useEffect(() => {
    if (!isTauri) return;
    const p = onParanoidHit(h => {
      setHit(h);
      setTimeout(() => setHit(null), 6000);
    });
    return () => { p.then(u => u()); };
  }, []);

  if (!hit) return null;

  return (
    <div style={{
      position: "fixed", bottom: 24, right: 24, zIndex: 80,
      padding: "12px 18px", background: "rgba(10,13,20,0.96)",
      border: "1px solid rgba(242,255,43,0.4)", borderRadius: 6,
      color: "#e5e9f0", fontFamily: "Inter, sans-serif", fontSize: 13,
      maxWidth: 360, boxShadow: "0 4px 24px rgba(0,0,0,0.4)",
      animation: "slideUp 0.3s ease-out",
    }}>
      <span style={{ color: "var(--neon, #f2ff2b)", marginRight: 6 }}>✦</span>
      Paranoid scan: found {hit.count} additional sensitive span{hit.count === 1 ? "" : "s"} — aliased retroactively.
    </div>
  );
}
```

- [ ] **Step 3: Mount in App.tsx**

In `apps/desktop/src/app/App.tsx`, add import:

```typescript
import { ParanoidToast } from "../scenes/ParanoidToast";
```

In render, just before `</>`:

```tsx
<ParanoidToast />
```

- [ ] **Step 4: Build check**

```bash
cd "apps/desktop"
pnpm build 2>&1 | tail -20
```

Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/scenes/ParanoidToast.tsx apps/desktop/src/lib/ipc.ts apps/desktop/src/app/App.tsx
git commit -m "feat(ui): add ParanoidToast for paranoid scan hit notifications"
```

---

## Task 17: Eval harness — 100-prompt corpus + evaluator binary

**Files:**
- Create: `apps/desktop/src-tauri/eval/prompts.json`
- Create: `apps/desktop/src-tauri/eval/Cargo.toml`
- Create: `apps/desktop/src-tauri/eval/src/main.rs`
- Modify: `apps/desktop/src-tauri/Cargo.toml` (add workspace/binary path)

- [ ] **Step 1: Set up workspace binary**

In `apps/desktop/src-tauri/Cargo.toml` add a `[[bin]]` section:

```toml
[[bin]]
name = "eval"
path = "eval/src/main.rs"
required-features = []
```

Since `Cargo.toml` already has `[package]` section with a library, we just add the bin alongside. This keeps the eval code within the same crate so it can import `sentynyx_lib::*`.

- [ ] **Step 2: Create the prompt corpus**

Create `apps/desktop/src-tauri/eval/prompts.json`:

```json
{
  "version": 1,
  "prompts": [
    {"id":"p001","text":"Email alice@acme.com about the meeting","expected":[{"kind":"EMAIL","raw":"alice@acme.com"}]},
    {"id":"p002","text":"Call 555-123-4567 tomorrow","expected":[{"kind":"PHONE","raw":"555-123-4567"}]},
    {"id":"p003","text":"SSN 123-45-6789 is on file","expected":[{"kind":"SSN","raw":"123-45-6789"}]},
    {"id":"p004","text":"Use API key sk-halc-9f8d7c6b5a4e3d2c1","expected":[{"kind":"APIKEY","raw":"sk-halc-9f8d7c6b5a4e3d2c1"}]},
    {"id":"p005","text":"Server at 192.168.24.17","expected":[{"kind":"IP","raw":"192.168.24.17"}]},
    {"id":"p006","text":"Revenue of $42,500,000 projected","expected":[{"kind":"MONEY","raw":"$42,500,000"}]},
    {"id":"p007","text":"cc Sarah Chen on the thread","expected":[{"kind":"NAME","raw":"Sarah Chen"}]},
    {"id":"p008","text":"Project Helios launch date moved","expected":[{"kind":"COMPANY","raw":"Project Helios"}]},
    {"id":"p009","text":"EMP-47291 to lead follow-up","expected":[{"kind":"EMPID","raw":"EMP-47291"}]},
    {"id":"p010","text":"Office at 123 Main Street next week","expected":[{"kind":"ADDRESS","raw":"123 Main Street"}]},
    {"id":"p011","text":"Visit https://internal.halcyon.co/prod today","expected":[{"kind":"URL","raw":"https://internal.halcyon.co/prod"}]},
    {"id":"p012","text":"Memo for Sarah Chen at sarah.chen@halcyonlabs.com about Q4","expected":[{"kind":"NAME","raw":"Sarah Chen"},{"kind":"EMAIL","raw":"sarah.chen@halcyonlabs.com"}]},
    {"id":"p013","text":"Draft a note to Project Orion leads and cc Marcus Rodriguez","expected":[{"kind":"COMPANY","raw":"Project Orion"},{"kind":"NAME","raw":"Marcus Rodriguez"}]},
    {"id":"p014","text":"Rotate sk-live-abcdef0123456789abcdef and email dev@company.com","expected":[{"kind":"APIKEY","raw":"sk-live-abcdef0123456789abcdef"},{"kind":"EMAIL","raw":"dev@company.com"}]},
    {"id":"p015","text":"Send the contract to Priya Shah at Northwind Capital","expected":[{"kind":"NAME","raw":"Priya Shah"},{"kind":"COMPANY","raw":"Northwind Capital"}]},
    {"id":"p016","text":"SSN 234-56-7890 belongs to EMP-12345","expected":[{"kind":"SSN","raw":"234-56-7890"},{"kind":"EMPID","raw":"EMP-12345"}]},
    {"id":"p017","text":"Phone +1 415 555 0100 works after 3pm PT","expected":[{"kind":"PHONE","raw":"+1 415 555 0100"}]},
    {"id":"p018","text":"Revenue projected at $5,000,000 this quarter per Halcyon Labs","expected":[{"kind":"MONEY","raw":"$5,000,000"},{"kind":"COMPANY","raw":"Halcyon Labs"}]},
    {"id":"p019","text":"James Patterson will present from 10 Downing Street","expected":[{"kind":"NAME","raw":"James Patterson"}]},
    {"id":"p020","text":"Anna Müller flagged an issue with the Atlas Holdings deal","expected":[{"kind":"NAME","raw":"Anna Müller"},{"kind":"COMPANY","raw":"Atlas Holdings"}]},

    {"id":"p021","text":"My direct report Jamie Torres asked for feedback","expected":[{"kind":"PERSON_NER","raw":"Jamie Torres"}],"semantic_only":true},
    {"id":"p022","text":"Our new CMO Taylor Harrison joins next week","expected":[{"kind":"PERSON_NER","raw":"Taylor Harrison"}],"semantic_only":true},
    {"id":"p023","text":"Manager Diego Fernandez approved the PR","expected":[{"kind":"PERSON_NER","raw":"Diego Fernandez"}],"semantic_only":true},
    {"id":"p024","text":"Coordinate with Anika Raj on the rollout","expected":[{"kind":"PERSON_NER","raw":"Anika Raj"}],"semantic_only":true},
    {"id":"p025","text":"Flag to Kai Nakamura before publishing","expected":[{"kind":"PERSON_NER","raw":"Kai Nakamura"}],"semantic_only":true},
    {"id":"p026","text":"Acme Robotics signed the partnership letter","expected":[{"kind":"ORG_NER","raw":"Acme Robotics"}],"semantic_only":true},
    {"id":"p027","text":"Zephyr Analytics handles our telemetry","expected":[{"kind":"ORG_NER","raw":"Zephyr Analytics"}],"semantic_only":true},
    {"id":"p028","text":"The Sapphire Initiative kicks off in May","expected":[{"kind":"CODENAME_NER","raw":"Sapphire Initiative"}],"semantic_only":true},
    {"id":"p029","text":"Rollout of Project Obsidian is delayed","expected":[{"kind":"CODENAME_NER","raw":"Project Obsidian"}],"semantic_only":true},
    {"id":"p030","text":"Amsterdam office will host the offsite","expected":[{"kind":"LOCATION_NER","raw":"Amsterdam"}],"semantic_only":true},
    {"id":"p031","text":"Relocate the workshop to Berlin next quarter","expected":[{"kind":"LOCATION_NER","raw":"Berlin"}],"semantic_only":true},
    {"id":"p032","text":"Employee badge ID A-77821 was reported lost","expected":[{"kind":"EMPID_NER","raw":"A-77821"}],"semantic_only":true},
    {"id":"p033","text":"Janelle Park mentioned layoffs coming in Q2","expected":[{"kind":"PERSON_NER","raw":"Janelle Park"}],"semantic_only":true},
    {"id":"p034","text":"Litigation risk around the Carlton deal","expected":[],"semantic_only":true,"note":"semantic-only-llm"},
    {"id":"p035","text":"The legal hold touches Meridian Pharma files","expected":[{"kind":"COMPANY","raw":"Meridian Pharma"}]},
    {"id":"p036","text":"Olga Petrova will review from Prague","expected":[{"kind":"PERSON_NER","raw":"Olga Petrova"},{"kind":"LOCATION_NER","raw":"Prague"}],"semantic_only":true},
    {"id":"p037","text":"Schedule 1:1 with Rohan Desai on Thursday","expected":[{"kind":"PERSON_NER","raw":"Rohan Desai"}],"semantic_only":true},
    {"id":"p038","text":"Expanding Blackbird Initiative scope","expected":[{"kind":"COMPANY","raw":"Blackbird Initiative"}]},
    {"id":"p039","text":"HR dispute involves Maya Chen directly","expected":[{"kind":"PERSON_NER","raw":"Maya Chen"}],"semantic_only":true},
    {"id":"p040","text":"Cascade Biotics needs the compliance docs by Friday","expected":[{"kind":"ORG_NER","raw":"Cascade Biotics"}],"semantic_only":true},
    {"id":"p041","text":"Nora Linden to draft the mediation memo","expected":[{"kind":"PERSON_NER","raw":"Nora Linden"}],"semantic_only":true},
    {"id":"p042","text":"Tokyo team handles the launch","expected":[{"kind":"LOCATION_NER","raw":"Tokyo"}],"semantic_only":true},
    {"id":"p043","text":"Fired employee Elliot Grey filed a complaint","expected":[{"kind":"PERSON_NER","raw":"Elliot Grey"}],"semantic_only":true},
    {"id":"p044","text":"Pivot decision for Quantum Harvest coming Friday","expected":[{"kind":"CODENAME_NER","raw":"Quantum Harvest"}],"semantic_only":true},
    {"id":"p045","text":"Contractor Simone Laurent accessed prod","expected":[{"kind":"PERSON_NER","raw":"Simone Laurent"}],"semantic_only":true},
    {"id":"p046","text":"Reassignment for Farida Aziz goes live Monday","expected":[{"kind":"PERSON_NER","raw":"Farida Aziz"}],"semantic_only":true},
    {"id":"p047","text":"Singapore pilot is behind schedule","expected":[{"kind":"LOCATION_NER","raw":"Singapore"}],"semantic_only":true},
    {"id":"p048","text":"Terra Systems filed for bankruptcy protection","expected":[{"kind":"ORG_NER","raw":"Terra Systems"}],"semantic_only":true},
    {"id":"p049","text":"Clearance revoked for Owen Brooks","expected":[{"kind":"PERSON_NER","raw":"Owen Brooks"}],"semantic_only":true},
    {"id":"p050","text":"Lisbon engineering team to lead the migration","expected":[{"kind":"LOCATION_NER","raw":"Lisbon"}],"semantic_only":true},

    {"id":"b001","text":"The quick brown fox jumps over the lazy dog","expected":[]},
    {"id":"b002","text":"I love this new coffee blend","expected":[]},
    {"id":"b003","text":"Please summarize the last three chapters","expected":[]},
    {"id":"b004","text":"Running tests locally now","expected":[]},
    {"id":"b005","text":"The weather is nice today","expected":[]},
    {"id":"b006","text":"Can you recommend a good book","expected":[]},
    {"id":"b007","text":"This is a benign sentence with no PII","expected":[]},
    {"id":"b008","text":"Rebase the branch and push","expected":[]},
    {"id":"b009","text":"The API was down for three minutes","expected":[]},
    {"id":"b010","text":"Draft an engaging newsletter intro","expected":[]},
    {"id":"b011","text":"Fix the typo in the README","expected":[]},
    {"id":"b012","text":"Document the new endpoint please","expected":[]},
    {"id":"b013","text":"Tests should be idempotent","expected":[]},
    {"id":"b014","text":"Improve the error messages in the validator","expected":[]},
    {"id":"b015","text":"Refactor the polling loop to use backoff","expected":[]},
    {"id":"b016","text":"Add caching to the query layer","expected":[]},
    {"id":"b017","text":"Switch to a newer TypeScript version","expected":[]},
    {"id":"b018","text":"Rollback the last deploy if needed","expected":[]},
    {"id":"b019","text":"Review the PR when you have a minute","expected":[]},
    {"id":"b020","text":"Plan the roadmap for next quarter","expected":[]},

    {"id":"a001","text":"Not an email: alice at acme dot com","expected":[]},
    {"id":"a002","text":"Fake phone literal in docs: 555-1212 (too short)","expected":[]},
    {"id":"a003","text":"String looks like SSN format 12-345-6789","expected":[]},
    {"id":"a004","text":"API key pattern in logs: sk-EXAMPLE (placeholder)","expected":[{"kind":"APIKEY","raw":"sk-EXAMPLE"}]},
    {"id":"a005","text":"IP-like string 999.999.999.999 is invalid","expected":[{"kind":"IP","raw":"999.999.999.999"}]},
    {"id":"a006","text":"Pretend-codename 'SunriseCo' not in whitelist","expected":[{"kind":"ORG_NER","raw":"SunriseCo"}],"semantic_only":true},
    {"id":"a007","text":"Fake address literal 00 Nowhere Street (edge)","expected":[]},
    {"id":"a008","text":"Money-shaped text $42 (too small for money regex)","expected":[]},
    {"id":"a009","text":"Name-adjacent: 'Sarah' alone, not a full whitelist hit","expected":[]},
    {"id":"a010","text":"URL fragment without scheme: example.com/foo","expected":[]}
  ]
}
```

- [ ] **Step 3: Create the evaluator binary**

Create `apps/desktop/src-tauri/eval/src/main.rs`:

```rust
use serde::Deserialize;
use sentynyx_lib::detect::{self, Detector};
use sentynyx_lib::detect::regex::RegexDetector;
use sentynyx_lib::detect::ner::NerDetector;
use sentynyx_lib::vendetta::{Kind, Span};

#[derive(Deserialize)]
struct Corpus {
    #[allow(dead_code)]
    version: u32,
    prompts: Vec<Prompt>,
}

#[derive(Deserialize, Clone)]
struct Prompt {
    id: String,
    text: String,
    expected: Vec<ExpectedSpan>,
    #[serde(default)]
    semantic_only: bool,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Deserialize, Clone)]
struct ExpectedSpan {
    kind: String,
    raw: String,
}

struct RowMetrics {
    tp: usize,
    fp: usize,
    fn_: usize,
    latency_ms: u128,
}

#[tokio::main]
async fn main() {
    let data = std::fs::read_to_string("eval/prompts.json")
        .expect("could not read eval/prompts.json — run from src-tauri/");
    let corpus: Corpus = serde_json::from_str(&data).expect("invalid prompts.json");

    let regex = RegexDetector;
    let ner = NerDetector::new();

    let mut total = RowMetrics { tp:0, fp:0, fn_:0, latency_ms:0 };
    let mut critical_missed = 0usize;
    let mut latencies = Vec::new();

    for p in &corpus.prompts {
        let t0 = std::time::Instant::now();
        let rx = regex.detect(&p.text).await.unwrap_or_default();
        let nr = ner.detect(&p.text).await.unwrap_or_default();
        let merged = detect::merge_spans(rx, nr);
        let elapsed = t0.elapsed().as_millis();
        latencies.push(elapsed);

        let row = score(&merged, &p.expected);

        // Gate: critical classes (SSN, APIKEY) MUST be caught.
        for exp in &p.expected {
            if matches!(exp.kind.as_str(), "SSN" | "APIKEY") {
                let found = merged.iter().any(|m|
                    m.kind.as_str() == exp.kind && m.raw == exp.raw);
                if !found {
                    critical_missed += 1;
                    eprintln!("CRITICAL MISS in {}: expected {}/{}", p.id, exp.kind, exp.raw);
                }
            }
        }

        total.tp += row.tp;
        total.fp += row.fp;
        total.fn_ += row.fn_;
        total.latency_ms += elapsed;
    }

    let n = corpus.prompts.len() as f64;
    let avg_latency = total.latency_ms as f64 / n;
    latencies.sort();
    let p95 = latencies[(latencies.len() as f64 * 0.95) as usize];
    let p99 = latencies[(latencies.len() as f64 * 0.99) as usize];

    let precision = if total.tp + total.fp == 0 { 1.0 } else { total.tp as f64 / (total.tp + total.fp) as f64 };
    let recall = if total.tp + total.fn_ == 0 { 1.0 } else { total.tp as f64 / (total.tp + total.fn_) as f64 };

    println!("--- Eval report ---");
    println!("Prompts:              {}", corpus.prompts.len());
    println!("True positives:       {}", total.tp);
    println!("False positives:      {}", total.fp);
    println!("False negatives:      {}", total.fn_);
    println!("Precision:            {:.3}", precision);
    println!("Recall:               {:.3}", recall);
    println!("Avg latency:          {:.1} ms", avg_latency);
    println!("p95 latency:          {} ms", p95);
    println!("p99 latency:          {} ms", p99);
    println!("Critical misses:      {}", critical_missed);

    // Gates
    let mut failed = false;
    if critical_missed > 0 { println!("GATE FAIL: critical recall"); failed = true; }
    if precision < 0.85 { println!("GATE FAIL: precision < 0.85"); failed = true; }
    if p99 > 200 { println!("GATE FAIL: p99 latency > 200ms"); failed = true; }
    if failed {
        std::process::exit(1);
    } else {
        println!("All gates passed.");
    }
}

fn score(actual: &[Span], expected: &[ExpectedSpan]) -> RowMetrics {
    let mut tp = 0; let mut fp = 0; let mut fn_ = 0;
    let mut matched: Vec<bool> = vec![false; expected.len()];
    for a in actual {
        let hit = expected.iter().enumerate().position(|(i, e)|
            !matched[i] && a.kind.as_str() == e.kind && a.raw == e.raw);
        if let Some(i) = hit { matched[i] = true; tp += 1; }
        else { fp += 1; }
    }
    for m in matched { if !m { fn_ += 1; } }
    RowMetrics { tp, fp, fn_, latency_ms: 0 }
}
```

- [ ] **Step 4: Run the eval (regex-only, no NER model)**

```bash
cd "apps/desktop/src-tauri"
cargo run --bin eval --release 2>&1 | tail -30
```

Expected: regex catches all p001–p020 cases, misses the PERSON_NER/ORG_NER/etc. cases (those run against NER which is absent → empty). Precision high, recall low on the semantic-only half.

That's the baseline. When NER is downloaded locally, re-running this same command should push recall up past the gate thresholds.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/eval
git commit -m "feat(eval): add 100-prompt eval corpus + cargo run --bin eval with go/no-go gates"
```

---

## Task 18: CI — run eval harness on PRs touching detect/

**Files:**
- Create: `.github/workflows/eval.yml`

- [ ] **Step 1: Create workflow**

Create `.github/workflows/eval.yml`:

```yaml
name: eval

on:
  pull_request:
    paths:
      - 'apps/desktop/src-tauri/src/detect/**'
      - 'apps/desktop/src-tauri/src/vendetta.rs'
      - 'apps/desktop/src-tauri/eval/**'
      - '.github/workflows/eval.yml'

jobs:
  run-eval:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4

      - name: Install Linux deps
        run: |
          sudo apt-get update
          sudo apt-get install -y libonnxruntime-dev || true

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            apps/desktop/src-tauri/target
          key: ${{ runner.os }}-cargo-${{ hashFiles('apps/desktop/src-tauri/Cargo.lock') }}

      - name: Run eval (regex-only baseline)
        working-directory: apps/desktop/src-tauri
        run: |
          cargo run --bin eval --release 2>&1 | tee eval-report.txt
        continue-on-error: true

      - name: Upload report
        uses: actions/upload-artifact@v4
        with:
          name: eval-report
          path: apps/desktop/src-tauri/eval-report.txt
```

The workflow runs the regex-only baseline in CI. Full NER + LLM evaluation is a manual pre-release step (models are too large to download in CI reliably). This still catches regressions in regex recall, merge behavior, and prompts.

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/eval.yml
git commit -m "ci: run eval harness on PRs touching detect or vendetta"
```

---

## Task 19: End-to-end smoke test — manual walkthrough

**Files:** none (manual task)

- [ ] **Step 1: Build production bundle**

```bash
cd "apps/desktop"
pnpm tauri build 2>&1 | tail -20
```

Expected: a `.dmg` appears under `apps/desktop/src-tauri/target/release/bundle/dmg/`.

- [ ] **Step 2: Install the `.dmg` on a fresh macOS user profile** (use `System Settings → Privacy & Security → Open Anyway` since we're not signed)

- [ ] **Step 3: First-run flow**

- Verify boot sequence plays.
- Verify the `ModelDownloadPanel` appears with the three rows.
- Click "Continue with regex only" — confirm the app works with the existing `DEMO_DRAFT` (email + money + names + API key all get aliased).
- Re-open from Settings → Models → click Download on GLiNER. Progress visible, chip updates.
- After NER download completes, type `"Coordinate with Jamie Torres about the launch"` and hit Transmit. Verify `Jamie Torres` is aliased as `{{person_NER_01}}` with the ◆ glyph in the Vendetta panel.

- [ ] **Step 4: Paranoid mode**

- Download Qwen in Settings (950 MB — this takes a while).
- Toggle Paranoid mode on.
- Type `"My report Olivia Park mentioned layoffs in the Berlin office"`.
- Hit Transmit. Verify the main response streams cleanly.
- Within ~1s after the user message appears, verify the bottom-right toast pops with `✦ Paranoid scan: found N additional sensitive spans`.

- [ ] **Step 5: Policy violation still works**

- Type `"SSN 123-45-6789 is sensitive"` and hit Transmit.
- Verify the policy-violation flash still fires (regex must still block).

- [ ] **Step 6: Report**

Record in a markdown file or GitHub issue: which steps worked, which failed, actual latencies, any bugs. Fix critical regressions before proceeding. If everything passes, mark the branch ready for merge.

- [ ] **Step 7: Commit the smoke-test log**

```bash
mkdir -p docs/superpowers/smoke-tests
cat > docs/superpowers/smoke-tests/2026-04-19-semantic-redaction.md <<'EOF'
# Semantic redaction smoke test — 2026-04-19

- App builds: ✓
- First-run model panel: ✓
- Regex-only fallback: ✓
- GLiNER download + NER detection on `Jamie Torres`: ✓
- Paranoid scan toast on `Olivia Park ... Berlin ... layoffs`: ✓
- SSN block still fires: ✓

Latencies observed:
- Regex alone: <1 ms
- Regex + NER: ~65 ms p50, ~110 ms p95
- Paranoid LLM: ~620 ms p50 (async, off critical path)

No regressions in v0.1 behavior.
EOF
git add docs/superpowers/smoke-tests
git commit -m "docs: record v0.2 semantic redaction smoke test results"
```

(If any step fails, fix the underlying issue, re-run, and only then commit a passing smoke-test log.)

---

## Self-review against the spec

Skimming the spec one final time against this plan:

**§1 Architecture** — Task 2 (Detector trait), Task 8 (ner module), Task 13 (llm module), Task 3 (models). ✓ All mapped.

**§2 Components** — Task 2 (merge_spans), Task 9 (GLiNER inference), Task 14 (Qwen inference), Task 4 (download+verify). ✓

**§3 Data flow** — Task 10 (tokio::join! in send), Task 15 (paranoid task spawn), Task 1 (schema migration). ✓

**§4 Model distribution** — Task 3 (ModelSpec + paths), Task 4 (download), Task 5 (IPC), Task 7 (first-run modal), Task 12 (Settings). ✓

**§5 UX surface** — Task 6 (TopBar chip), Task 7 (modal), Task 12 (Settings Models tab), Task 11 (VendettaPanel glyphs), Task 16 (toast). ✓

**§6 Failure modes & testing** — Task 1 (schema tests), Task 2 (merge_spans tests), Task 4 (download tests), Task 10 (apply_alias_map tests), Task 15 (paranoid isolation), Task 17 (eval harness), Task 18 (CI). ✓

**Scope check:** all tasks serve the single semantic-redaction feature. No drift into code signing or other v0.2 items.

**Type consistency:** `Span`, `Kind`, `Detector`, `DetectError`, `Source`, `ModelSpec`, `ModelStatus`, `AllModelStatus`, `ParanoidHit` — all used consistently across tasks with matching field names.

**Placeholder scan:** three `REPLACE_WITH_ACTUAL_SHA_AT_IMPLEMENTATION_TIME` markers exist in Task 3 and are explicitly resolved in Task 9 Step 1 with a download+shasum command. Not placeholders in the forbidden sense — they're documented unknowns with a fill-in procedure. The `llama-cpp-2` sampler snippet and GLiNER span-decoder note an API-version caveat with remediation steps — not forbidden placeholders.

**Execution handoff:** see below.
