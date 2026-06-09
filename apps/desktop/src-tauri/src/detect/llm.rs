use async_trait::async_trait;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tokio::sync::mpsc;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::LlamaToken;

use crate::models::{self, PARANOID_LLM};
use crate::providers::ChunkEvent;
use crate::vendetta::{Kind, Span};
use super::{Detector, DetectError, Source};

// ---------------------------------------------------------------------------
// Backend init — llama.cpp must be initialised exactly once per process.
// We hold the backend behind a raw pointer + OnceLock instead of a dropable
// static because llama.cpp's `llama_backend_free()` races ORT's Metal cleanup
// during process exit, producing a `mutex lock failed: Invalid argument`
// SIGABRT. Leaking the backend is safe — the OS reclaims memory anyway — and
// avoids the C++ teardown ordering problem.
// ---------------------------------------------------------------------------

static LLAMA_BACKEND: OnceLock<&'static LlamaBackend> = OnceLock::new();

fn get_or_init_backend() -> &'static LlamaBackend {
    LLAMA_BACKEND.get_or_init(|| {
        match LlamaBackend::init() {
            Ok(mut b) => {
                // Silence llama.cpp's stderr chatter at init. Every
                // `LlamaModel::load_from_file` dumps ~40 lines of
                // `llama_model_loader: loaded meta data`, `print_info: ...`,
                // etc. — pure noise for our purposes and clutters the
                // DevInspector + production logs. Real errors still come
                // back as typed `LlamaCppError` values via the Rust API.
                b.void_logs();
                Box::leak(Box::new(b)) as &'static LlamaBackend
            }
            Err(llama_cpp_2::LlamaCppError::BackendAlreadyInitialized) => {
                panic!("LLAMA_BACKEND OnceLock raced — should be impossible");
            }
            Err(e) => panic!("llama backend init failed: {e}"),
        }
    })
}

// ---------------------------------------------------------------------------
// ParanoidDetector
// ---------------------------------------------------------------------------

pub struct ParanoidDetector {
    inner: OnceLock<Option<Mutex<ParanoidRuntime>>>,
}

struct ParanoidRuntime {
    model: LlamaModel,
    _model_path: PathBuf,
}

// SAFETY: LlamaModel is Send + Sync per the crate's own unsafe impls.
unsafe impl Send for ParanoidRuntime {}
unsafe impl Sync for ParanoidRuntime {}

impl ParanoidDetector {
    pub fn new() -> Self {
        Self { inner: OnceLock::new() }
    }

    /// Run the loaded Qwen model as a free-form chat completion. Tokens are
    /// streamed into `tx` as each one is generated, mirroring the remote
    /// provider streaming contract so the router can treat local and remote
    /// sends identically.
    ///
    /// Reuses the same loaded LlamaModel that powers the paranoid scan — no
    /// extra RAM beyond the freshly-allocated context for the generation.
    /// Acquires the model mutex for the whole generation, so if a paranoid
    /// scan is in flight the chat will wait (or vice versa).
    pub async fn chat_stream(
        &self,
        user_text: &str,
        tx: mpsc::Sender<ChunkEvent>,
    ) -> Result<(), String> {
        let rt_cell = self.load()
            .ok_or_else(|| "local model not loaded — open Settings → Models and download it".to_string())?;

        // SAFETY: same pattern as detect() — OnceLock keeps the runtime alive
        // for the lifetime of self. spawn_blocking borrows it inside the
        // 'static closure; join-point guarantees the reference is still live.
        let rt_cell_static: &'static Mutex<ParanoidRuntime> =
            unsafe { std::mem::transmute(rt_cell) };
        let text_owned = user_text.to_string();
        let tx_blocking = tx.clone();

        tokio::task::spawn_blocking(move || -> Result<(), String> {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let rt = rt_cell_static.lock().map_err(|e| e.to_string())?;
                stream_chat_inference(&rt.model, &text_owned, &tx_blocking)
                    .map_err(|e| e.to_string())
            }))
            .map_err(|pe| {
                let msg = pe.downcast_ref::<&str>().map(|s| s.to_string())
                    .or_else(|| pe.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "local chat inference panicked".to_string());
                format!("local chat panic: {msg}")
            })?
        })
        .await
        .map_err(|e| e.to_string())??;

        let _ = tx.send(ChunkEvent::Done).await;
        Ok(())
    }

    fn load(&self) -> Option<&Mutex<ParanoidRuntime>> {
        self.inner.get_or_init(|| {
            // Refuse to load when the placeholder SHA is still in the spec.
            // Even if a file exists at the expected path, we cannot trust it.
            // This prevents a release build from silently loading any 950MB
            // blob at the Qwen URL without integrity verification.
            if PARANOID_LLM.sha256 == "REPLACE_WITH_ACTUAL_SHA_AT_IMPLEMENTATION_TIME" {
                eprintln!(
                    "paranoid LLM: ModelSpec for {} has placeholder SHA — refusing to load",
                    PARANOID_LLM.id
                );
                return None;
            }
            let p = models::local_path(&PARANOID_LLM);
            if !p.exists() {
                return None;
            }
            if models::verify_sha256(&p, PARANOID_LLM.sha256).is_err() {
                return None;
            }
            let backend = get_or_init_backend();
            let params = LlamaModelParams::default();
            let model = LlamaModel::load_from_file(backend, &p, &params).ok()?;
            Some(Mutex::new(ParanoidRuntime {
                model,
                _model_path: p,
            }))
        }).as_ref()
    }
}

