pub mod regex;
pub mod ner;
pub mod ner_sidecar;
pub mod llm;
pub mod custom;
pub mod structured;

use async_trait::async_trait;
use serde::Serialize;
use crate::vendetta::Span;
#[cfg(test)]
use crate::vendetta::Kind;

#[derive(Debug, thiserror::Error)]
pub enum DetectError {
    #[error("model not loaded: {0}")]
    ModelNotLoaded(String),
    #[error("inference failed: {0}")]
    Inference(String),
    #[error("timeout")]
    Timeout,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Source { Regex, Ner, Llm }

#[async_trait]
pub trait Detector: Send + Sync {
    fn source(&self) -> Source;
    async fn detect(&self, text: &str) -> Result<Vec<Span>, DetectError>;
}

/// Merge regex and NER spans. Regex wins on overlap.
/// Non-overlapping NER spans are kept. Both inputs should already be span-valid
/// (start < end, offsets within text bounds).
pub fn merge_spans(mut regex: Vec<Span>, ner: Vec<Span>) -> Vec<Span> {
    for n in ner {
        let overlaps = regex.iter().any(|r| ranges_overlap(r.start, r.end, n.start, n.end));
        if !overlaps {
            regex.push(n);
        }
    }
    regex.sort_by_key(|s| s.start);
    regex
}

fn ranges_overlap(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> bool {
    a_start < b_end && b_start < a_end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sp(start: usize, end: usize, kind: Kind, raw: &str, alias: &str) -> Span {
        Span { start, end, kind, raw: raw.to_string(), alias: alias.to_string(), confidence: 1.0 }
    }

    #[test]
    fn merge_preserves_non_overlapping_ner_spans() {
        let regex = vec![sp(0, 5, Kind::EMAIL, "a@b.c", "{{email_01}}")];
        let ner = vec![sp(10, 18, Kind::PERSON_NER, "Jamie T.", "{{person_NER_01}}")];
        let merged = merge_spans(regex, ner);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].start, 0);
        assert_eq!(merged[1].start, 10);
    }

    #[test]
    fn merge_drops_ner_when_regex_overlaps() {
        let regex = vec![sp(0, 10, Kind::NAME, "Sarah Chen", "{{person_01}}")];
        let ner = vec![sp(6, 14, Kind::PERSON_NER, "Chen Smith", "{{person_NER_01}}")];
        let merged = merge_spans(regex, ner);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].alias, "{{person_01}}");
    }

    #[test]
    fn merge_drops_ner_fully_inside_regex() {
        let regex = vec![sp(0, 20, Kind::EMAIL, "alice@example.com", "{{email_01}}")];
        let ner = vec![sp(5, 10, Kind::PERSON_NER, "inner", "{{person_NER_01}}")];
        let merged = merge_spans(regex, ner);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn merge_sorts_output_by_start() {
        let regex = vec![sp(20, 25, Kind::EMAIL, "b@c.d", "{{email_01}}")];
        let ner = vec![sp(0, 5, Kind::PERSON_NER, "Ana", "{{person_NER_01}}")];
        let merged = merge_spans(regex, ner);
        assert_eq!(merged[0].start, 0);
        assert_eq!(merged[1].start, 20);
    }

    #[test]
    fn merge_handles_empty_inputs() {
        assert!(merge_spans(vec![], vec![]).is_empty());
        let r = vec![sp(0, 5, Kind::EMAIL, "a@b.c", "{{email_01}}")];
        assert_eq!(merge_spans(r.clone(), vec![]).len(), 1);
        assert_eq!(merge_spans(vec![], r).len(), 1);
    }

    #[test]
    fn ranges_overlap_boundary_cases() {
        // touching but not overlapping
        assert!(!ranges_overlap(0, 5, 5, 10));
        assert!(!ranges_overlap(5, 10, 0, 5));
        // single-point overlap
        assert!(ranges_overlap(0, 6, 5, 10));
        // identical ranges
        assert!(ranges_overlap(0, 5, 0, 5));
        // one contains the other
        assert!(ranges_overlap(0, 10, 3, 7));
    }
}
