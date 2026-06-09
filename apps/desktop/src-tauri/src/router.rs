use crate::providers::{Provider, ChunkEvent, openai::OpenAI, anthropic::Anthropic, google::Google, xai::Xai, openrouter::OpenRouter};
use tokio::sync::mpsc;

pub fn provider_for(model_id: &str) -> Option<(&'static str, Box<dyn Provider>)> {
    // OpenRouter ids are dynamic (`openrouter:vendor/model`), so they route by
    // prefix rather than the exact-id table below.
    if model_id.starts_with("openrouter:") {
        return Some(("openrouter", Box::new(OpenRouter)));
    }
    match model_id {
        "gpt-5" | "gpt-5-mini" | "o4" => Some(("openai", Box::new(OpenAI))),
        "claude-opus-4" | "claude-sonnet" | "claude-haiku" => Some(("anthropic", Box::new(Anthropic))),
        "gemini-2-5-pro" | "gemini-flash" => Some(("google", Box::new(Google))),
        "grok-4" => Some(("xai", Box::new(Xai))),
        _ => None,
    }
}

pub async fn dispatch(api_key: &str, provider: Box<dyn Provider>, model_id: &str, aliased_prompt: &str, tx: mpsc::Sender<ChunkEvent>) -> Result<(), String> {
    provider.stream(api_key, model_id, aliased_prompt, tx).await
}
