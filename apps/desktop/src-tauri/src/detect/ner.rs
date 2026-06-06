use async_trait::async_trait;
use ndarray::{Array2, Array3};
use ort::{session::Session, value::Tensor};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tokenizers::{InputSequence, Tokenizer};
use crate::models::{self, GLINER_SMALL, GLINER_TOKENIZER};
use crate::vendetta::{Kind, Span};
use super::{Detector, DetectError, Source};

/// GLiNER label → Kind mapping.  The label strings must match exactly what the
/// model was trained on (case-sensitive, whitespace-sensitive).
pub const NER_LABELS: &[(&str, Kind)] = &[
    ("person",                    Kind::PERSON_NER),
    ("organization",              Kind::ORG_NER),
    ("internal-project-codename", Kind::CODENAME_NER),
    ("location",                  Kind::LOCATION_NER),
    ("employee-id-code",          Kind::EMPID_NER),
];

/// Maximum span width (in words) supported by the GLiNER model.
/// Must match `max_width` in `gliner_config.json` (= 12).
const MAX_WIDTH: usize = 12;

/// GLiNER special token IDs as defined by the model's vocabulary.
/// These must match the tokenizer.json vocab entries exactly.
/// <<ENT>> token ID in the GLiNER vocabulary (= 128002). Documented here as a reference;
/// the actual values flow through the pre-tokenized encoding automatically.
#[allow(dead_code)]
const ENT_TOKEN_ID: u32 = 128_002;
/// <<SEP>> token ID in the GLiNER vocabulary (= 128003).
#[allow(dead_code)]
const SEP_TOKEN_ID: u32 = 128_003;

/// Regex for GLiNER's WhitespaceTokenSplitter: `\w+(?:[-_]\w+)*|\S`
/// This keeps hyphenated/underscore words together and splits punctuation separately.
const WHITESPACE_SPLITTER_PATTERN: &str = r"\w+(?:[-_]\w+)*|\S";

// ---------------------------------------------------------------------------

pub struct NerDetector {
    inner: OnceLock<Option<Mutex<NerRuntime>>>,
}

struct NerRuntime {
    session: Session,
    tokenizer: Tokenizer,
    _onnx_path: PathBuf,
}

impl NerDetector {
    pub fn new() -> Self {
        Self { inner: OnceLock::new() }
    }

    fn load(&self) -> Option<&Mutex<NerRuntime>> {
        self.inner.get_or_init(|| {
            let onnx = models::local_path(&GLINER_SMALL);
            let tok = models::local_path(&GLINER_TOKENIZER);
            if !onnx.exists() || !tok.exists() {
                return None;
            }
            if models::verify_sha256(&onnx, GLINER_SMALL.sha256).is_err() {
                return None;
            }
            if models::verify_sha256(&tok, GLINER_TOKENIZER.sha256).is_err() {
                return None;
            }
            let session = Session::builder().ok()?.commit_from_file(&onnx).ok()?;
            let tokenizer = Tokenizer::from_file(&tok).ok()?;
            Some(Mutex::new(NerRuntime { session, tokenizer, _onnx_path: onnx }))
        }).as_ref()
    }
}

impl Default for NerDetector {
    fn default() -> Self { Self::new() }
}

/// Process-local lazy NerRuntime for the single-threaded sidecar path.
/// Used by `run_inference_sync` so the sidecar binary can call straight
/// into the same inference code without spinning up a tokio runtime or
/// bouncing through `spawn_blocking`.
static SYNC_RUNTIME: std::sync::OnceLock<Option<Mutex<NerRuntime>>> = std::sync::OnceLock::new();

