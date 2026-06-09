//! Local privacy proxy — an OpenAI-compatible endpoint on 127.0.0.1 so ANY
//! OpenAI-SDK client (Cursor, Continue, scripts, langchain…) can point its
//! `base_url` at Sentynyx and get the full Vendetta perimeter: detection →
//! aliasing → critical egress blocks → provider dispatch → de-aliased
//! responses. The provider never sees raw values; the client gets them back.
//!
//! Scope decisions (v1, all deliberate):
//! - **Loopback only.** The listener binds 127.0.0.1 and nothing else — there
//!   is no auth because there is no remote surface. Documented in TUTORIAL.
//! - **No new dependencies.** One route on one loopback socket doesn't need a
//!   web framework; requests are parsed with a minimal HTTP/1.1 reader
//!   (Content-Length bodies, `Connection: close` responses).
//! - **Regex + custom watchlist layers only.** NER and the paranoid LLM stay
//!   out of the proxy hot path for latency; the desktop composer remains the
//!   full four-layer experience.
//! - **Fresh alias map per request.** Aliases are consistent within a request
//!   (same email → same token across messages) but deliberately NOT linkable
//!   across requests.
//! - `sentynyx-local` is not proxyable (it needs the in-app model runtime);
//!   `ollama:*` models work and keep their zero-egress property.

use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch, Mutex};

use crate::providers::ChunkEvent;
use crate::vendetta::{self, Span};
use crate::{keys, router};

type SharedStore = Arc<Mutex<crate::store::Store>>;

pub const DEFAULT_PORT: u16 = 4242;
const MAX_HEAD_BYTES: usize = 32 * 1024;
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

/// Cloud model ids the proxy advertises on GET /v1/models, by provider key.
/// Keep in sync with `apps/desktop/src/lib/models.ts` — the router is the
/// actual dispatch authority; this list only feeds client model pickers.
const ADVERTISED_MODELS: &[(&str, &str)] = &[
    ("gpt-5", "openai"), ("gpt-5-mini", "openai"), ("o4", "openai"),
    ("claude-opus-4", "anthropic"), ("claude-sonnet", "anthropic"), ("claude-haiku", "anthropic"),
    ("gemini-2-5-pro", "google"), ("gemini-flash", "google"),
    ("grok-4", "xai"),
    ("openrouter:meta-llama/llama-4-maverick", "openrouter"),
    ("openrouter:deepseek/deepseek-v4-pro", "openrouter"),
    ("openrouter:mistralai/mistral-large-2512", "openrouter"),
    ("openrouter:qwen/qwen3.7-plus", "openrouter"),
    ("openrouter:cohere/command-a", "openrouter"),
];

struct Running {
    port: u16,
    shutdown: watch::Sender<bool>,
}

static RUNNING: Lazy<Mutex<Option<Running>>> = Lazy::new(|| Mutex::new(None));

pub async fn status() -> Option<u16> {
    RUNNING.lock().await.as_ref().map(|r| r.port)
}

pub async fn stop() {
    if let Some(r) = RUNNING.lock().await.take() {
        let _ = r.shutdown.send(true);
        eprintln!("[proxy] stop requested (was on port {})", r.port);
    }
}

/// Bind 127.0.0.1:`port` and serve until `stop()`. Returns the bound port.
pub async fn start(store: SharedStore, port: u16) -> Result<u16, String> {
    let mut guard = RUNNING.lock().await;
    if let Some(r) = &*guard {
        return Err(format!("proxy already running on port {}", r.port));
    }
    // Loopback bind is the security boundary — never a wildcard address.
    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|e| format!("bind 127.0.0.1:{port}: {e}"))?;
    let actual = listener.local_addr().map_err(|e| e.to_string())?.port();
    let (tx, rx) = watch::channel(false);
    *guard = Some(Running { port: actual, shutdown: tx });
    drop(guard);

    tokio::spawn(async move {
        let mut shutdown = rx;
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() { break; }
                }
                accepted = listener.accept() => {
                    let Ok((sock, _addr)) = accepted else { continue };
                    let store = store.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_conn(sock, store).await {
                            eprintln!("[proxy] conn error: {e}");
                        }
                    });
                }
            }
        }
        eprintln!("[proxy] listener closed");
    });

    eprintln!("[proxy] OpenAI-compatible endpoint on http://127.0.0.1:{actual}/v1");
    Ok(actual)
}

