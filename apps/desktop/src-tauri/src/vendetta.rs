use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Kind {
    EMAIL, PHONE, SSN, IP, APIKEY, URL, ADDRESS, MONEY, NAME, COMPANY, EMPID,
    // Payment / banking
    CREDITCARD, IBAN, US_BANK, SWIFT_BIC, EIN,
    // Secrets
    JWT, PRIVATE_KEY, CONNECTION_STRING,
    // Identity documents
    DOB, PASSPORT, DRIVERS_LICENSE,
    // National / government identifiers (region packs)
    US_ITIN, CA_SIN, UK_NHS, UK_NINO, AU_TFN, AADHAAR,
    // Medical
    MRN, NPI, DEA, HEALTH_ID,
    // Legal
    CASE_NO,
    // Crypto / network
    CRYPTO_WALLET, IPV6, MAC_ADDRESS,
    // User-defined watchlist terms (see detect::custom). Never blocks.
    CUSTOM,
    PERSON_NER, ORG_NER, CODENAME_NER, LOCATION_NER, EMPID_NER,
}

impl Kind {
    pub fn label(&self) -> &'static str {
        match self {
            Kind::EMAIL => "email", Kind::PHONE => "phone", Kind::SSN => "ssn",
            Kind::IP => "ip", Kind::APIKEY => "api-key", Kind::URL => "url",
            Kind::ADDRESS => "address", Kind::MONEY => "amount",
            Kind::NAME => "person", Kind::COMPANY => "entity", Kind::EMPID => "employee-id",
            Kind::CREDITCARD => "card", Kind::IBAN => "iban", Kind::US_BANK => "bank",
            Kind::SWIFT_BIC => "swift", Kind::EIN => "ein",
            Kind::JWT => "jwt", Kind::PRIVATE_KEY => "private-key", Kind::CONNECTION_STRING => "conn-string",
            Kind::DOB => "dob", Kind::PASSPORT => "passport", Kind::DRIVERS_LICENSE => "license",
            Kind::US_ITIN => "itin", Kind::CA_SIN => "sin", Kind::UK_NHS => "nhs",
            Kind::UK_NINO => "nino", Kind::AU_TFN => "tfn", Kind::AADHAAR => "aadhaar",
            Kind::MRN => "mrn", Kind::NPI => "npi", Kind::DEA => "dea", Kind::HEALTH_ID => "member-id",
            Kind::CASE_NO => "case",
            Kind::CRYPTO_WALLET => "wallet", Kind::IPV6 => "ipv6", Kind::MAC_ADDRESS => "mac",
            Kind::CUSTOM => "custom",
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
            Kind::CREDITCARD => "CREDITCARD", Kind::IBAN => "IBAN", Kind::US_BANK => "US_BANK",
            Kind::SWIFT_BIC => "SWIFT_BIC", Kind::EIN => "EIN",
            Kind::JWT => "JWT", Kind::PRIVATE_KEY => "PRIVATE_KEY", Kind::CONNECTION_STRING => "CONNECTION_STRING",
            Kind::DOB => "DOB", Kind::PASSPORT => "PASSPORT", Kind::DRIVERS_LICENSE => "DRIVERS_LICENSE",
            Kind::US_ITIN => "US_ITIN", Kind::CA_SIN => "CA_SIN", Kind::UK_NHS => "UK_NHS",
            Kind::UK_NINO => "UK_NINO", Kind::AU_TFN => "AU_TFN", Kind::AADHAAR => "AADHAAR",
            Kind::MRN => "MRN", Kind::NPI => "NPI", Kind::DEA => "DEA", Kind::HEALTH_ID => "HEALTH_ID",
            Kind::CASE_NO => "CASE_NO",
            Kind::CRYPTO_WALLET => "CRYPTO_WALLET", Kind::IPV6 => "IPV6", Kind::MAC_ADDRESS => "MAC_ADDRESS",
            Kind::CUSTOM => "CUSTOM",
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
    /// Detection confidence in [0,1]. Deterministic + checksum-validated hits
    /// are 1.0; anchored heuristics lower; NER carries its model score. Older
    /// stored spans (pre-confidence) default to 1.0 so they keep deserializing.
    #[serde(default = "default_confidence")]
    pub confidence: f32,
}

fn default_confidence() -> f32 { 1.0 }

/// Detection-pack taxonomy. `core` and `secrets` are the safety floor and are
/// never disableable; the other packs can be switched off in Settings (the
/// `disabled_packs` setting holds a JSON array of these ids). NER, paranoid,
/// and custom-watchlist spans don't belong to a pack and are always active.
pub fn pack_for(kind: &Kind) -> &'static str {
    match kind {
        Kind::CREDITCARD | Kind::IBAN | Kind::US_BANK | Kind::SWIFT_BIC | Kind::EIN => "payment",
        Kind::DOB | Kind::PASSPORT | Kind::DRIVERS_LICENSE => "identity",
        Kind::US_ITIN | Kind::CA_SIN | Kind::UK_NHS | Kind::UK_NINO | Kind::AU_TFN
        | Kind::AADHAAR => "national-id",
        Kind::MRN | Kind::NPI | Kind::DEA | Kind::HEALTH_ID => "medical",
        Kind::CASE_NO => "legal",
        Kind::CRYPTO_WALLET | Kind::IPV6 | Kind::MAC_ADDRESS | Kind::IP => "network",
        Kind::APIKEY | Kind::JWT | Kind::PRIVATE_KEY | Kind::CONNECTION_STRING => "secrets",
        _ => "core",
    }
}

/// Packs a user may switch off. Deliberately excludes `core` and `secrets`:
/// emails/SSNs/API keys/private keys are the floor of the product promise.
pub const TOGGLEABLE_PACKS: &[&str] =
    &["payment", "identity", "national-id", "medical", "legal", "network"];

/// Baseline confidence for a regex-detected kind. A span only reaches this
/// point if it passed its validator, so the score reflects how *specific* the
/// match is, not whether it's valid. Checksum/structural-distinct classes are
/// certain; anchored-but-loose and pure-heuristic classes are graded down so
/// the Dev Inspector (and any future threshold) can tell them apart. NER and
/// LLM spans set their own score at construction and don't use this.
pub fn confidence_for(kind: &Kind) -> f32 {
    match kind {
        // Checksum-validated or cryptographically distinctive → certain.
        Kind::CREDITCARD | Kind::IBAN | Kind::US_BANK | Kind::SWIFT_BIC
        | Kind::NPI | Kind::DEA | Kind::CA_SIN | Kind::UK_NHS | Kind::AU_TFN
        | Kind::AADHAAR | Kind::SSN | Kind::IP | Kind::IPV6 | Kind::CRYPTO_WALLET
        | Kind::PRIVATE_KEY | Kind::CONNECTION_STRING | Kind::CUSTOM => 1.0,
        // Highly distinctive structural format, no checksum.
        Kind::EMAIL | Kind::URL | Kind::APIKEY | Kind::JWT | Kind::MAC_ADDRESS
        | Kind::EIN | Kind::US_ITIN => 0.95,
        // Context-anchored with a weak value check (presence of a digit, a
        // plausible date, structural rules).
        Kind::DOB | Kind::PASSPORT | Kind::DRIVERS_LICENSE | Kind::MRN
        | Kind::HEALTH_ID | Kind::CASE_NO | Kind::UK_NINO => 0.85,
        // Unanchored heuristics with real false-positive surface.
        Kind::PHONE | Kind::MONEY | Kind::ADDRESS | Kind::EMPID => 0.75,
        // NER/LLM kinds set their own at construction; this is only a fallback.
        _ => 0.8,
    }
}

