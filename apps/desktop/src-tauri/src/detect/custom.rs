//! User-defined watchlist detection.
//!
//! Users add their own sensitive terms (project codenames, client names,
//! internal hostnames…) in Settings → Custom watchlist. Terms are stored as a
//! JSON array under the `custom_watchlist` settings key, matched
//! case-insensitively as whole words, aliased as `⟦custom_NN⟧`, and never
//! block egress.
//!
//! This lives outside `vendetta::PATTERNS` on purpose: PATTERNS is a pure,
//! process-wide static shared by the eval harness and tests — user config
//! doesn't belong in it, and keeping it out preserves eval determinism.

use once_cell::sync::Lazy;
use regex::Regex;
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;

use crate::store::Store;
use crate::vendetta::{Kind, Span};

pub const SETTING_KEY: &str = "custom_watchlist";
const MAX_TERMS: usize = 200;
const MIN_TERM_LEN: usize = 2;
const MAX_TERM_LEN: usize = 120;

/// Compiled-regex cache keyed on the raw settings string. Re-reading the
/// setting every call is one indexed SQLite PK lookup (send() already does
/// several); string-comparing it against the cache key means we never need
/// invalidation wiring in `set_setting` — the regex rebuilds exactly when the
/// stored value changes.
static CACHE: Lazy<RwLock<(String, Option<Regex>)>> =
    Lazy::new(|| RwLock::new((String::new(), None)));

/// Detect watchlist terms in `text`. Returns un-aliased spans (the caller
/// merges them and runs `apply_alias_map`). Fails open: a missing setting,
/// malformed JSON, or a pathological pattern yields no matches, never an error.
pub async fn custom_spans(store: &Arc<Mutex<Store>>, text: &str) -> Vec<Span> {
    let raw_setting: String = {
        let s = store.lock().await;
        s.conn
            .query_row(
                "SELECT value FROM settings WHERE key=?",
                rusqlite::params![SETTING_KEY],
                |r| r.get(0),
            )
            .unwrap_or_default()
    };
    spans_for(&raw_setting, text)
}

/// Pure core, separated so tests don't need a Store.
pub fn spans_for(raw_setting: &str, text: &str) -> Vec<Span> {
    {
        let g = CACHE.read().expect("watchlist cache poisoned");
        if g.0 == raw_setting {
            return match &g.1 {
                Some(re) => find(re, text),
                None => Vec::new(),
            };
        }
    }
    let re = build(raw_setting);
    let out = match &re {
        Some(r) => find(r, text),
        None => Vec::new(),
    };
    *CACHE.write().expect("watchlist cache poisoned") = (raw_setting.to_string(), re);
    out
}

fn build(raw: &str) -> Option<Regex> {
    let terms: Vec<String> = serde_json::from_str(raw).ok()?;
    let mut cleaned: Vec<String> = terms
        .into_iter()
        .map(|t| t.trim().to_string())
        .filter(|t| (MIN_TERM_LEN..=MAX_TERM_LEN).contains(&t.len()))
        .take(MAX_TERMS)
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    // Longest alternative first so "Project Nightfall Beta" beats "Project
    // Nightfall" when both are listed.
    cleaned.sort_by(|a, b| b.len().cmp(&a.len()));
    let body = cleaned
        .iter()
        .map(|t| regex::escape(t))
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&format!(r"(?i)\b(?:{body})\b")).ok()
}

fn find(re: &Regex, text: &str) -> Vec<Span> {
    re.find_iter(text)
        .map(|m| Span {
            start: m.start(),
            end: m.end(),
            kind: Kind::CUSTOM,
            raw: m.as_str().to_string(),
            alias: String::new(),
            // User explicitly listed these terms — maximal confidence.
            confidence: 1.0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_terms_case_insensitively_as_whole_words() {
        let setting = r#"["Project Nightfall","Acme Corp"]"#;
        let spans = spans_for(setting, "Status of project nightfall and Acme Corp?");
        assert_eq!(spans.len(), 2);
        assert!(spans.iter().all(|s| matches!(s.kind, Kind::CUSTOM)));
        // substring of a longer word must NOT match
        assert!(spans_for(setting, "AcmeCorpHoldings filed").is_empty());
    }

    #[test]
    fn regex_metacharacters_in_terms_are_escaped() {
        let setting = r#"["a.b*c(d"]"#;
        // Must compile (no panic) and match only the literal text.
        assert_eq!(spans_for(setting, "found a.b*c(d here").len(), 1);
        assert!(spans_for(setting, "found aXbYcZd here").is_empty());
    }

    #[test]
    fn malformed_or_empty_settings_fail_open() {
        assert!(spans_for("", "anything").is_empty());
        assert!(spans_for("not json", "anything").is_empty());
        assert!(spans_for(r#"["x"]"#, "x too short to count").is_empty()); // < MIN_TERM_LEN
    }

    #[test]
    fn term_cap_is_enforced() {
        let many: Vec<String> = (0..400).map(|i| format!("term{i:04}")).collect();
        let setting = serde_json::to_string(&many).unwrap();
        // term0199 is within the cap, term0350 is beyond it.
        assert_eq!(spans_for(&setting, "saw term0199 today").len(), 1);
        assert!(spans_for(&setting, "saw term0350 today").is_empty());
    }

    #[test]
    fn longer_term_wins_within_watchlist() {
        let setting = r#"["Project Nightfall","Project Nightfall Beta"]"#;
        let spans = spans_for(setting, "ship Project Nightfall Beta now");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].raw, "Project Nightfall Beta");
    }
}
