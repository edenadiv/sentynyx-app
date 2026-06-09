use super::{ChunkEvent, Provider};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::json;
use tokio::sync::mpsc;

/// OpenRouter (https://openrouter.ai) — one BYOK key, hundreds of models
/// behind an OpenAI-compatible API. Model ids arrive prefixed
/// (`openrouter:vendor/model`) so the router can dispatch without a
/// hardcoded id list; the prefix is stripped before the wire call.
pub struct OpenRouter;

#[async_trait]
impl Provider for OpenRouter {
    async fn stream(&self, api_key: &str, model: &str, prompt: &str, tx: mpsc::Sender<ChunkEvent>) -> Result<(), String> {
        let body = json!({
            "model": model_name(model),
            "stream": true,
            "messages": [{"role": "user", "content": prompt}],
        });
        let client = reqwest::Client::new();
        let res = client.post("https://openrouter.ai/api/v1/chat/completions")
            .bearer_auth(api_key)
            // App attribution headers OpenRouter asks integrators to send.
            .header("HTTP-Referer", "https://github.com/edenadiv/sentynyx-app")
            .header("X-Title", "Sentynyx")
            .json(&body)
            .send().await.map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            let s = res.status();
            let t = res.text().await.unwrap_or_default();
            return Err(format!("openrouter {}: {}", s, t));
        }
        let mut stream = res.bytes_stream();
        let mut buf = String::new();
        while let Some(b) = stream.next().await {
            let bytes = b.map_err(|e| e.to_string())?;
            buf.push_str(&String::from_utf8_lossy(&bytes));
            while let Some(idx) = buf.find("\n\n") {
                let event = buf[..idx].to_string();
                buf.drain(..idx + 2);
                for line in event.lines() {
                    let Some(data) = line.strip_prefix("data: ") else { continue };
                    if data.trim() == "[DONE]" { let _ = tx.send(ChunkEvent::Done).await; return Ok(()); }
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(c) = v["choices"][0]["delta"]["content"].as_str() {
                            if !c.is_empty() { let _ = tx.send(ChunkEvent::Token(c.to_string())).await; }
                        }
                    }
                }
            }
        }
        let _ = tx.send(ChunkEvent::Done).await;
        Ok(())
    }
}

/// Strip the `openrouter:` routing prefix; vendor/model ids pass through
/// untouched (they contain a slash, e.g. `meta-llama/llama-4-maverick`).
fn model_name(id: &str) -> &str {
    id.strip_prefix("openrouter:").unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_routing_prefix_keeps_vendor_path() {
        assert_eq!(model_name("openrouter:meta-llama/llama-4-maverick"), "meta-llama/llama-4-maverick");
        assert_eq!(model_name("deepseek/deepseek-v4-pro"), "deepseek/deepseek-v4-pro");
    }
}
