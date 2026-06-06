use async_trait::async_trait;
use std::collections::HashMap;
use crate::vendetta::{self, Span};
use super::{Detector, DetectError, Source};

/// Stateless wrapper that runs the existing regex engine with a fresh alias map.
/// For the merge pipeline we don't want per-call aliases here — the caller
/// (`commands::send`) owns the conversation alias state. This detector returns
/// spans with empty aliases; the caller re-runs aliasing on the merged set.
pub struct RegexDetector;

#[async_trait]
impl Detector for RegexDetector {
    fn source(&self) -> Source { Source::Regex }

    async fn detect(&self, text: &str) -> Result<Vec<Span>, DetectError> {
        let mut map = HashMap::new();
        let mut counters = HashMap::new();
        // Clear aliases — the merge caller applies its own alias state afterward.
        let mut spans = vendetta::detect(text, &mut map, &mut counters);
        for s in spans.iter_mut() { s.alias.clear(); }
        Ok(spans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vendetta::Kind;

    #[tokio::test]
    async fn regex_detector_returns_email_spans() {
        let d = RegexDetector;
        let spans = d.detect("email alice@acme.com now").await.unwrap();
        assert!(spans.iter().any(|s| matches!(s.kind, Kind::EMAIL)));
    }

    #[tokio::test]
    async fn regex_detector_returns_empty_aliases() {
        let d = RegexDetector;
        let spans = d.detect("email a@b.c").await.unwrap();
        for s in &spans { assert!(s.alias.is_empty()); }
    }
}
