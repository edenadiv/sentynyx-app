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

#[derive(Debug, Clone)]
pub enum ChunkEvent {
    Token(String),
    Done,
    Error(String),
}
