export type Kind =
  | "EMAIL" | "PHONE" | "SSN" | "IP" | "APIKEY" | "URL"
  | "ADDRESS" | "MONEY" | "NAME" | "COMPANY" | "EMPID"
  // Payment / banking
  | "CREDITCARD" | "IBAN" | "US_BANK" | "SWIFT_BIC" | "EIN"
  // Secrets
  | "JWT" | "PRIVATE_KEY"
  // Identity documents
  | "DOB" | "PASSPORT" | "DRIVERS_LICENSE"
  // Medical
  | "MRN" | "NPI" | "DEA" | "HEALTH_ID"
  // Legal
  | "CASE_NO"
  // Crypto / network
  | "CRYPTO_WALLET" | "IPV6"
  // User-defined watchlist
  | "CUSTOM"
  | "PERSON_NER" | "ORG_NER" | "CODENAME_NER" | "LOCATION_NER" | "EMPID_NER";

export interface Span {
  start: number;
  end: number;
  kind: Kind;
  raw: string;
  alias: string;
}

export interface Model {
  id: string;
  name: string;
  provider: string;
  ctx: string;
  flash: string;
  color: string;
}

export interface Conversation {
  id: string;
  title: string;
  time: string;
  pinned?: boolean;
  shield?: boolean;
}

export interface Message {
  id?: string;
  role: "user" | "assistant";
  text: string;
  spans?: Span[];
  streaming?: boolean;
  error?: string;
  aliasedPrompt?: string;
}

export interface BlockReason { kind: string; rule: string; class: string; desc: string }

export interface AuditEntry {
  id: string; ts: string; kind: string; raw_hash: string;
  alias: string; action: string; prev_hash: string; sig: string;
}

export interface AuditMetrics {
  redactions_total: number;
  blocks_total: number;
  classes: number;
  redactions_24h: number;
  redactions_7d: number;
  blocks_7d: number;
}

export interface Tweaks {
  accent: string;
  density: "comfy" | "dense";
  starfield: boolean;
  scanAnim: boolean;
  defaultModelIdx: number;
}

// Externally-tagged shape matching Rust's #[serde(rename_all = "snake_case")] without #[serde(tag)]
// Unit variants serialize as bare strings; struct variants as { variantname: { ...fields } }
export type ModelStatus =
  | "missing"
  | "ready"
  | { downloading: { percent: number } }
  | { error: { msg: string } };

export interface AllModelStatus {
  ner: ModelStatus;
  ner_tokenizer: ModelStatus;
  llm: ModelStatus;
}

export interface ModelProgressEvent {
  id: string;
  done: number;
  total: number;
  percent: number;
}

export type ModelStatusKind = "missing" | "downloading" | "ready" | "error";

export function modelStatusKind(s: ModelStatus): ModelStatusKind {
  if (s === "missing") return "missing";
  if (s === "ready") return "ready";
  if (typeof s === "object" && "downloading" in s) return "downloading";
  return "error";
}