// ---------------------------------------------------------------------------
// HTTP plumbing
// ---------------------------------------------------------------------------

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

/// Parse "METHOD /path HTTP/1.1" + headers out of the head bytes. Returns
/// (method, path, content_length).
fn parse_request_head(head: &str) -> Option<(String, String, usize)> {
    let mut lines = head.split("\r\n");
    let request_line = lines.next()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut content_length = 0usize;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else { continue };
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value.trim().parse().ok()?;
        }
    }
    Some((method, path, content_length))
}

async fn read_request(sock: &mut TcpStream) -> Result<Request, String> {
    let mut buf: Vec<u8> = Vec::with_capacity(2048);
    let mut chunk = [0u8; 2048];
    let head_end = loop {
        if let Some(pos) = find_head_end(&buf) {
            break pos;
        }
        if buf.len() > MAX_HEAD_BYTES {
            return Err("request head too large".into());
        }
        let n = sock.read(&mut chunk).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("connection closed mid-head".into());
        }
        buf.extend_from_slice(&chunk[..n]);
    };

    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let (method, path, content_length) =
        parse_request_head(&head).ok_or("malformed request head")?;
    if content_length > MAX_BODY_BYTES {
        return Err("request body too large".into());
    }

    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < content_length {
        let n = sock.read(&mut chunk).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("connection closed mid-body".into());
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);
    Ok(Request { method, path, body })
}

fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

async fn write_json(sock: &mut TcpStream, status: &str, body: &Value) -> Result<(), String> {
    let payload = body.to_string();
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    sock.write_all(head.as_bytes()).await.map_err(|e| e.to_string())?;
    sock.write_all(payload.as_bytes()).await.map_err(|e| e.to_string())?;
    sock.flush().await.map_err(|e| e.to_string())
}

fn openai_error(message: &str, code: &str) -> Value {
    json!({ "error": { "message": message, "type": "invalid_request_error", "param": null, "code": code } })
}

// ---------------------------------------------------------------------------
// Request handling
// ---------------------------------------------------------------------------

async fn handle_conn(mut sock: TcpStream, store: SharedStore) -> Result<(), String> {
    let req = match read_request(&mut sock).await {
        Ok(r) => r,
        Err(e) => {
            let _ = write_json(&mut sock, "400 Bad Request", &openai_error(&e, "bad_request")).await;
            return Ok(());
        }
    };

    match (req.method.as_str(), req.path.split('?').next().unwrap_or("")) {
        ("GET", "/v1/models") => {
            let data: Vec<Value> = ADVERTISED_MODELS
                .iter()
                .filter(|(_, provider)| keys::has(provider))
                .map(|(id, provider)| json!({ "id": id, "object": "model", "owned_by": provider }))
                .collect();
            write_json(&mut sock, "200 OK", &json!({ "object": "list", "data": data })).await
        }
        ("POST", "/v1/chat/completions") => chat_completions(&mut sock, store, &req.body).await,
        _ => {
            write_json(
                &mut sock,
                "404 Not Found",
                &openai_error(
                    "Sentynyx privacy proxy serves POST /v1/chat/completions and GET /v1/models",
                    "not_found",
                ),
            )
            .await
        }
    }
}

