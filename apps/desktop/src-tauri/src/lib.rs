pub mod vendetta;
mod audit;
mod store;
mod keys;
mod router;
mod providers;
mod commands;
mod proxy;
pub mod detect;
pub mod models;
#[cfg(feature = "team-cloud")]
mod telemetry;
#[cfg(feature = "team-cloud")]
mod cloud;

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::detect::llm::ParanoidDetector;
use crate::detect::ner::NerDetector;

pub struct AppState {
    pub store: Arc<Mutex<store::Store>>,
    /// NER detector shared with every `send` handler. Wrapping in Arc keeps
    /// the single ORT session loaded across IPC calls.
    ///
    /// NOTE: we wanted to use `NerSidecarDetector` here for crash isolation
    /// (the `spm_precompiled` panic surfaced in earlier sessions), but the
    /// sidecar architecture has an unresolved issue where ORT's Metal init
    /// hangs forever when the child process is spawned from Tauri's tokio
    /// runtime. The sidecar works fine from a plain shell. Root cause isn't
    /// isolated yet (likely a Metal libdispatch interaction). Re-enable by
    /// swapping `NerDetector` → `NerSidecarDetector` here once fixed — the
    /// sidecar binary + IPC plumbing stay committed as forward work.
    pub ner_detector: Arc<NerDetector>,
    /// Paranoid LLM detector shared across sends. `ParanoidDetector` caches
    /// the loaded model in a per-instance `OnceLock`, so a singleton here
    /// avoids re-loading ~500 MB of GGUF weights on every send().
    pub paranoid_detector: Arc<ParanoidDetector>,
    /// Keeps the Sentry init guard alive for the process. Drop flushes
    /// pending events. No-op when telemetry is disabled (default).
    /// Team-tier only — absent from the public open-source build.
    #[cfg(feature = "team-cloud")]
    #[allow(dead_code)]
    pub telemetry: telemetry::TelemetryGuard,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(feature = "team-cloud")]
    let telemetry_guard = telemetry::init();
    #[cfg(feature = "team-cloud")]
    telemetry::track("app.launched", &[]);

    let store = store::Store::open_default().expect("failed to open sqlite store");

    // Hydrate the persisted telemetry preference so `track()` respects the
    // user's previous choice from the moment the app launches.
    #[cfg(feature = "team-cloud")]
    if let Ok(v) = store.conn.query_row::<String, _, _>(
        "SELECT value FROM settings WHERE key='telemetry_enabled'",
        [], |r| r.get(0),
    ) {
        telemetry::set_enabled(v == "1");
    }

    let ner_detector = Arc::new(NerDetector::new());
    let paranoid_detector = Arc::new(ParanoidDetector::new());
    let store_arc = Arc::new(Mutex::new(store));
    let state = AppState {
        store: Arc::clone(&store_arc),
        ner_detector: Arc::clone(&ner_detector),
        paranoid_detector: Arc::clone(&paranoid_detector),
        #[cfg(feature = "team-cloud")]
        telemetry: telemetry_guard,
    };

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(state)
        .setup(move |_app| {
            // Pre-warm the NER sidecar so the first real send() doesn't pay
            // the ~2 s ONNX session init inside its 500 ms budget. Failure
            // to spawn / warm is non-fatal: send() will fall back to
            // regex-only if the sidecar isn't available.
            let ner = Arc::clone(&ner_detector);
            tauri::async_runtime::spawn(async move {
                use detect::Detector;
                if let Err(e) = ner.detect("warmup").await {
                    eprintln!("ner sidecar warmup failed: {e}");
                }
            });

            // Pre-warm the Qwen GGUF too. Cold-load is ~17 s on M1 (469 MB
            // from disk + Metal context init + first forward pass); paying
            // it here in the background means the user's first paranoid
            // scan or first `sentynyx-local` chat hits a warm model and
            // returns sub-second. No-op when the GGUF isn't downloaded yet —
            // `detect()` returns `ModelNotLoaded` without any real work.
            let paranoid = Arc::clone(&paranoid_detector);
            tauri::async_runtime::spawn(async move {
                use detect::Detector;
                if let Err(e) = paranoid.detect("warmup").await {
                    eprintln!("paranoid/local LLM warmup failed: {e}");
                }
            });

            // Team-tier audit sync. Periodic (5 min) — ships the
            // unuploaded tail of the audit chain to the CF Worker when
            // team mode is enabled. Silent no-op when disabled.
            // Team-tier only — compiled out of the public open-source build.
            #[cfg(feature = "team-cloud")]
            {
                let sync_store = Arc::clone(&store_arc);
                tauri::async_runtime::spawn(async move {
                    cloud::run_periodic(sync_store).await;
                });
            }

            // Privacy proxy autostart — honors the persisted toggle so the
            // loopback endpoint is up before any client's first request.
            {
                let proxy_store = Arc::clone(&store_arc);
                tauri::async_runtime::spawn(async move {
                    let (enabled, port) = {
                        let s = proxy_store.lock().await;
                        let enabled = s.conn.query_row::<String, _, _>(
                            "SELECT value FROM settings WHERE key='proxy_enabled'", [], |r| r.get(0),
                        ).map(|v| v == "1").unwrap_or(false);
                        let port = s.conn.query_row::<String, _, _>(
                            "SELECT value FROM settings WHERE key='proxy_port'", [], |r| r.get(0),
                        ).ok().and_then(|v| v.parse().ok()).unwrap_or(proxy::DEFAULT_PORT);
                        (enabled, port)
                    };
                    if enabled {
                        match proxy::start(proxy_store.clone(), port).await {
                            Ok(_) => proxy::record_error(&proxy_store, None).await,
                            Err(e) => {
                                eprintln!("[proxy] autostart failed: {e}");
                                proxy::record_error(&proxy_store, Some(&e)).await;
                            }
                        }
                    }
                });
            }

            // Idle-unload supervisor is disabled while NER runs in-process:
            // `NerDetector` caches the ORT session in a OnceLock for lifetime
            // sharing with `spawn_blocking`, which doesn't expose a safe
            // drop path. The sidecar variant supports it (see commit
            // 410d903); re-enable together with the sidecar swap in lib.rs.
            Ok(())
        });

    // The IPC surface is split by build profile. `generate_handler!` is a
    // macro and can't take per-argument `#[cfg]`, so we invoke it twice —
    // exactly one branch compiles. The base list is identical between them;
    // the team-cloud build appends the proprietary `team_*` commands.
    #[cfg(feature = "team-cloud")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::detect,
        commands::detect_with_ner,
        commands::send,
        commands::consensus,
        commands::list_conversations,
        commands::load_conversation,
        commands::new_conversation,
        commands::set_api_key,
        commands::validate_api_key,
        commands::has_api_key,
        commands::list_configured_providers,
        commands::list_audit,
        commands::audit_metrics,
        commands::model_status,
        commands::download_model,
        commands::delete_model,
        commands::set_paranoid_mode,
        commands::get_paranoid_mode,
        commands::get_setting,
        commands::set_setting,
        commands::export_data,
        commands::delete_all_data,
        commands::set_telemetry_enabled,
        commands::get_telemetry_enabled,
        commands::system_stats,
        commands::build_info,
        commands::ollama_list_models,
        commands::ollama_health,
        commands::proxy_start,
        commands::proxy_stop,
        commands::proxy_status,
        commands::team_status,
        commands::team_generate_signing_key,
        commands::team_configure,
        commands::team_set_enabled,
        commands::team_upload_now,
    ]);

    #[cfg(not(feature = "team-cloud"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::detect,
        commands::detect_with_ner,
        commands::send,
        commands::consensus,
        commands::list_conversations,
        commands::load_conversation,
        commands::new_conversation,
        commands::set_api_key,
        commands::validate_api_key,
        commands::has_api_key,
        commands::list_configured_providers,
        commands::list_audit,
        commands::audit_metrics,
        commands::model_status,
        commands::download_model,
        commands::delete_model,
        commands::set_paranoid_mode,
        commands::get_paranoid_mode,
        commands::get_setting,
        commands::set_setting,
        commands::export_data,
        commands::delete_all_data,
        commands::set_telemetry_enabled,
        commands::get_telemetry_enabled,
        commands::system_stats,
        commands::build_info,
        commands::ollama_list_models,
        commands::ollama_health,
        commands::proxy_start,
        commands::proxy_stop,
        commands::proxy_status,
    ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