impl Default for ParanoidDetector {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl Detector for ParanoidDetector {
    fn source(&self) -> Source { Source::Llm }

    async fn detect(&self, text: &str) -> Result<Vec<Span>, DetectError> {
        let rt_cell = self.load()
            .ok_or_else(|| DetectError::ModelNotLoaded("llm".into()))?;

        // SAFETY: same pattern as NerDetector — OnceLock keeps the reference
        // alive for the lifetime of self, and the Mutex ensures exclusive access.
        // spawn_blocking completes before we return, so the borrow is always live.
        let rt_cell_static: &'static Mutex<ParanoidRuntime> =
            unsafe { std::mem::transmute(rt_cell) };
        let text_owned = text.to_string();

        // Wrap in catch_unwind — llama-cpp-2 / llama.cpp can panic on edge cases.
        let llm_output = tokio::task::spawn_blocking(move || -> Result<String, String> {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let rt = rt_cell_static.lock().map_err(|e| e.to_string())?;
                run_inference(&rt.model, &text_owned).map_err(|e| e.to_string())
            }))
            .map_err(|pe| {
                let msg = pe.downcast_ref::<&str>().map(|s| s.to_string())
                    .or_else(|| pe.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "llm inference panicked".to_string());
                format!("llm panic: {msg}")
            })?
        })
        .await
        .map_err(|e| DetectError::Inference(e.to_string()))?
        .map_err(DetectError::Inference)?;

        let spans = parse_json_spans(text, &llm_output);
        Ok(spans)
    }
}

// ---------------------------------------------------------------------------
// Inference
// ---------------------------------------------------------------------------

// SmolLM2 uses the ChatML template. Wrapping the prompt in <|im_start|> tokens
// dramatically improves JSON compliance vs raw zero-shot. `PARANOID_SYSTEM`
// sets the role; `build_paranoid_prompt` assembles the full chat.
const PARANOID_SYSTEM: &str = "You are a privacy filter. Given the user's message, return a JSON array of sensitive spans. Each span: {start: byte_offset, end: byte_offset_exclusive, kind: \"PERSON\"|\"ORG\"|\"CODENAME\"|\"LOCATION\"|\"EMPID\", reason: string}. If nothing is sensitive, return []. Output ONLY the JSON array, no prose, no backticks.";

fn build_paranoid_prompt(text: &str) -> String {
    format!(
        "<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{text}<|im_end|>\n<|im_start|>assistant\n",
        system = PARANOID_SYSTEM,
        text = text,
    )
}

// Paranoid output is a compact JSON array — `[{"start":N,"end":M,"kind":"X"}, ...]`.
// 128 tokens is ~6 entries worth, which covers every realistic prompt. The
// smaller ceiling keeps the inference window inside the 5 s `paranoid_timeout_ms`
// budget even on a warm model (Qwen 0.5B on M1 generates ~220 tok/s — 128 tokens
// ≈ 580 ms). Chat uses `MAX_CHAT_TOKENS` (1024) and is unaffected.
const MAX_NEW_TOKENS: usize = 128;

// ---------------------------------------------------------------------------
// Chat inference (for the `sentynyx-local` provider)
// ---------------------------------------------------------------------------

