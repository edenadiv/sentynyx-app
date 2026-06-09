use crate::audit::AuditEntry;
use crate::keys;
use crate::router;
use crate::store::{ConversationRow, MessageRow, AuditMetrics};
use crate::vendetta::{self, Span};
use crate::AppState;
use crate::providers::ChunkEvent;
use serde::{Serialize, Deserialize};
use tauri::{Emitter, State};
use tokio::sync::mpsc;
use std::collections::HashMap;

/// Read a settings-table row as u64, or None if missing / unparseable.
/// Thin helper for the timeout lookups so `send` stays readable.
async fn read_setting_u64(
    store: &std::sync::Arc<tokio::sync::Mutex<crate::store::Store>>,
    key: &str,
) -> Option<u64> {
    let s = store.lock().await;
    let v: Result<String, _> = s.conn.query_row(
        "SELECT value FROM settings WHERE key=?",
        rusqlite::params![key],
        |r| r.get(0),
    );
    v.ok().and_then(|s| s.parse::<u64>().ok())
}

/// Resolve the configured Ollama base URL, defaulting to the local daemon.
pub(crate) async fn read_ollama_base_url(
    store: &std::sync::Arc<tokio::sync::Mutex<crate::store::Store>>,
) -> String {
    let s = store.lock().await;
    let v: Result<String, _> = s.conn.query_row(
        "SELECT value FROM settings WHERE key='ollama_base_url'",
        [], |r| r.get(0),
    );
    v.ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://localhost:11434".to_string())
}

/// Packs the user switched off in Settings (`disabled_packs` = JSON array).
/// Sanitized against TOGGLEABLE_PACKS so the core/secrets safety floor can
/// never be disabled, even by hand-editing the settings table.
pub(crate) async fn read_disabled_packs(
    store: &std::sync::Arc<tokio::sync::Mutex<crate::store::Store>>,
) -> std::collections::HashSet<String> {
    let s = store.lock().await;
    let v: Result<String, _> = s.conn.query_row(
        "SELECT value FROM settings WHERE key='disabled_packs'",
        [], |r| r.get(0),
    );
    let Ok(raw) = v else { return Default::default() };
    serde_json::from_str::<Vec<String>>(&raw)
        .map(|ids| ids.into_iter()
            .filter(|id| vendetta::TOGGLEABLE_PACKS.contains(&id.as_str()))
            .collect())
        .unwrap_or_default()
}

pub(crate) fn filter_disabled_packs(
    spans: Vec<Span>,
    disabled: &std::collections::HashSet<String>,
) -> Vec<Span> {
    if disabled.is_empty() { return spans; }
    spans.into_iter()
        .filter(|s| !disabled.contains(vendetta::pack_for(&s.kind)))
        .collect()
}

/// True only when the Ollama base URL points at the local loopback interface —
/// inference happens on this machine with no network egress. **Fails closed**:
/// any host we can't positively identify as loopback is treated as remote, so
/// a parse slip can never downgrade a real remote endpoint to "no egress"
/// (which would skip aliasing and leak raw text to the network).
fn ollama_host_is_local(base_url: &str) -> bool {
    let after_scheme = base_url.split_once("://").map(|(_, r)| r).unwrap_or(base_url);
    let authority = after_scheme.split(['/', '?', '#']).next().unwrap_or("");
    // Drop any userinfo (user:pass@host).
    let hostport = authority.rsplit('@').next().unwrap_or(authority);
    let host = if let Some(rest) = hostport.strip_prefix('[') {
        // IPv6 literal: [::1]:port -> ::1
        rest.split(']').next().unwrap_or("")
    } else {
        hostport.split(':').next().unwrap_or("")
    };
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

fn source_for_kind(k: &vendetta::Kind) -> &'static str {
    match k {
        vendetta::Kind::PERSON_NER | vendetta::Kind::ORG_NER
        | vendetta::Kind::CODENAME_NER | vendetta::Kind::LOCATION_NER
        | vendetta::Kind::EMPID_NER => "ner",
        _ => "regex",
    }
}

#[derive(Serialize)]
pub struct DetectResult { pub spans: Vec<Span> }

#[tauri::command]
pub async fn detect(text: String, state: State<'_, AppState>, conv_id: Option<String>) -> Result<DetectResult, String> {
    // Watchlist + pack lookups lock the store internally — fetch before
    // taking the lock below or the same task would double-lock the tokio Mutex.
    let custom = crate::detect::custom::custom_spans(&state.store, &text).await;
    let disabled = read_disabled_packs(&state.store).await;
    let store = state.store.clone();
    let store = store.lock().await;
    let (mut map, mut counters) = if let Some(cid) = conv_id.as_deref() {
        store.load_alias_state(cid).unwrap_or_default()
    } else { (HashMap::new(), HashMap::new()) };
    let regex_spans = filter_disabled_packs(
        vendetta::detect(&text, &mut map, &mut counters), &disabled);
    let merged = crate::detect::merge_spans(regex_spans, custom);
    let spans = vendetta::apply_alias_map(&merged, &mut map, &mut counters);
    Ok(DetectResult { spans })
}

/// Runs regex + NER in parallel and returns merged spans. Used by the
/// Composer's live-highlight loop so NER-detected names / orgs / codenames /
/// locations / employee IDs appear in real time as the user pauses typing.
///
/// No aliasing state is persisted here — aliases are minted against a fresh
/// local map just so the Vendetta panel has something to show. The real
/// per-conversation alias map is owned by `send()` and only written when the
/// user actually transmits.
#[tauri::command]
pub async fn detect_with_ner(
    text: String,
    state: State<'_, AppState>,
) -> Result<DetectResult, String> {
    use crate::detect::Detector;
    let regex_det = crate::detect::regex::RegexDetector;
    let ner_det = state.ner_detector.clone();
    let ner_timeout_ms = read_setting_u64(&state.store, "ner_timeout_ms")
        .await
        .unwrap_or(500);
    let (regex_result, ner_result) = tokio::join!(
        regex_det.detect(&text),
        tokio::time::timeout(
            std::time::Duration::from_millis(ner_timeout_ms),
            ner_det.detect(&text),
        ),
    );
    let regex_spans = regex_result.map_err(|e| e.to_string())?;
    let ner_spans: Vec<crate::vendetta::Span> = match ner_result {
        Ok(Ok(s)) => s,
        Ok(Err(crate::detect::DetectError::ModelNotLoaded(_))) => vec![],
        Ok(Err(e)) => { eprintln!("live-ner detect error: {e}"); vec![] }
        Err(_timeout) => vec![],
    };
    // Merge precedence: built-ins beat custom watchlist terms, custom beats
    // NER (a user-listed term is explicit intent; NER is probabilistic).
    let custom = crate::detect::custom::custom_spans(&state.store, &text).await;
    let disabled = read_disabled_packs(&state.store).await;
    let regex_spans = filter_disabled_packs(regex_spans, &disabled);
    let merged = crate::detect::merge_spans(
        crate::detect::merge_spans(regex_spans, custom),
        ner_spans,
    );
    let mut map: crate::vendetta::AliasMap = HashMap::new();
    let mut counters: HashMap<String, usize> = HashMap::new();
    let spans = vendetta::apply_alias_map(&merged, &mut map, &mut counters);
    Ok(DetectResult { spans })
}

