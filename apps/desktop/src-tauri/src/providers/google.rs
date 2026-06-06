use super::{ChunkEvent, Provider};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::json;
use tokio::sync::mpsc;

pub struct Google;

#[async_trait]
impl Provider for Google {
    async fn stream(&self, api_key: &str, model: &str, prompt: &str, tx: mpsc::Sender<ChunkEvent>) -> Result<(), String> {
        let m = model_name(model);
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            m, api_key
        );
        let body = json!({
            "contents": [{"role":"user","parts":[{"text": prompt}]}],
        });
        let res = reqwest::Client::new().post(&url).json(&body).send().await.map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            let s = res.status();
            let t = res.text().await.unwrap_or_default();
            return Err(format!("google {}: {}", s, t));
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
                        if let Some(parts) = v["candidates"][0]["content"]["parts"].as_array() {
                            for p in parts {
                                if let Some(t) = p["text"].as_str() {
                                    let _ = tx.send(ChunkEvent::Token(t.to_string())).await;
                                }
                            }
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
        "gemini-2-5-pro" => "gemini-2.5-pro",
        "gemini-flash"   => "gemini-2.5-flash",
        other => other,
    }
}