/// `cap == 0` aliases the whole match. `cap == 1` aliases capture group 1 —
/// used for context-anchored patterns ("MRN: 00482931" matches the anchor +
/// value but only the value is sensitive). Rust regex has no lookbehind, so
/// anchor-then-capture is the precision tool for classes whose bare values
/// are too generic to detect safely (DOB, passport, member IDs, case numbers).
struct Pattern { kind: Kind, re: Regex, cap: usize }

fn p(kind: Kind, re: &str) -> Pattern {
    Pattern { kind, re: Regex::new(re).unwrap(), cap: 0 }
}
fn pc(kind: Kind, re: &str) -> Pattern {
    Pattern { kind, re: Regex::new(re).unwrap(), cap: 1 }
}

/// Pattern order is SECURITY-LOAD-BEARING. The overlap sort is stable on
/// (start, length) ties, so insertion order decides which kind wins when two
/// patterns claim the same span. Blocking kinds come first: "account no:
/// 4111111111111111" matches both the anchored US_BANK capture and
/// CREDITCARD at identical offsets — the tie MUST resolve to the blocking
/// kind or a Luhn-valid card number would be aliased instead of blocked.
/// Same for NPI-vs-PHONE on bare 10-digit values: the anchored pack entry
/// precedes the legacy generic so the more specific kind wins.
static PATTERNS: Lazy<Vec<Pattern>> = Lazy::new(|| vec![
    // ---- 1. Blocking, high-specificity ------------------------------------
    p(Kind::PRIVATE_KEY, r"-----BEGIN (?:[A-Z][A-Z ]{0,24})?PRIVATE KEY-----(?:(?s).{0,4096}?-----END (?:[A-Z][A-Z ]{0,24})?PRIVATE KEY-----)?"),
    p(Kind::CREDITCARD, r"\b\d(?:[ \-]?\d){12,18}\b"),
    p(Kind::IBAN,       r"\b[A-Z]{2}\d{2}(?:[ ]?[A-Z0-9]){11,30}\b"),
    // Database/broker connection strings with embedded credentials — the
    // password travels in the URI, so this is as sensitive as an API key.
    p(Kind::CONNECTION_STRING, r"(?i)\b(?:postgres(?:ql)?|mysql|mongodb(?:\+srv)?|redis|rediss|amqps?)://[^\s:/@]+:[^\s/@]+@[^\s/]+"),
    // ---- 2. Anchored packs (alias) -----------------------------------------
    pc(Kind::US_BANK, r"(?i)\b(?:aba|routing|rtn)(?:\s*(?:no|number|#))?\.?[:\s]+(\d{9})\b"),
    pc(Kind::US_BANK, r"(?i)\b(?:account|acct)(?:\s*(?:no|number|#))?\.?[:\s]+(\d{6,17})\b"),
    pc(Kind::SWIFT_BIC, r"\b(?i:swift|bic)(?i:\s*(?:code|no|number|#))?\.?[:\s]+([A-Z]{6}[A-Z0-9]{2}(?:[A-Z0-9]{3})?)\b"),
    pc(Kind::EIN, r"(?i)\b(?:ein|employer identification number|tax id)(?:\s*(?:no|number|#))?\.?[:\s]+(\d{2}-\d{7})\b"),
    pc(Kind::DOB, r"(?i)\b(?:dob|date of birth|birth\s?date|born(?:\s+on)?)\.?[:\s]+(\d{1,2}[/\-\.]\d{1,2}[/\-\.](?:\d{4}|\d{2})|\d{4}-\d{2}-\d{2}|(?:jan(?:uary)?|feb(?:ruary)?|mar(?:ch)?|apr(?:il)?|may|jun(?:e)?|jul(?:y)?|aug(?:ust)?|sep(?:t(?:ember)?)?|oct(?:ober)?|nov(?:ember)?|dec(?:ember)?)\.?\s+\d{1,2},?\s+\d{4})\b"),
    pc(Kind::PASSPORT, r"(?i)\bpassport(?:\s*(?:no|number|#))?\.?[:\s]+([A-Z0-9]{6,9})\b"),
    pc(Kind::DRIVERS_LICENSE, r"(?i)\b(?:driver'?s?\s+licen[cs]e|dl)(?:\s*(?:no|number|#))?\.?[:\s]+([A-Z0-9\-]{4,13})\b"),
    // National / government identifiers. All context-anchored (bare digit
    // runs are ambiguous across regions) and checksum-validated where the
    // scheme defines one. NHS values are 3-3-4 like US phone numbers — the
    // anchored entry must precede PHONE so the tie resolves to NHS.
    pc(Kind::US_ITIN, r"(?i)\bitin(?:\s*(?:no|number|#))?\.?[:\s]+(9\d{2}-(?:7\d|8[0-8]|9[0-24-9])-\d{4})\b"),
    pc(Kind::CA_SIN, r"(?i)\b(?:sin|social insurance)(?:\s*(?:no|number|#))?\.?[:\s]+(\d{3}[ \-]?\d{3}[ \-]?\d{3})\b"),
    pc(Kind::UK_NHS, r"(?i)\bnhs(?:\s*(?:no|number|#))?\.?[:\s]+(\d{3}[ \-]?\d{3}[ \-]?\d{4})\b"),
    pc(Kind::UK_NINO, r"(?i)\b(?:national insurance|nino)(?:\s*(?:no|number|#))?\.?[:\s]+([A-Za-z]{2}\d{6}[A-Da-d])\b"),
    pc(Kind::AU_TFN, r"(?i)\b(?:tfn|tax file number)(?:\s*(?:no|number|#))?\.?[:\s]+(\d{3}[ \-]?\d{3}[ \-]?\d{3})\b"),
    pc(Kind::AADHAAR, r"(?i)\baadh?aar(?:\s*(?:no|number|#))?\.?[:\s]+(\d{4}[ \-]?\d{4}[ \-]?\d{4})\b"),
    pc(Kind::MRN, r"(?i)\b(?:mrn|medical record)(?:\s*(?:no|number|#))?\.?[:\s]+([A-Z0-9\-]{5,12})\b"),
    pc(Kind::NPI, r"(?i)\bnpi(?:\s*(?:no|number|#))?\.?[:\s]+(\d{10})\b"),
    pc(Kind::DEA, r"(?i)\bdea(?:\s*(?:no|number|reg(?:istration)?|#))?\.?[:\s]+([A-Za-z]{2}\d{7})\b"),
    pc(Kind::HEALTH_ID, r"(?i)\b(?:member|subscriber|insurance|policy)\s*(?:id)?\s*(?:no|number|#)?\.?[:\s]+([A-Z0-9\-]{6,15})\b"),
    pc(Kind::CASE_NO, r"(?i)\b(?:case|docket|matter)\s*(?:no|number|#)?\.?[:\s]+([A-Z0-9][A-Z0-9:\.\-]{3,19})\b"),
    p(Kind::CASE_NO, r"\b\d{1,2}:\d{2}-(?:cv|cr|cm|md|mc|mj|po|sw)-\d{2,6}(?:-[A-Z]{2,4})?\b"),
    p(Kind::CRYPTO_WALLET, r"\b0x[a-fA-F0-9]{40}\b|\bbc1[ac-hj-np-z02-9]{25,62}\b|\b[13][a-km-zA-HJ-NP-Z1-9]{25,34}\b"),
    p(Kind::JWT, r"\beyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}"),
    // MAC before IPV6: a colon-separated MAC also matches the IPv6 candidate
    // shape (the IPv6 validator rejects it, but ordering keeps intent clear).
    p(Kind::MAC_ADDRESS, r"\b[0-9A-Fa-f]{2}(?:[:\-][0-9A-Fa-f]{2}){5}\b"),
    p(Kind::IPV6, r"\b[A-Fa-f0-9]{1,4}(?::[A-Fa-f0-9]{0,4}){2,7}\b"),
    // ---- 3. Legacy generics -------------------------------------------------
    p(Kind::EMAIL,   r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b"),
    p(Kind::PHONE,   r"\b(?:\+?\d{1,3}[\s.\-]?)?(?:\(?\d{3}\)?[\s.\-]?)\d{3}[\s.\-]?\d{4}\b"),
    p(Kind::SSN,     r"\b\d{3}-\d{2}-\d{4}\b"),
    p(Kind::IP,      r"\b(?:\d{1,3}\.){3}\d{1,3}\b"),
    p(Kind::APIKEY,  r"\b(?:sk-|sk_live_|sk_test_|pk_|rk_live_|AKIA|ASIA|ghp_|gho_|ghu_|ghs_|ghr_|github_pat_|glpat-|xox[baprs]-|xapp-)[A-Za-z0-9_\-]{10,}\b|\bAIza[0-9A-Za-z_\-]{35}\b|\bya29\.[0-9A-Za-z_\-\.]{20,}\b"),
    p(Kind::URL,     r"\bhttps?://[^\s)]+"),
    p(Kind::ADDRESS, r"\b\d{1,5}\s+[A-Z][a-zA-Z]+(?:\s+[A-Z][a-zA-Z]+){0,3}\s+(?:Street|St|Avenue|Ave|Road|Rd|Blvd|Lane|Ln|Drive|Dr|Court|Ct|Way)\b"),
    p(Kind::MONEY,   r"\$\s?\d{1,3}(?:,\d{3})+(?:\.\d+)?|\$\s?\d{4,}(?:\.\d+)?"),
    p(Kind::EMPID,   r"\bEMP-\d{4,6}\b"),
    // NAME/COMPANY regex entries removed for launch: they were hardcoded demo
    // lists ("Sarah Chen", "Project Helios"). Arbitrary names/orgs/codenames
    // are the NER + paranoid layers' job. The enum variants stay — stored
    // spans from older databases must keep deserializing.
]);

pub fn is_critical(k: &Kind) -> bool {
    matches!(
        k,
        Kind::SSN | Kind::APIKEY | Kind::CREDITCARD | Kind::IBAN | Kind::PRIVATE_KEY
            | Kind::CONNECTION_STRING
    )
}

/// Per-kind egress-block copy. The single source of truth for what the
/// PolicyViolation screen says — `commands::send` builds its `BlockReason`
/// from this, and the frontend `CRITICAL` map in lib/vendetta.ts mirrors it
/// verbatim for the instant client-side pre-flash.
pub struct BlockPolicy {
    pub rule: &'static str,
    pub class: &'static str,
    pub desc: &'static str,
}

pub fn block_policy(kind: &Kind) -> Option<BlockPolicy> {
    match kind {
        Kind::SSN => Some(BlockPolicy {
            rule: "Social Security number in outbound payload",
            class: "CRITICAL_IDENTITY",
            desc: "Sentynyx never sends SSNs to a third-party model endpoint. Remove it, or switch to a local model — local sends never leave this machine.",
        }),
        Kind::APIKEY => Some(BlockPolicy {
            rule: "Live API credential in outbound payload",
            class: "CRITICAL_SECRET",
            desc: "Exposed API keys grant immediate access to provider accounts and billing. Rotate the key at its issuer and keep secrets out of prompts — or switch to a local model.",
        }),
        Kind::CREDITCARD => Some(BlockPolicy {
            rule: "Payment card number in outbound payload",
            class: "CRITICAL_PAYMENT",
            desc: "This is a checksum-valid card number. Card data must never reach a third-party model endpoint. Remove it, or switch to a local model.",
        }),
        Kind::IBAN => Some(BlockPolicy {
            rule: "Bank account (IBAN) in outbound payload",
            class: "CRITICAL_PAYMENT",
            desc: "International bank account numbers are blocked from egress. Remove it, or switch to a local model.",
        }),
        Kind::PRIVATE_KEY => Some(BlockPolicy {
            rule: "Private key material in outbound payload",
            class: "CRITICAL_SECRET",
            desc: "Private keys must never leave this machine. Treat this key as compromised and rotate it before continuing.",
        }),
        Kind::CONNECTION_STRING => Some(BlockPolicy {
            rule: "Database credentials in outbound payload",
            class: "CRITICAL_SECRET",
            desc: "This connection string carries a live password in the URI. Rotate the credential and keep connection strings out of prompts — or switch to a local model.",
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Post-regex validators. A regex narrows the haystack; the validator decides.
// Called at hit-collection time — before overlap resolution — so an invalid
// hit can never shadow an overlapping valid one. Every checksum here is
// mirrored in lib/vendetta.ts (the live-highlight preview must agree with the
// engine or users see phantom protection / phantom blocks).
// ---------------------------------------------------------------------------

pub fn validate(kind: &Kind, raw: &str) -> bool {
    match kind {
        Kind::CREDITCARD => validators::credit_card(raw),
        Kind::IBAN => validators::iban(raw),
        Kind::US_BANK => validators::us_bank(raw),
        Kind::SWIFT_BIC => validators::swift_country(raw),
        Kind::NPI => validators::npi(raw),
        Kind::DEA => validators::dea(raw),
        Kind::CA_SIN => validators::ca_sin(raw),
        Kind::UK_NHS => validators::uk_nhs(raw),
        Kind::UK_NINO => validators::uk_nino(raw),
        Kind::AU_TFN => validators::au_tfn(raw),
        Kind::AADHAAR => validators::aadhaar(raw),
        Kind::DOB => validators::date_plausible(raw),
        Kind::IP => validators::ipv4_octets(raw),
        Kind::IPV6 => raw.parse::<std::net::Ipv6Addr>().is_ok(),
        Kind::CRYPTO_WALLET => validators::crypto_wallet(raw),
        Kind::PASSPORT | Kind::DRIVERS_LICENSE | Kind::MRN | Kind::HEALTH_ID | Kind::CASE_NO => {
            validators::has_digit(raw)
        }
        _ => true,
    }
}

mod validators {
    /// Luhn checksum over the digits of `s` (non-digits ignored by callers).
    pub fn luhn(digits: &[u32]) -> bool {
        if digits.is_empty() { return false; }
        let sum = digits.iter().rev().enumerate().fold(0u32, |acc, (i, &d)| {
            let v = if i % 2 == 1 { let x = d * 2; if x > 9 { x - 9 } else { x } } else { d };
            acc + v
        });
        sum % 10 == 0
    }

    fn digits_of(s: &str) -> Vec<u32> {
        s.chars().filter_map(|c| c.to_digit(10)).collect()
    }

    pub fn credit_card(raw: &str) -> bool {
        let d = digits_of(raw);
        let len = d.len();
        if !(13..=19).contains(&len) { return false; }
        // Issuer (IIN) prefix gate WITH per-brand length rules: Visa,
        // Mastercard (incl. 2-series), Amex, Discover, JCB, Diners. Both
        // checks matter — an 18-digit run starting "37…" can pass Luhn by
        // chance, but a real Amex PAN is exactly 15 digits.
        let n2 = d[0] * 10 + d[1];
        let n3 = n2 * 10 + d[2];
        let n4 = n3 * 10 + d[3];
        let brand_ok = (d[0] == 4 && matches!(len, 13 | 16 | 19))            // Visa
            || ((51..=55).contains(&n2) && len == 16)                        // Mastercard
            || ((2221..=2720).contains(&n4) && len == 16)                    // Mastercard 2-series
            || ((n2 == 34 || n2 == 37) && len == 15)                         // Amex
            || ((n4 == 6011 || n2 == 65 || (644..=649).contains(&n3))
                && (16..=19).contains(&len))                                  // Discover
            || (n2 == 35 && (16..=19).contains(&len))                        // JCB
            || ((n2 == 36 || n2 == 38) && (14..=19).contains(&len));         // Diners
        brand_ok && luhn(&d)
    }

    /// IBAN registry country prefixes (SEPA + common). Not exhaustive of every
    /// experimental code, but covers the registry; unknown prefixes fail closed.
    const IBAN_COUNTRIES: &[&str] = &[
        "AD","AE","AL","AT","AZ","BA","BE","BG","BH","BI","BR","BY","CH","CR","CY","CZ",
        "DE","DJ","DK","DO","EE","EG","ES","FI","FK","FO","FR","GB","GE","GI","GL","GR",
        "GT","HR","HU","IE","IL","IQ","IS","IT","JO","KW","KZ","LB","LC","LI","LT","LU",
        "LV","LY","MC","MD","ME","MK","MN","MR","MT","MU","NI","NL","NO","OM","PK","PL",
        "PS","PT","QA","RO","RS","RU","SA","SC","SD","SE","SI","SK","SM","SO","ST","SV",
        "TL","TN","TR","UA","VA","VG","XK",
    ];

    pub fn iban(raw: &str) -> bool {
        let s: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
        if s.len() < 15 || s.len() > 34 { return false; }
        let cc = &s[..2];
        if !IBAN_COUNTRIES.contains(&cc) { return false; }
        // Rotate first 4 chars to the end, map A=10..Z=35, streaming mod 97.
        let rotated = format!("{}{}", &s[4..], &s[..4]);
        let mut acc: u32 = 0;
        for ch in rotated.chars() {
            let v = match ch {
                '0'..='9' => ch as u32 - '0' as u32,
                'A'..='Z' => ch as u32 - 'A' as u32 + 10,
                _ => return false,
            };
            acc = if v < 10 { (acc * 10 + v) % 97 } else { (acc * 100 + v) % 97 };
        }
        acc == 1
    }

    /// US_BANK covers two anchored shapes: 9-digit ABA routing numbers
    /// (checksum + prefix gate) and 6–17 digit account numbers (anchor-only).
    pub fn us_bank(raw: &str) -> bool {
        let d = digits_of(raw);
        if d.len() != 9 { return d.len() >= 6 && d.len() <= 17; }
        let prefix = d[0] * 10 + d[1];
        let prefix_ok = prefix <= 12
            || (21..=32).contains(&prefix)
            || (61..=72).contains(&prefix)
            || prefix == 80;
        let checksum = 3 * (d[0] + d[3] + d[6]) + 7 * (d[1] + d[4] + d[7]) + (d[2] + d[5] + d[8]);
        prefix_ok && checksum % 10 == 0
    }

    /// ISO-3166-ish country gate for SWIFT/BIC chars 5–6. IBAN registry plus
    /// major non-IBAN financial centers.
    pub fn swift_country(raw: &str) -> bool {
        if raw.len() < 6 { return false; }
        let cc = raw[4..6].to_ascii_uppercase();
        const EXTRA: &[&str] = &[
            "US","CA","AU","NZ","JP","CN","HK","SG","IN","KR","TW","TH","MY","ID","PH",
            "VN","ZA","NG","KE","GH","MA","MX","AR","CL","CO","PE","UY","PA","EC","BO",
            "PY","VE","AM","UZ","TJ","KG","NP","BD","LK","MM","KH","LA","BN","MO","FJ",
        ];
        IBAN_COUNTRIES.contains(&cc.as_str()) || EXTRA.contains(&cc.as_str())
    }

    /// NPI check digit: Luhn over "80840" + the 10 digits.
    pub fn npi(raw: &str) -> bool {
        let d = digits_of(raw);
        if d.len() != 10 { return false; }
        let mut full: Vec<u32> = vec![8, 0, 8, 4, 0];
        full.extend_from_slice(&d);
        luhn(&full)
    }

    /// DEA registration: (d1+d3+d5) + 2*(d2+d4+d6), units digit == d7;
    /// first letter is a registrant-type code.
    pub fn dea(raw: &str) -> bool {
        let chars: Vec<char> = raw.chars().collect();
        if chars.len() != 9 { return false; }
        let first = chars[0].to_ascii_uppercase();
        if !matches!(first, 'A' | 'B' | 'F' | 'G' | 'M' | 'P' | 'R' | 'X') { return false; }
        let d: Vec<u32> = chars[2..].iter().filter_map(|c| c.to_digit(10)).collect();
        if d.len() != 7 { return false; }
        let sum = (d[0] + d[2] + d[4]) + 2 * (d[1] + d[3] + d[5]);
        sum % 10 == d[6]
    }

    /// Canadian Social Insurance Number: 9 digits, Luhn.
    pub fn ca_sin(raw: &str) -> bool {
        let d = digits_of(raw);
        d.len() == 9 && luhn(&d)
    }

    /// UK NHS number: 10 digits; mod-11 with weights 10..2; check digit is
    /// 11 - (sum % 11), where 11 → 0 and 10 → invalid.
    pub fn uk_nhs(raw: &str) -> bool {
        let d = digits_of(raw);
        if d.len() != 10 { return false; }
        let sum: u32 = d[..9].iter().enumerate().map(|(i, &x)| x * (10 - i as u32)).sum();
        let check = match 11 - (sum % 11) {
            11 => 0,
            10 => return false,
            c => c,
        };
        check == d[9]
    }

    /// UK National Insurance number: two prefix letters (D/F/I/Q/U/V banned,
    /// second letter additionally not O, a few pairs administratively
    /// invalid), six digits, suffix A–D.
    pub fn uk_nino(raw: &str) -> bool {
        let s = raw.to_ascii_uppercase();
        let b = s.as_bytes();
        if b.len() != 9 { return false; }
        let banned = |c: u8| matches!(c, b'D' | b'F' | b'I' | b'Q' | b'U' | b'V');
        if banned(b[0]) || banned(b[1]) || b[1] == b'O' { return false; }
        if matches!(&s[..2], "BG" | "GB" | "NK" | "KN" | "TN" | "NT" | "ZZ") { return false; }
        true
    }

    /// Australian Tax File Number: 9 digits; weighted sum (1,4,3,7,5,8,6,9,10)
    /// divisible by 11.
    pub fn au_tfn(raw: &str) -> bool {
        let d = digits_of(raw);
        if d.len() != 9 { return false; }
        const W: [u32; 9] = [1, 4, 3, 7, 5, 8, 6, 9, 10];
        d.iter().zip(W).map(|(&x, w)| x * w).sum::<u32>() % 11 == 0
    }

    /// Indian Aadhaar: 12 digits, first 2–9, Verhoeff checksum.
    pub fn aadhaar(raw: &str) -> bool {
        let d = digits_of(raw);
        if d.len() != 12 || d[0] < 2 { return false; }
        verhoeff(&d)
    }

    fn verhoeff(digits: &[u32]) -> bool {
        const D: [[u32; 10]; 10] = [
            [0,1,2,3,4,5,6,7,8,9],[1,2,3,4,0,6,7,8,9,5],[2,3,4,0,1,7,8,9,5,6],
            [3,4,0,1,2,8,9,5,6,7],[4,0,1,2,3,9,5,6,7,8],[5,9,8,7,6,0,4,3,2,1],
            [6,5,9,8,7,1,0,4,3,2],[7,6,5,9,8,2,1,0,4,3],[8,7,6,5,9,3,2,1,0,4],
            [9,8,7,6,5,4,3,2,1,0],
        ];
        const P: [[u32; 10]; 8] = [
            [0,1,2,3,4,5,6,7,8,9],[1,5,7,6,2,8,3,0,9,4],[5,8,0,3,7,9,6,1,4,2],
            [8,9,1,6,0,4,3,5,2,7],[9,4,5,3,1,2,6,8,7,0],[4,2,8,6,5,7,3,9,0,1],
            [2,7,9,3,8,0,6,4,1,5],[7,0,4,6,9,1,3,2,5,8],
        ];
        let mut c: u32 = 0;
        for (i, &digit) in digits.iter().rev().enumerate() {
            c = D[c as usize][P[i % 8][digit as usize] as usize];
        }
        c == 0
    }

    /// Plausibility for anchored DOB values in any of the three matched
    /// shapes: numeric d/m/y (either order), ISO yyyy-mm-dd, "March 3, 1978".
    pub fn date_plausible(raw: &str) -> bool {
        let year_ok = |y: u32| (1900..=2100).contains(&y);
        // ISO yyyy-mm-dd
        if raw.len() == 10 && raw.as_bytes()[4] == b'-' {
            let parts: Vec<u32> = raw.split('-').filter_map(|x| x.parse().ok()).collect();
            return parts.len() == 3
                && year_ok(parts[0])
                && (1..=12).contains(&parts[1])
                && (1..=31).contains(&parts[2]);
        }
        // Numeric d/m/y or m/d/y with -, /, or .
        let numeric: Vec<u32> = raw.split(['/', '-', '.']).filter_map(|x| x.trim().parse().ok()).collect();
        if numeric.len() == 3 {
            let (a, b, y) = (numeric[0], numeric[1], numeric[2]);
            let day_month_ok = (a >= 1 && a <= 12 && b >= 1 && b <= 31)
                || (b >= 1 && b <= 12 && a >= 1 && a <= 31);
            let y_ok = if y >= 100 { year_ok(y) } else { true }; // 2-digit years: accept
            return day_month_ok && y_ok;
        }
        // Month-name form: "March 3, 1978" — month validity is enforced by the
        // regex alternation; check day + year.
        let nums: Vec<u32> = raw
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        if nums.len() == 2 {
            return (1..=31).contains(&nums[0]) && year_ok(nums[1]);
        }
        false
    }

    pub fn ipv4_octets(raw: &str) -> bool {
        raw.split('.').filter_map(|o| o.parse::<u32>().ok()).filter(|&o| o <= 255).count() == 4
            && raw.split('.').count() == 4
    }

    /// ETH (0x + 40 hex) and bech32 (bc1…) pass on shape; base58 `[13]…`
    /// addresses must pass Base58Check (4-byte double-SHA256 checksum) —
    /// 2^-32 selectivity kills shortened-hash / ID false positives.
    pub fn crypto_wallet(raw: &str) -> bool {
        if raw.starts_with("0x") || raw.starts_with("bc1") { return true; }
        base58check(raw)
    }

    fn base58check(s: &str) -> bool {
        const ALPHABET: &str = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
        let mut bytes: Vec<u8> = vec![0];
        for ch in s.chars() {
            let Some(v) = ALPHABET.find(ch) else { return false; };
            let mut carry = v as u32;
            for b in bytes.iter_mut().rev() {
                carry += (*b as u32) * 58;
                *b = (carry & 0xff) as u8;
                carry >>= 8;
            }
            while carry > 0 {
                bytes.insert(0, (carry & 0xff) as u8);
                carry >>= 8;
            }
        }
        // Leading '1's encode leading zero bytes.
        let leading_ones = s.chars().take_while(|&c| c == '1').count();
        let mut payload: Vec<u8> = vec![0; leading_ones];
        let first_nonzero = bytes.iter().position(|&b| b != 0).unwrap_or(bytes.len());
        payload.extend_from_slice(&bytes[first_nonzero..]);
        if payload.len() < 5 { return false; }
        let (body, checksum) = payload.split_at(payload.len() - 4);
        use sha2::{Digest, Sha256};
        let h = Sha256::digest(Sha256::digest(body));
        h[..4] == *checksum
    }

    /// Anchor-only classes still need at least one digit in the captured
    /// value — with `(?i)` the [A-Z0-9] classes go case-insensitive, so plain
    /// words after the anchor ("passport details: attached") would otherwise
    /// slip through.
    pub fn has_digit(raw: &str) -> bool {
        raw.chars().any(|c| c.is_ascii_digit())
    }
}

/// An alias map keyed on `KIND::lowercased-raw` -> alias token.
pub type AliasMap = HashMap<String, String>;

fn alias_key(kind: &Kind, raw: &str) -> String {
    format!("{}::{}", kind.as_str(), raw.to_lowercase())
}

pub fn detect(text: &str, map: &mut AliasMap, counters: &mut HashMap<String, usize>) -> Vec<Span> {
    let mut hits: Vec<(usize, usize, Kind, String)> = Vec::new();
    for p in PATTERNS.iter() {
        if p.cap == 0 {
            for m in p.re.find_iter(text) {
                if !validate(&p.kind, m.as_str()) { continue; }
                hits.push((m.start(), m.end(), p.kind.clone(), m.as_str().to_string()));
            }
        } else {
            for c in p.re.captures_iter(text) {
                let Some(g) = c.get(p.cap) else { continue };
                if !validate(&p.kind, g.as_str()) { continue; }
                hits.push((g.start(), g.end(), p.kind.clone(), g.as_str().to_string()));
            }
        }
    }
    // Stable sort: position asc, longer-first. Ties resolve to PATTERNS
    // insertion order — see the ordering comment above PATTERNS.
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
        let confidence = confidence_for(&kind);
        out.push(Span { start: s, end: e, kind, raw, alias, confidence });
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
            raw: s.raw.clone(), alias, confidence: s.confidence,
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

    fn run(text: &str) -> Vec<Span> {
        let mut m = AliasMap::new();
        let mut c = HashMap::new();
        detect(text, &mut m, &mut c)
    }

    #[test]
    fn detects_email_and_money() {
        let spans = run("email alice@acme.com for $42,500,000");
        assert_eq!(spans.len(), 2);
        assert!(matches!(spans[0].kind, Kind::EMAIL));
    }

    #[test]
    fn ssn_is_critical() {
        let spans = run("ssn 123-45-6789 is sensitive");
        assert_eq!(spans.len(), 1);
        assert!(is_critical(&spans[0].kind));
    }

    #[test]
    fn aliasize_replaces_spans() {
        let mut m = AliasMap::new(); let mut c = HashMap::new();
        let text = "reach me at alice@acme.com or (415) 555-0142";
        let spans = detect(text, &mut m, &mut c);
        let out = aliasize(text, &spans);
        assert!(out.contains("\u{27E6}email_01\u{27E7}"));
        assert!(out.contains("\u{27E6}phone_01\u{27E7}"));
        assert!(!out.contains("alice@acme.com"));
    }

    #[test]
    fn demo_name_lists_are_gone() {
        // The hardcoded demo regex lists were removed for launch — arbitrary
        // names/orgs are the NER layer's job now.
        let spans = run("Sarah Chen is presenting Project Helios at Halcyon Labs");
        assert!(spans.is_empty());
    }

    #[test]
    fn legacy_name_spans_still_deserialize() {
        // Old DBs persisted spans with kinds whose regexes are gone. The enum
        // variants must keep deserializing forever.
        let s: Span = serde_json::from_str(
            r#"{"start":0,"end":10,"kind":"NAME","raw":"Sarah Chen","alias":"⟦person_01⟧"}"#
        ).unwrap();
        assert!(matches!(s.kind, Kind::NAME));
        let c: Span = serde_json::from_str(
            r#"{"start":0,"end":14,"kind":"COMPANY","raw":"Project Helios","alias":"⟦entity_01⟧"}"#
        ).unwrap();
        assert!(matches!(c.kind, Kind::COMPANY));
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
        let s1 = Span { start: 0, end: 5, kind: Kind::EMAIL, raw: "a@b.c".into(), alias: String::new(), confidence: 1.0 };
        let s2 = Span { start: 10, end: 15, kind: Kind::EMAIL, raw: "a@b.c".into(), alias: String::new(), confidence: 1.0 };
        let aliased = apply_alias_map(&[s1, s2], &mut m, &mut c);
        assert_eq!(aliased[0].alias, "\u{27E6}email_01\u{27E7}");
        assert_eq!(aliased[1].alias, "\u{27E6}email_01\u{27E7}");  // same raw -> same alias
    }

    #[test]
    fn apply_alias_map_respects_ner_kinds() {
        let mut m = AliasMap::new();
        let mut c = HashMap::new();
        let sp = Span { start: 0, end: 10, kind: Kind::PERSON_NER, raw: "Jamie".into(), alias: String::new(), confidence: 0.9 };
        let aliased = apply_alias_map(&[sp], &mut m, &mut c);
        assert_eq!(aliased[0].alias, "\u{27E6}person_01\u{27E7}");
    }

    // ---- Payment / banking --------------------------------------------------

    #[test]
    fn credit_card_luhn_valid_detected_and_blocks() {
        for text in [
            "Card on file: 4111 1111 1111 1111",
            "charge 5500-0000-0000-0004 instead",
            "AmEx 378282246310005 expires 09/27",
        ] {
            let spans = run(text);
            assert_eq!(spans.len(), 1, "{text}");
            assert!(matches!(spans[0].kind, Kind::CREDITCARD), "{text}");
            assert!(is_critical(&spans[0].kind));
        }
    }

    #[test]
    fn credit_card_luhn_fail_or_bad_iin_ignored() {
        assert!(run("order ref 4111 1111 1111 1112 shipped").is_empty()); // Luhn fail
        assert!(run("tracking 1234 5678 9012 3456").is_empty());          // bad IIN + Luhn fail
    }

    #[test]
    fn iban_mod97_valid_detected_and_blocks() {
        let spans = run("wire to DE89 3704 0044 0532 0130 00 today");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::IBAN));
        assert!(is_critical(&spans[0].kind));
        let spans = run("GB82WEST12345698765432 is the GBP account");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::IBAN));
    }

    #[test]
    fn iban_invalid_checksum_or_country_ignored() {
        assert!(run("DE89 3704 0044 0532 0130 01 ref").is_empty()); // mod-97 fail
        assert!(run("ref US12 3456 7890 1234 5678").is_empty());     // not an IBAN country
    }

    #[test]
    fn aba_routing_checksum() {
        let spans = run("routing number: 021000021");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::US_BANK));
        assert_eq!(spans[0].raw, "021000021");
        assert!(run("routing number: 123456789").is_empty()); // checksum fail
        assert!(run("the code 021000021 appears").is_empty()); // no anchor
    }

    #[test]
    fn blocking_kind_wins_span_tie() {
        // "account no: <PAN>" matches both the anchored US_BANK capture and
        // CREDITCARD at identical offsets. The blocking kind must win or a
        // valid card would be aliased instead of blocked.
        let spans = run("account no: 4111111111111111");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::CREDITCARD));
    }

    #[test]
    fn swift_bic_anchored_only() {
        let spans = run("SWIFT: DEUTDEFF");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::SWIFT_BIC));
        let spans = run("BIC code BOFAUS3NXXX");
        assert_eq!(spans.len(), 1);
        assert!(run("Swift is a programming language").is_empty());
    }

    #[test]
    fn ein_anchored() {
        let spans = run("their tax id 84-1234567 is on the W-9");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::EIN));
        assert!(run("part 12-3456789 of the doc").is_empty());
    }

    // ---- Secrets --------------------------------------------------------------

    #[test]
    fn expanded_api_key_formats() {
        for text in [
            "key github_pat_11ABCDEFG0123456789abcdefgh",
            "ci token glpat-AbCdEfGhIjKlMnOpQrSt",
            "google AIzaSyA1bC2dE3fG4hI5jK6lM7nO8pQ9rS0tU1v",
            // Deliberately scanner-safe: test prefix + a body shorter than the
            // 24 chars GitHub's secret patterns require. Our regex needs ≥10,
            // so the sk_test_ branch is still exercised.
            "stripe sk_test_4eC39HqLyjWDarjt",
        ] {
            let spans = run(text);
            assert_eq!(spans.len(), 1, "{text}");
            assert!(matches!(spans[0].kind, Kind::APIKEY), "{text}");
        }
        assert!(run("ghp_short").is_empty());
    }

    #[test]
    fn jwt_detected() {
        let spans = run("token eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJVadQssw5c");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::JWT));
        assert!(run("eyJust.kidding.here").is_empty());
        assert!(run("data.tar.gz attached").is_empty());
    }

    #[test]
    fn private_key_blocks() {
        let full = "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEA7\n-----END RSA PRIVATE KEY-----";
        let spans = run(full);
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::PRIVATE_KEY));
        assert!(is_critical(&spans[0].kind));
        assert_eq!(spans[0].raw, full); // full block captured, not just header
        let spans = run("-----BEGIN OPENSSH PRIVATE KEY-----");
        assert_eq!(spans.len(), 1);
        assert!(run("my private key is in 1Password").is_empty());
    }

    // ---- Identity --------------------------------------------------------------

    #[test]
    fn dob_anchored_and_plausible() {
        for text in ["DOB: 04/12/1985", "date of birth 1990-07-22", "born on March 3, 1978"] {
            let spans = run(text);
            assert_eq!(spans.len(), 1, "{text}");
            assert!(matches!(spans[0].kind, Kind::DOB), "{text}");
        }
        assert!(run("DOB: 13/45/9999").is_empty());      // implausible
        assert!(run("the 04/12/1985 build").is_empty()); // no anchor
    }

    #[test]
    fn passport_and_license_need_anchor_and_digit() {
        let spans = run("passport no: N1234567");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::PASSPORT));
        assert!(run("passport details: attached").is_empty());
        let spans = run("driver's license no: D123-4567-8901");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::DRIVERS_LICENSE));
        assert!(run("driver's license renewal form").is_empty());
    }

    // ---- Medical ----------------------------------------------------------------

    #[test]
    fn mrn_anchored() {
        let spans = run("MRN: 00482931 admitted Tuesday");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::MRN));
        assert!(run("MRN: pending").is_empty());
        assert!(run("id 00482931 in the export").is_empty());
    }

    #[test]
    fn npi_checksum() {
        let spans = run("NPI: 1234567893"); // CMS documented valid example
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::NPI));
        // Checksum-fail: must NOT classify as NPI. The bare 10-digit run is
        // still legitimately claimed by PHONE (any 10-digit number is) —
        // over-redaction in the safe direction, so we assert kind, not count.
        let spans = run("NPI: 1234567890");
        assert!(spans.iter().all(|s| !matches!(s.kind, Kind::NPI)));
    }

    #[test]
    fn dea_checksum() {
        let spans = run("DEA: BB1388568");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::DEA));
        assert!(run("DEA: BB1388569").is_empty()); // checksum fail
        assert!(run("the DEA investigation continues").is_empty());
    }

    #[test]
    fn health_member_id_anchored() {
        let spans = run("member ID: XR4-882-991");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::HEALTH_ID));
        assert!(run("member of the board").is_empty());
    }

    // ---- Legal -------------------------------------------------------------------

    #[test]
    fn case_numbers() {
        let spans = run("Case No. 4:22-cv-00321-KAW is scheduled");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::CASE_NO));
        let spans = run("see 1:21-cv-04567 for the ruling"); // unanchored federal shape
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::CASE_NO));
        assert!(run("in case you missed it").is_empty());
        assert!(run("matter: urgent").is_empty());
    }

    // ---- Crypto / network ----------------------------------------------------------

    #[test]
    fn crypto_wallets() {
        let spans = run("send to 0x742d35Cc6634C0532925a3b844Bc454e4438f44e");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::CRYPTO_WALLET));
        let spans = run("btc 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"); // genesis, base58check-valid
        assert_eq!(spans.len(), 1);
        let spans = run("bech32 bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq");
        assert_eq!(spans.len(), 1);
        assert!(run("btc 1A1zP1eP5QGefi2DMPTfTL5SLmv7Divfff").is_empty()); // checksum fail
        assert!(run("commit 3353c3e point release").is_empty());
    }

    #[test]
    fn ipv6_parses_real_addresses_only() {
        let spans = run("node at 2001:0db8:85a3:0000:0000:8a2e:0370:7334");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::IPV6));
        let spans = run("link-local fe80::1ff:fe23:4567:890a");
        assert_eq!(spans.len(), 1);
        assert!(run("standup at 10:30:45 sharp").is_empty()); // time
        // MACs are their own kind now — and must never classify as IPv6.
        let spans = run("MAC 00:1A:2B:3C:4D:5E");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::MAC_ADDRESS));
        let spans = run("nic at 00-1a-2b-3c-4d-5e");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::MAC_ADDRESS));
    }

    #[test]
    fn connection_strings_with_credentials_block() {
        for text in [
            "broke prod: postgres://admin:hunter2@db.internal:5432/main",
            "mongodb+srv://svc:S3cr3t@cluster0.example.net/app",
            "redis://default:p4ss@cache.example.com:6379",
        ] {
            let spans = run(text);
            assert_eq!(spans.len(), 1, "{text}");
            assert!(matches!(spans[0].kind, Kind::CONNECTION_STRING), "{text}");
            assert!(is_critical(&spans[0].kind));
        }
        // No credentials in the URI → not a secret; URL pattern may still alias.
        let spans = run("postgres://db.internal:5432/main is the host");
        assert!(spans.iter().all(|s| !matches!(s.kind, Kind::CONNECTION_STRING)));
    }

    #[test]
    fn national_ids_checksum_validated() {
        // CA SIN (Luhn)
        let spans = run("SIN: 046 454 286 on the application");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::CA_SIN));
        assert!(run("SIN: 046 454 287 noted").iter().all(|s| !matches!(s.kind, Kind::CA_SIN)));
        // UK NHS (mod-11) — value is phone-shaped; the anchored kind must win.
        let spans = run("NHS number: 943 476 5919");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::UK_NHS));
        assert!(run("NHS number: 943 476 5918").iter().all(|s| !matches!(s.kind, Kind::UK_NHS)));
        // UK NINO (structure rules)
        let spans = run("national insurance no: AB123456C");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::UK_NINO));
        // Banned prefix letter → not a NINO. (The "insurance no:" anchor
        // still aliases the value as HEALTH_ID — over-redaction, safe.)
        assert!(run("national insurance no: QQ123456C").iter().all(|s| !matches!(s.kind, Kind::UK_NINO)));
        // AU TFN (weighted mod-11)
        let spans = run("TFN: 123 456 782 for payroll");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::AU_TFN));
        assert!(run("TFN: 123 456 789 invalid").iter().all(|s| !matches!(s.kind, Kind::AU_TFN)));
        // Aadhaar (Verhoeff)
        let spans = run("aadhaar no: 2341 2341 2346");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::AADHAAR));
        assert!(run("aadhaar no: 2341 2341 2340").is_empty());
        // US ITIN (structure: 9xx with valid group)
        let spans = run("ITIN: 912-70-1234 filed");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::US_ITIN));
        // 93 is not a valid ITIN group — but the bare SSN shape still catches
        // it downstream, which is the safe direction.
        let spans = run("ITIN: 912-93-1234 filed");
        assert!(spans.iter().all(|s| !matches!(s.kind, Kind::US_ITIN)));
    }

    #[test]
    fn ipv4_octets_validated() {
        let spans = run("server at 10.0.0.255");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::IP));
        assert!(run("version 999.999.999.999 of nothing").is_empty());
    }

    // ---- Policy invariants -----------------------------------------------------------

    #[test]
    fn every_critical_kind_has_block_policy() {
        let all = [
            Kind::EMAIL, Kind::PHONE, Kind::SSN, Kind::IP, Kind::APIKEY, Kind::URL,
            Kind::ADDRESS, Kind::MONEY, Kind::NAME, Kind::COMPANY, Kind::EMPID,
            Kind::CREDITCARD, Kind::IBAN, Kind::US_BANK, Kind::SWIFT_BIC, Kind::EIN,
            Kind::JWT, Kind::PRIVATE_KEY, Kind::CONNECTION_STRING,
            Kind::DOB, Kind::PASSPORT, Kind::DRIVERS_LICENSE,
            Kind::US_ITIN, Kind::CA_SIN, Kind::UK_NHS, Kind::UK_NINO, Kind::AU_TFN, Kind::AADHAAR,
            Kind::MRN, Kind::NPI, Kind::DEA, Kind::HEALTH_ID, Kind::CASE_NO,
            Kind::CRYPTO_WALLET, Kind::IPV6, Kind::MAC_ADDRESS, Kind::CUSTOM,
            Kind::PERSON_NER, Kind::ORG_NER, Kind::CODENAME_NER, Kind::LOCATION_NER,
            Kind::EMPID_NER,
        ];
        for k in all {
            assert_eq!(
                is_critical(&k),
                block_policy(&k).is_some(),
                "is_critical and block_policy disagree for {:?}", k
            );
        }
    }

    #[test]
    fn custom_kind_never_blocks() {
        assert!(!is_critical(&Kind::CUSTOM));
    }

    #[test]
    fn pack_taxonomy_covers_safety_floor() {
        // The safety floor must never be toggleable.
        assert_eq!(pack_for(&Kind::SSN), "core");
        assert_eq!(pack_for(&Kind::EMAIL), "core");
        assert_eq!(pack_for(&Kind::APIKEY), "secrets");
        assert_eq!(pack_for(&Kind::PRIVATE_KEY), "secrets");
        assert_eq!(pack_for(&Kind::CONNECTION_STRING), "secrets");
        assert!(!TOGGLEABLE_PACKS.contains(&"core"));
        assert!(!TOGGLEABLE_PACKS.contains(&"secrets"));
        // Every toggleable id is produced by at least one kind.
        for id in TOGGLEABLE_PACKS {
            let covered = [
                Kind::CREDITCARD, Kind::DOB, Kind::UK_NHS, Kind::MRN,
                Kind::CASE_NO, Kind::MAC_ADDRESS,
            ].iter().any(|k| pack_for(k) == *id);
            assert!(covered, "no kind maps to pack {id}");
        }
    }

    #[test]
    fn confidence_grades_by_specificity() {
        // Checksum-validated → certain.
        let s = run("Card on file: 4111 1111 1111 1111");
        assert_eq!(s[0].confidence, 1.0);
        // Context-anchored weak check → mid.
        let s = run("MRN: 00482931 today");
        assert!((s[0].confidence - 0.85).abs() < 1e-6, "{}", s[0].confidence);
        // Unanchored heuristic → lower.
        let s = run("call 555-123-4567 please");
        assert!(s[0].confidence < 0.8 && s[0].confidence > 0.0);
        // Old stored spans without the field deserialize to 1.0.
        let legacy: Span = serde_json::from_str(
            r#"{"start":0,"end":5,"kind":"EMAIL","raw":"a@b.c","alias":"x"}"#
        ).unwrap();
        assert_eq!(legacy.confidence, 1.0);
    }
}
