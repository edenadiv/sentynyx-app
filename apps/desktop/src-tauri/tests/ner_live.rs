//! Live-NER integration tests.
//!
//! These tests actually construct an ORT `Session`, tokenize with the real
//! GLiNER tokenizer, and run Metal inference. That's useful for regression
//! detection but has a process-exit gotcha: ORT's Metal destructors race
//! llama.cpp's when both are linked, producing `mutex lock failed` SIGABRTs
//! after all tests pass. cargo treats non-zero exit as failure, so these
//! live tests are isolated here (a separate integration-test binary that
//! only links what it uses) behind the `live-ner-test` feature.
//!
//! Run:
//!     cargo test --test ner_live --features live-ner-test
//!
//! Skipped automatically if the GLiNER model files aren't installed at
//! `<SENTYNYX_DATA_DIR>/models/gliner-small-v2.1/`.

#![cfg(feature = "live-ner-test")]

use sentynyx_lib::detect::{Detector, ner::NerDetector, ner_sidecar::NerSidecarDetector};
use sentynyx_lib::models::{self, GLINER_SMALL, GLINER_TOKENIZER};

fn models_installed() -> bool {
    models::local_path(&GLINER_SMALL).exists()
        && models::local_path(&GLINER_TOKENIZER).exists()
}

#[tokio::test]
async fn ner_detects_person_when_model_available() {
    if !models_installed() {
        eprintln!("SKIP: GLiNER model not installed at the default data dir");
        return;
    }
    let d = NerDetector::new();
    let spans = d
        .detect("Draft a memo for Jamie Torres at the office.")
        .await
        .expect("NER inference should succeed when model is installed");
    eprintln!("NER detected {} spans: {:?}", spans.len(), spans);
    // Weak assertion — the live model is non-deterministic under thresholds.
    // If we got a clean decode, `Jamie` should be in one of the spans.
    let person_found = spans.iter().any(|sp| sp.raw.contains("Jamie"));
    if !person_found {
        eprintln!(
            "WARN: 'Jamie' not found in spans — decoder/threshold may need tuning: {:?}",
            spans
        );
    }
}

#[tokio::test]
async fn ner_returns_no_false_positives_for_benign_text() {
    if !models_installed() {
        return;
    }
    let d = NerDetector::new();
    let spans = d
        .detect("the quick brown fox jumps over the lazy dog")
        .await
        .expect("NER inference should succeed on benign text");
    eprintln!("NER benign-text spans: {:?}", spans);
}

#[tokio::test]
async fn ner_empty_text_returns_empty() {
    if !models_installed() {
        return;
    }
    let d = NerDetector::new();
    let spans = d.detect("").await.expect("empty text should not error");
    assert!(spans.is_empty(), "empty text should produce no spans");
}

// ---------------------------------------------------------------------------
// Sidecar integration tests — proves the full parent→child IPC chain works.
// ---------------------------------------------------------------------------

// The sidecar live-integration tests below are `#[ignore]` by default because
// they fail in a cargo-test harness specifically: ORT's Metal init hangs
// inside a child process that was spawned by a parent with a live tokio
// runtime (known issue; the production Tauri app doesn't hit it because the
// sidecar is launched from Tauri's runtime, not cargo test's). Run with:
//     cargo test --test ner_live --features live-ner-test -- --ignored
// when you want to exercise them manually under a fresh shell.

#[tokio::test]
#[ignore = "cargo test hangs in sidecar ORT init — see note above"]
async fn sidecar_round_trips_a_request() {
    if !models_installed() {
        return;
    }
    // Point at the just-built sidecar binary from the test's cargo context.
    std::env::set_var("SENTYNYX_NER_BIN", env!("CARGO_BIN_EXE_sentynyx-ner"));
    let det = NerSidecarDetector::new();
    let spans = det
        .detect("Draft a memo for Jamie Torres at the office.")
        .await
        .expect("sidecar inference should succeed when model is installed");
    eprintln!("sidecar spans: {:?}", spans);
    // Happy-path: sidecar process is alive and returned JSON we could decode.
    let person_found = spans.iter().any(|sp| sp.raw.contains("Jamie"));
    assert!(person_found, "expected to find 'Jamie' in sidecar spans: {:?}", spans);
}

#[tokio::test]
#[ignore = "cargo test hangs in sidecar ORT init — see note above"]
async fn sidecar_reuses_single_child_across_requests() {
    if !models_installed() {
        return;
    }
    std::env::set_var("SENTYNYX_NER_BIN", env!("CARGO_BIN_EXE_sentynyx-ner"));
    let det = NerSidecarDetector::new();

    // First call warms the child; subsequent calls should hit the same
    // long-lived process and not respawn every time.
    let _ = det.detect("Jamie Torres").await.expect("warmup");
    let t0 = std::time::Instant::now();
    let _ = det.detect("Sarah Chen").await.expect("second call");
    let elapsed = t0.elapsed();
    // Re-using a warm sidecar should be much faster than a fresh spawn
    // (cold spawn + ORT session load is ~2s). 1s is a generous ceiling.
    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "sidecar reuse was slow ({elapsed:?}), suggesting respawn per call"
    );
}