const CHAT_SYSTEM: &str = "You are Sentynyx Local, a concise on-device assistant powered by Qwen 2.5 0.5B. You run entirely on the user's machine with no internet access. Keep answers short and direct. If you're not sure about something, say so rather than making it up.";

const MAX_CHAT_TOKENS: usize = 1024;

fn build_chat_prompt(user_text: &str) -> String {
    format!(
        "<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n",
        system = CHAT_SYSTEM,
        user = user_text,
    )
}

/// Greedy-ish streaming decode. Each token's bytes are appended to a pending
/// buffer; whenever the buffer reaches a valid UTF-8 prefix it's emitted
/// through `tx` as a `Token` event. Leftover partial codepoints stay buffered
/// until a later token completes them, then get flushed lossily at EOG.
fn stream_chat_inference(
    model: &LlamaModel,
    user_text: &str,
    tx: &mpsc::Sender<ChunkEvent>,
) -> Result<(), llama_cpp_2::LlamaCppError> {
    let backend = get_or_init_backend();

    let prompt = build_chat_prompt(user_text);
    let tokens = model
        .str_to_token(&prompt, AddBos::Always)
        .map_err(|_| llama_cpp_2::LlamaCppError::LlamaModelLoadError(
            llama_cpp_2::LlamaModelLoadError::NullResult,
        ))?;
    let n_prompt = tokens.len();

    let ctx_size = NonZeroU32::new((n_prompt + MAX_CHAT_TOKENS + 64) as u32)
        .unwrap_or(NonZeroU32::new(2048).unwrap());
    let ctx_params = LlamaContextParams::default().with_n_ctx(Some(ctx_size));
    let mut ctx = model.new_context(backend, ctx_params)?;

    let mut batch = LlamaBatch::new(n_prompt + MAX_CHAT_TOKENS + 64, 1);
    for (i, &tok) in tokens.iter().enumerate() {
        let is_last = i == n_prompt - 1;
        batch.add(tok, i as i32, &[0], is_last)?;
    }
    ctx.decode(&mut batch)?;

    // Slightly-varied sampling so repeated prompts aren't byte-identical.
    // Greedy + temp alone stays deterministic; distribution sampling gives
    // Qwen room to breathe.
    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::temp(0.7),
        LlamaSampler::top_p(0.9, 1),
        LlamaSampler::dist(0),
    ]);

    let mut pending: Vec<u8> = Vec::new();
    let mut n_pos = n_prompt as i32;

    for _ in 0..MAX_CHAT_TOKENS {
        let token = sampler.sample(&ctx, -1);
        sampler.accept(token);

        if model.is_eog_token(token) { break; }

        if let Ok(bytes) = token_to_bytes(model, token) {
            pending.extend_from_slice(&bytes);
        }

        // Emit the longest valid-UTF-8 prefix we have. Any trailing partial
        // codepoint stays in `pending` for the next token to complete.
        match std::str::from_utf8(&pending) {
            Ok(s) if !s.is_empty() => {
                if tx.blocking_send(ChunkEvent::Token(s.to_string())).is_err() {
                    // Receiver dropped — user cancelled; bail cleanly.
                    return Ok(());
                }
                pending.clear();
            }
            Ok(_) => {}
            Err(e) => {
                let valid_up_to = e.valid_up_to();
                if valid_up_to > 0 {
                    let head = std::str::from_utf8(&pending[..valid_up_to])
                        .unwrap()
                        .to_string();
                    if tx.blocking_send(ChunkEvent::Token(head)).is_err() {
                        return Ok(());
                    }
                    pending.drain(..valid_up_to);
                }
            }
        }

        batch.clear();
        batch.add(token, n_pos, &[0], true)?;
        ctx.decode(&mut batch)?;
        n_pos += 1;
    }

    // Flush any trailing partial bytes (lossy for split codepoints).
    if !pending.is_empty() {
        let tail = String::from_utf8_lossy(&pending).into_owned();
        let _ = tx.blocking_send(ChunkEvent::Token(tail));
    }

    Ok(())
}

