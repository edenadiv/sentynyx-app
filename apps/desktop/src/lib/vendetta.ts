import type { Kind, Span } from "./types";

const PATTERNS: { kind: Kind; re: RegExp }[] = [
  { kind: "EMAIL",   re: /\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b/g },
  { kind: "PHONE",   re: /\b(?:\+?\d{1,3}[\s.-]?)?(?:\(?\d{3}\)?[\s.-]?)\d{3}[\s.-]?\d{4}\b/g },
  { kind: "SSN",     re: /\b\d{3}-\d{2}-\d{4}\b/g },
  { kind: "IP",      re: /\b(?:\d{1,3}\.){3}\d{1,3}\b/g },
  { kind: "APIKEY",  re: /\b(?:sk-|pk_|AKIA|ghp_|xox[baprs]-)[A-Za-z0-9_\-]{10,}\b/g },
  { kind: "URL",     re: /\bhttps?:\/\/[^\s)]+/g },
  { kind: "ADDRESS", re: /\b\d{1,5}\s+[A-Z][a-zA-Z]+(?:\s+[A-Z][a-zA-Z]+){0,3}\s+(?:Street|St|Avenue|Ave|Road|Rd|Blvd|Lane|Ln|Drive|Dr|Court|Ct|Way)\b/g },
  { kind: "MONEY",   re: /\$\s?\d{1,3}(?:,\d{3})+(?:\.\d+)?|\$\s?\d{4,}(?:\.\d+)?/g },
  { kind: "NAME",    re: /\b(?:Sarah Chen|Marcus Rodriguez|Elena Volkov|James Patterson|Priya Shah|Nikolai Ivanov|David Kim|Anna Müller)\b/g },
  { kind: "COMPANY", re: /\b(?:Project Helios|Project Orion|Northwind Capital|Halcyon Labs|Blackbird Initiative|Atlas Holdings|Meridian Pharma)\b/g },
  { kind: "EMPID",   re: /\bEMP-\d{4,6}\b/g },
];

export const LABELS: Record<Kind, string> = {
  EMAIL: "email", PHONE: "phone", SSN: "ssn", IP: "ip",
  APIKEY: "api-key", URL: "url", ADDRESS: "address", MONEY: "amount",
  NAME: "person", COMPANY: "entity", EMPID: "employee-id",
  PERSON_NER: "person", ORG_NER: "entity", CODENAME_NER: "codename",
  LOCATION_NER: "location", EMPID_NER: "employee-id",
};

// Keep these entries in sync with `vendetta::is_critical` in
// apps/desktop/src-tauri/src/vendetta.rs — that function is the egress source
// of truth; this map only exists for the instant client-side pre-flash so we
// avoid a round-trip before showing the PolicyViolation scene.
export const CRITICAL: Partial<Record<Kind, { name: string; class: string; desc: string }>> = {
  SSN: {
    name: "SSN in outbound payload",
    class: "PII_LEVEL_3",
    desc: "Social Security Numbers are prohibited from egress to any third-party model endpoint under HALCYON-SEC-08. Tokenize with PII_LEVEL_3 vault or use Sentynyx Local.",
  },
  APIKEY: {
    name: "API key in outbound payload",
    class: "PII_LEVEL_2",
    desc: "Exposed API keys grant immediate access to provider accounts and billing surface. Rotate the key at its issuer and move it to a server-side secret manager before retrying.",
  },
};

/** Local-only detect used for realtime highlights. Server-side detect owns the real alias map. */
export function detect(text: string): Span[] {
  const hits: Span[] = [];
  for (const p of PATTERNS) {
    p.re.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = p.re.exec(text)) !== null) {
      hits.push({ start: m.index, end: m.index + m[0].length, kind: p.kind, raw: m[0], alias: "" });
      if (m.index === p.re.lastIndex) p.re.lastIndex++;
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
      map.set(key, `\u27E6${LABELS[h.kind]}_${idx}\u27E7`);
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
