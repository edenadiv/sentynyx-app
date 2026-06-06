use super::{ChunkEvent, Provider};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::detect::llm::ParanoidDetector;

/// On-device chat provider backed by the same Qwen 2.5 0.5B GGUF that the
/// paranoid scanner uses. Shares the detector Arc so there's no extra model
/// load — the mutex inside the detector serializes chat and paranoid runs.
pub struct Local {
    pub detector: Arc<ParanoidDetector>,
}

#[async_trait]
impl Provider for Local {
    async fn stream(
        &self,
        _api_key: &str,
        _model: &str,
        prompt: &str,
        tx: mpsc::Sender<ChunkEvent>,
    ) -> Result<(), String> {
        self.detector.chat_stream(prompt, tx).await
    }
}