fn load_sync_runtime() -> Option<&'static Mutex<NerRuntime>> {
    SYNC_RUNTIME
        .get_or_init(|| {
            let onnx = models::local_path(&GLINER_SMALL);
            let tok = models::local_path(&GLINER_TOKENIZER);
            if !onnx.exists() || !tok.exists() {
                return None;
            }
            if models::verify_sha256(&onnx, GLINER_SMALL.sha256).is_err() {
                return None;
            }
            if models::verify_sha256(&tok, GLINER_TOKENIZER.sha256).is_err() {
                return None;
            }
            let session = Session::builder().ok()?.commit_from_file(&onnx).ok()?;
            let tokenizer = Tokenizer::from_file(&tok).ok()?;
            Some(Mutex::new(NerRuntime {
                session,
                tokenizer,
                _onnx_path: onnx,
            }))
        })
        .as_ref()
}

/// Synchronous inference entry point for the sidecar binary. No tokio, no
/// spawn_blocking, no awaits — just straight inference on the caller's thread.
/// Returns a human-readable error string on any failure (model missing,
/// session init failure, tokenizer failure, ONNX run failure).
pub fn run_inference_sync(text: &str) -> Result<Vec<Span>, String> {
    let cell = load_sync_runtime().ok_or_else(|| "ner model not loaded".to_string())?;
    let mut rt = cell.lock().map_err(|e| e.to_string())?;
    run_inference(&mut rt, text)
}

#[async_trait]
impl Detector for NerDetector {
    fn source(&self) -> Source { Source::Ner }

    async fn detect(&self, text: &str) -> Result<Vec<Span>, DetectError> {
        let rt_cell = self.load().ok_or_else(|| DetectError::ModelNotLoaded("ner".into()))?;
        // SAFETY: The NerRuntime lives inside a OnceLock<Option<Mutex<_>>> which itself
        // lives inside NerDetector.  We extend the lifetime here because spawn_blocking
        // requires 'static.  The runtime is safe because:
        //   1. OnceLock guarantees the reference remains valid for the lifetime of self.
        //   2. The Mutex ensures exclusive access from the blocking thread.
        //   3. The spawned task completes before we return from detect, so the borrow
        //      is always live when the closure executes.
        let rt_cell_static: &'static Mutex<NerRuntime> =
            unsafe { std::mem::transmute(rt_cell) };
        let text_owned = text.to_string();

        // Wrap the blocking work in catch_unwind: upstream crates (spm_precompiled,
        // tokenizers) have been observed to panic on certain inputs. A panic here
        // would kill the tokio worker; we want a clean DetectError instead so the
        // send() fallback path (regex-only) just proceeds normally.
        let result = tokio::task::spawn_blocking(move || -> Result<Vec<Span>, String> {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut rt = rt_cell_static.lock().map_err(|e| e.to_string())?;
                run_inference(&mut rt, &text_owned)
            }))
            .map_err(|pe| {
                let msg = pe.downcast_ref::<&str>().map(|s| s.to_string())
                    .or_else(|| pe.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "tokenizer/inference panicked".to_string());
                format!("ner panic: {msg}")
            })?
        })
        .await
        .map_err(|e| DetectError::Inference(e.to_string()))?
        .map_err(DetectError::Inference)?;

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Preprocessing
// ---------------------------------------------------------------------------

/// Replicate GLiNER's WhitespaceTokenSplitter: `\w+(?:[-_]\w+)*|\S`
/// Returns `(word, byte_start, byte_end)` tuples.
fn split_words(text: &str) -> Vec<(&str, usize, usize)> {
    let re = regex::Regex::new(WHITESPACE_SPLITTER_PATTERN)
        .expect("WHITESPACE_SPLITTER_PATTERN is valid");
    re.find_iter(text)
        .map(|m| (m.as_str(), m.start(), m.end()))
        .collect()
}

/// Build ONNX inputs for a single (text, labels) pair using the same
/// tokenization strategy as the GLiNER Python library (is_split_into_words=True).
///
/// Returns:
///   (input_ids, attention_mask, words_mask, text_lengths, span_idx, span_mask,
///    word_char_spans)
///
/// where `word_char_spans[i] = (byte_start, byte_end)` for word i.
#[allow(clippy::type_complexity)]
fn build_inputs(
    tokenizer: &Tokenizer,
    text: &str,
    labels: &[&str],
) -> Result<
    (
        Vec<i64>,  // input_ids
        Vec<i64>,  // attention_mask
        Vec<i64>,  // words_mask
        i64,       // text_lengths (scalar = num_words)
        Vec<[i64; 2]>, // span_idx [num_spans, 2]
        Vec<bool>, // span_mask [num_spans]
        Vec<(usize, usize)>, // word char spans
    ),
    String,