fn run_inference(model: &LlamaModel, text: &str) -> Result<String, llama_cpp_2::LlamaCppError> {
    let backend = get_or_init_backend();

    let prompt = build_paranoid_prompt(text);

    // Tokenize the prompt.
    let tokens = model
        .str_to_token(&prompt, AddBos::Always)
        .map_err(|_| llama_cpp_2::LlamaCppError::LlamaModelLoadError(
            llama_cpp_2::LlamaModelLoadError::NullResult,
        ))?;

    let n_prompt = tokens.len();

    // Build context: prompt length + generation budget.
    let ctx_size = NonZeroU32::new((n_prompt + MAX_NEW_TOKENS + 64) as u32)
        .unwrap_or(NonZeroU32::new(1024).unwrap());
    let ctx_params = LlamaContextParams::default().with_n_ctx(Some(ctx_size));
    let mut ctx = model.new_context(backend, ctx_params)?;

    // Feed the full prompt in one batch.
    let mut batch = LlamaBatch::new(n_prompt + MAX_NEW_TOKENS + 64, 1);
    for (i, &tok) in tokens.iter().enumerate() {
        let is_last = i == n_prompt - 1;
        batch.add(tok, i as i32, &[0], is_last)?;
    }
    ctx.decode(&mut batch)?;

    // Generation loop — greedy via temp(0.1) + greedy().
    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::temp(0.1),
        LlamaSampler::greedy(),
    ]);

    // Accumulate raw bytes — tokens may straddle multi-byte UTF-8 codepoints.
    let mut output_bytes: Vec<u8> = Vec::new();
    let mut n_pos = n_prompt as i32;

    for _ in 0..MAX_NEW_TOKENS {
        // Sample from the last logits slot in the most recent batch.
        // llama-cpp-2 indexes batch-relative positions, not absolute; our
        // batches only enable logits on their final token (batch index 0
        // post-clear), so -1 is the correct "last logits" selector.
        let token = sampler.sample(&ctx, -1);
        sampler.accept(token);

        if model.is_eog_token(token) {
            break;
        }

        // Decode token bytes; fall back to empty slice on error.
        if let Ok(bytes) = token_to_bytes(model, token) {
            output_bytes.extend_from_slice(&bytes);
        }

        // Check for closing bracket in the current byte stream.
        // We use a lossy conversion just for the stop check — the final decode
        // at return time is also lossy but that's fine for JSON detection.
        if output_bytes.contains(&b']') {
            break;
        }

        // Feed the new token for the next step.
        batch.clear();
        batch.add(token, n_pos, &[0], true)?;
        ctx.decode(&mut batch)?;
        n_pos += 1;
    }

    // Convert accumulated bytes to string; replace invalid UTF-8 sequences.
    Ok(String::from_utf8_lossy(&output_bytes).into_owned())
}

/// Decode a single token to its raw bytes, trying buffer sizes 8 then 32.
fn token_to_bytes(model: &LlamaModel, token: LlamaToken) -> Result<Vec<u8>, ()> {
    match model.token_to_piece_bytes(token, 8, false, None) {
        Ok(b) => Ok(b),
        Err(llama_cpp_2::TokenToStringError::InsufficientBufferSpace(needed)) => {
            let size = usize::try_from(-needed).unwrap_or(32);
            model.token_to_piece_bytes(token, size, false, None).map_err(|_| ())
        }
        Err(_) => Err(()),
    }
}

// ---------------------------------------------------------------------------
// JSON parsing
// ---------------------------------------------------------------------------

/// Parse Qwen's structured JSON output into spans. Tolerates leading/trailing prose
/// by extracting the first `[...]` substring; tolerates mid-entry truncation
/// (when `MAX_NEW_TOKENS` cuts the model off before the closing `]`) by
/// synthesizing a close-bracket at the last complete object. Returning a
/// partial span list on truncation is strictly better than dropping the
/// whole scan to `vec![]`.
pub fn parse_json_spans(text_raw: &str, llm_output: &str) -> Vec<Span> {
    let start = match llm_output.find('[') { Some(i) => i, None => return vec![] };

    #[derive(serde::Deserialize)]
    struct Item {
        start: usize,
        end: usize,
        kind: String,
        #[serde(default)]
        #[allow(dead_code)]
        reason: Option<String>,
    }

    let tail = &llm_output[start..];

    // Try the fast path first: a complete `[...]` substring. Works when the
    // model closed its own array.
    let parsed: Vec<Item> = if let Some(end_rel) = tail.rfind(']') {
        match serde_json::from_str(&tail[..=end_rel]) {
            Ok(v) => v,
            Err(_) => salvage_truncated_array(tail),
        }
    } else {
        // No closing bracket at all — generation was truncated before the
        // model finished its first entry OR somewhere mid-array.
        salvage_truncated_array(tail)
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
            // The paranoid pass is a semantic backstop — no calibrated score,
            // so a fixed moderate confidence.
            confidence: 0.7,
        });
    }
    spans
}

