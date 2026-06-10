use super::{ChunkEvent, Provider};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::json;
use tokio::sync::mpsc;

pub struct Xai;

#[async_trait]
impl Provider for Xai {
    async fn stream(&self, api_key: &str, model: &str, prompt: &str, tx: mpsc::Sender<ChunkEvent>) -> Result<(), String> {
        let body = json!({
            "model": model_name(model),
            "stream": true,
            "messages":[{"role":"user","content": prompt}],
        });
        let res = reqwest::Client::new()
            .post("https://api.x.ai/v1/chat/completions")
            .bearer_auth(api_key)
            .json(&body)
            .send().await.map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            let s = res.status();
            let t = res.text().await.unwrap_or_default();
            return Err(super::friendly_http_error("xai", s.as_u16(), &t));
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

fn model_name(id: &str) -> &str {
    match id { "grok-4" => "grok-4-latest", other => other }
}
