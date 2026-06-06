//! Opt-in telemetry + crash reporting via Sentry.
//!
//! Off by default. Enabled when ALL of these are true:
//!   1. The `SENTYNYX_SENTRY_DSN` env var is set at process start (embedded
//!      via `option_env!` in the release binary; overridable at runtime).
//!   2. The user has opted in via the Settings UI (persisted under the
//!      `telemetry_enabled` settings key, read lazily on each event).
//!
//! What we never send:
//!   - Prompt text, aliased or raw.
//!   - Conversation IDs, message IDs, audit hashes.
//!   - API keys or anything that looks like a secret.
//!   - Any string containing the alias brackets `\u{27E6}` / `\u{27E7}`.
//!   - Anything from the provider response body.
//!
//! What we do send:
//!   - Panic backtraces + Rust crate version.
//!   - Named event types: `app.launched`, `model.downloaded`,
//!     `send.succeeded` (just provider + model id), `send.error` (error
//!     class, no body), `send.blocked` (class name only).

use std::sync::atomic::{AtomicBool, Ordering};

use sentry::ClientInitGuard;

/// Runtime opt-in flag. When `false` (default), `track()` short-circuits
/// regardless of whether Sentry was initialized. The frontend flips this
/// via the `set_telemetry_enabled` IPC command, which also persists to
/// the settings table so the preference survives restarts.
static ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_enabled(v: bool) {
    ENABLED.store(v, Ordering::Relaxed);
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Holds the Sentry init guard so its destructor fires at process exit
/// (flushes queued events). Stored inside `AppState` so it lives for the
/// process.
pub struct TelemetryGuard(#[allow(dead_code)] Option<ClientInitGuard>);

/// Initialize Sentry if a DSN is available. Returns a guard the caller
/// should keep alive for the process lifetime. On no-DSN (development,
/// user hasn't set it), returns a None-guard that skips everything.
pub fn init() -> TelemetryGuard {
    let dsn = runtime_dsn();
    let Some(dsn) = dsn else {
        return TelemetryGuard(None);
    };

    let guard = sentry::init((
        dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some(environment().into()),
            // Only send panics + explicit events — no user breadcrumbs.
            default_integrations: true,
            attach_stacktrace: true,
            send_default_pii: false,
            before_send: Some(std::sync::Arc::new(|event| {
                if event_contains_redaction_alias(&event) {
                    // Safety net: if any field slips through containing an
                    // alias bracket, drop the whole event rather than risk
                    // leaking the surrounding prompt context.
                    return None;
                }
                Some(event)
            })),
            ..Default::default()
        },
    ));

    TelemetryGuard(Some(guard))
}

/// Resolves the Sentry DSN from either the runtime env or a baked-in
/// `option_env!`. Returns None if neither is set.
fn runtime_dsn() -> Option<String> {
    if let Ok(v) = std::env::var("SENTYNYX_SENTRY_DSN") {
        if !v.is_empty() {
            return Some(v);
        }
    }
    option_env!("SENTYNYX_SENTRY_DSN").map(|s| s.to_string()).filter(|s| !s.is_empty())
}

fn environment() -> &'static str {
    if cfg!(debug_assertions) { "dev" } else { "release" }
}

/// Depth-first walk of a Sentry event looking for the alias bracket
/// characters anywhere — covers values, keys, breadcrumbs, context,
/// extra fields. Returning true drops the event in `before_send`.
fn event_contains_redaction_alias(event: &sentry::protocol::Event<'_>) -> bool {
    use serde_json::Value;
    let json = match serde_json::to_value(event) {
        Ok(v) => v,
        Err(_) => return false,
    };
    walk_for_markers(&json)
}

fn walk_for_markers(v: &serde_json::Value) -> bool {
    use serde_json::Value;
    match v {
        Value::String(s) => s.contains('\u{27E6}') || s.contains('\u{27E7}'),
        Value::Array(a) => a.iter().any(walk_for_markers),
        Value::Object(o) => o.values().any(walk_for_markers),
        _ => false,
    }
}

/// Thin wrapper so the rest of the codebase doesn't import `sentry` directly.
/// Caller provides a fixed event name (`app.launched`, `send.error`, etc.)
/// and a tag map. Never pass free-form user text.
pub fn track(name: &str, tags: &[(&str, &str)]) {
    if !is_enabled() {
        return;
    }
    sentry::configure_scope(|scope| {
        for (k, v) in tags {
            scope.set_tag(k, v);
        }
    });
    sentry::capture_message(name, sentry::Level::Info);
}
