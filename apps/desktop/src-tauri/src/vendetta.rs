use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Kind {
    EMAIL, PHONE, SSN, IP, APIKEY, URL, ADDRESS, MONEY, NAME, COMPANY, EMPID,
    PERSON_NER, ORG_NER, CODENAME_NER, LOCATION_NER, EMPID_NER,
}

impl Kind {
    pub fn label(&self) -> &'static str {
        match self {
            Kind::EMAIL => "email", Kind::PHONE => "phone", Kind::SSN => "ssn",
            Kind::IP => "ip", Kind::APIKEY => "api-key", Kind::URL => "url",
            Kind::ADDRESS => "address", Kind::MONEY => "amount",
            Kind::NAME => "person", Kind::COMPANY => "entity", Kind::EMPID => "employee-id",
            Kind::PERSON_NER => "person", Kind::ORG_NER => "entity",
            Kind::CODENAME_NER => "codename", Kind::LOCATION_NER => "location",
            Kind::EMPID_NER => "employee-id",
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::EMAIL => "EMAIL", Kind::PHONE => "PHONE", Kind::SSN => "SSN",
            Kind::IP => "IP", Kind::APIKEY => "APIKEY", Kind::URL => "URL",
            Kind::ADDRESS => "ADDRESS", Kind::MONEY => "MONEY",
            Kind::NAME => "NAME", Kind::COMPANY => "COMPANY", Kind::EMPID => "EMPID",
            Kind::PERSON_NER => "PERSON_NER", Kind::ORG_NER => "ORG_NER",
            Kind::CODENAME_NER => "CODENAME_NER", Kind::LOCATION_NER => "LOCATION_NER",
            Kind::EMPID_NER => "EMPID_NER",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub kind: Kind,
    pub raw: String,
    pub alias: String,
}

struct Pattern { kind: Kind, re: Regex }

static PATTERNS: Lazy<Vec<Pattern>> = Lazy::new(|| vec![
    Pattern { kind: Kind::EMAIL,   re: Regex::new(r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b").unwrap() },
    Pattern { kind: Kind::PHONE,   re: Regex::new(r"\b(?:\+?\d{1,3}[\s.\-]?)?(?:\(?\d{3}\)?[\s.\-]?)\d{3}[\s.\-]?\d{4}\b").unwrap() },
    Pattern { kind: Kind::SSN,     re: Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap() },
    Pattern { kind: Kind::IP,      re: Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap() },
    Pattern { kind: Kind::APIKEY,  re: Regex::new(r"\b(?:sk-|pk_|AKIA|ghp_|xox[baprs]-)[A-Za-z0-9_\-]{10,}\b").unwrap() },
    Pattern { kind: Kind::URL,     re: Regex::new(r"\bhttps?://[^\s)]+").unwrap() },
    Pattern { kind: Kind::ADDRESS, re: Regex::new(r"\b\d{1,5}\s+[A-Z][a-zA-Z]+(?:\s+[A-Z][a-zA-Z]+){0,3}\s+(?:Street|St|Avenue|Ave|Road|Rd|Blvd|Lane|Ln|Drive|Dr|Court|Ct|Way)\b").unwrap() },
    Pattern { kind: Kind::MONEY,   re: Regex::new(r"\$\s?\d{1,3}(?:,\d{3})+(?:\.\d+)?|\$\s?\d{4,}(?:\.\d+)?").unwrap() },
    Pattern { kind: Kind::NAME,    re: Regex::new(r"\b(?:Sarah Chen|Marcus Rodriguez|Elena Volkov|James Patterson|Priya Shah|Nikolai Ivanov|David Kim|Anna Müller)\b").unwrap() },
    Pattern { kind: Kind::COMPANY, re: Regex::new(r"\b(?:Project Helios|Project Orion|Northwind Capital|Halcyon Labs|Blackbird Initiative|Atlas Holdings|Meridian Pharma)\b").unwrap() },
    Pattern { kind: Kind::EMPID,   re: Regex::new(r"\bEMP-\d{4,6}\b").unwrap() },
]);

pub fn is_critical(k: &Kind) -> bool { matches!(k, Kind::SSN | Kind::APIKEY) }

/// An alias map keyed on `KIND::lowercased-raw` -> alias token.
pub type AliasMap = HashMap<String, String>;

fn alias_key(kind: &Kind, raw: &str) -> String {
    format!("{}::{}", kind.as_str(), raw.to_lowercase())
}

pub fn detect(text: &str, map: &mut AliasMap, counters: &mut HashMap<String, usize>) -> Vec<Span> {
    let mut hits: Vec<(usize, usize, Kind, String)> = Vec::new();
    for p in PATTERNS.iter() {
        for m in p.re.find_iter(text) {
            hits.push((m.start(), m.end(), p.kind.clone(), m.as_str().to_string()));
        }
    }
    hits.sort_by(|a, b| a.0.cmp(&b.0).then((b.1 - b.0).cmp(&(a.1 - a.0))));

    let mut out: Vec<Span> = Vec::new();
    let mut cursor: isize = -1;
    for (s, e, kind, raw) in hits {
        if (s as isize) < cursor { continue; }
        cursor = e as isize;
        let key = alias_key(&kind, &raw);
        let alias = if let Some(a) = map.get(&key) {
            a.clone()
        } else {
            let c = counters.entry(kind.as_str().to_string()).or_insert(0);
            *c += 1;
            let a = format!("\u{27E6}{}_{:02}\u{27E7}", kind.label(), *c);
            map.insert(key, a.clone());
            a
        };
        out.push(Span { start: s, end: e, kind, raw, alias });
    }
    out
}

/// Given spans that may not have aliases assigned yet, walk them through the
/// conversation alias map — reusing existing aliases where the same raw token
/// has been seen, or minting new ones.
pub fn apply_alias_map(
    spans: &[Span],
    map: &mut AliasMap,
    counters: &mut std::collections::HashMap<String, usize>,
) -> Vec<Span> {
    let mut out = Vec::with_capacity(spans.len());
    for s in spans {
        let key = alias_key(&s.kind, &s.raw);
        let alias = if let Some(a) = map.get(&key) {
            a.clone()
        } else {
            let c = counters.entry(s.kind.as_str().to_string()).or_insert(0);
            *c += 1;
            let a = format!("\u{27E6}{}_{:02}\u{27E7}", s.kind.label(), *c);
            map.insert(key, a.clone());
            a
        };
        out.push(Span {
            start: s.start, end: s.end, kind: s.kind.clone(),
            raw: s.raw.clone(), alias,
        });
    }
    out
}

/// Produce an aliased version of `text` using the spans already computed by `detect`.
pub fn aliasize(text: &str, spans: &[Span]) -> String {
    let mut result = String::with_capacity(text.len());
    let mut cur = 0usize;
    for s in spans {
        if s.start > cur { result.push_str(&text[cur..s.start]); }
        result.push_str(&s.alias);
        cur = s.end;
    }
    if cur < text.len() { result.push_str(&text[cur..]); }
    result
}

/// A reverse map from alias -> raw using the already-seen spans (original case).
pub fn build_reverse_from_spans(spans: &[Span]) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for s in spans {
        m.insert(s.alias.clone(), s.raw.clone());
    }
    m
}

/// Re-hydrate aliased tokens using a span-derived reverse map (preserves original case).
/// Aliases use `\u{27E6}..._NN\u{27E7}` (mathematical double brackets) so the LLM
/// doesn't interpret them as Handlebars/Mustache templating.
pub fn rehydrate_stream_with(buf: &mut String, chunk: &str, reverse: &HashMap<String, String>) -> String {
    buf.push_str(chunk);
    // If we find an opening bracket with no closing bracket yet, hold back the
    // tail of the buffer until the closing bracket arrives — so a split like
    // "see \u{27E6}email" / "_01\u{27E7}" reassembles before rehydration.
    let safe_end = match buf.rfind('\u{27E6}') {
        Some(pos) => {
            let after = &buf[pos..];
            if after.contains('\u{27E7}') { buf.len() } else { pos }
        }
        None => buf.len(),
    };
    let emitable: String = buf[..safe_end].to_string();
    let remainder: String = buf[safe_end..].to_string();
    *buf = remainder;

    let mut out = emitable;
    for (alias, raw) in reverse.iter() {
        out = out.replace(alias.as_str(), raw);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_email_and_money() {
        let mut m = AliasMap::new(); let mut c = HashMap::new();
        let spans = detect("email alice@acme.com for $42,500,000", &mut m, &mut c);
        assert_eq!(spans.len(), 2);
        assert!(matches!(spans[0].kind, Kind::EMAIL));
    }

    #[test]
    fn ssn_is_critical() {
        let mut m = AliasMap::new(); let mut c = HashMap::new();
        let spans = detect("ssn 123-45-6789 is sensitive", &mut m, &mut c);
        assert_eq!(spans.len(), 1);
        assert!(is_critical(&spans[0].kind));
    }

    #[test]
    fn aliasize_replaces_spans() {
        let mut m = AliasMap::new(); let mut c = HashMap::new();
        let text = "cc Sarah Chen at sarah.chen@halcyonlabs.com";
        let spans = detect(text, &mut m, &mut c);
        let out = aliasize(text, &spans);
        assert!(out.contains("\u{27E6}person_01\u{27E7}"));
        assert!(out.contains("\u{27E6}email_01\u{27E7}"));
        assert!(!out.contains("Sarah Chen"));
    }

    #[test]
    fn rehydrate_round_trips() {
        let mut m = AliasMap::new(); let mut c = HashMap::new();
        let text = "Email alice@acme.com";
        let spans = detect(text, &mut m, &mut c);
        let aliased = aliasize(text, &spans);
        let reverse = build_reverse_from_spans(&spans);
        let mut buf = String::new();
        let out = rehydrate_stream_with(&mut buf, &aliased, &reverse);
        assert_eq!(out, "Email alice@acme.com");
    }

    #[test]
    fn partial_alias_buffers_across_chunks() {
        let mut m = AliasMap::new(); let mut c = HashMap::new();
        let text = "Email alice@acme.com";
        let spans = detect(text, &mut m, &mut c);
        let reverse = build_reverse_from_spans(&spans);
        let mut buf = String::new();
        let p1 = rehydrate_stream_with(&mut buf, "hello \u{27E6}em", &reverse);
        assert_eq!(p1, "hello ");
        let p2 = rehydrate_stream_with(&mut buf, "ail_01\u{27E7}!", &reverse);
        assert_eq!(p2, "alice@acme.com!");
    }

    #[test]
    fn ner_kinds_have_stable_labels() {
        assert_eq!(Kind::PERSON_NER.as_str(), "PERSON_NER");
        assert_eq!(Kind::ORG_NER.as_str(), "ORG_NER");
        assert_eq!(Kind::CODENAME_NER.as_str(), "CODENAME_NER");
        assert_eq!(Kind::LOCATION_NER.as_str(), "LOCATION_NER");
        assert_eq!(Kind::EMPID_NER.as_str(), "EMPID_NER");
        assert_eq!(Kind::PERSON_NER.label(), "person");
        assert_eq!(Kind::CODENAME_NER.label(), "codename");
    }

    #[test]
    fn apply_alias_map_mints_consistent_aliases() {
        let mut m = AliasMap::new();
        let mut c = HashMap::new();
        let s1 = Span { start: 0, end: 5, kind: Kind::EMAIL, raw: "a@b.c".into(), alias: String::new() };
        let s2 = Span { start: 10, end: 15, kind: Kind::EMAIL, raw: "a@b.c".into(), alias: String::new() };
        let aliased = apply_alias_map(&[s1, s2], &mut m, &mut c);
        assert_eq!(aliased[0].alias, "\u{27E6}email_01\u{27E7}");
        assert_eq!(aliased[1].alias, "\u{27E6}email_01\u{27E7}");  // same raw -> same alias
    }

    #[test]
    fn apply_alias_map_respects_ner_kinds() {
        let mut m = AliasMap::new();
        let mut c = HashMap::new();
        let sp = Span { start: 0, end: 10, kind: Kind::PERSON_NER, raw: "Jamie".into(), alias: String::new() };
        let aliased = apply_alias_map(&[sp], &mut m, &mut c);
        assert_eq!(aliased[0].alias, "\u{27E6}person_01\u{27E7}");
    }
}