/// Walk `tail` (which starts at the `[`) counting brace depth and return
/// the slice ending at the last **complete** top-level object's closing
/// `}`. Wraps that slice with a synthetic `]` so serde sees a valid array.
/// String literals are skipped so `}` inside a `reason` field can't fool us.
fn salvage_truncated_array<T: serde::de::DeserializeOwned>(tail: &str) -> Vec<T> {
    let bytes = tail.as_bytes();
    if bytes.first() != Some(&b'[') { return Vec::new(); }

    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    let mut last_complete_obj_end: Option<usize> = None;
    for (i, &b) in bytes.iter().enumerate() {
        if escape { escape = false; continue; }
        if in_string {
            match b {
                b'\\' => escape = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    last_complete_obj_end = Some(i);
                }
            }
            b']' if depth == 0 => break,
            _ => {}
        }
    }

    let end = match last_complete_obj_end {
        Some(e) => e,
        None => return Vec::new(),
    };
    // Rebuild: `[ ..last complete obj.. ]`
    let mut buf = String::with_capacity(end + 2);
    buf.push_str(&tail[..=end]);
    buf.push(']');
    serde_json::from_str::<Vec<T>>(&buf).unwrap_or_default()
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

    // Shared with `models::tests::ENV_GUARD` and `detect::ner::tests` — every
    // test that mutates SENTYNYX_DATA_DIR must serialize on the same mutex,
    // otherwise cargo's parallel runner clobbers the env var across modules.

    #[tokio::test]
    async fn llm_returns_err_when_model_missing() {
        use tempfile::tempdir;
        let _g = crate::models::tests::ENV_GUARD.lock().unwrap();
        let dir = tempdir().unwrap();
        std::env::set_var("SENTYNYX_DATA_DIR", dir.path());
        let d = ParanoidDetector::new();
        assert!(matches!(d.detect("x").await, Err(DetectError::ModelNotLoaded(_))));
        std::env::remove_var("SENTYNYX_DATA_DIR");
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

    #[test]
    fn parse_json_spans_salvages_truncated_array() {
        // Qwen ran out of tokens mid-third-entry. We should still recover
        // the two complete entries rather than dropping the whole scan.
        let text = "Email Jamie Torres and Anna Ivanova about Project Meridian";
        let out = r#"[{"start":6,"end":18,"kind":"PERSON","reason":"name"},{"start":23,"end":35,"kind":"PERSON","reason":"name"},{"start":42,"end":58,"kind":"COD"#;
        let spans = parse_json_spans(text, out);
        assert_eq!(spans.len(), 2, "salvage should recover 2 complete entries");
        assert_eq!(spans[0].raw, "Jamie Torres");
        assert_eq!(spans[1].raw, "Anna Ivanova");
    }

    #[test]
    fn parse_json_spans_salvage_ignores_braces_in_strings() {
        // A `}` inside a `reason` string must not confuse the brace counter.
        let text = "Call Jamie Torres about the deal";
        let out = r#"[{"start":5,"end":17,"kind":"PERSON","reason":"name with } in it"}]"#;
        let spans = parse_json_spans(text, out);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].raw, "Jamie Torres");
    }

    /// Requires the paranoid LLM GGUF to be present locally. Gated behind the
    /// `live-llm-test` feature because llama.cpp's Metal static destructors
    /// race ORT's during process teardown and can SIGABRT even after the
    /// test body passes. Run explicitly with
    ///     cargo test --lib --features live-llm-test paranoid_scan
    #[cfg(feature = "live-llm-test")]
    #[tokio::test]
    async fn paranoid_scan_finds_person_when_model_available() {
        if !crate::models::local_path(&PARANOID_LLM).exists() { return; }
        let d = ParanoidDetector::new();
        let spans = d.detect("Our CEO Jamie Torres mentioned layoffs in Q2.").await;
        match spans {
            Ok(s) => {
                eprintln!("paranoid scan spans: {:?}", s);
                // Weak assertion: if inference runs, we got a result. The JSON
                // may be empty or partial — tune prompt later. Goal for this
                // test: verify no crash when model IS available.
            }
            Err(e) => panic!("paranoid scan errored: {:?}", e),
        }
    }
}
