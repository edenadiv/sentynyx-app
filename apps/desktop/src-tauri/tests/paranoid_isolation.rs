// This test file documents the invariant that the main send() path MUST return
// SendMeta successfully even if paranoid mode is enabled and the LLM model is
// absent, fails, or panics. The paranoid task is spawned with `tokio::spawn`
// and its lifecycle is independent of the main send() return.
//
// Covered by:
// - unit test `llm::tests::llm_returns_err_when_model_missing` (verifies ModelNotLoaded is clean)
// - code structure: `tokio::spawn(async move { ... })` in commands::send has no `?` on detector errors
// - the main send() already has its own error paths that don't depend on the paranoid task
//
// A true end-to-end test of this invariant requires a Tauri test harness, which
// adds significant complexity for limited value. For v0.2 we rely on manual smoke
// testing (Task 19) plus the code review guarantee that no `.await?` on the
// paranoid branch can propagate to send()'s return.

#[test]
fn placeholder_documenting_invariant() {
    // Intentionally empty — documentation lives in the file comment above.
}