> {
    // 1. Split text into words with char offsets.
    let word_spans = split_words(text);
    let words: Vec<&str> = word_spans.iter().map(|(w, _, _)| *w).collect();
    let char_spans: Vec<(usize, usize)> = word_spans.iter().map(|(_, s, e)| (*s, *e)).collect();
    let num_words = words.len();

    // 2. Build the pre-tokenized word list passed to the tokenizer.
    //    Format: [<<ENT>>, label1, <<ENT>>, label2, ..., <<SEP>>, word0, word1, ...]
    //    Each element is treated as one "word" by the SentencePiece tokenizer.
    let mut input_words: Vec<String> = Vec::new();
    for label in labels {
        input_words.push("<<ENT>>".to_string());
        input_words.push(label.to_string());
    }
    input_words.push("<<SEP>>".to_string());
    let prompt_len = input_words.len(); // number of "words" in the label prompt (before text words)
    for w in &words {
        input_words.push(w.to_string());
    }

    // 3. Tokenize with is_pretokenized=True (equivalent to HF's is_split_into_words=True).
    let word_strs: Vec<&str> = input_words.iter().map(String::as_str).collect();
    let encoding = tokenizer
        .encode(
            InputSequence::from(word_strs.as_slice()),
            true, // add_special_tokens ([CLS] and [SEP])
        )
        .map_err(|e| format!("tokenize: {e}"))?;

    let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
    // Override <<ENT>> and <<SEP>> IDs — the tokenizer might split them if not
    // added as special tokens; the GLiNER model expects their exact IDs.
    // In practice, <<ENT>>==128002 and <<SEP>>==128003 are added tokens and
    // tokenize correctly as single tokens, but we assert this is the case by
    // checking no substitutions are needed (the IDs match our constants).
    let attn: Vec<i64> = encoding.get_attention_mask().iter().map(|&x| x as i64).collect();

    // 4. Build words_mask: token → (1-indexed word index in *text*, 0 for label tokens).
    let word_ids = encoding.get_word_ids(); // &[Option<u32>]
    let mut words_mask: Vec<i64> = Vec::with_capacity(ids.len());
    for wid_opt in word_ids {
        match wid_opt {
            None => words_mask.push(0), // [CLS] / [SEP] padding tokens
            Some(wi) => {
                let wi = *wi as usize;
                if wi < prompt_len {
                    words_mask.push(0); // label-prompt token
                } else {
                    words_mask.push((wi - prompt_len + 1) as i64); // 1-indexed text word
                }
            }
        }
    }

    // 5. Build span_idx / span_mask:
    //    For each start word i and width w (0..MAX_WIDTH), generate span [i, i+w].
    //    If i+w >= num_words the span is invalid (span_mask=false) but we still
    //    include it (clamped) to keep a regular rectangular shape.
    let mut span_idx: Vec<[i64; 2]> = Vec::with_capacity(num_words * MAX_WIDTH);
    let mut span_mask: Vec<bool> = Vec::with_capacity(num_words * MAX_WIDTH);
    for wi in 0..num_words {
        for wid in 0..MAX_WIDTH {
            let end_wi = wi + wid;
            if end_wi < num_words {
                span_idx.push([wi as i64, end_wi as i64]);
                span_mask.push(true);
            } else {
                span_idx.push([wi as i64, (num_words.saturating_sub(1)) as i64]);
                span_mask.push(false);
            }
        }
    }

    Ok((ids, attn, words_mask, num_words as i64, span_idx, span_mask, char_spans))
}

// ---------------------------------------------------------------------------
// Inference
// ---------------------------------------------------------------------------

