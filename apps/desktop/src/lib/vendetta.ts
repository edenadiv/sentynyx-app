import type { Kind, Span } from "./types";

// ---------------------------------------------------------------------------
// Client-side mirror of the Rust Vendetta engine (src-tauri/src/vendetta.rs).
// Used ONLY for the realtime composer highlights and the instant client-side
// critical pre-flash — the Rust engine is the egress source of truth. The
// pattern list, the PATTERN ORDER (stable-sort ties resolve to insertion
// order — blocking kinds must come first), and every checksum validator must
// stay in lockstep with Rust, or users see phantom protection / phantom
// blocks. JS dialect notes: no scoped (?i:) groups (WKWebView compat), so
// anchored patterns use a whole-pattern `i` flag and case-sensitive value
// constraints move into validators.
// ---------------------------------------------------------------------------

/// `cap` = true → the sensitive value is capture group 1 (context-anchored
/// pattern). Every anchored pattern ends with its capture group, so group
/// offsets are computed as a trailing substring of the match — no reliance on
/// the regex `d` flag.
const PATTERNS: { kind: Kind; re: RegExp; cap?: boolean }[] = [
  // ---- 1. Blocking, high-specificity ----
  { kind: "PRIVATE_KEY", re: /-----BEGIN (?:[A-Z][A-Z ]{0,24})?PRIVATE KEY-----(?:[\s\S]{0,4096}?-----END (?:[A-Z][A-Z ]{0,24})?PRIVATE KEY-----)?/g },
  { kind: "CREDITCARD", re: /\b\d(?:[ -]?\d){12,18}\b/g },
  { kind: "IBAN", re: /\b[A-Z]{2}\d{2}(?: ?[A-Z0-9]){11,30}\b/g },
  { kind: "CONNECTION_STRING", re: /\b(?:postgres(?:ql)?|mysql|mongodb(?:\+srv)?|redis|rediss|amqps?):\/\/[^\s:/@]+:[^\s/@]+@[^\s/]+/gi },
  { kind: "CONNECTION_STRING", re: /\b(?:AccountKey|SharedAccessKey)=([A-Za-z0-9+/]{43,86}={0,2})/gi, cap: true },
  { kind: "APIKEY", re: /\baws_?secret_?access_?key\b["']?\s*[:=]\s*["']?([A-Za-z0-9/+=]{40})\b/gi, cap: true },
  { kind: "APIKEY", re: /\bauthorization:\s*bearer\s+([A-Za-z0-9._~+/-]{16,}=*)/gi, cap: true },
  // Generic credential assignments — broad anchor, the real decision lives in
  // credentialValue() (entropy + stoplist), exactly like the Rust validator.
  { kind: "CREDENTIAL", re: /\b(?:pass(?:word|wd|phrase)|pwd|secret|token|api[_-]?key|access[_-]?key|client[_-]?secret|auth[_-]?token|credentials?)\b["']?\s*(?:=>|[:=])\s*["']?([^\s"'`,;]{8,})/gi, cap: true },
  // ---- 2. Anchored packs ----
  { kind: "US_BANK", re: /\b(?:aba|routing|rtn)(?:\s*(?:no|number|#))?\.?[:\s]+(\d{9})\b/gi, cap: true },
  { kind: "US_BANK", re: /\b(?:account|acct)(?:\s*(?:no|number|#))?\.?[:\s]+(\d{6,17})\b/gi, cap: true },
  { kind: "US_BANK", re: /\b(?:sort\s*code)(?:\s*(?:no|number|#))?\.?[:\s]+(\d{2}[- ]?\d{2}[- ]?\d{2})\b/gi, cap: true },
  { kind: "SWIFT_BIC", re: /\b(?:swift|bic)(?:\s*(?:code|no|number|#))?\.?[:\s]+([A-Z]{6}[A-Z0-9]{2}(?:[A-Z0-9]{3})?)\b/gi, cap: true },
  { kind: "EIN", re: /\b(?:ein|employer identification number|tax id)(?:\s*(?:no|number|#))?\.?[:\s]+(\d{2}-\d{7})\b/gi, cap: true },
  { kind: "DOB", re: /\b(?:dob|date of birth|birth ?date|born(?: on)?)\.?[:\s]+(\d{1,2}[/\-.]\d{1,2}[/\-.](?:\d{4}|\d{2})|\d{4}-\d{2}-\d{2}|(?:jan(?:uary)?|feb(?:ruary)?|mar(?:ch)?|apr(?:il)?|may|jun(?:e)?|jul(?:y)?|aug(?:ust)?|sep(?:t(?:ember)?)?|oct(?:ober)?|nov(?:ember)?|dec(?:ember)?)\.? +\d{1,2},? +\d{4})\b/gi, cap: true },
  { kind: "PASSPORT", re: /\bpassport(?:\s*(?:no|number|#))?\.?[:\s]+([A-Z0-9]{6,9})\b/gi, cap: true },
  { kind: "DRIVERS_LICENSE", re: /\b(?:driver'?s? licen[cs]e|dl)(?:\s*(?:no|number|#))?\.?[:\s]+([A-Z0-9-]{4,13})\b/gi, cap: true },
  // Passport MRZ (ICAO 9303 TD3 line 2) — four check digits in the validator.
  { kind: "MRZ", re: /(?:^|[^A-Z0-9<])([A-Z0-9<]{9}\d[A-Z<]{3}\d{6}\d[MFX<]\d{6}\d[A-Z0-9<]{14}\d{2})(?=[^A-Z0-9<]|$)/g, cap: true },
  // VIN: unanchored, ISO 3779 check digit does the real filtering.
  { kind: "VIN", re: /\b[A-HJ-NPR-Z0-9]{17}\b/g },
  { kind: "US_ITIN", re: /\bitin(?:\s*(?:no|number|#))?\.?[:\s]+(9\d{2}-(?:7\d|8[0-8]|9[0-24-9])-\d{4})\b/gi, cap: true },
  { kind: "CA_SIN", re: /\b(?:sin|social insurance)(?:\s*(?:no|number|#))?\.?[:\s]+(\d{3}[ -]?\d{3}[ -]?\d{3})\b/gi, cap: true },
  { kind: "UK_NHS", re: /\bnhs(?:\s*(?:no|number|#))?\.?[:\s]+(\d{3}[ -]?\d{3}[ -]?\d{4})\b/gi, cap: true },
  { kind: "UK_NINO", re: /\b(?:national insurance|nino)(?:\s*(?:no|number|#))?\.?[:\s]+([A-Za-z]{2}\d{6}[A-Da-d])\b/gi, cap: true },
  { kind: "AU_TFN", re: /\b(?:tfn|tax file number)(?:\s*(?:no|number|#))?\.?[:\s]+(\d{3}[ -]?\d{3}[ -]?\d{3})\b/gi, cap: true },
  { kind: "AADHAAR", re: /\baadh?aar(?:\s*(?:no|number|#))?\.?[:\s]+(\d{4}[ -]?\d{4}[ -]?\d{4})\b/gi, cap: true },
  { kind: "IT_CF", re: /\b[A-Z]{6}\d{2}[ABCDEHLMPRST]\d{2}[A-Z]\d{3}[A-Z]\b/g },
  { kind: "ES_DNI", re: /\b(?:\d{8}|[XYZ]\d{7})[TRWAGMYFPDXBNJZSQVHLCKE]\b/g },
  { kind: "BR_CPF", re: /\b\d{3}\.\d{3}\.\d{3}-\d{2}\b/g },
  { kind: "BR_CPF", re: /\bcpf(?:\s*(?:no|number|nº|#))?\.?[:\s]+(\d{3}\.?\d{3}\.?\d{3}-?\d{2})\b/gi, cap: true },
  { kind: "PL_PESEL", re: /\bpesel(?:\s*(?:no|number|#))?\.?[:\s]+(\d{11})\b/gi, cap: true },
  { kind: "MRN", re: /\b(?:mrn|medical record)(?:\s*(?:no|number|#))?\.?[:\s]+([A-Z0-9-]{5,12})\b/gi, cap: true },
  { kind: "NPI", re: /\bnpi(?:\s*(?:no|number|#))?\.?[:\s]+(\d{10})\b/gi, cap: true },
  { kind: "DEA", re: /\bdea(?:\s*(?:no|number|reg(?:istration)?|#))?\.?[:\s]+([A-Za-z]{2}\d{7})\b/gi, cap: true },
  // Medicare MBI before the generic HEALTH_ID anchor (same-span tie must
  // resolve to the specific kind, mirroring the Rust ordering).
  { kind: "MEDICARE_MBI", re: /\b(?:medicare|mbi)(?:\s*(?:no|number|id|#))?\.?[:\s]+([0-9][A-Za-z][A-Za-z0-9][0-9]-?[A-Za-z][A-Za-z0-9][0-9]-?[A-Za-z]{2}[0-9]{2})\b/gi, cap: true },
  { kind: "HEALTH_ID", re: /\b(?:member|subscriber|insurance|policy)\s*(?:id)?\s*(?:no|number|#)?\.?[:\s]+([A-Z0-9-]{6,15})\b/gi, cap: true },
  { kind: "CASE_NO", re: /\b(?:case|docket|matter)\s*(?:no|number|#)?\.?[:\s]+([A-Z0-9][A-Z0-9:.-]{3,19})\b/gi, cap: true },
  { kind: "CASE_NO", re: /\b\d{1,2}:\d{2}-(?:cv|cr|cm|md|mc|mj|po|sw)-\d{2,6}(?:-[A-Z]{2,4})?\b/g },
  { kind: "CRYPTO_WALLET", re: /\b0x[a-fA-F0-9]{40}\b|\bbc1[ac-hj-np-z02-9]{25,62}\b|\b[13][a-km-zA-HJ-NP-Z1-9]{25,34}\b/g },
  { kind: "JWT", re: /\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}/g },
  { kind: "MAC_ADDRESS", re: /\b[0-9A-Fa-f]{2}(?:[:-][0-9A-Fa-f]{2}){5}\b/g },
  { kind: "IPV6", re: /\b[A-Fa-f0-9]{1,4}(?::[A-Fa-f0-9]{0,4}){2,7}\b/g },
  // ---- 3. Legacy generics ----
  { kind: "EMAIL",   re: /\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b/g },
  { kind: "PHONE",   re: /\b(?:\+?\d{1,3}[\s.-]?)?(?:\(?\d{3}\)?[\s.-]?)\d{3}[\s.-]?\d{4}\b/g },
  { kind: "SSN",     re: /\b\d{3}-\d{2}-\d{4}\b/g },
  { kind: "IP",      re: /\b(?:\d{1,3}\.){3}\d{1,3}\b/g },
  { kind: "APIKEY",  re: /\b(?:sk-|sk_live_|sk_test_|pk_|rk_live_|AKIA|ASIA|ghp_|gho_|ghu_|ghs_|ghr_|github_pat_|glpat-|xox[baprs]-|xapp-|hf_|npm_|pypi-|glsa_|dop_v1_|shpat_|shpss_|figd_|lin_api_|tfp_)[A-Za-z0-9_-]{10,}\b|\bAIza[0-9A-Za-z_-]{35}\b|\bya29\.[0-9A-Za-z_.-]{20,}\b|\bSG\.[A-Za-z0-9_-]{16,}\.[A-Za-z0-9_-]{16,}\b/g },
  { kind: "URL",     re: /\bhttps?:\/\/[^\s)]+/g },
  { kind: "ADDRESS", re: /\b\d{1,5}\s+[A-Z][a-zA-Z]+(?:\s+[A-Z][a-zA-Z]+){0,3}\s+(?:Street|St|Avenue|Ave|Road|Rd|Blvd|Lane|Ln|Drive|Dr|Court|Ct|Way)\b/g },
  { kind: "MONEY",   re: /\$\s?\d{1,3}(?:,\d{3})+(?:\.\d+)?|\$\s?\d{4,}(?:\.\d+)?/g },
  { kind: "EMPID",   re: /\bEMP-\d{4,6}\b/g },
  // NAME/COMPANY demo lists removed — arbitrary names/orgs are NER's job.
];

export const LABELS: Record<Kind, string> = {
  EMAIL: "email", PHONE: "phone", SSN: "ssn", IP: "ip",
  APIKEY: "api-key", URL: "url", ADDRESS: "address", MONEY: "amount",
  NAME: "person", COMPANY: "entity", EMPID: "employee-id",
  CREDITCARD: "card", IBAN: "iban", US_BANK: "bank", SWIFT_BIC: "swift", EIN: "ein",
  JWT: "jwt", PRIVATE_KEY: "private-key", CONNECTION_STRING: "conn-string",
  CREDENTIAL: "credential",
  DOB: "dob", PASSPORT: "passport", DRIVERS_LICENSE: "license", VIN: "vin",
  MRZ: "passport-mrz",
  US_ITIN: "itin", CA_SIN: "sin", UK_NHS: "nhs", UK_NINO: "nino",
  AU_TFN: "tfn", AADHAAR: "aadhaar", IT_CF: "codice-fiscale", ES_DNI: "dni", BR_CPF: "cpf", PL_PESEL: "pesel",
  MRN: "mrn", NPI: "npi", DEA: "dea", HEALTH_ID: "member-id",
  MEDICARE_MBI: "medicare-mbi",
  CASE_NO: "case",
  CRYPTO_WALLET: "wallet", IPV6: "ipv6", MAC_ADDRESS: "mac",
  CUSTOM: "custom",
  PERSON_NER: "person", ORG_NER: "entity", CODENAME_NER: "codename",
  LOCATION_NER: "location", EMPID_NER: "employee-id",
};

// Keep these entries in sync with `vendetta::block_policy` in
// apps/desktop/src-tauri/src/vendetta.rs — that function is the egress source
// of truth; this map only exists for the instant client-side pre-flash so we
// avoid a round-trip before showing the PolicyViolation scene.
export const CRITICAL: Partial<Record<Kind, { name: string; class: string; desc: string }>> = {
  SSN: {
    name: "Social Security number in outbound payload",
    class: "CRITICAL_IDENTITY",
    desc: "Sentynyx never sends SSNs to a third-party model endpoint. Remove it, or switch to a local model — local sends never leave this machine.",
  },
  APIKEY: {
    name: "Live API credential in outbound payload",
    class: "CRITICAL_SECRET",
    desc: "Exposed API keys grant immediate access to provider accounts and billing. Rotate the key at its issuer and keep secrets out of prompts — or switch to a local model.",
  },
  CREDITCARD: {
    name: "Payment card number in outbound payload",
    class: "CRITICAL_PAYMENT",
    desc: "This is a checksum-valid card number. Card data must never reach a third-party model endpoint. Remove it, or switch to a local model.",
  },
  IBAN: {
    name: "Bank account (IBAN) in outbound payload",
    class: "CRITICAL_PAYMENT",
    desc: "International bank account numbers are blocked from egress. Remove it, or switch to a local model.",
  },
  PRIVATE_KEY: {
    name: "Private key material in outbound payload",
    class: "CRITICAL_SECRET",
    desc: "Private keys must never leave this machine. Treat this key as compromised and rotate it before continuing.",
  },
  CONNECTION_STRING: {
    name: "Database credentials in outbound payload",
    class: "CRITICAL_SECRET",
    desc: "This connection string carries a live password in the URI. Rotate the credential and keep connection strings out of prompts — or switch to a local model.",
  },
  CREDENTIAL: {
    name: "Password or secret assignment in outbound payload",
    class: "CRITICAL_SECRET",
    desc: "This looks like a live credential (a password/secret/token assignment with a high-entropy value). Remove it or replace the value with a placeholder — or switch to a local model.",
  },
};

// ---------------------------------------------------------------------------
// Validators — mirror of vendetta.rs `mod validators`. A highlight the engine
// would drop is a phantom; a phantom on a BLOCKING class would false-block.
// ---------------------------------------------------------------------------

function digitsOf(s: string): number[] {
  return [...s].filter(c => c >= "0" && c <= "9").map(Number);
}

function luhn(d: number[]): boolean {
  if (d.length === 0) return false;
  let sum = 0;
  for (let i = 0; i < d.length; i++) {
    let v = d[d.length - 1 - i];
    if (i % 2 === 1) { v *= 2; if (v > 9) v -= 9; }
    sum += v;
  }
  return sum % 10 === 0;
}

function creditCard(raw: string): boolean {
  const d = digitsOf(raw);
  const len = d.length;
  if (len < 13 || len > 19) return false;
  const n2 = d[0] * 10 + d[1];
  const n3 = n2 * 10 + d[2];
  const n4 = n3 * 10 + d[3];
  const brandOk =
    (d[0] === 4 && (len === 13 || len === 16 || len === 19)) ||
    (n2 >= 51 && n2 <= 55 && len === 16) ||
    (n4 >= 2221 && n4 <= 2720 && len === 16) ||
    ((n2 === 34 || n2 === 37) && len === 15) ||
    ((n4 === 6011 || n2 === 65 || (n3 >= 644 && n3 <= 649)) && len >= 16 && len <= 19) ||
    (n2 === 35 && len >= 16 && len <= 19) ||
    ((n2 === 36 || n2 === 38) && len >= 14 && len <= 19);
  return brandOk && luhn(d);
}

const IBAN_COUNTRIES = new Set([
  "AD","AE","AL","AT","AZ","BA","BE","BG","BH","BI","BR","BY","CH","CR","CY","CZ",
  "DE","DJ","DK","DO","EE","EG","ES","FI","FK","FO","FR","GB","GE","GI","GL","GR",
  "GT","HR","HU","IE","IL","IQ","IS","IT","JO","KW","KZ","LB","LC","LI","LT","LU",
  "LV","LY","MC","MD","ME","MK","MN","MR","MT","MU","NI","NL","NO","OM","PK","PL",
  "PS","PT","QA","RO","RS","RU","SA","SC","SD","SE","SI","SK","SM","SO","ST","SV",
  "TL","TN","TR","UA","VA","VG","XK",
]);

const IBAN_LEN: Record<string, number> = {
  NO:15, BE:16, DK:18, FI:18, FO:18, GL:18, NL:18, FK:18, SD:18, MK:19, SI:19,
  AT:20, BA:20, EE:20, KZ:20, LT:20, LU:20, MN:20, XK:20,
  CH:21, CR:21, HR:21, LI:21, LV:21,
  BG:22, BH:22, DE:22, GB:22, GE:22, IE:22, ME:22, RS:22, VA:22,
  AE:23, GI:23, IL:23, IQ:23, TL:23, OM:23, SO:23,
  AD:24, CZ:24, ES:24, MD:24, PK:24, RO:24, SA:24, SE:24, SK:24, VG:24, TN:24,
  EG:25, PT:25, LY:25, ST:25, IS:26, TR:26,
  FR:27, GR:27, IT:27, MC:27, MR:27, SM:27, BI:27, DJ:27,
  AL:28, AZ:28, BY:28, CY:28, DO:28, GT:28, HU:28, LB:28, PL:28, SV:28, NI:28,
  BR:29, PS:29, QA:29, UA:29, JO:30, KW:30, MU:30, MT:31, SC:31, LC:32, RU:33,
};

function iban(raw: string): boolean {
  const s = raw.replace(/\s+/g, "");
  if (s.length < 15 || s.length > 34) return false;
  if (!IBAN_COUNTRIES.has(s.slice(0, 2))) return false;
  const expected = IBAN_LEN[s.slice(0, 2)];
  if (expected !== undefined && s.length !== expected) return false;
  const rotated = s.slice(4) + s.slice(0, 4);
  let acc = 0;
  for (const ch of rotated) {
    let v: number;
    if (ch >= "0" && ch <= "9") v = ch.charCodeAt(0) - 48;
    else if (ch >= "A" && ch <= "Z") v = ch.charCodeAt(0) - 65 + 10;
    else return false;
    acc = v < 10 ? (acc * 10 + v) % 97 : (acc * 100 + v) % 97;
  }
  return acc === 1;
}

function usBank(raw: string): boolean {
  const d = digitsOf(raw);
  if (d.length !== 9) return d.length >= 6 && d.length <= 17;
  const prefix = d[0] * 10 + d[1];
  const prefixOk = prefix <= 12 || (prefix >= 21 && prefix <= 32)
    || (prefix >= 61 && prefix <= 72) || prefix === 80;
  const checksum = 3 * (d[0] + d[3] + d[6]) + 7 * (d[1] + d[4] + d[7]) + (d[2] + d[5] + d[8]);
  return prefixOk && checksum % 10 === 0;
}

const SWIFT_EXTRA = new Set([
  "US","CA","AU","NZ","JP","CN","HK","SG","IN","KR","TW","TH","MY","ID","PH",
  "VN","ZA","NG","KE","GH","MA","MX","AR","CL","CO","PE","UY","PA","EC","BO",
  "PY","VE","AM","UZ","TJ","KG","NP","BD","LK","MM","KH","LA","BN","MO","FJ",
]);

function swiftBic(raw: string): boolean {
  // The whole anchored pattern runs with the `i` flag, so re-assert what Rust
  // enforces in the regex itself: BIC values are uppercase.
  if (raw !== raw.toUpperCase()) return false;
  if (raw.length < 6) return false;
  const cc = raw.slice(4, 6);
  return IBAN_COUNTRIES.has(cc) || SWIFT_EXTRA.has(cc);
}

function npi(raw: string): boolean {
  const d = digitsOf(raw);
  if (d.length !== 10) return false;
  return luhn([8, 0, 8, 4, 0, ...d]);
}

function dea(raw: string): boolean {
  if (raw.length !== 9) return false;
  const first = raw[0].toUpperCase();
  if (!"ABFGMPRX".includes(first)) return false;
  const d = digitsOf(raw.slice(2));
  if (d.length !== 7) return false;
  const sum = (d[0] + d[2] + d[4]) + 2 * (d[1] + d[3] + d[5]);
  return sum % 10 === d[6];
}

function caSin(raw: string): boolean {
  const d = digitsOf(raw);
  return d.length === 9 && luhn(d);
}

function ukNhs(raw: string): boolean {
  const d = digitsOf(raw);
  if (d.length !== 10) return false;
  let sum = 0;
  for (let i = 0; i < 9; i++) sum += d[i] * (10 - i);
  let check = 11 - (sum % 11);
  if (check === 11) check = 0;
  if (check === 10) return false;
  return check === d[9];
}

function ukNino(raw: string): boolean {
  const s = raw.toUpperCase();
  if (s.length !== 9) return false;
  const banned = (c: string) => "DFIQUV".includes(c);
  if (banned(s[0]) || banned(s[1]) || s[1] === "O") return false;
  if (["BG", "GB", "NK", "KN", "TN", "NT", "ZZ"].includes(s.slice(0, 2))) return false;
  return true;
}

function auTfn(raw: string): boolean {
  const d = digitsOf(raw);
  if (d.length !== 9) return false;
  const w = [1, 4, 3, 7, 5, 8, 6, 9, 10];
  return d.reduce((acc, x, i) => acc + x * w[i], 0) % 11 === 0;
}

const VERHOEFF_D = [
  [0,1,2,3,4,5,6,7,8,9],[1,2,3,4,0,6,7,8,9,5],[2,3,4,0,1,7,8,9,5,6],
  [3,4,0,1,2,8,9,5,6,7],[4,0,1,2,3,9,5,6,7,8],[5,9,8,7,6,0,4,3,2,1],
  [6,5,9,8,7,1,0,4,3,2],[7,6,5,9,8,2,1,0,4,3],[8,7,6,5,9,3,2,1,0,4],
  [9,8,7,6,5,4,3,2,1,0],
];
const VERHOEFF_P = [
  [0,1,2,3,4,5,6,7,8,9],[1,5,7,6,2,8,3,0,9,4],[5,8,0,3,7,9,6,1,4,2],
  [8,9,1,6,0,4,3,5,2,7],[9,4,5,3,1,2,6,8,7,0],[4,2,8,6,5,7,3,9,0,1],
  [2,7,9,3,8,0,6,4,1,5],[7,0,4,6,9,1,3,2,5,8],
];

function aadhaar(raw: string): boolean {
  const d = digitsOf(raw);
  if (d.length !== 12 || d[0] < 2) return false;
  let c = 0;
  const rev = [...d].reverse();
  for (let i = 0; i < rev.length; i++) c = VERHOEFF_D[c][VERHOEFF_P[i % 8][rev[i]]];
  return c === 0;
}

function datePlausible(raw: string): boolean {
  const yearOk = (y: number) => y >= 1900 && y <= 2100;
  if (raw.length === 10 && raw[4] === "-") {
    const parts = raw.split("-").map(Number);
    return parts.length === 3 && yearOk(parts[0]) && parts[1] >= 1 && parts[1] <= 12 && parts[2] >= 1 && parts[2] <= 31;
  }
  const numeric = raw.split(/[/\-.]/).map(s => s.trim()).filter(Boolean).map(Number);
  if (numeric.length === 3 && numeric.every(n => !Number.isNaN(n))) {
    const [a, b, y] = numeric;
    const dayMonthOk = (a >= 1 && a <= 12 && b >= 1 && b <= 31) || (b >= 1 && b <= 12 && a >= 1 && a <= 31);
    const yOk = y >= 100 ? yearOk(y) : true;
    return dayMonthOk && yOk;
  }
  const nums = raw.split(/[^0-9]+/).filter(Boolean).map(Number);
  if (nums.length === 2) return nums[0] >= 1 && nums[0] <= 31 && yearOk(nums[1]);
  return false;
}

function ipv4Octets(raw: string): boolean {
  const parts = raw.split(".");
  return parts.length === 4 && parts.every(o => /^\d{1,3}$/.test(o) && Number(o) <= 255);
}

function ipv6Parses(raw: string): boolean {
  if (raw.includes(":::")) return false;
  const halves = raw.split("::");
  if (halves.length > 2) return false;
  const groupsOf = (s: string) => (s === "" ? [] : s.split(":"));
  const valid = (g: string) => /^[0-9A-Fa-f]{1,4}$/.test(g);
  if (halves.length === 2) {
    const left = groupsOf(halves[0]);
    const right = groupsOf(halves[1]);
    if (![...left, ...right].every(valid)) return false;
    return left.length + right.length <= 7; // "::" stands for ≥1 zero group
  }
  const gs = groupsOf(halves[0]);
  return gs.length === 8 && gs.every(valid);
}

// --- Minimal synchronous SHA-256 (crypto.subtle is async; detect() is sync) ---
const SHA_K = [
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

function sha256(data: Uint8Array): Uint8Array {
  const rotr = (x: number, n: number) => (x >>> n) | (x << (32 - n));
  const len = data.length;
  const bitLen = len * 8;
  const padded = new Uint8Array(((len + 8) >> 6 << 6) + 64);
  padded.set(data);
  padded[len] = 0x80;
  const dv = new DataView(padded.buffer);
  dv.setUint32(padded.length - 4, bitLen >>> 0);
  dv.setUint32(padded.length - 8, Math.floor(bitLen / 0x100000000));
  let h0 = 0x6a09e667, h1 = 0xbb67ae85, h2 = 0x3c6ef372, h3 = 0xa54ff53a;
  let h4 = 0x510e527f, h5 = 0x9b05688c, h6 = 0x1f83d9ab, h7 = 0x5be0cd19;
  const w = new Int32Array(64);
  for (let off = 0; off < padded.length; off += 64) {
    for (let i = 0; i < 16; i++) w[i] = dv.getInt32(off + i * 4);
    for (let i = 16; i < 64; i++) {
      const s0 = rotr(w[i - 15], 7) ^ rotr(w[i - 15], 18) ^ (w[i - 15] >>> 3);
      const s1 = rotr(w[i - 2], 17) ^ rotr(w[i - 2], 19) ^ (w[i - 2] >>> 10);
      w[i] = (w[i - 16] + s0 + w[i - 7] + s1) | 0;
    }
    let a = h0, b = h1, c = h2, d = h3, e = h4, f = h5, g = h6, h = h7;
    for (let i = 0; i < 64; i++) {
      const S1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
      const ch = (e & f) ^ (~e & g);
      const t1 = (h + S1 + ch + SHA_K[i] + w[i]) | 0;
      const S0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
      const maj = (a & b) ^ (a & c) ^ (b & c);
      const t2 = (S0 + maj) | 0;
      h = g; g = f; f = e; e = (d + t1) | 0;
      d = c; c = b; b = a; a = (t1 + t2) | 0;
    }
    h0 = (h0 + a) | 0; h1 = (h1 + b) | 0; h2 = (h2 + c) | 0; h3 = (h3 + d) | 0;
    h4 = (h4 + e) | 0; h5 = (h5 + f) | 0; h6 = (h6 + g) | 0; h7 = (h7 + h) | 0;
  }
  const out = new Uint8Array(32);
  const ov = new DataView(out.buffer);
  [h0, h1, h2, h3, h4, h5, h6, h7].forEach((v, i) => ov.setInt32(i * 4, v));
  return out;
}

const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

function base58check(s: string): boolean {
  let bytes: number[] = [0];
  for (const ch of s) {
    const v = B58.indexOf(ch);
    if (v < 0) return false;
    let carry = v;
    for (let i = bytes.length - 1; i >= 0; i--) {
      carry += bytes[i] * 58;
      bytes[i] = carry & 0xff;
      carry >>= 8;
    }
    while (carry > 0) { bytes.unshift(carry & 0xff); carry >>= 8; }
  }
  const leadingOnes = [...s].findIndex(c => c !== "1");
  const ones = leadingOnes === -1 ? s.length : leadingOnes;
  const firstNonzero = bytes.findIndex(b => b !== 0);
  const body = bytes.slice(firstNonzero === -1 ? bytes.length : firstNonzero);
  const payload = new Uint8Array(ones + body.length);
  payload.set(body, ones);
  if (payload.length < 5) return false;
  const data = payload.slice(0, payload.length - 4);
  const checksum = payload.slice(payload.length - 4);
  const h = sha256(sha256(data));
  return h[0] === checksum[0] && h[1] === checksum[1] && h[2] === checksum[2] && h[3] === checksum[3];
}

function cryptoWallet(raw: string): boolean {
  if (raw.startsWith("0x") || raw.startsWith("bc1")) return true;
  return base58check(raw);
}

const hasDigit = (raw: string) => /\d/.test(raw);

/** ISO 3779 VIN check digit — mirror of validators::vin. */
function vin(raw: string): boolean {
  if (raw.length !== 17) return false;
  const T: Record<string, number> = {
    A: 1, J: 1, B: 2, K: 2, S: 2, C: 3, L: 3, T: 3, D: 4, M: 4, U: 4,
    E: 5, N: 5, V: 5, F: 6, W: 6, G: 7, P: 7, X: 7, H: 8, Y: 8, R: 9, Z: 9,
  };
  const W = [8, 7, 6, 5, 4, 3, 2, 10, 0, 9, 8, 7, 6, 5, 4, 3, 2];
  let sum = 0;
  for (let i = 0; i < 17; i++) {
    const c = raw[i];
    const v = c >= "0" && c <= "9" ? Number(c) : T[c];
    if (v === undefined) return false;
    sum += v * W[i];
  }
  const r = sum % 11;
  return raw[8] === (r === 10 ? "X" : String(r));
}

/** Codice Fiscale check character — mirror of validators::codice_fiscale. */
function codiceFiscale(raw: string): boolean {
  if (raw.length !== 16) return false;
  const ODD_D = [1, 0, 5, 7, 9, 13, 15, 17, 19, 21];
  const ODD_L = [1, 0, 5, 7, 9, 13, 15, 17, 19, 21, 2, 4, 18, 20, 11, 3, 6, 8, 12, 14, 16, 10, 22, 25, 24, 23];
  let total = 0;
  for (let i = 0; i < 15; i++) {
    const c = raw[i];
    const digit = c >= "0" && c <= "9";
    const letter = c >= "A" && c <= "Z";
    if (!digit && !letter) return false;
    const idx = digit ? c.charCodeAt(0) - 48 : c.charCodeAt(0) - 65;
    total += i % 2 === 0 ? (digit ? ODD_D[idx] : ODD_L[idx]) : idx;
  }
  return raw[15] === String.fromCharCode(65 + (total % 26));
}

/** Polish PESEL weighted mod-10 — mirror of validators::pl_pesel. */
function plPesel(raw: string): boolean {
  const d = digitsOf(raw);
  if (d.length !== 11) return false;
  const w = [1, 3, 7, 9, 1, 3, 7, 9, 1, 3];
  let sum = 0;
  for (let i = 0; i < 10; i++) sum += d[i] * w[i];
  return (10 - (sum % 10)) % 10 === d[10];
}

/** Brazilian CPF check digits — mirror of validators::br_cpf. */
function brCpf(raw: string): boolean {
  const d = digitsOf(raw);
  if (d.length !== 11) return false;
  if (d.every(x => x === d[0])) return false;
  for (const n of [9, 10]) {
    let sum = 0;
    for (let i = 0; i < n; i++) sum += d[i] * (n + 1 - i);
    const r = sum % 11;
    if (d[n] !== (r < 2 ? 0 : 11 - r)) return false;
  }
  return true;
}

/** Spanish DNI/NIE mod-23 check letter — mirror of validators::es_dni. */
function esDni(raw: string): boolean {
  const LETTERS = "TRWAGMYFPDXBNJZSQVHLCKE";
  const body = raw.slice(0, -1);
  const normalized = body[0] === "X" ? "0" + body.slice(1)
    : body[0] === "Y" ? "1" + body.slice(1)
    : body[0] === "Z" ? "2" + body.slice(1)
    : body;
  const n = Number(normalized);
  if (!Number.isInteger(n)) return false;
  return raw[raw.length - 1] === LETTERS[n % 23];
}

/** SSA structural rules — mirror of validators::ssn. */
function ssnStructure(raw: string): boolean {
  const d = digitsOf(raw);
  if (d.length !== 9) return false;
  const area = d[0] * 100 + d[1] * 10 + d[2];
  const group = d[3] * 10 + d[4];
  const serial = d[5] * 1000 + d[6] * 100 + d[7] * 10 + d[8];
  return area !== 0 && area !== 666 && area < 900 && group !== 0 && serial !== 0;
}

/** ICAO 9303 TD3 MRZ line-2 check digits — mirror of validators::mrz_td3. */
function mrzTd3(raw: string): boolean {
  if (raw.length !== 44) return false;
  const val = (c: string): number | null =>
    c >= "0" && c <= "9" ? Number(c) : c === "<" ? 0 : c >= "A" && c <= "Z" ? c.charCodeAt(0) - 55 : null;
  const check = (s: string, expect: string): boolean => {
    const w = [7, 3, 1];
    let sum = 0;
    for (let i = 0; i < s.length; i++) {
      const v = val(s[i]);
      if (v === null) return false;
      sum += v * w[i % 3];
    }
    return String(sum % 10) === expect;
  };
  const composite = raw.slice(0, 10) + raw.slice(13, 20) + raw.slice(21, 43);
  return check(raw.slice(0, 9), raw[9])
    && check(raw.slice(13, 19), raw[19])
    && check(raw.slice(21, 27), raw[27])
    && check(composite, raw[43]);
}

/** Medicare MBI per-position CMS classes — mirror of validators::medicare_mbi. */
function medicareMbi(raw: string): boolean {
  const s = raw.replace(/-/g, "").toUpperCase();
  if (s.length !== 11) return false;
  const letterOk = (c: string) => c >= "A" && c <= "Z" && !"SLOIBZ".includes(c);
  const digit = (c: string) => c >= "0" && c <= "9";
  return s[0] >= "1" && s[0] <= "9"
    && letterOk(s[1]) && (letterOk(s[2]) || digit(s[2])) && digit(s[3])
    && letterOk(s[4]) && (letterOk(s[5]) || digit(s[5])) && digit(s[6])
    && letterOk(s[7]) && letterOk(s[8]) && digit(s[9]) && digit(s[10]);
}

/** Shannon entropy in bits/char — prose ≈2–2.8, generated secrets ≈3.5+. */
function shannonEntropy(s: string): number {
  const counts = new Map<string, number>();
  for (const c of s) counts.set(c, (counts.get(c) ?? 0) + 1);
  const n = [...s].length;
  if (n === 0) return 0;
  let h = 0;
  for (const c of counts.values()) {
    const p = c / n;
    h -= p * Math.log2(p);
  }
  return h;
}

const CREDENTIAL_PLACEHOLDERS = [
  "password", "passwd", "passphrase", "changeme", "change-me", "change_me",
  "example", "sample", "dummy", "placeholder", "redacted", "secret",
  "your-", "your_", "xxxx", "****", "1234567", "abcdefg", "qwerty",
];

/** Mirror of validators::credential_value — see vendetta.rs for the rationale. */
function credentialValue(raw: string): boolean {
  if (raw.length < 8) return false;
  const first = raw[0];
  if ("$%<{[".includes(first) || raw.includes("${") || raw.includes("{{")) return false;
  const lower = raw.toLowerCase();
  if (["true", "false", "null", "none", "nil", "undefined"].includes(lower)) return false;
  if (CREDENTIAL_PLACEHOLDERS.some(p => lower.includes(p))) return false;
  if (!/[\d\W_]/.test(raw)) return false;
  return shannonEntropy(raw) >= 3.0;
}

// ---------------------------------------------------------------------------
// Detection packs (mirror of vendetta::pack_for). `core` and `secrets` are
// the safety floor and cannot be disabled; the rest are user-toggleable via
// Settings (`disabled_packs` setting, hydrated through setDisabledPacks).
// ---------------------------------------------------------------------------

export const TOGGLEABLE_PACKS: { id: string; name: string; hint: string }[] = [
  { id: "payment", name: "Payment & banking", hint: "cards · IBAN · routing · SWIFT · EIN" },
  { id: "identity", name: "Identity documents", hint: "DOB · passport + MRZ · driver's license · VIN" },
  { id: "national-id", name: "National IDs", hint: "ITIN · SIN · NHS · NINO · TFN · Aadhaar · CF · DNI · CPF · PESEL" },
  { id: "medical", name: "Medical", hint: "MRN · NPI · DEA · member IDs · Medicare MBI" },
  { id: "legal", name: "Legal", hint: "case & docket numbers" },
  { id: "network", name: "Network & crypto", hint: "IPs · MAC · wallets" },
];

export function packFor(kind: Kind): string {
  switch (kind) {
    case "CREDITCARD": case "IBAN": case "US_BANK": case "SWIFT_BIC": case "EIN":
      return "payment";
    case "DOB": case "PASSPORT": case "DRIVERS_LICENSE": case "VIN": case "MRZ":
      return "identity";
    case "US_ITIN": case "CA_SIN": case "UK_NHS": case "UK_NINO": case "AU_TFN": case "AADHAAR":
    case "IT_CF": case "ES_DNI": case "BR_CPF": case "PL_PESEL":
      return "national-id";
    case "MRN": case "NPI": case "DEA": case "HEALTH_ID": case "MEDICARE_MBI":
      return "medical";
    case "CASE_NO":
      return "legal";
    case "CRYPTO_WALLET": case "IPV6": case "MAC_ADDRESS": case "IP":
      return "network";
    case "APIKEY": case "JWT": case "PRIVATE_KEY": case "CONNECTION_STRING":
    case "CREDENTIAL":
      return "secrets";
    default:
      return "core";
  }
}

let DISABLED_PACKS = new Set<string>();

export function setDisabledPacks(ids: string[]): void {
  DISABLED_PACKS = new Set(ids.filter(id => TOGGLEABLE_PACKS.some(p => p.id === id)));
}

/// Mirror of `vendetta::confidence_for` — keep the tiers in lockstep so the
/// live preview shows the same confidence the engine records.
export function confidenceFor(kind: Kind): number {
  switch (kind) {
    case "CREDITCARD": case "IBAN": case "US_BANK": case "SWIFT_BIC":
    case "NPI": case "DEA": case "CA_SIN": case "UK_NHS": case "AU_TFN":
    case "AADHAAR": case "IT_CF": case "ES_DNI": case "BR_CPF": case "PL_PESEL": case "SSN": case "IP": case "IPV6": case "CRYPTO_WALLET":
    case "PRIVATE_KEY": case "CONNECTION_STRING": case "CUSTOM": case "VIN":
    case "MRZ":
      return 1.0;
    case "EMAIL": case "URL": case "APIKEY": case "JWT": case "MAC_ADDRESS":
    case "EIN": case "US_ITIN": case "MEDICARE_MBI":
      return 0.95;
    case "DOB": case "PASSPORT": case "DRIVERS_LICENSE": case "MRN":
    case "HEALTH_ID": case "CASE_NO": case "UK_NINO": case "CREDENTIAL":
      return 0.85;
    case "PHONE": case "MONEY": case "ADDRESS": case "EMPID":
      return 0.75;
    default:
      return 0.8;
  }
}

export function validate(kind: Kind, raw: string): boolean {
  switch (kind) {
    case "CREDITCARD": return creditCard(raw);
    case "IBAN": return iban(raw);
    case "US_BANK": return usBank(raw);
    case "SWIFT_BIC": return swiftBic(raw);
    case "NPI": return npi(raw);
    case "DEA": return dea(raw);
    case "CA_SIN": return caSin(raw);
    case "UK_NHS": return ukNhs(raw);
    case "UK_NINO": return ukNino(raw);
    case "AU_TFN": return auTfn(raw);
    case "AADHAAR": return aadhaar(raw);
    case "IT_CF": return codiceFiscale(raw);
    case "ES_DNI": return esDni(raw);
    case "BR_CPF": return brCpf(raw);
    case "PL_PESEL": return plPesel(raw);
    case "DOB": return datePlausible(raw);
    case "SSN": return ssnStructure(raw);
    case "IP": return ipv4Octets(raw);
    case "IPV6": return ipv6Parses(raw);
    case "CRYPTO_WALLET": return cryptoWallet(raw);
    case "CREDENTIAL": return credentialValue(raw);
    case "VIN": return vin(raw);
    case "MRZ": return mrzTd3(raw);
    case "MEDICARE_MBI": return medicareMbi(raw);
    case "PASSPORT":
    case "DRIVERS_LICENSE":
    case "MRN":
    case "HEALTH_ID":
    case "CASE_NO":
      return hasDigit(raw);
    default: return true;
  }
}

// ---------------------------------------------------------------------------
// Custom watchlist (mirror of detect/custom.rs). App.tsx hydrates this from
// the `custom_watchlist` setting on mount; SettingsPanel calls it on save so
// live highlights update without a restart.
// ---------------------------------------------------------------------------

let CUSTOM_RE: RegExp | null = null;

function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export function setCustomTerms(terms: string[]): void {
  const cleaned = terms
    .map(t => t.trim())
    .filter(t => t.length >= 2 && t.length <= 120)
    .slice(0, 200)
    .sort((a, b) => b.length - a.length);
  CUSTOM_RE = cleaned.length
    ? new RegExp(`\\b(?:${cleaned.map(escapeRegExp).join("|")})\\b`, "gi")
    : null;
}

/** Local-only detect used for realtime highlights. Server-side detect owns the real alias map. */
// ---------------------------------------------------------------------------
// Structured-data stage (mirror of detect/structured.rs). Column headers in
// pasted CSV/TSV/semicolon/pipe tables drive detection: every cell under an
// `ssn`/`email`/`card_number`-style header is sensitive even when the bare
// value matches no pattern. Same safety rules as Rust: checksum-invalid
// values under blocking headers downgrade to CUSTOM (alias, never block),
// ragged rows are skipped, 5k-cell cap.
// ---------------------------------------------------------------------------

const STRUCT_DELIMS = [",", "\t", ";", "|"];
const STRUCT_MAX_CELLS = 5000;

function kindForHeader(rawHeader: string): Kind | null {
  const h = rawHeader.trim().toLowerCase().replace(/[^a-z0-9]+/g, " ");
  const words = new Set(h.split(/\s+/).filter(Boolean));
  const has = (w: string) => words.has(w);
  const contains = (sub: string) => h.includes(sub);
  if (contains("social security") || has("ssn")) return "SSN";
  if (contains("credit card") || contains("card number") || has("pan") || has("cc")) return "CREDITCARD";
  if (has("iban")) return "IBAN";
  if (has("routing") || has("aba") || contains("account number") || has("acct")) return "US_BANK";
  if (has("swift") || has("bic")) return "SWIFT_BIC";
  if (has("ein")) return "EIN";
  if (has("itin")) return "US_ITIN";
  if (has("sin")) return "CA_SIN";
  if (has("nhs")) return "UK_NHS";
  if (has("nino") || contains("national insurance")) return "UK_NINO";
  if (has("tfn")) return "AU_TFN";
  if (has("aadhaar") || has("aadhar")) return "AADHAAR";
  if (has("mbi") || contains("medicare")) return "MEDICARE_MBI";
  if (has("npi")) return "NPI";
  if (has("dea")) return "DEA";
  if (has("mrn") || contains("medical record")) return "MRN";
  if (contains("member id") || contains("subscriber") || contains("policy number")) return "HEALTH_ID";
  if (has("vin")) return "VIN";
  if (has("passport")) return "PASSPORT";
  if (contains("driver") || has("dl") || contains("licen")) return "DRIVERS_LICENSE";
  if (has("dob") || contains("birth")) return "DOB";
  if (has("email") || has("mail")) return "EMAIL";
  if (has("phone") || has("mobile") || has("cell") || has("tel") || has("fax")) return "PHONE";
  if (has("ip") || contains("ip address")) return "IP";
  if (has("mac")) return "MAC_ADDRESS";
  if (has("wallet") || has("btc") || has("eth")) return "CRYPTO_WALLET";
  if (contains("address") || has("street")) return "ADDRESS";
  if (has("salary") || has("income") || contains("compensation") || has("wage")) return "MONEY";
  if (has("name") || has("firstname") || has("lastname") || has("surname") || has("fullname")) return "NAME";
  if (contains("case number") || has("docket")) return "CASE_NO";
  if (has("password") || has("secret") || has("token") || contains("api key") || has("apikey")) return "CREDENTIAL";
  return null;
}

const STRUCT_EMPTY = new Set(["", "null", "none", "n/a", "na", "nil", "-", "--", "unknown", "tbd"]);

function headerShape(line: string): { delim: string; ncols: number } | null {
  for (const d of STRUCT_DELIMS) {
    if (!line.includes(d)) continue;
    const cells = line.split(d);
    const plausible = cells.length >= 2
      && cells.every(c => { const t = c.trim(); return t.length > 0 && t.length <= 40 && !t.includes("@"); })
      && cells.some(c => kindForHeader(c) !== null);
    if (plausible) return { delim: d, ncols: cells.length };
  }
  return null;
}

export function structuredSpans(text: string): Span[] {
  const out: Span[] = [];
  let cellsSeen = 0;
  const lines: { line: string; off: number }[] = [];
  let off = 0;
  for (const line of text.split("\n")) {
    lines.push({ line, off });
    off += line.length + 1;
  }
  let i = 0;
  while (i < lines.length) {
    const shape = headerShape(lines[i].line);
    if (!shape) { i++; continue; }
    const { delim, ncols } = shape;
    let blockEnd = i + 1;
    while (blockEnd < lines.length
      && lines[blockEnd].line.split(delim).length === ncols
      && lines[blockEnd].line.trim() !== "") blockEnd++;
    if (blockEnd - i < 2) { i++; continue; }
    const mapped = lines[i].line.split(delim).map(kindForHeader);
    if (mapped.every(k => k === null)) { i = blockEnd; continue; }
    for (let r = i + 1; r < blockEnd; r++) {
      const { line, off: lineOff } = lines[r];
      let cellOff = 0;
      const cells = line.split(delim);
      for (let col = 0; col < cells.length; col++) {
        const cell = cells[col];
        const kind = mapped[col];
        if (kind) {
          if (++cellsSeen > STRUCT_MAX_CELLS) return out;
          const trimmed = cell.trim();
          if (!STRUCT_EMPTY.has(trimmed.toLowerCase())) {
            const k: Kind = validate(kind, trimmed) ? kind : (CRITICAL[kind] ? "CUSTOM" : kind);
            const lead = cell.length - cell.trimStart().length;
            const start = lineOff + cellOff + lead;
            out.push({ start, end: start + trimmed.length, kind: k, raw: trimmed, alias: "", confidence: 0.9 });
          }
        }
        cellOff += cell.length + delim.length;
      }
    }
    i = blockEnd;
  }
  return out;
}

export function detect(text: string): Span[] {
  const hits: Span[] = [];
  for (const p of PATTERNS) {
    if (DISABLED_PACKS.has(packFor(p.kind))) continue;
    p.re.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = p.re.exec(text)) !== null) {
      let start = m.index;
      let end = m.index + m[0].length;
      let raw = m[0];
      if (p.cap && m[1] !== undefined) {
        // Every anchored pattern ends with its capture group (anchor + value),
        // so the value is the trailing substring of the match.
        raw = m[1];
        start = end - raw.length;
      }
      if (validate(p.kind, raw)) {
        hits.push({ start, end, kind: p.kind, raw, alias: "", confidence: confidenceFor(p.kind) });
      }
      if (m.index === p.re.lastIndex) p.re.lastIndex++;
    }
  }
  // Structured (CSV column) spans: after built-ins so regex wins exact ties,
  // before custom. Pack toggles apply per mapped kind, like Rust.
  for (const sp of structuredSpans(text)) {
    if (!DISABLED_PACKS.has(packFor(sp.kind))) hits.push(sp);
  }
  // Custom terms last: stable sort keeps built-ins ahead on exact ties.
  if (CUSTOM_RE) {
    CUSTOM_RE.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = CUSTOM_RE.exec(text)) !== null) {
      hits.push({ start: m.index, end: m.index + m[0].length, kind: "CUSTOM", raw: m[0], alias: "", confidence: 1.0 });
      if (m.index === CUSTOM_RE.lastIndex) CUSTOM_RE.lastIndex++;
    }
  }
  hits.sort((a, b) => a.start - b.start || (b.end - b.start) - (a.end - a.start));
  const out: Span[] = [];
  let cursor = -1;
  for (const h of hits) { if (h.start >= cursor) { out.push(h); cursor = h.end; } }
  const counters: Record<string, number> = {};
  const map = new Map<string, string>();
  for (const h of out) {
    const key = h.kind + "::" + h.raw.toLowerCase();
    if (!map.has(key)) {
      counters[h.kind] = (counters[h.kind] || 0) + 1;
      const idx = String(counters[h.kind]).padStart(2, "0");
      map.set(key, `⟦${LABELS[h.kind]}_${idx}⟧`);
    }
    h.alias = map.get(key)!;
  }
  return out;
}

export type DetectionSource = "regex" | "ner" | "llm";

export function sourceForKind(kind: string): DetectionSource {
  if (kind.endsWith("_LLM")) return "llm";
  if (kind.endsWith("_NER")) return "ner";
  return "regex";
}

export function sourceGlyph(src: DetectionSource): string {
  return src === "regex" ? "∎" : src === "ner" ? "◆" : "✦";
}

export function sourceTooltip(src: DetectionSource): string {
  return src === "regex" ? "Detected by regex pattern"
       : src === "ner"   ? "Detected by GLiNER semantic model"
       :                   "Detected by paranoid LLM scan";
}