#[derive(Serialize)]
pub struct SendMeta {
    pub assistant_msg_id: String,
    pub aliased_prompt: String,
    pub spans: Vec<Span>,
    pub blocked: Option<BlockReason>,
    /// Per-send instrumentation trace — every timing and artifact captured
    /// during the Vendetta pipeline so the dev inspector can reconstruct the
    /// full picture without re-running anything.
    pub trace: PipelineTrace,
}

/// Everything the DevInspector needs to understand what happened on a single
/// send: how long each stage took, what each detector produced, the exact
/// aliased payload that went over the wire to the provider.
///
/// This struct is intentionally wide. Redacted content only — raw user text
/// is NOT included (the frontend already has it from `pt.text` at call time).
#[derive(Serialize, Clone, Default)]
pub struct PipelineTrace {
    pub text_len: usize,
    /// Wall-clock duration of the regex future. Because regex + NER run
    /// concurrently via `tokio::join!`, this is the future's own runtime,
    /// not a serial slice of send() wall-clock.
    pub regex_ms: u64,
    pub regex_spans_count: usize,
    pub ner_ms: u64,
    pub ner_spans_count: usize,
    /// "ok" | "timeout" | "not_loaded" | "error" — lets the inspector
    /// explain why NER produced nothing without digging in console logs.
    pub ner_status: String,
    pub ner_error: Option<String>,
    /// Matches from the user-defined custom watchlist (Settings → Watchlist).
    pub custom_spans_count: usize,
    pub merge_ms: u64,
    pub alias_ms: u64,
    /// Total wall-clock inside send() from entry to just before the stream
    /// spawn. Critical vs. parallel waits included.
    pub total_pre_dispatch_ms: u64,
    pub merged_spans_count: usize,
    /// The exact payload the provider receives (aliased). Showing this in
    /// the inspector is the whole point — you can copy it into any LLM
    /// playground and reproduce the upstream behavior byte-for-byte.
    pub aliased_prompt: String,
    pub provider: String,
    pub model_id: String,
    pub paranoid_enabled: bool,
    /// Raw regex output (pre-merge). Kept alongside `ner_spans` so the
    /// inspector can show "regex found X, NER found Y, merge kept Z" —
    /// handy for tuning the `regex wins on overlap` rule.
    pub regex_spans: Vec<Span>,
    pub ner_spans: Vec<Span>,
}

/// Post-stream trace — emitted as `vendetta://trace-stream` once the provider
/// response finishes (or errors). The frontend joins this with the initial
/// `SendMeta.trace` by `msg_id` to get the full picture.
#[derive(Serialize, Clone)]
pub struct StreamTrace {
    pub conv_id: String,
    pub msg_id: String,
    /// Milliseconds from dispatch to the first token chunk. `None` if the
    /// stream errored before producing any tokens.
    pub ttft_ms: Option<u64>,
    pub total_stream_ms: u64,
    pub chunks: usize,
    pub bytes: usize,
    /// Raw provider response in aliased form — what the model actually
    /// produced before local rehydration. Shows whether the model preserved
    /// our ⟦...⟧ tokens or tried to rewrite them.
    pub response_aliased: String,
    /// Final user-visible response after local rehydration.
    pub response_rehydrated: String,
    pub error: Option<String>,
}