fn run_inference(rt: &mut NerRuntime, text: &str) -> Result<Vec<Span>, String> {
    let labels: Vec<&str> = NER_LABELS.iter().map(|(l, _)| *l).collect();

    if text.trim().is_empty() {
        return Ok(vec![]);
    }

    let (input_ids, attn_mask, words_mask, num_words, span_idx, span_mask, char_spans) =
        build_inputs(&rt.tokenizer, text, &labels)?;

    if num_words == 0 {
        return Ok(vec![]);
    }

    let seq_len = input_ids.len();
    let num_spans = span_idx.len();

    // Shape (1, seq_len)
    let ids_arr = Array2::from_shape_vec((1, seq_len), input_ids)
        .map_err(|e| format!("ids shape: {e}"))?;
    let attn_arr = Array2::from_shape_vec((1, seq_len), attn_mask)
        .map_err(|e| format!("attn shape: {e}"))?;
    let wmask_arr = Array2::from_shape_vec((1, seq_len), words_mask)
        .map_err(|e| format!("wmask shape: {e}"))?;
    // text_lengths shape (1, 1)
    let tlen_arr = Array2::from_shape_vec((1, 1), vec![num_words])
        .map_err(|e| format!("tlen shape: {e}"))?;
    // span_idx shape (1, num_spans, 2)
    let span_flat: Vec<i64> = span_idx.into_iter().flat_map(|[s, e]| [s, e]).collect();
    let sidx_arr = Array3::from_shape_vec((1, num_spans, 2), span_flat)
        .map_err(|e| format!("sidx shape: {e}"))?;
    // span_mask shape (1, num_spans) — bool
    let smask_arr = Array2::from_shape_vec((1, num_spans), span_mask)
        .map_err(|e| format!("smask shape: {e}"))?;

    let t_ids  = Tensor::from_array(ids_arr).map_err(|e| format!("input_ids tensor: {e}"))?;
    let t_attn = Tensor::from_array(attn_arr).map_err(|e| format!("attn tensor: {e}"))?;
    let t_wmask= Tensor::from_array(wmask_arr).map_err(|e| format!("wmask tensor: {e}"))?;
    let t_tlen = Tensor::from_array(tlen_arr).map_err(|e| format!("tlen tensor: {e}"))?;
    let t_sidx = Tensor::from_array(sidx_arr).map_err(|e| format!("sidx tensor: {e}"))?;
    let t_smask= Tensor::from_array(smask_arr).map_err(|e| format!("smask tensor: {e}"))?;

    let input_map = ort::inputs![
        "input_ids"      => t_ids,
        "attention_mask" => t_attn,
        "words_mask"     => t_wmask,
        "text_lengths"   => t_tlen,
        "span_idx"       => t_sidx,
        "span_mask"      => t_smask,
    ].map_err(|e| format!("ort inputs: {e}"))?;

    let outputs = rt
        .session
        .run(input_map)
        .map_err(|e| format!("ort run: {e}"))?;

    let logits_val = outputs.get("logits").ok_or("missing 'logits' output")?;
    let (shape, data) = logits_val
        .try_extract_raw_tensor::<f32>()
        .map_err(|e| format!("extract logits: {e}"))?;

    decode_gliner_spans(shape, data, &char_spans, text)
}

// ---------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------

