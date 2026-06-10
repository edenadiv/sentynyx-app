pub mod openai;
pub mod anthropic;
pub mod google;
pub mod xai;
pub mod local;
pub mod ollama;
pub mod openrouter;

use async_trait::async_trait;
use tokio::sync::mpsc;

#[async_trait]
pub trait Provider: Send + Sync {
    async fn stream(&self, api_key: &str, model: &str, prompt: &str, tx: mpsc::Sender<ChunkEvent>) -> Result<(), String>;
}

/// Turn a provider HTTP error into something a human can act on. Providers
/// return JSON blobs ({"error":{"message":...}}); surface the message, map
/// the common statuses to plain language, and keep the raw status for the
/// curious. Falls back to a trimmed body when there's no JSON message.
pub fn friendly_http_error(provider: &str, status: u16, body: &str) -> String {
    let msg = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            ["message", "error"].iter().find_map(|_| None::<String>)
                .or_else(|| v["error"]["message"].as_str().map(str::to_string))
                .or_else(|| v["error"].as_str().map(str::to_string))
                .or_else(|| v["message"].as_str().map(str::to_string))
        })
        .unwrap_or_else(|| {
            let t = body.trim();
            if t.len() > 160 { format!("{}…", &t[..t.char_indices().take_while(|(i, _)| *i < 160).count()]) } else { t.to_string() }
        });
    let hint = match status {
        401 | 403 => " — the API key looks invalid or revoked; re-add it in Settings (⌘,)",
        404 => " — model not found for this account",
        429 => " — rate limit or quota exhausted at the provider",
        500..=599 => " — provider-side outage, retry shortly",
        _ => "",
    };
    format!("{provider} {status}: {msg}{hint}")
}

#[derive(Debug, Clone)]
pub enum ChunkEvent {
    Token(String),
    Done,
    Error(String),
}
