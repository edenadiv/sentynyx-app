use super::{ChunkEvent, Provider};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::json;
use tokio::sync::mpsc;

pub struct Anthropic;

#[async_trait]
impl Provider for Anthropic {
    async fn stream(&self, api_key: &str, model: &str, prompt: &str, tx: mpsc::Sender<ChunkEvent>) -> Result<(), String> {
        let body = json!({
            "model": model_name(model),
            "max_tokens": 2048,
            "stream": true,
            "messages": [{"role":"user","content": prompt}],
        });
        let res = reqwest::Client::new()
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send().await.map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            let s = res.status();
            let t = res.text().await.unwrap_or_default();
            return Err(format!("anthropic {}: {}", s, t));
        }
        let mut stream = res.bytes_stream();
        let mut buf = String::new();
        while let Some(b) = stream.next().await {
            let bytes = b.map_err(|e| e.to_string())?;
            buf.push_str(&String::from_utf8_lossy(&bytes));
            while let Some(idx) = buf.find("\n\n") {
                let event = buf[..idx].to_string();
                buf.drain(..idx+2);
                for line in event.lines() {
                    let Some(data) = line.strip_prefix("data: ") else { continue };
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                        match v["type"].as_str() {
                            Some("content_block_delta") => {
                                if let Some(t) = v["delta"]["text"].as_str() {
                                    let _ = tx.send(ChunkEvent::Token(t.to_string())).await;
                                }
                            }
                            Some("message_stop") => { let _ = tx.send(ChunkEvent::Done).await; return Ok(()); }
                            _ => {}
                        }
                    }
                }
            }
        }
        let _ = tx.send(ChunkEvent::Done).await;
        Ok(())
    }
}

fn model_name(id: &str) -> &str {
    match id {
        "claude-opus-4" => "claude-opus-4-5",
        "claude-sonnet" => "claude-sonnet-4-5",
        "claude-haiku"  => "claude-haiku-4-5",
        other => other,
    }
}
