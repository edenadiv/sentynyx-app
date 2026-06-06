use super::{ChunkEvent, Provider};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::json;
use tokio::sync::mpsc;

/// Local-model provider backed by an Ollama server (https://ollama.com).
///
/// Talks to Ollama's native `POST /api/chat`, which streams newline-delimited
/// JSON (NDJSON): one JSON object per line, each carrying an incremental
/// `message.content`, terminated by an object with `done: true`. There is no
/// API key — the only configuration is the base URL (default
/// `http://localhost:11434`), injected by the caller from the settings table.
///
/// Model ids arrive prefixed (`ollama:<name>`) so the rest of the app can
/// recognize Ollama routing; we strip the prefix before putting the real
/// model name on the wire. Ollama names themselves may contain colons
/// (e.g. `qwen2.5:0.5b`), which `strip_prefix` leaves intact.
pub struct Ollama {
    pub base_url: String,
}

#[async_trait]
impl Provider for Ollama {
    async fn stream(
        &self,
        _api_key: &str,
        model: &str,
        prompt: &str,
        tx: mpsc::Sender<ChunkEvent>,
    ) -> Result<(), String> {
        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));
        let body = json!({
            "model": model_name(model),
            "stream": true,
            "messages": [{"role": "user", "content": prompt}],
        });
        let client = reqwest::Client::new();
        let res = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("ollama: cannot reach {url}: {e}"))?;

        if !res.status().is_success() {
            let s = res.status();
            let t = res.text().await.unwrap_or_default();
            return Err(format!("ollama {}: {}", s, t));
        }

        let mut stream = res.bytes_stream();
        let mut buf = String::new();
        while let Some(b) = stream.next().await {
            let bytes = b.map_err(|e| e.to_string())?;
            buf.push_str(&String::from_utf8_lossy(&bytes));
            // NDJSON: one JSON object per line. Process every complete line and
            // hold any partial trailing line in `buf` until the next chunk.
            while let Some(idx) = buf.find('\n') {
                let line = buf[..idx].trim().to_string();
                buf.drain(..idx + 1);
                if line.is_empty() {
                    continue;
                }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                    if let Some(c) = v["message"]["content"].as_str() {
                        if !c.is_empty() {
                            let _ = tx.send(ChunkEvent::Token(c.to_string())).await;
                        }
                    }
                    // The terminal frame carries `done: true` (and usually an
                    // empty content) — emit Done regardless of its content.
                    if v["done"].as_bool().unwrap_or(false) {
                        let _ = tx.send(ChunkEvent::Done).await;
                        return Ok(());
                    }
                }
            }
        }

        // Flush a trailing line that arrived without a final newline.
        let tail = buf.trim();
        if !tail.is_empty() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(tail) {
                if let Some(c) = v["message"]["content"].as_str() {
                    if !c.is_empty() {
                        let _ = tx.send(ChunkEvent::Token(c.to_string())).await;
                    }
                }
            }
        }
        let _ = tx.send(ChunkEvent::Done).await;
        Ok(())
    }
}

/// Strip the `ollama:` routing prefix to recover the real model name Ollama
/// knows it by. A bare name (no prefix) is returned unchanged.
fn model_name(id: &str) -> &str {
    id.strip_prefix("ollama:").unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_routing_prefix_but_keeps_inner_colons() {
        assert_eq!(model_name("ollama:llama3.2"), "llama3.2");
        assert_eq!(model_name("ollama:qwen2.5:0.5b"), "qwen2.5:0.5b");
        assert_eq!(model_name("llama3.2"), "llama3.2");
    }
}