/// Paranoid-scan trace — emitted as `vendetta://trace-paranoid` once the
/// Qwen semantic scan finishes (or times out). Only fires when paranoid
/// mode was actually enabled for this send.
#[derive(Serialize, Clone)]
pub struct ParanoidTrace {
    pub conv_id: String,
    pub msg_id: String,
    pub ms: u64,
    pub spans_found: usize,
    pub timed_out: bool,
    pub error: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct BlockReason { pub kind: String, pub rule: String, pub class: String, pub desc: String }

#[derive(Deserialize)]
pub struct SendArgs { pub conv_id: String, pub model_id: String, pub text: String }

#[derive(Serialize, Clone)]
pub struct StreamChunk { pub conv_id: String, pub msg_id: String, pub delta: String, pub done: bool, pub error: Option<String> }

/// Wrap a future so its completion produces both the result and the
/// wall-clock elapsed inside the future. Used to time regex + NER accurately
/// while they run concurrently under `tokio::join!`.
async fn timed<F, T>(fut: F) -> (T, u64)
where F: std::future::Future<Output = T>
{
    let t = std::time::Instant::now();
    let out = fut.await;
    (out, t.elapsed().as_millis() as u64)
}

#[tauri::command]
pub async fn send(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    args: SendArgs,
) -> Result<SendMeta, String> {
    let t_entry = std::time::Instant::now();
    let store = state.store.clone();

    // Detect: regex (always) + NER in parallel. `timed` wraps each future so
    // we get accurate per-stage ms even though they run concurrently.
    use crate::detect::Detector;
    let regex_det = crate::detect::regex::RegexDetector;
    let ner_det = state.ner_detector.clone();

    let ner_timeout_ms = read_setting_u64(&store, "ner_timeout_ms").await.unwrap_or(500);
    let (regex_pair, ner_pair) = tokio::join!(
        timed(regex_det.detect(&args.text)),
        timed(tokio::time::timeout(
            std::time::Duration::from_millis(ner_timeout_ms),
            ner_det.detect(&args.text),
        )),
    );
    let (regex_result, regex_ms) = regex_pair;
    let (ner_result, ner_ms) = ner_pair;

    let regex_spans = regex_result.map_err(|e| e.to_string())?;
    // Respect the user's pack toggles BEFORE the critical-block check below —
    // a disabled pack must neither alias nor block.
    let disabled_packs = read_disabled_packs(&store).await;
    let regex_spans = filter_disabled_packs(regex_spans, &disabled_packs);
    let (ner_spans, ner_status, ner_error): (Vec<crate::vendetta::Span>, &str, Option<String>) =
        match ner_result {
            Ok(Ok(spans)) => (spans, "ok", None),
            Ok(Err(crate::detect::DetectError::ModelNotLoaded(m))) => (vec![], "not_loaded", Some(m)),
            Ok(Err(e)) => { let msg = e.to_string(); eprintln!("ner detect error: {msg}"); (vec![], "error", Some(msg)) }
            Err(_) => { eprintln!("ner detect timeout after {ner_timeout_ms}ms"); (vec![], "timeout", None) }
        };

    let regex_spans_trace = regex_spans.clone();
    let ner_spans_trace = ner_spans.clone();

    // User watchlist terms. Built-ins beat custom on overlap; custom beats NER.
    let custom_spans = crate::detect::custom::custom_spans(&store, &args.text).await;
    let custom_spans_count = custom_spans.len();

    let t_merge = std::time::Instant::now();
    let merged_pre_alias = crate::detect::merge_spans(
        crate::detect::merge_spans(regex_spans, custom_spans),
        ner_spans,
    );
    let merge_ms = t_merge.elapsed().as_millis() as u64;

    let t_alias = std::time::Instant::now();
    let (map, counters, spans, aliased) = {
        let s = store.lock().await;
        let (mut m, mut c) = s.load_alias_state(&args.conv_id).unwrap_or_default();
        let spans = vendetta::apply_alias_map(&merged_pre_alias, &mut m, &mut c);
        let aliased = vendetta::aliasize(&args.text, &spans);
        (m, c, spans, aliased)
    };
    let alias_ms = t_alias.elapsed().as_millis() as u64;

    // Paranoid-enabled lookup up-front so the trace can declare the state
    // even if we early-return on a critical block.
    let paranoid_enabled: bool = {
        let s = store.lock().await;
        let v: Result<String, _> = s.conn.query_row(
            "SELECT value FROM settings WHERE key='paranoid_mode'",
            [], |r| r.get(0)
        );
        v.as_deref().map(|x| x != "0").unwrap_or(true)
    };

    // Provider routing has three shapes:
    //   - `sentynyx-local`: bundled on-device GGUF (no egress).
    //   - `ollama:<name>`: an Ollama server. A loopback base URL means no
    //     egress (treated like local — raw text, no scan); a remote base URL
    //     means egress, so it's aliased + scanned like any cloud provider.
    //   - everything else: a remote cloud provider via the static router.
    let is_ollama = args.model_id.starts_with("ollama:");
    let ollama_base = if is_ollama {
        read_ollama_base_url(&store).await
    } else {
        String::new()
    };
    let ollama_local = is_ollama && ollama_host_is_local(&ollama_base);
    // "no egress" — content never leaves the machine, so the critical-class
    // block, paranoid scan, and aliasing are all skipped below (each of those
    // branches keys off this single flag).
    let is_local = args.model_id == "sentynyx-local" || ollama_local;
    let provider_name = if args.model_id == "sentynyx-local" {
        "local".to_string()
    } else if is_ollama {
        "ollama".to_string()
    } else {
        router::provider_for(&args.model_id)
            .map(|(n, _)| n.to_string())
            .unwrap_or_default()
    };

    let build_trace = |aliased: &str| PipelineTrace {
        text_len: args.text.len(),
        regex_ms,
        regex_spans_count: regex_spans_trace.len(),
        ner_ms,
        ner_spans_count: ner_spans_trace.len(),
        ner_status: ner_status.to_string(),
        ner_error: ner_error.clone(),
        custom_spans_count,
        merge_ms,
        alias_ms,
        total_pre_dispatch_ms: t_entry.elapsed().as_millis() as u64,
        merged_spans_count: merged_pre_alias.len(),
        aliased_prompt: aliased.to_string(),
        provider: provider_name.clone(),
        model_id: args.model_id.clone(),
        paranoid_enabled,
        regex_spans: regex_spans_trace.clone(),
        ner_spans: ner_spans_trace.clone(),
    };

    // Check critical egress-blocking classes. Local provider is exempt —
    // content never leaves the machine, so SSN/APIKEY blocking doesn't apply.
    if !is_local {
    if let Some(critical) = spans.iter().find(|s| vendetta::is_critical(&s.kind)) {
        let mut s = store.lock().await;
        let src = source_for_kind(&critical.kind);
        let _ = s.append_audit_for_spans(&[critical.clone()], "BLOCK", src);
        // Per-kind copy lives in vendetta::block_policy — the single source of
        // truth shared (verbatim) with the frontend CRITICAL map.
        let bp = vendetta::block_policy(&critical.kind)
            .expect("every critical kind has a block policy (unit-tested invariant)");
        let block = BlockReason {
            kind: critical.kind.as_str().to_string(),
            rule: bp.rule.to_string(),
            class: bp.class.to_string(),
            desc: bp.desc.to_string(),
        };
        let trace = build_trace(&aliased);
        eprintln!("[vendetta] BLOCKED conv={} text={} regex={}ms/{} ner={}ms/{} ({}) alias={}ms total={}ms critical={}",
            args.conv_id, args.text.len(),
            regex_ms, trace.regex_spans_count,
            ner_ms, trace.ner_spans_count, trace.ner_status,
            alias_ms, trace.total_pre_dispatch_ms,
            critical.kind.as_str());
        return Ok(SendMeta {
            assistant_msg_id: String::new(),
            aliased_prompt: aliased,
            spans,
            blocked: Some(block),
            trace,
        });
    }
    } // end !is_local critical block

    // Persist alias state + user message.
    let user_msg_id = uuid::Uuid::new_v4().to_string();
    let asst_msg_id = uuid::Uuid::new_v4().to_string();
    {
        let mut s = store.lock().await;
        s.save_alias_state(&args.conv_id, &map, &counters).ok();
        let now = chrono::Utc::now().to_rfc3339();
        s.insert_message(&MessageRow {
            id: user_msg_id.clone(), conv_id: args.conv_id.clone(), role: "user".into(),
            text_raw: args.text.clone(), text_aliased: aliased.clone(), spans: spans.clone(),
            created_at: now,
        }).ok();
        if !spans.is_empty() {
            let (ner_audit, reg_audit): (Vec<_>, Vec<_>) = spans.iter().cloned()
                .partition(|sp| matches!(sp.kind,
                    vendetta::Kind::PERSON_NER | vendetta::Kind::ORG_NER
                    | vendetta::Kind::CODENAME_NER | vendetta::Kind::LOCATION_NER
                    | vendetta::Kind::EMPID_NER));
            if !reg_audit.is_empty() { s.append_audit_for_spans(&reg_audit, "ALIAS", "regex").ok(); }
            if !ner_audit.is_empty() { s.append_audit_for_spans(&ner_audit, "ALIAS", "ner").ok(); }
            let _ = app.emit("audit://new", ());
        }
    }

    // Paranoid LLM scan — fire-and-forget. Emits `vendetta://trace-paranoid`
    // on completion regardless of whether spans were found, so the inspector
    // always shows paranoid cost. Skipped for local sends because (a) the
    // text isn't leaving the machine so there's no privacy lift, and (b)
    // paranoid + chat would contend for the same LlamaModel mutex.
    if paranoid_enabled && !is_local {
        use crate::detect::Detector;
        let app_emit = app.clone();
        let store_clone = store.clone();
        let text_clone = args.text.clone();
        let conv_id_clone = args.conv_id.clone();
        let msg_id_clone = asst_msg_id.clone();
        let paranoid = state.paranoid_detector.clone();
        let paranoid_timeout_ms = read_setting_u64(&store, "paranoid_timeout_ms")
            .await
            .unwrap_or(5_000);
        tokio::spawn(async move {
            let t_p = std::time::Instant::now();
            let r = tokio::time::timeout(
                std::time::Duration::from_millis(paranoid_timeout_ms),
                paranoid.detect(&text_clone),
            ).await;
            let elapsed = t_p.elapsed().as_millis() as u64;
            let (spans, timed_out, error): (Vec<crate::vendetta::Span>, bool, Option<String>) = match r {
                Ok(Ok(s)) => (s, false, None),
                Ok(Err(e)) => (vec![], false, Some(e.to_string())),
                Err(_) => (vec![], true, None),
            };
            let spans_found = spans.len();
            let _ = app_emit.emit("vendetta://trace-paranoid", ParanoidTrace {
                conv_id: conv_id_clone.clone(),
                msg_id: msg_id_clone.clone(),
                ms: elapsed,
                spans_found,
                timed_out,
                error: error.clone(),
            });
            eprintln!("[vendetta] paranoid msg={} {}ms spans={} timed_out={} err={:?}",
                msg_id_clone, elapsed, spans_found, timed_out, error);
            if spans.is_empty() { return; }
            {
                let mut s = store_clone.lock().await;
                let _ = s.append_audit_for_spans(&spans, "PARANOID", "llm");
            }
            let _ = app_emit.emit("paranoid://hit", serde_json::json!({
                "conv_id": conv_id_clone,
                "msg_id": msg_id_clone,
                "count": spans_found,
                "spans": spans,
            }));
            let _ = app_emit.emit("audit://new", ());
        });
    }

    // Resolve provider + key. Local has no remote key, no remote dispatch —
    // it gets a thin wrapper around the loaded Qwen runtime.
    let (pname, provider, api_key): (&'static str, Box<dyn crate::providers::Provider>, String) =
        if args.model_id == "sentynyx-local" {
            (
                "local",
                Box::new(crate::providers::local::Local {
                    detector: state.paranoid_detector.clone(),
                }),
                String::new(),
            )
        } else if is_ollama {
            // Ollama needs no API key; the base URL was resolved above.
            (
                "ollama",
                Box::new(crate::providers::ollama::Ollama { base_url: ollama_base.clone() }),
                String::new(),
            )
        } else {
            let Some((n, p)) = router::provider_for(&args.model_id) else {
                return Err(format!("no provider for model {}", args.model_id));
            };
            let Some(k) = keys::get(n) else {
                return Err(format!("no API key configured for {}", n));
            };
            (n, p, k)
        };

    // Local sees raw text (no privacy benefit to aliasing when the model
    // runs on-device). Remote always sees the aliased form.
    let prompt_for_provider = if is_local { args.text.clone() } else { aliased.clone() };

    let trace = build_trace(&prompt_for_provider);
    eprintln!("[vendetta] send conv={} msg={} text={} regex={}ms/{} ner={}ms/{} ({}) merge={}ms/{} alias={}ms total={}ms -> {}/{}",
        args.conv_id, asst_msg_id, args.text.len(),
        regex_ms, trace.regex_spans_count,
        ner_ms, trace.ner_spans_count, trace.ner_status,
        merge_ms, trace.merged_spans_count,
        alias_ms, trace.total_pre_dispatch_ms,
        pname, args.model_id);

    // Spawn streaming task. Rehydrate each chunk, time TTFT, emit a
    // `vendetta://trace-stream` on completion.
    let app_emit = app.clone();
    let conv_id = args.conv_id.clone();
    let msg_id = asst_msg_id.clone();
    let aliased_clone = prompt_for_provider.clone();
    // For local, rehydration is a no-op (we sent raw text, model outputs raw
    // text) but the empty reverse map handles that correctly.
    let spans_for_reverse: Vec<Span> = if is_local { Vec::new() } else { spans.clone() };
    let model_id = args.model_id.clone();
    let store_clone = store.clone();

    tokio::spawn(async move {
        let t_dispatch = std::time::Instant::now();
        let (tx, mut rx) = mpsc::channel::<ChunkEvent>(64);
        let model_id_inner = model_id.clone();
        let aliased_inner = aliased_clone.clone();
        let key_inner = api_key.clone();

        tokio::spawn(async move {
            if let Err(e) = router::dispatch(&key_inner, provider, &model_id_inner, &aliased_inner, tx.clone()).await {
                let _ = tx.send(ChunkEvent::Error(e)).await;
            }
        });

        let reverse = vendetta::build_reverse_from_spans(&spans_for_reverse);
        let mut buf = String::new();
        let mut assembled_raw = String::new();
        let mut assembled_aliased = String::new();
        let mut ttft_ms: Option<u64> = None;
        let mut chunks = 0usize;
        let mut bytes = 0usize;
        let mut stream_error: Option<String> = None;

        while let Some(evt) = rx.recv().await {
            match evt {
                ChunkEvent::Token(t) => {
                    if ttft_ms.is_none() {
                        ttft_ms = Some(t_dispatch.elapsed().as_millis() as u64);
                    }
                    chunks += 1;
                    bytes += t.len();
                    assembled_aliased.push_str(&t);
                    let emit = vendetta::rehydrate_stream_with(&mut buf, &t, &reverse);
                    if !emit.is_empty() {
                        assembled_raw.push_str(&emit);
                        let _ = app_emit.emit("message://chunk", StreamChunk {
                            conv_id: conv_id.clone(), msg_id: msg_id.clone(),
                            delta: emit, done: false, error: None
                        });
                    }
                }
                ChunkEvent::Done => {
                    let remainder = std::mem::take(&mut buf);
                    if !remainder.is_empty() {
                        assembled_raw.push_str(&remainder);
                        let _ = app_emit.emit("message://chunk", StreamChunk {
                            conv_id: conv_id.clone(), msg_id: msg_id.clone(),
                            delta: remainder, done: false, error: None
                        });
                    }
                    let _ = app_emit.emit("message://chunk", StreamChunk {
                        conv_id: conv_id.clone(), msg_id: msg_id.clone(),
                        delta: String::new(), done: true, error: None
                    });

                    let mut s = store_clone.lock().await;
                    let now = chrono::Utc::now().to_rfc3339();
                    s.insert_message(&MessageRow {
                        id: msg_id.clone(), conv_id: conv_id.clone(), role: "assistant".into(),
                        text_raw: assembled_raw.clone(), text_aliased: assembled_aliased.clone(),
                        spans: spans_for_reverse.clone(), created_at: now,
                    }).ok();
                    break;
                }
                ChunkEvent::Error(e) => {
                    stream_error = Some(e.clone());
                    let _ = app_emit.emit("message://chunk", StreamChunk {
                        conv_id: conv_id.clone(), msg_id: msg_id.clone(),
                        delta: String::new(), done: true, error: Some(e)
                    });
                    break;
                }
            }
        }

        let total_stream_ms = t_dispatch.elapsed().as_millis() as u64;
        eprintln!("[vendetta] stream msg={} ttft={:?} total={}ms chunks={} bytes={} err={:?}",
            msg_id, ttft_ms, total_stream_ms, chunks, bytes, stream_error);
        let _ = app_emit.emit("vendetta://trace-stream", StreamTrace {
            conv_id: conv_id.clone(),
            msg_id: msg_id.clone(),
            ttft_ms,
            total_stream_ms,
            chunks,
            bytes,
            response_aliased: assembled_aliased,
            response_rehydrated: assembled_raw,
            error: stream_error,
        });
    });

    Ok(SendMeta {
        assistant_msg_id: asst_msg_id,
        aliased_prompt: aliased,
        spans,
        blocked: None,
        trace,
    })
}

#[derive(Deserialize)]
pub struct ConsensusArgs { pub conv_id: String, pub model_ids: Vec<String>, pub text: String }

#[derive(Serialize, Clone)]
pub struct ConsensusColumn { pub model_id: String, pub msg_id: String }

#[tauri::command]
pub async fn consensus(app: tauri::AppHandle, state: State<'_, AppState>, args: ConsensusArgs) -> Result<Vec<ConsensusColumn>, String> {
    let mut cols = Vec::new();
    for m in &args.model_ids {
        let meta = send(app.clone(), state.clone(), SendArgs {
            conv_id: args.conv_id.clone(),
            model_id: m.clone(),
            text: args.text.clone(),
        }).await?;
        cols.push(ConsensusColumn { model_id: m.clone(), msg_id: meta.assistant_msg_id });
    }
    Ok(cols)
}

#[tauri::command]
pub async fn list_conversations(state: State<'_, AppState>) -> Result<Vec<ConversationRow>, String> {
    let s = state.store.lock().await;
    s.list_conversations().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_conversation(state: State<'_, AppState>, conv_id: String) -> Result<Vec<MessageRow>, String> {
    let s = state.store.lock().await;
    s.load_messages(&conv_id).map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct NewConvArgs { pub title: String, pub model_id: String }

#[tauri::command]
pub async fn new_conversation(state: State<'_, AppState>, args: NewConvArgs) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let mut s = state.store.lock().await;
    s.new_conversation(&id, &args.title, &args.model_id).map_err(|e| e.to_string())?;
    Ok(id)
}

#[derive(Deserialize)]
pub struct SetKeyArgs { pub provider: String, pub secret: String }

#[derive(Serialize)]
pub struct SetKeyResult {
    /// "keychain" or "file" — lets the UI honestly label where the secret lives.
    pub storage: &'static str,
}

#[tauri::command]
pub fn set_api_key(args: SetKeyArgs) -> Result<SetKeyResult, String> {
    let outcome = keys::set(&args.provider, &args.secret).map_err(|e| e.to_string())?;
    Ok(SetKeyResult {
        storage: match outcome {
            keys::SetOutcome::Keychain => "keychain",
            keys::SetOutcome::FileFallback => "file",
        },
    })
}

#[derive(Deserialize)]
pub struct ValidateKeyArgs {
    pub provider: String,
    /// If given, validate this secret without storing it (used from the
    /// first-run wizard and SettingsPanel before hitting Save). If None,
    /// validates the currently-stored secret (used to confirm config drift).
    pub secret: Option<String>,
}

#[derive(Serialize)]
pub struct ValidateKeyResult {
    pub ok: bool,
    /// Human-readable hint for why the key failed. `None` if ok.
    pub reason: Option<String>,
}

/// Lightweight "does this key work" check. For each provider we hit the
/// cheapest known endpoint (usually GET /models) and interpret the HTTP
/// status. Any 4xx that isn't 429/5xx counts as "key is bad".
#[tauri::command]
pub async fn validate_api_key(args: ValidateKeyArgs) -> Result<ValidateKeyResult, String> {
    let key = match args.secret {
        Some(s) if !s.trim().is_empty() => s,
        _ => match keys::get(&args.provider) {
            Some(k) => k,
            None => return Ok(ValidateKeyResult {
                ok: false,
                reason: Some("no key configured".into()),
            }),
        },
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let (url, auth_header, auth_value): (String, &'static str, String) = match args.provider.as_str() {
        "openai" => (
            "https://api.openai.com/v1/models".to_string(),
            "authorization",
            format!("Bearer {key}"),
        ),
        "anthropic" => (
            "https://api.anthropic.com/v1/models".to_string(),
            "x-api-key",
            key.clone(),
        ),
        "google" => (
            // Google uses the key as a query param, not a header.
            format!("https://generativelanguage.googleapis.com/v1beta/models?key={key}"),
            "",
            String::new(),
        ),
        "xai" => (
            "https://api.x.ai/v1/models".to_string(),
            "authorization",
            format!("Bearer {key}"),
        ),
        "openrouter" => (
            // /key returns the key's own metadata — 200 iff the key is live.
            "https://openrouter.ai/api/v1/key".to_string(),
            "authorization",
            format!("Bearer {key}"),
        ),
        other => return Ok(ValidateKeyResult {
            ok: false,
            reason: Some(format!("unknown provider: {other}")),
        }),
    };

    let mut req = client.get(&url);
    if !auth_header.is_empty() {
        req = req.header(auth_header, auth_value);
    }
    // Anthropic requires a version header even on /models.
    if args.provider == "anthropic" {
        req = req.header("anthropic-version", "2023-06-01");
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => return Ok(ValidateKeyResult {
            ok: false,
            reason: Some(format!("network error: {e}")),
        }),
    };

    let status = resp.status();
    if status.is_success() {
        return Ok(ValidateKeyResult { ok: true, reason: None });
    }
    // 429 = rate-limited but key is valid. Treat as ok.
    if status.as_u16() == 429 {
        return Ok(ValidateKeyResult { ok: true, reason: None });
    }
    let body = resp.text().await.unwrap_or_default();
    let hint = match status.as_u16() {
        401 => "invalid API key".to_string(),
        403 => "key lacks permission (check org/project access)".to_string(),
        _ => format!("provider returned {} — {}", status, body.chars().take(160).collect::<String>()),
    };
    Ok(ValidateKeyResult { ok: false, reason: Some(hint) })
}

#[tauri::command]
pub fn has_api_key(provider: String) -> bool { keys::has(&provider) }

#[tauri::command]
pub fn list_configured_providers() -> Vec<String> {
    ["openai", "anthropic", "google", "xai", "openrouter"].iter()
        .filter(|p| keys::has(p))
        .map(|p| p.to_string())
        .collect()
}

/// List the models installed on the configured Ollama server (`GET /api/tags`).
/// Errors if the daemon is unreachable; the frontend treats that as "no Ollama
/// models available" and simply omits the Ollama group from the picker.
#[tauri::command]
pub async fn ollama_list_models(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let base = read_ollama_base_url(&state.store).await;
    let url = format!("{}/api/tags", base.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&url).send().await
        .map_err(|e| format!("cannot reach Ollama at {base}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Ollama returned {}", resp.status()));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let models = v["models"].as_array()
        .map(|arr| arr.iter()
            .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
            .collect::<Vec<_>>())
        .unwrap_or_default();
    Ok(models)
}

#[derive(Serialize)]
pub struct OllamaHealth {
    pub reachable: bool,
    pub base_url: String,
    pub model_count: usize,
}

/// Soft reachability probe for the Settings UI. Never errors on a down daemon —
/// returns `reachable: false` so the panel renders a clear "not running" state
/// instead of surfacing a thrown exception.
#[tauri::command]
pub async fn ollama_health(state: State<'_, AppState>) -> Result<OllamaHealth, String> {
    let base = read_ollama_base_url(&state.store).await;
    let url = format!("{}/api/tags", base.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;
    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let v: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            let count = v["models"].as_array().map(|a| a.len()).unwrap_or(0);
            Ok(OllamaHealth { reachable: true, base_url: base, model_count: count })
        }
        _ => Ok(OllamaHealth { reachable: false, base_url: base, model_count: 0 }),
    }
}

// ---------------------------------------------------------------------------
// Privacy proxy lifecycle. The proxy module owns the listener; these commands
// only flip it and persist the user's preference so lib.rs can autostart it.
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct ProxyStatus { pub running: bool, pub port: Option<u16> }

#[tauri::command]
pub async fn proxy_status() -> Result<ProxyStatus, String> {
    let port = crate::proxy::status().await;
    Ok(ProxyStatus { running: port.is_some(), port })
}

#[tauri::command]
pub async fn proxy_start(state: State<'_, AppState>, port: Option<u16>) -> Result<ProxyStatus, String> {
    let port = port.unwrap_or(crate::proxy::DEFAULT_PORT);
    let bound = crate::proxy::start(state.store.clone(), port).await?;
    {
        let s = state.store.lock().await;
        let _ = s.conn.execute(
            "INSERT INTO settings(key,value) VALUES('proxy_enabled','1')
             ON CONFLICT(key) DO UPDATE SET value='1'", []);
        let _ = s.conn.execute(
            "INSERT INTO settings(key,value) VALUES('proxy_port',?1)
             ON CONFLICT(key) DO UPDATE SET value=?1", [bound.to_string()]);
    }
    Ok(ProxyStatus { running: true, port: Some(bound) })
}

#[tauri::command]
pub async fn proxy_stop(state: State<'_, AppState>) -> Result<ProxyStatus, String> {
    crate::proxy::stop().await;
    {
        let s = state.store.lock().await;
        let _ = s.conn.execute(
            "INSERT INTO settings(key,value) VALUES('proxy_enabled','0')
             ON CONFLICT(key) DO UPDATE SET value='0'", []);
    }
    Ok(ProxyStatus { running: false, port: None })
}

#[tauri::command]
pub async fn list_audit(state: State<'_, AppState>, limit: Option<i64>) -> Result<Vec<AuditEntry>, String> {
    let s = state.store.lock().await;
    s.list_audit(limit.unwrap_or(50)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn audit_metrics(state: State<'_, AppState>) -> Result<AuditMetrics, String> {
    let s = state.store.lock().await;
    s.audit_metrics().map_err(|e| e.to_string())
}

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
        llm: models::status(&models::PARANOID_LLM),
    }
}

#[derive(Deserialize)]
pub struct ModelIdArgs { pub id: String }

fn spec_by_id(id: &str) -> Option<&'static ModelSpec> {
    match id {
        "gliner-small-v2.1" => Some(&models::GLINER_SMALL),
        "gliner-small-v2.1-tokenizer" => Some(&models::GLINER_TOKENIZER),
        "paranoid-llm" | "qwen3-1.5b-q4km" => Some(&models::PARANOID_LLM),
        _ => None,
    }
}

#[tauri::command]
pub async fn download_model(app: tauri::AppHandle, args: ModelIdArgs) -> Result<(), String> {
    let spec = spec_by_id(&args.id).ok_or_else(|| format!("unknown model id: {}", args.id))?;
    let app_emit = app.clone();
    let id = args.id.clone();
    models::ensure_local(spec, move |done, total| {
        let clamped_done = done.min(total);
        let pct = if total > 0 { (clamped_done * 100 / total) as u32 } else { 0 };
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
    // Default: ON. Paranoid semantic scan is a core promise of the product.
    // User can explicitly disable via Settings, which writes '0'.
    Ok(v.as_deref().map(|x| x != "0").unwrap_or(true))
}

// ---------------------------------------------------------------------------
// Generic settings KV — used for tweaks, first-run flag, alias mode, etc.
// Keys are free-form strings; values are arbitrary strings (frontend picks
// its own serialization, usually JSON).
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct SetSettingArgs { pub key: String, pub value: String }

#[tauri::command]
pub async fn get_setting(state: State<'_, AppState>, key: String) -> Result<Option<String>, String> {
    let s = state.store.lock().await;
    let v: Result<String, _> = s.conn.query_row(
        "SELECT value FROM settings WHERE key=?",
        rusqlite::params![key],
        |r| r.get(0),
    );
    Ok(v.ok())
}

#[tauri::command]
pub async fn set_setting(state: State<'_, AppState>, args: SetSettingArgs) -> Result<(), String> {
    let s = state.store.lock().await;
    s.conn.execute(
        "INSERT INTO settings(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![args.key, args.value],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Data export + delete-all (GDPR-style user controls)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct ExportResult {
    /// Absolute path to the directory containing the exported files.
    pub dest: String,
    /// The files that were copied.
    pub files: Vec<String>,
}

/// Copies `sentynyx.db` and `secrets.json` (if present) into a
/// timestamped sibling directory under ~/Downloads/. Models are NOT
/// exported — they're reproducible from the HuggingFace URL and
/// would bloat the export to 1+ GB.
#[tauri::command]
pub fn export_data() -> Result<ExportResult, String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let home = std::env::var_os("HOME")
        .ok_or_else(|| "HOME env var not set".to_string())?;
    let dest = std::path::PathBuf::from(home)
        .join("Downloads")
        .join(format!("sentynyx-export-{ts}"));
    std::fs::create_dir_all(&dest).map_err(|e| format!("mkdir: {e}"))?;

    let mut files = Vec::new();
    let data_root = crate::models::models_root()
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "couldn't resolve app data root".to_string())?;
    for name in ["sentynyx.db", "secrets.json"] {
        let src = data_root.join(name);
        if src.exists() {
            let dst = dest.join(name);
            std::fs::copy(&src, &dst).map_err(|e| format!("copy {name}: {e}"))?;
            #[cfg(unix)]
            if name == "secrets.json" {
                // Preserve the 0600 perms on the secrets file.
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o600));
            }
            files.push(name.to_string());
        }
    }
    Ok(ExportResult {
        dest: dest.to_string_lossy().into_owned(),
        files,
    })
}

#[derive(Deserialize)]
pub struct SetTelemetryArgs { pub enabled: bool }

#[tauri::command]
pub async fn set_telemetry_enabled(state: State<'_, AppState>, args: SetTelemetryArgs) -> Result<(), String> {
    // Telemetry is a team-cloud-only surface; in the public build there's no
    // Sentry integration to toggle, so we just persist the preference.
    #[cfg(feature = "team-cloud")]
    crate::telemetry::set_enabled(args.enabled);
    let s = state.store.lock().await;
    s.conn.execute(
        "INSERT INTO settings(key,value) VALUES('telemetry_enabled',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![if args.enabled { "1" } else { "0" }],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_telemetry_enabled(state: State<'_, AppState>) -> Result<bool, String> {
    let s = state.store.lock().await;
    let v: Result<String, _> = s.conn.query_row(
        "SELECT value FROM settings WHERE key='telemetry_enabled'",
        [], |r| r.get(0),
    );
    Ok(v.as_deref().map(|x| x == "1").unwrap_or(false))
}

/// Nukes the Sentynyx app data directory (SQLite DB, secrets file, and any
/// downloaded models). Also clears keychain entries for all known providers.
/// Caller should restart the app afterwards — the existing AppState holds
/// live handles to files we just deleted.
#[tauri::command]
pub async fn delete_all_data(state: State<'_, AppState>) -> Result<(), String> {
    // Close the live SQLite connection by letting the lock drop before we
    // delete the file.
    drop(state.store.lock().await);

    let data_root = crate::models::models_root()
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "couldn't resolve app data root".to_string())?;

    // Clear keychain entries for every provider we know about, best-effort.
    for p in ["openai", "anthropic", "google", "xai", "openrouter"] {
        if let Ok(entry) = keyring::Entry::new("sentynyx", p) {
            let _ = entry.delete_credential();
        }
    }

    if data_root.exists() {
        std::fs::remove_dir_all(&data_root)
            .map_err(|e| format!("remove_dir_all {}: {}", data_root.display(), e))?;
    }
    Ok(())
}

#[derive(Serialize)]
pub struct SystemStats {
    /// Resident-set size in megabytes.
    pub rss_mb: u64,
    /// Seconds since the current process started.
    pub uptime_sec: u64,
    /// Crate version from Cargo metadata — surfaces in the About dialog so a
    /// screenshot tells us which build the user is on.
    pub version: &'static str,
    pub pid: u32,
}

/// Snapshot of the current Sentynyx process for the About dialog. Called on
/// open and again every ~2 s while it's visible. First call is a touch heavy
/// as `sysinfo` enumerates processes; subsequent refreshes are cheap.
// ---------------------------------------------------------------------------
// Team-tier audit sync (Phase 5 client wiring — v0.3.2)
// ---------------------------------------------------------------------------
//
// Admin bootstrap flow:
//   1. `team_generate_signing_key` — mints an Ed25519 keypair; private key
//      lands in the OS keychain, returns the base64 public key.
//   2. Admin POSTs that pubkey + team name + owner email to the CF Worker
//      via their own out-of-band tool (the Worker's `/admin/teams` route).
//      The Worker returns a `team_id`.
//   3. `team_configure` — stores `team_id + member_email + endpoint` in
//      the local settings table. After this, the periodic task + manual
//      upload both work.
//   4. `team_set_enabled(true)` — flips the opt-in switch.
//
// Status + manual flush exposed for the Settings UI.

#[cfg(feature = "team-cloud")]
#[derive(Serialize)]
pub struct TeamStatus {
    pub enabled: bool,
    pub configured: bool,
    pub team_id: Option<String>,
    pub member_email: Option<String>,
    pub endpoint: String,
    pub last_upload_at: Option<i64>,
    pub pending_count: i64,
    pub has_signing_key: bool,
}

#[cfg(feature = "team-cloud")]
#[tauri::command]
pub async fn team_status(state: State<'_, AppState>) -> Result<TeamStatus, String> {
    let s = state.store.lock().await;
    let enabled = read_setting_str(&s, crate::cloud::KEY_ENABLED).as_deref() == Some("1");
    let team_id = read_setting_str(&s, crate::cloud::KEY_TEAM_ID);
    let member_email = read_setting_str(&s, crate::cloud::KEY_MEMBER_EMAIL);
    let endpoint = read_setting_str(&s, crate::cloud::KEY_ENDPOINT)
        .unwrap_or_else(|| crate::cloud::DEFAULT_ENDPOINT.to_string());
    let last_upload_at = read_setting_str(&s, crate::cloud::KEY_LAST_UPLOAD_AT)
        .and_then(|v| v.parse::<i64>().ok());
    let pending_count = s.count_unuploaded_audit().unwrap_or(0);
    drop(s);

    let has_signing_key = keyring::Entry::new(
        crate::cloud::KEYCHAIN_SERVICE,
        crate::cloud::KEYCHAIN_KEY_NAME,
    ).and_then(|e| e.get_password()).is_ok();

    Ok(TeamStatus {
        enabled,
        configured: team_id.is_some() && member_email.is_some(),
        team_id,
        member_email,
        endpoint,
        last_upload_at,
        pending_count,
        has_signing_key,
    })
}

#[cfg(feature = "team-cloud")]
fn read_setting_str(s: &crate::store::Store, key: &str) -> Option<String> {
    s.conn.query_row(
        "SELECT value FROM settings WHERE key=?",
        rusqlite::params![key],
        |r| r.get::<_, String>(0),
    ).ok()
}

#[cfg(feature = "team-cloud")]
async fn write_setting_str(state: &State<'_, AppState>, key: &str, value: &str) -> Result<(), String> {
    let s = state.store.lock().await;
    s.conn.execute(
        "INSERT INTO settings(key, value) VALUES(?, ?) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![key, value],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(feature = "team-cloud")]
#[derive(Serialize)]
pub struct GenerateKeyResult {
    /// Base64 Ed25519 public key — 32 bytes. Admin pastes this into the
    /// CF Worker's `POST /admin/teams` `audit_pubkey` field.
    pub public_key: String,
}

/// Generates a fresh Ed25519 keypair. Private key → OS keychain (same
/// service string as provider API keys, different key name); public key
/// returned so the admin can register it with the server.
#[cfg(feature = "team-cloud")]
#[tauri::command]
pub fn team_generate_signing_key() -> Result<GenerateKeyResult, String> {
    let pub_b64 = crate::cloud::generate_and_persist_signing_key()
        .map_err(|e| e.to_string())?;
    Ok(GenerateKeyResult { public_key: pub_b64 })
}

#[cfg(feature = "team-cloud")]
#[derive(Deserialize)]
pub struct TeamConfigureArgs {
    pub team_id: String,
    pub member_email: String,
    /// Optional override. Falls back to `cloud::DEFAULT_ENDPOINT`.
    pub endpoint: Option<String>,
}

#[cfg(feature = "team-cloud")]
#[tauri::command]
pub async fn team_configure(
    state: State<'_, AppState>,
    args: TeamConfigureArgs,
) -> Result<(), String> {
    if args.team_id.trim().is_empty() { return Err("team_id required".into()); }
    if args.member_email.trim().is_empty() { return Err("member_email required".into()); }
    write_setting_str(&state, crate::cloud::KEY_TEAM_ID, args.team_id.trim()).await?;
    write_setting_str(&state, crate::cloud::KEY_MEMBER_EMAIL, args.member_email.trim()).await?;
    if let Some(ep) = args.endpoint {
        if !ep.trim().is_empty() {
            write_setting_str(&state, crate::cloud::KEY_ENDPOINT, ep.trim()).await?;
        }
    }
    Ok(())
}

#[cfg(feature = "team-cloud")]
#[derive(Deserialize)]
pub struct SetEnabledArgs { pub enabled: bool }

#[cfg(feature = "team-cloud")]
#[tauri::command]
pub async fn team_set_enabled(
    state: State<'_, AppState>,
    args: SetEnabledArgs,
) -> Result<(), String> {
    write_setting_str(&state, crate::cloud::KEY_ENABLED, if args.enabled { "1" } else { "0" }).await
}

/// Manually triggers a sync tick. Used by the "Sync now" button in
/// Settings → Team. Returns the outcome so the UI can render feedback
/// ("✓ synced 47 events" vs "⚠ server returned 403").
#[cfg(feature = "team-cloud")]
#[tauri::command]
pub async fn team_upload_now(
    state: State<'_, AppState>,
) -> Result<crate::cloud::SyncOutcome, String> {
    crate::cloud::sync_once(&state.store).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn system_stats() -> Result<SystemStats, String> {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let pid = std::process::id();
    let p = Pid::from_u32(pid);
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&[p]));
    let (rss_bytes, uptime) = sys.process(p)
        .map(|proc| (proc.memory(), proc.run_time()))
        .unwrap_or((0, 0));
    Ok(SystemStats {
        rss_mb: rss_bytes / (1024 * 1024),
        uptime_sec: uptime,
        version: env!("CARGO_PKG_VERSION"),
        pid,
    })
}

/// Reports which optional, compile-time features this binary was built with so
/// the frontend can hide UI for surfaces that aren't actually wired in. `cfg!`
/// evaluates at compile time but yields a runtime bool, so these values always
/// match exactly what `invoke_handler` registered — the binary is the single
/// source of truth (a stale Vite define could drift; this can't).
#[derive(Serialize)]
pub struct BuildInfo {
    /// True only in commercial builds (`cargo build --features team-cloud`).
    /// Gates the Settings → Team panel.
    pub team_cloud: bool,
    /// Whether Sentry telemetry is compiled in (currently tied to team-cloud).
    /// Gates the Settings → Telemetry toggle.
    pub telemetry_available: bool,
    pub version: &'static str,
}

#[tauri::command]
pub fn build_info() -> BuildInfo {
    BuildInfo {
        team_cloud: cfg!(feature = "team-cloud"),
        telemetry_available: cfg!(feature = "team-cloud"),
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Security-load-bearing: a loopback Ollama endpoint sends RAW text (no
    // egress), so misclassifying a remote host as local would leak PII. These
    // pin the fail-closed behavior.
    #[test]
    fn ollama_loopback_hosts_are_local() {
        for u in [
            "http://localhost:11434",
            "http://127.0.0.1:11434",
            "http://127.0.0.1",
            "http://[::1]:11434",
            "https://localhost:11434/",
            "http://localhost",
        ] {
            assert!(ollama_host_is_local(u), "expected local: {u}");
        }
    }

    #[test]
    fn ollama_remote_or_ambiguous_hosts_are_not_local() {
        for u in [
            "http://192.168.1.50:11434",
            "http://10.0.0.5:11434",
            "https://ollama.example.com",
            "http://my-gpu-box:11434",
            "http://localhost.evil.com:11434", // suffix attack — must be remote
            "https://[2001:db8::1]:11434",
            "http://0.0.0.0:11434",            // ambiguous — fail closed to remote
            "",
            "garbage",
        ] {
            assert!(!ollama_host_is_local(u), "expected remote/fail-closed: {u}");
        }
    }
}