/// Decode logits → Span list.
///
/// GLiNER logit tensor shape: [batch=1, num_words, max_width, num_labels]
///
/// For each (word_start, width, label) triple, sigmoid(logit) > 0.5 indicates
/// a detected entity.  We apply greedy non-overlapping selection: once a
/// character range is claimed, later overlapping spans are skipped.
fn decode_gliner_spans(
    shape: &[i64],
    data: &[f32],
    char_spans: &[(usize, usize)],
    text: &str,
) -> Result<Vec<Span>, String> {
    // Expected: [1, num_words, max_width, num_labels]
    if shape.len() != 4 {
        return Err(format!(
            "unexpected logits rank {} (expected 4)",
            shape.len()
        ));
    }
    let num_words = shape[1] as usize;
    let max_width = shape[2] as usize;
    let num_labels = shape[3] as usize;

    if char_spans.len() != num_words {
        return Err(format!(
            "char_spans len {} != num_words {}",
            char_spans.len(),
            num_words
        ));
    }

    // Collect (prob, word_start, word_end, label_idx) above threshold.
    const THRESHOLD: f32 = 0.5;
    let mut candidates: Vec<(f32, usize, usize, usize)> = Vec::new();

    for wi in 0..num_words {
        for wid in 0..max_width {
            let end_wi = wi + wid;
            if end_wi >= num_words {
                break;
            }
            for li in 0..num_labels {
                let flat_idx = ((wi * max_width + wid) * num_labels) + li;
                if flat_idx >= data.len() {
                    break;
                }
                let raw = data[flat_idx];
                let prob = 1.0_f32 / (1.0 + (-raw).exp());
                if prob > THRESHOLD {
                    candidates.push((prob, wi, end_wi, li));
                }
            }
        }
    }

    // Sort by score descending for greedy NMS.
    candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut out: Vec<Span> = Vec::new();
    let mut claimed: Vec<(usize, usize)> = Vec::new(); // claimed char ranges

    for (prob, wi, end_wi, li) in candidates {
        let char_start = char_spans[wi].0;
        let char_end = char_spans[end_wi].1;

        if char_start >= char_end || char_end > text.len() {
            continue;
        }
        // Skip if overlapping with an already-claimed span.
        if claimed.iter().any(|(s, e)| *s < char_end && char_start < *e) {
            continue;
        }
        claimed.push((char_start, char_end));

        let kind = NER_LABELS
            .get(li)
            .map(|(_, k)| k.clone())
            .unwrap_or(Kind::PERSON_NER);
        let raw_text = text[char_start..char_end].to_string();
        out.push(Span {
            start: char_start,
            end: char_end,
            kind,
            raw: raw_text,
            alias: String::new(),
        });

    }

    out.sort_by_key(|s| s.start);
    Ok(out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // Shared with `models::tests::ENV_GUARD` and `detect::llm::tests` — every
    // test that mutates SENTYNYX_DATA_DIR must serialize on the same mutex,
    // otherwise cargo's parallel runner clobbers the env var across modules.

    // -----------------------------------------------------------------------
    // Unit tests for preprocessing — no model required
    // -----------------------------------------------------------------------

    #[test]
    fn split_words_basic() {
        let words = split_words("Hello, World!");
        assert_eq!(words, vec![
            ("Hello", 0, 5),
            (",", 5, 6),
            ("World", 7, 12),
            ("!", 12, 13),
        ]);
    }

    #[test]
    fn split_words_hyphenated() {
        let words = split_words("Call 555-1234 now");
        // hyphenated number kept as one token
        assert_eq!(words[1], ("555-1234", 5, 13));
    }

    #[test]
    fn split_words_empty() {
        assert!(split_words("").is_empty());
        assert!(split_words("   ").is_empty());
    }

    // -----------------------------------------------------------------------
    // No-model path
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn ner_returns_err_when_model_missing() {
        let _g = crate::models::tests::ENV_GUARD.lock().unwrap();
        let dir = tempdir().unwrap();
        std::env::set_var("SENTYNYX_DATA_DIR", dir.path());
        let d = NerDetector::new();
        let r = d.detect("anything").await;
        assert!(matches!(r, Err(DetectError::ModelNotLoaded(_))));
        std::env::remove_var("SENTYNYX_DATA_DIR");
    }

    // Model-dependent tests that actually load ORT and spin up a Metal
    // context are in `tests/ner_live.rs` and gated behind the `live-ner-test`
    // feature. Keeping them out of the default `cargo test --lib` run avoids
    // a known C++ static-destructor race between ORT and llama.cpp at
    // process exit that SIGABRTs after the tests pass.
}
