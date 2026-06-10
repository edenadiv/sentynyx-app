use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// SCREAMING_SNAKE variant names are intentional: they serialize verbatim
// (serde uses the variant name) and `as_str()` returns them as the stable
// wire/audit identifiers — renaming to camel case would break every stored
// span and audit row.
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Kind {
    EMAIL, PHONE, SSN, IP, APIKEY, URL, ADDRESS, MONEY, NAME, COMPANY, EMPID,
    // Payment / banking
    CREDITCARD, IBAN, US_BANK, SWIFT_BIC, EIN,
    // Secrets
    JWT, PRIVATE_KEY, CONNECTION_STRING, CREDENTIAL,
    // Identity documents
    DOB, PASSPORT, DRIVERS_LICENSE, VIN, MRZ,
    // National / government identifiers (region packs)
    US_ITIN, CA_SIN, UK_NHS, UK_NINO, AU_TFN, AADHAAR,
    // Medical
    MRN, NPI, DEA, HEALTH_ID, MEDICARE_MBI,
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
            Kind::CREDENTIAL => "credential",
            Kind::DOB => "dob", Kind::PASSPORT => "passport", Kind::DRIVERS_LICENSE => "license",
            Kind::VIN => "vin", Kind::MRZ => "passport-mrz",
            Kind::US_ITIN => "itin", Kind::CA_SIN => "sin", Kind::UK_NHS => "nhs",
            Kind::UK_NINO => "nino", Kind::AU_TFN => "tfn", Kind::AADHAAR => "aadhaar",
            Kind::MRN => "mrn", Kind::NPI => "npi", Kind::DEA => "dea", Kind::HEALTH_ID => "member-id",
            Kind::MEDICARE_MBI => "medicare-mbi",
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
            Kind::CREDENTIAL => "CREDENTIAL",
            Kind::DOB => "DOB", Kind::PASSPORT => "PASSPORT", Kind::DRIVERS_LICENSE => "DRIVERS_LICENSE",
            Kind::VIN => "VIN", Kind::MRZ => "MRZ",
            Kind::US_ITIN => "US_ITIN", Kind::CA_SIN => "CA_SIN", Kind::UK_NHS => "UK_NHS",
            Kind::UK_NINO => "UK_NINO", Kind::AU_TFN => "AU_TFN", Kind::AADHAAR => "AADHAAR",
            Kind::MRN => "MRN", Kind::NPI => "NPI", Kind::DEA => "DEA", Kind::HEALTH_ID => "HEALTH_ID",
            Kind::MEDICARE_MBI => "MEDICARE_MBI",
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
        Kind::DOB | Kind::PASSPORT | Kind::DRIVERS_LICENSE | Kind::VIN | Kind::MRZ => "identity",
        Kind::US_ITIN | Kind::CA_SIN | Kind::UK_NHS | Kind::UK_NINO | Kind::AU_TFN
        | Kind::AADHAAR => "national-id",
        Kind::MRN | Kind::NPI | Kind::DEA | Kind::HEALTH_ID | Kind::MEDICARE_MBI => "medical",
        Kind::CASE_NO => "legal",
        Kind::CRYPTO_WALLET | Kind::IPV6 | Kind::MAC_ADDRESS | Kind::IP => "network",
        Kind::APIKEY | Kind::JWT | Kind::PRIVATE_KEY | Kind::CONNECTION_STRING
        | Kind::CREDENTIAL => "secrets",
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
        | Kind::PRIVATE_KEY | Kind::CONNECTION_STRING | Kind::CUSTOM | Kind::VIN
        | Kind::MRZ => 1.0,
        // Highly distinctive structural format, no checksum.
        Kind::EMAIL | Kind::URL | Kind::APIKEY | Kind::JWT | Kind::MAC_ADDRESS
        | Kind::EIN | Kind::US_ITIN | Kind::MEDICARE_MBI => 0.95,
        // Context-anchored with a weak value check (presence of a digit, a
        // plausible date, structural rules, entropy).
        Kind::DOB | Kind::PASSPORT | Kind::DRIVERS_LICENSE | Kind::MRN
        | Kind::HEALTH_ID | Kind::CASE_NO | Kind::UK_NINO | Kind::CREDENTIAL => 0.85,
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
    // Azure storage/service-bus connection strings: the AccountKey/
    // SharedAccessKey value IS the credential (88-char base64). Anchored on
    // the key name, so prose about "account keys" never matches.
    pc(Kind::CONNECTION_STRING, r"(?i)\b(?:AccountKey|SharedAccessKey)=([A-Za-z0-9+/]{43,86}={0,2})"),
    // AWS secret access keys have no distinctive prefix (40 base64 chars), so
    // they're context-anchored on the variable name they always travel with.
    pc(Kind::APIKEY, r#"(?i)\baws_?secret_?access_?key\b["']?\s*[:=]\s*["']?([A-Za-z0-9/+=]{40})\b"#),
    // A pasted Authorization header is a live credential regardless of scheme.
    pc(Kind::APIKEY, r"(?i)\bauthorization:\s*bearer\s+([A-Za-z0-9._~+/\-]{16,}=*)"),
    // Generic credential assignments (gitleaks-style catch-all): a key name
    // like password/secret/token followed by `:`/`=`/`=>` and a value token.
    // The regex is deliberately broad; `validators::credential_value` does
    // the real work (length, Shannon entropy, placeholder stoplist, env-var
    // templating) so `password: changeme` and prose survive. The anchor list
    // requires full words — bare "pass", "auth", or "key" never match.
    pc(Kind::CREDENTIAL, r#"(?i)\b(?:pass(?:word|wd|phrase)|pwd|secret|token|api[_\-]?key|access[_\-]?key|client[_\-]?secret|auth[_\-]?token|credentials?)\b["']?\s*(?:=>|[:=])\s*["']?([^\s"'`,;]{8,})"#),
    // ---- 2. Anchored packs (alias) -----------------------------------------
    pc(Kind::US_BANK, r"(?i)\b(?:aba|routing|rtn)(?:\s*(?:no|number|#))?\.?[:\s]+(\d{9})\b"),
    pc(Kind::US_BANK, r"(?i)\b(?:account|acct)(?:\s*(?:no|number|#))?\.?[:\s]+(\d{6,17})\b"),
    pc(Kind::SWIFT_BIC, r"\b(?i:swift|bic)(?i:\s*(?:code|no|number|#))?\.?[:\s]+([A-Z]{6}[A-Z0-9]{2}(?:[A-Z0-9]{3})?)\b"),
    pc(Kind::EIN, r"(?i)\b(?:ein|employer identification number|tax id)(?:\s*(?:no|number|#))?\.?[:\s]+(\d{2}-\d{7})\b"),
    pc(Kind::DOB, r"(?i)\b(?:dob|date of birth|birth\s?date|born(?:\s+on)?)\.?[:\s]+(\d{1,2}[/\-\.]\d{1,2}[/\-\.](?:\d{4}|\d{2})|\d{4}-\d{2}-\d{2}|(?:jan(?:uary)?|feb(?:ruary)?|mar(?:ch)?|apr(?:il)?|may|jun(?:e)?|jul(?:y)?|aug(?:ust)?|sep(?:t(?:ember)?)?|oct(?:ober)?|nov(?:ember)?|dec(?:ember)?)\.?\s+\d{1,2},?\s+\d{4})\b"),
    pc(Kind::PASSPORT, r"(?i)\bpassport(?:\s*(?:no|number|#))?\.?[:\s]+([A-Z0-9]{6,9})\b"),
    pc(Kind::DRIVERS_LICENSE, r"(?i)\b(?:driver'?s?\s+licen[cs]e|dl)(?:\s*(?:no|number|#))?\.?[:\s]+([A-Z0-9\-]{4,13})\b"),
    // Passport MRZ (ICAO 9303 TD3, line 2): document number, nationality,
    // DOB, sex, expiry in one 44-char machine-readable line. Four check
    // digits validated — doc number, DOB, expiry, and the composite — so a
    // random fixed-width token can't fake it. Boundary groups instead of \b:
    // '<' is a non-word char, so \b would match inside the filler runs.
    pc(Kind::MRZ, r"(?:^|[^A-Z0-9<])([A-Z0-9<]{9}\d[A-Z<]{3}\d{6}\d[MFX<]\d{6}\d[A-Z0-9<]{14}\d{2})(?:[^A-Z0-9<]|$)"),
    // VIN: unanchored 17-char (charset excludes I/O/Q) gated by the ISO 3779
    // check digit — ~1/11 selectivity on top of the strict charset. Known
    // miss: EU-market VINs without a valid North-American check digit.
    p(Kind::VIN, r"\b[A-HJ-NPR-Z0-9]{17}\b"),
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
    // Medicare MBI before the generic HEALTH_ID anchor — "medicare id:
    // 1EG4-TE5-MK73" satisfies both shapes and the specific kind must win
    // the span tie. Per-position CMS character classes live in the validator.
    pc(Kind::MEDICARE_MBI, r"(?i)\b(?:medicare|mbi)(?:\s*(?:no|number|id|#))?\.?[:\s]+([0-9][A-Za-z][A-Za-z0-9][0-9]-?[A-Za-z][A-Za-z0-9][0-9]-?[A-Za-z]{2}[0-9]{2})\b"),
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
    p(Kind::APIKEY,  r"\b(?:sk-|sk_live_|sk_test_|pk_|rk_live_|AKIA|ASIA|ghp_|gho_|ghu_|ghs_|ghr_|github_pat_|glpat-|xox[baprs]-|xapp-|hf_|npm_|pypi-|glsa_|dop_v1_|shpat_|shpss_|figd_|lin_api_|tfp_)[A-Za-z0-9_\-]{10,}\b|\bAIza[0-9A-Za-z_\-]{35}\b|\bya29\.[0-9A-Za-z_\-\.]{20,}\b|\bSG\.[A-Za-z0-9_\-]{16,}\.[A-Za-z0-9_\-]{16,}\b"),
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
            | Kind::CONNECTION_STRING | Kind::CREDENTIAL
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
        Kind::CREDENTIAL => Some(BlockPolicy {
            rule: "Password or secret assignment in outbound payload",
            class: "CRITICAL_SECRET",
            desc: "This looks like a live credential (a password/secret/token assignment with a high-entropy value). Remove it or replace the value with a placeholder — or switch to a local model.",
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
        Kind::SSN => validators::ssn(raw),
        Kind::IP => validators::ipv4_octets(raw),
        Kind::IPV6 => raw.parse::<std::net::Ipv6Addr>().is_ok(),
        Kind::CREDENTIAL => validators::credential_value(raw),
        Kind::VIN => validators::vin(raw),
        Kind::MRZ => validators::mrz_td3(raw),
        Kind::MEDICARE_MBI => validators::medicare_mbi(raw),
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

    /// Exact IBAN length per country (ISO 13616 registry). A wrong-length
    /// candidate with a coincidentally valid mod-97 is still not an IBAN.
    fn iban_len(cc: &str) -> Option<usize> {
        Some(match cc {
            "NO" => 15, "BE" => 16,
            "DK" | "FI" | "FO" | "GL" | "NL" | "FK" | "SD" => 18,
            "MK" | "SI" => 19,
            "AT" | "BA" | "EE" | "KZ" | "LT" | "LU" | "MN" | "XK" => 20,
            "CH" | "CR" | "HR" | "LI" | "LV" => 21,
            "BG" | "BH" | "DE" | "GB" | "GE" | "IE" | "ME" | "RS" | "VA" => 22,
            "AE" | "GI" | "IL" | "IQ" | "TL" | "OM" | "SO" => 23,
            "AD" | "CZ" | "ES" | "MD" | "PK" | "RO" | "SA" | "SE" | "SK" | "VG" | "TN" => 24,
            "EG" | "PT" | "LY" | "ST" => 25,
            "IS" | "TR" => 26,
            "FR" | "GR" | "IT" | "MC" | "MR" | "SM" | "BI" | "DJ" => 27,
            "AL" | "AZ" | "BY" | "CY" | "DO" | "GT" | "HU" | "LB" | "PL" | "SV" | "NI" => 28,
            "BR" | "PS" | "QA" | "UA" => 29,
            "JO" | "KW" | "MU" => 30,
            "MT" | "SC" => 31, "LC" => 32, "RU" => 33,
            _ => return None,
        })
    }

    pub fn iban(raw: &str) -> bool {
        let s: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
        if s.len() < 15 || s.len() > 34 { return false; }
        let cc = &s[..2];
        if !IBAN_COUNTRIES.contains(&cc) { return false; }
        if let Some(expected) = iban_len(cc) {
            if s.len() != expected { return false; }
        }
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
            let day_month_ok = ((1..=12).contains(&a) && (1..=31).contains(&b))
                || ((1..=12).contains(&b) && (1..=31).contains(&a));
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

    /// ISO 3779 VIN check digit: transliterate (I/O/Q never appear), weight
    /// positions [8,7,6,5,4,3,2,10,0,9,8,7,6,5,4,3,2], sum mod 11; remainder
    /// 10 is written 'X' and must equal position 9.
    pub fn vin(raw: &str) -> bool {
        if raw.len() != 17 { return false; }
        let translit = |c: char| -> Option<u32> {
            Some(match c {
                '0'..='9' => c as u32 - '0' as u32,
                'A' | 'J' => 1, 'B' | 'K' | 'S' => 2, 'C' | 'L' | 'T' => 3,
                'D' | 'M' | 'U' => 4, 'E' | 'N' | 'V' => 5, 'F' | 'W' => 6,
                'G' | 'P' | 'X' => 7, 'H' | 'Y' => 8, 'R' | 'Z' => 9,
                _ => return None,
            })
        };
        const W: [u32; 17] = [8, 7, 6, 5, 4, 3, 2, 10, 0, 9, 8, 7, 6, 5, 4, 3, 2];
        let mut sum = 0u32;
        for (i, c) in raw.chars().enumerate() {
            let Some(v) = translit(c) else { return false; };
            sum += v * W[i];
        }
        let check = match sum % 11 {
            10 => 'X',
            r => char::from_digit(r, 10).unwrap(),
        };
        raw.chars().nth(8) == Some(check)
    }

    /// SSA structural rules: area 000/666/9xx, group 00, and serial 0000
    /// are never issued. Rejecting them keeps obvious test/sample numbers
    /// (000-12-3456) from blocking sends while every real SSN still does.
    pub fn ssn(raw: &str) -> bool {
        let d = digits_of(raw);
        if d.len() != 9 { return false; }
        let area = d[0] * 100 + d[1] * 10 + d[2];
        let group = d[3] * 10 + d[4];
        let serial = d[5] * 1000 + d[6] * 100 + d[7] * 10 + d[8];
        area != 0 && area != 666 && area < 900 && group != 0 && serial != 0
    }

    /// ICAO 9303 TD3 (passport) MRZ line 2: check digits with weights 7,3,1
    /// over A–Z=10–35, digits, '<'=0 — for the document number, birth date,
    /// expiry date, and the composite over all three fields plus the
    /// personal-number block. Four independent digits ≈ 1/10⁴ FP floor on
    /// top of the rigid 44-char shape.
    pub fn mrz_td3(raw: &str) -> bool {
        if raw.len() != 44 { return false; }
        let b: Vec<char> = raw.chars().collect();
        let val = |c: char| -> Option<u32> {
            match c {
                '0'..='9' => Some(c as u32 - '0' as u32),
                'A'..='Z' => Some(c as u32 - 'A' as u32 + 10),
                '<' => Some(0),
                _ => None,
            }
        };
        let check = |s: &[char], expect: char| -> bool {
            const W: [u32; 3] = [7, 3, 1];
            let mut sum = 0u32;
            for (i, &c) in s.iter().enumerate() {
                let Some(v) = val(c) else { return false };
                sum += v * W[i % 3];
            }
            char::from_digit(sum % 10, 10) == Some(expect)
        };
        let composite: Vec<char> = b[0..10].iter().chain(&b[13..20]).chain(&b[21..43]).copied().collect();
        check(&b[0..9], b[9])
            && check(&b[13..19], b[19])
            && check(&b[21..27], b[27])
            && check(&composite, b[43])
    }

    /// Medicare Beneficiary Identifier (CMS spec): 11 chars, per-position
    /// classes — digits at 1/4/7/10/11 (position 1 is 1–9), letters at
    /// 2/5/8/9 drawn from A–Z minus S,L,O,I,B,Z, and positions 3/6 either.
    pub fn medicare_mbi(raw: &str) -> bool {
        let s: String = raw.chars().filter(|&c| c != '-').collect::<String>().to_ascii_uppercase();
        if s.len() != 11 { return false; }
        let letter_ok = |c: char| c.is_ascii_uppercase() && !matches!(c, 'S' | 'L' | 'O' | 'I' | 'B' | 'Z');
        let b: Vec<char> = s.chars().collect();
        ('1'..='9').contains(&b[0])
            && letter_ok(b[1])
            && (letter_ok(b[2]) || b[2].is_ascii_digit())
            && b[3].is_ascii_digit()
            && letter_ok(b[4])
            && (letter_ok(b[5]) || b[5].is_ascii_digit())
            && b[6].is_ascii_digit()
            && letter_ok(b[7])
            && letter_ok(b[8])
            && b[9].is_ascii_digit()
            && b[10].is_ascii_digit()
    }

    /// Shannon entropy over the character distribution, in bits/char.
    /// English prose words sit ≈2–2.8; generated secrets ≈3.5–4.5.
    pub fn shannon_entropy(s: &str) -> f64 {
        let mut counts: std::collections::HashMap<char, u32> = std::collections::HashMap::new();
        let mut n = 0u32;
        for c in s.chars() {
            *counts.entry(c).or_insert(0) += 1;
            n += 1;
        }
        if n == 0 { return 0.0; }
        counts.values().map(|&c| {
            let p = c as f64 / n as f64;
            -p * p.log2()
        }).sum()
    }

    /// Decides whether the value side of a `password=…`-style assignment is a
    /// real credential or a placeholder. Three gates, in cheapness order:
    ///
    /// 1. Templating / placeholder shapes: env interpolation (`$VAR`, `${…}`,
    ///    `{{…}}`, `%(…)`, `<value>`), literal keywords (true/false/null/…),
    ///    and stoplist substrings ("changeme", "example", "password"…).
    /// 2. Character mix: must contain a digit or symbol. Generated secrets
    ///    virtually always do; the all-letters case is dominated by prose
    ///    ("the secret: wonderful") and dictionary placeholders.
    /// 3. Shannon entropy ≥ 3.0 bits/char — prose words score ≈2–2.8,
    ///    generated secrets ≈3.5+. Known miss: doubled-word passwords like
    ///    "hunter2hunter2" (H≈2.8) — the paranoid LLM layer is the backstop.
    pub fn credential_value(raw: &str) -> bool {
        if raw.len() < 8 { return false; }
        let first = raw.chars().next().unwrap_or(' ');
        if matches!(first, '$' | '%' | '<' | '{' | '[') || raw.contains("${") || raw.contains("{{") {
            return false;
        }
        let lower = raw.to_ascii_lowercase();
        if matches!(lower.as_str(), "true" | "false" | "null" | "none" | "nil" | "undefined") {
            return false;
        }
        const PLACEHOLDER_SUBSTRINGS: &[&str] = &[
            "password", "passwd", "passphrase", "changeme", "change-me", "change_me",
            "example", "sample", "dummy", "placeholder", "redacted", "secret",
            "your-", "your_", "xxxx", "****", "1234567", "abcdefg", "qwerty",
        ];
        if PLACEHOLDER_SUBSTRINGS.iter().any(|p| lower.contains(p)) {
            return false;
        }
        if !raw.chars().any(|c| c.is_ascii_digit() || !c.is_alphanumeric()) {
            return false;
        }
        shannon_entropy(raw) >= 3.0
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
        // AWS secret access key — anchored on the variable name (no prefix).
        let spans = run(r#"aws_secret_access_key = "wJalrXUtnFEMIK7MDENGbPxRfiCYEXAMPLEKEY12""#);
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::APIKEY));
        assert!(is_critical(&spans[0].kind));
        // 39 chars → not a secret key shape.
        assert!(run(r#"aws_secret_access_key = "tooShortByOneChar123456789012345678901""#).is_empty());
        // Pasted Authorization header blocks regardless of token scheme.
        let spans = run("curl -H 'Authorization: Bearer sk-live-style-token-AbCdEf123456'");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::APIKEY));
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
        // Azure storage conn string: the AccountKey value blocks.
        let spans = run("DefaultEndpointsProtocol=https;AccountName=snx;AccountKey=YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXowMTIzNDU2Nzg5QUJDRA==;EndpointSuffix=core.windows.net");
        assert!(spans.iter().any(|s| matches!(s.kind, Kind::CONNECTION_STRING)), "{spans:?}");
        // Prose about account keys never matches (anchor requires '=value').
        assert!(run("rotate the AccountKey quarterly per policy").is_empty());
    }

    #[test]
    fn ssn_structural_rules() {
        // Real-shaped SSN blocks; SSA-impossible shapes pass through.
        let spans = run("my ssn is 123-45-6789");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::SSN));
        for fake in ["000-12-3456", "666-12-3456", "900-12-3456", "123-00-4567", "123-45-0000"] {
            assert!(run(&format!("ssn {fake} noted")).iter().all(|s| !matches!(s.kind, Kind::SSN)), "{fake}");
        }
    }

    #[test]
    fn iban_exact_country_lengths() {
        // Valid German IBAN (22 chars) detected…
        let spans = run("wire to DE89 3704 0044 0532 0130 00 today");
        assert!(spans.iter().any(|s| matches!(s.kind, Kind::IBAN)));
        // …but a German prefix at French length never validates, even if a
        // mod-97-passing string of that shape existed; quick negative via
        // truncation (also breaks mod-97, both gates agree).
        assert!(run("ref DE89 3704 0044 0532 0130 fail").iter().all(|s| !matches!(s.kind, Kind::IBAN)));
    }

    #[test]
    fn expanded_secret_prefixes_2() {
        for key in [
            "hf_AbCdEfGh123456789", "npm_a1b2c3d4e5f6g7h8", "pypi-AgEIcHlwaS5vcmc",
            "dop_v1_abcdef0123456789", "shpat_0123456789abcdef", "figd_abc123def456ghi",
            "SG.abcdefghij1234567.klmnopqrst7654321",
        ] {
            let spans = run(&format!("rotate {key} now"));
            assert!(spans.iter().any(|s| matches!(s.kind, Kind::APIKEY)), "{key}: {spans:?}");
        }
    }

    #[test]
    fn passport_mrz_check_digits_validated() {
        // Canonical ICAO 9303 specimen (Utopia passport, all four digits valid).
        let line2 = "L898902C36UTO7408122F1204159ZE184226B<<<<<10";
        let spans = run(&format!("ocr dump:\n{line2}\nend"));
        assert_eq!(spans.len(), 1, "{spans:?}");
        assert!(matches!(spans[0].kind, Kind::MRZ));
        assert_eq!(spans[0].raw, line2);
        assert_eq!(spans[0].confidence, 1.0);
        // One mutated digit breaks the document-number check digit.
        let bad = "L898903C36UTO7408122F1204159ZE184226B<<<<<10";
        assert!(run(bad).iter().all(|s| !matches!(s.kind, Kind::MRZ)));
        // Same-shape random filler fails all four digits.
        assert!(run("ABCDEFGHI0XYZ0101011M2020202QQQQQQQQQQQQQQ00")
            .iter().all(|s| !matches!(s.kind, Kind::MRZ)));
    }

    #[test]
    fn vin_check_digit_validated() {
        // Canonical valid VIN (check digit '3' at position 9), unanchored.
        let spans = run("totaled my civic, VIN 1HGCM82633A004352, need the claim letter");
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].kind, Kind::VIN));
        assert_eq!(spans[0].raw, "1HGCM82633A004352");
        assert_eq!(spans[0].confidence, 1.0);
        // One-digit mutation breaks the ISO 3779 check digit.
        assert!(run("ref 1HGCM82633A004353 in the export").is_empty());
        // 17 chars containing I/O/Q can't be a VIN (charset excludes them).
        assert!(run("order IOQ4567890123456 confirmed").is_empty());
    }

    #[test]
    fn medicare_mbi_anchored_and_structured() {
        for text in [
            "patient medicare id: 1EG4-TE5-MK73 on file",
            "MBI: 1EG4TE5MK73 per the claim",
        ] {
            let spans = run(text);
            assert_eq!(spans.len(), 1, "{text}");
            assert!(matches!(spans[0].kind, Kind::MEDICARE_MBI), "{text}: {spans:?}");
        }
        // S is an excluded letter — fails the CMS class for position 2; the
        // generic HEALTH_ID anchor doesn't apply ("medicare" isn't in its
        // anchor list), so the value passes through unaliased.
        assert!(run("medicare id: 1SG4-TE5-MK73 noted")
            .iter().all(|s| !matches!(s.kind, Kind::MEDICARE_MBI)));
        // Unanchored MBIs are out of scope (documented recall tradeoff).
        assert!(run("the code 1EG4TE5MK73 appears twice").is_empty());
    }

    #[test]
    fn credential_assignments_block_on_entropy() {
        // Real-looking credential assignments across common syntaxes → block.
        for text in [
            "set password=Tr0ub4dor&3xplor3 before deploy",
            r#"config has "api_key": "9aB3xQ7mLpZ2kf4w" in it"#,
            "export CLIENT_SECRET=wJalrXUtnFEMI7MDENGbPxRfiCY",
            ":auth_token => 'mZ9qLp42xKv7wRt3'",
        ] {
            let spans = run(text);
            assert!(
                spans.iter().any(|s| matches!(s.kind, Kind::CREDENTIAL)),
                "expected CREDENTIAL in {text:?}, got {spans:?}"
            );
            assert!(is_critical(&Kind::CREDENTIAL));
        }
        // Placeholders, templating, prose, and low entropy → no hit.
        for text in [
            "password: changeme then rotate",            // stoplist
            "password=${DB_PASSWORD} from the env",      // templating
            "the secret: remember to rotate quarterly",  // prose, low entropy
            "token = <your-token-here> goes in .env",    // angle placeholder
            "password: hunter2h", // 8 chars but low entropy, no digit pattern
            "secret: wonderful",  // all-letters prose word
        ] {
            let spans = run(text);
            assert!(
                spans.iter().all(|s| !matches!(s.kind, Kind::CREDENTIAL)),
                "false positive in {text:?}: {spans:?}"
            );
        }
    }

    #[test]
    fn credential_value_validator_gates() {
        use super::validators::{credential_value, shannon_entropy};
        // Entropy sanity: prose low, generated high.
        assert!(shannon_entropy("remember") < 3.0);
        assert!(shannon_entropy("wJalrXUtnFEMI7MDENGbPxRfiCY") > 3.5);
        // Gate order: length, templating, keyword literals, stoplist, charmix.
        assert!(!credential_value("short1!"));            // < 8
        assert!(!credential_value("$ENV_VAR_NAME"));      // templating
        assert!(!credential_value("{{ vault.password }}"));
        assert!(!credential_value("undefined"));          // literal
        assert!(!credential_value("MyPassword123!"));     // stoplist substring
        assert!(!credential_value("Wonderful"));          // no digit/symbol
        assert!(credential_value("Tr0ub4dor&3xplor3"));
        assert!(credential_value("correct-horse-battery"));
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
            Kind::JWT, Kind::PRIVATE_KEY, Kind::CONNECTION_STRING, Kind::CREDENTIAL,
            Kind::DOB, Kind::PASSPORT, Kind::DRIVERS_LICENSE, Kind::VIN, Kind::MRZ,
            Kind::US_ITIN, Kind::CA_SIN, Kind::UK_NHS, Kind::UK_NINO, Kind::AU_TFN, Kind::AADHAAR,
            Kind::MRN, Kind::NPI, Kind::DEA, Kind::HEALTH_ID, Kind::MEDICARE_MBI, Kind::CASE_NO,
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
        assert_eq!(pack_for(&Kind::CREDENTIAL), "secrets");
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