/// Flatten an OpenAI `messages` array into the single prompt string the
/// Provider trait takes — same shape the desktop composer sends. Content may
/// be a plain string or an array of `{type:"text", text}` parts.
fn flatten_messages(messages: &[Value]) -> String {
    let mut flat = String::new();
    for m in messages {
        let role = m["role"].as_str().unwrap_or("user");
        let content = match &m["content"] {
            Value::String(s) => s.clone(),
            Value::Array(parts) => parts
                .iter()
                .filter_map(|p| p["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        };
        if content.is_empty() {
            continue;
        }
        let role_label = match role {
            "system" => "System",
            "assistant" => "Assistant",
            _ => "User",
        };
        flat.push_str(role_label);
        flat.push_str(": ");
        flat.push_str(&content);
        flat.push_str("\n\n");
    }
    flat.trim_end().to_string()
}

/// Pure core of the perimeter: fresh-alias the flat prompt and decide
/// block-vs-forward. Split out so tests can drive it without a socket.
pub(crate) struct Prepared {
    pub aliased: String,
    pub spans: Vec<Span>,
    pub block: Option<(String, &'static str, &'static str, &'static str)>, // (kind, rule, class, desc)
}

pub(crate) fn prepare_payload(flat: &str, merged_spans: Vec<Span>) -> Prepared {
    let mut map = vendetta::AliasMap::default();
    let mut counters: HashMap<String, usize> = HashMap::new();
    let spans = vendetta::apply_alias_map(&merged_spans, &mut map, &mut counters);
    let aliased = vendetta::aliasize(flat, &spans);
    let block = spans
        .iter()
        .find(|s| vendetta::is_critical(&s.kind))
        .and_then(|s| {
            vendetta::block_policy(&s.kind)
                .map(|p| (s.kind.as_str().to_string(), p.rule, p.class, p.desc))
        });
    Prepared { aliased, spans, block }
}

async fn chat_completions(
    sock: &mut TcpStream,
    store: SharedStore,
    body: &[u8],
) -> Result<(), String> {
    let v: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(e) => {
            return write_json(
                sock,
                "400 Bad Request",
                &openai_error(&format!("invalid JSON body: {e}"), "invalid_json"),
            )
            .await;
        }
    };
    let model = v["model"].as_str().unwrap_or("").to_string();
    let want_stream = v["stream"].as_bool().unwrap_or(false);
    let Some(messages) = v["messages"].as_array() else {
        return write_json(sock, "400 Bad Request", &openai_error("missing messages[]", "invalid_request")).await;
    };
    let flat = flatten_messages(messages);
    if flat.is_empty() {
        return write_json(sock, "400 Bad Request", &openai_error("empty messages", "invalid_request")).await;
    }

    // Detection: regex + custom watchlist, honoring the user's pack toggles.
    use crate::detect::Detector;
    let regex_spans = crate::detect::regex::RegexDetector
        .detect(&flat)
        .await
        .map_err(|e| e.to_string())?;
    let disabled = crate::commands::read_disabled_packs(&store).await;
    let regex_spans = crate::commands::filter_disabled_packs(regex_spans, &disabled);
    let custom = crate::detect::custom::custom_spans(&store, &flat).await;
    let merged = crate::detect::merge_spans(regex_spans, custom);

    let prepared = prepare_payload(&flat, merged);

    if let Some((kind, rule, _class, desc)) = &prepared.block {
        {
            let mut s = store.lock().await;
            let critical: Vec<Span> = prepared
                .spans
                .iter()
                .filter(|s| vendetta::is_critical(&s.kind))
                .cloned()
                .collect();
            let _ = s.append_audit_for_spans(&critical, "BLOCK", "proxy");
        }
        eprintln!("[proxy] BLOCKED egress: {kind} ({rule})");
        return write_json(
            sock,
            "400 Bad Request",
            &openai_error(
                &format!("Sentynyx blocked egress — {rule}. {desc}"),
                "sentynyx_policy_block",
            ),
        )
        .await;
    }

    // Resolve provider + key, mirroring commands::send.
    let (provider, api_key): (Box<dyn crate::providers::Provider>, String) =
        if let Some(name) = model.strip_prefix("ollama:") {
            let _ = name;
            let base = crate::commands::read_ollama_base_url(&store).await;
            (Box::new(crate::providers::ollama::Ollama { base_url: base }), String::new())
        } else if model == "sentynyx-local" {
            return write_json(
                sock,
                "400 Bad Request",
                &openai_error(
                    "sentynyx-local is only available inside the app; use an ollama:* model for zero-egress proxying",
                    "model_not_proxyable",
                ),
            )
            .await;
        } else {
            let Some((pname, p)) = router::provider_for(&model) else {
                return write_json(
                    sock,
                    "404 Not Found",
                    &openai_error(&format!("unknown model: {model}"), "model_not_found"),
                )
                .await;
            };
            let Some(key) = keys::get(pname) else {
                return write_json(
                    sock,
                    "401 Unauthorized",
                    &openai_error(
                        &format!("no API key configured for {pname} — add one in Sentynyx Settings"),
                        "no_provider_key",
                    ),
                )
                .await;
            };
            (p, key)
        };

    // Audit the aliasing before anything leaves the machine.
    if !prepared.spans.is_empty() {
        let mut s = store.lock().await;
        let _ = s.append_audit_for_spans(&prepared.spans, "ALIAS", "proxy");
    }

    // Dispatch with the aliased payload; de-alias the stream on the way back.
    let (tx, mut rx) = mpsc::channel::<ChunkEvent>(64);
    let aliased = prepared.aliased.clone();
    let model_clone = model.clone();
    tokio::spawn(async move {
        if let Err(e) = router::dispatch(&api_key, provider, &model_clone, &aliased, tx.clone()).await {
            let _ = tx.send(ChunkEvent::Error(e)).await;
        }
    });

    let reverse = vendetta::build_reverse_from_spans(&prepared.spans);
    let mut buf = String::new();
    let completion_id = format!("chatcmpl-snx-{}", chrono::Utc::now().timestamp_millis());
    let created = chrono::Utc::now().timestamp();

    if want_stream {
        sock.write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        )
        .await
        .map_err(|e| e.to_string())?;
        let sse_chunk = |delta: Value, finish: Option<&str>| {
            json!({
                "id": completion_id, "object": "chat.completion.chunk", "created": created,
                "model": model,
                "choices": [{ "index": 0, "delta": delta, "finish_reason": finish }]
            })
        };
        while let Some(evt) = rx.recv().await {
            match evt {
                ChunkEvent::Token(t) => {
                    let emit = vendetta::rehydrate_stream_with(&mut buf, &t, &reverse);
                    if !emit.is_empty() {
                        let frame = format!("data: {}\n\n", sse_chunk(json!({ "content": emit }), None));
                        sock.write_all(frame.as_bytes()).await.map_err(|e| e.to_string())?;
                    }
                }
                ChunkEvent::Done => {
                    let remainder = std::mem::take(&mut buf);
                    if !remainder.is_empty() {
                        let frame = format!("data: {}\n\n", sse_chunk(json!({ "content": remainder }), None));
                        sock.write_all(frame.as_bytes()).await.map_err(|e| e.to_string())?;
                    }
                    let fin = format!("data: {}\n\ndata: [DONE]\n\n", sse_chunk(json!({}), Some("stop")));
                    sock.write_all(fin.as_bytes()).await.map_err(|e| e.to_string())?;
                    break;
                }
                ChunkEvent::Error(e) => {
                    let frame = format!(
                        "data: {}\n\ndata: [DONE]\n\n",
                        json!({ "error": { "message": e, "type": "upstream_error" } })
                    );
                    sock.write_all(frame.as_bytes()).await.map_err(|e| e.to_string())?;
                    break;
                }
            }
        }
        sock.flush().await.map_err(|e| e.to_string())
    } else {
        let mut full = String::new();
        let mut upstream_err: Option<String> = None;
        while let Some(evt) = rx.recv().await {
            match evt {
                ChunkEvent::Token(t) => {
                    full.push_str(&vendetta::rehydrate_stream_with(&mut buf, &t, &reverse));
                }
                ChunkEvent::Done => {
                    full.push_str(&std::mem::take(&mut buf));
                    break;
                }
                ChunkEvent::Error(e) => {
                    upstream_err = Some(e);
                    break;
                }
            }
        }
        if let Some(e) = upstream_err {
            return write_json(sock, "502 Bad Gateway", &openai_error(&e, "upstream_error")).await;
        }
        write_json(
            sock,
            "200 OK",
            &json!({
                "id": completion_id, "object": "chat.completion", "created": created,
                "model": model,
                "choices": [{
                    "index": 0,
                    "message": { "role": "assistant", "content": full },
                    "finish_reason": "stop"
                }],
                "usage": { "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 }
            }),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::Detector;

    #[test]
    fn parses_request_head() {
        let head = "POST /v1/chat/completions HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 42\r\nContent-Type: application/json";
        let (m, p, l) = parse_request_head(head).unwrap();
        assert_eq!(m, "POST");
        assert_eq!(p, "/v1/chat/completions");
        assert_eq!(l, 42);
        // Header casing must not matter.
        let (_, _, l) = parse_request_head("GET /v1/models HTTP/1.1\r\ncontent-length: 7").unwrap();
        assert_eq!(l, 7);
        assert!(parse_request_head("").is_none());
    }

    #[test]
    fn flattens_string_and_part_messages() {
        let messages = vec![
            json!({ "role": "system", "content": "be terse" }),
            json!({ "role": "user", "content": [ { "type": "text", "text": "hello" } ] }),
            json!({ "role": "assistant", "content": "" }),
        ];
        let flat = flatten_messages(&messages);
        assert_eq!(flat, "System: be terse\n\nUser: hello");
    }

    #[tokio::test]
    async fn prepare_blocks_critical_and_aliases_pii() {
        let det = crate::detect::regex::RegexDetector;
        // SSN → block with the SSN policy.
        let flat = "User: my ssn is 123-45-6789";
        let spans = det.detect(flat).await.unwrap();
        let prepared = prepare_payload(flat, spans);
        let (kind, _rule, class, _desc) = prepared.block.expect("SSN must block");
        assert_eq!(kind, "SSN");
        assert_eq!(class, "CRITICAL_IDENTITY");

        // Email → aliased, not blocked, and the alias round-trips back.
        let flat = "User: mail dana.reyes@example.com about the renewal";
        let spans = det.detect(flat).await.unwrap();
        let prepared = prepare_payload(flat, spans);
        assert!(prepared.block.is_none());
        assert!(!prepared.aliased.contains("dana.reyes@example.com"));
        assert!(prepared.aliased.contains("\u{27E6}email_01\u{27E7}"));
        let reverse = vendetta::build_reverse_from_spans(&prepared.spans);
        let mut buf = String::new();
        let mut out = vendetta::rehydrate_stream_with(&mut buf, "ping \u{27E6}email_01\u{27E7} now", &reverse);
        out.push_str(&buf);
        assert!(out.contains("dana.reyes@example.com"), "{out}");
    }

    #[test]
    fn head_end_detection() {
        assert_eq!(find_head_end(b"a\r\n\r\nbody"), Some(1));
        assert_eq!(find_head_end(b"no end yet"), None);
    }

    /// Full loopback round-trip on an ephemeral port: an SSN in the payload
    /// must come back as an OpenAI-shaped 400 with our policy code, before
    /// any provider/key resolution (so no keychain access in CI).
    #[tokio::test]
    async fn live_loopback_blocks_critical() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Mutex::new(
            crate::store::Store::open_at(&dir.path().join("t.db")).unwrap(),
        ));
        let port = start(store, 0).await.unwrap();

        let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let body = r#"{"model":"gpt-5","messages":[{"role":"user","content":"my ssn is 123-45-6789"}]}"#;
        let req = format!(
            "POST /v1/chat/completions HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(), body
        );
        s.write_all(req.as_bytes()).await.unwrap();
        let mut resp = String::new();
        s.read_to_string(&mut resp).await.unwrap();
        stop().await;

        assert!(resp.starts_with("HTTP/1.1 400"), "{resp}");
        assert!(resp.contains("sentynyx_policy_block"), "{resp}");
        assert!(resp.contains("Social Security"), "{resp}");
    }
}
