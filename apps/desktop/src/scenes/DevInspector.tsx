import { useMemo, useState } from "react";
import type { CSSProperties } from "react";
import type { PipelineTrace, StreamTrace, ParanoidTrace } from "../lib/ipc";
import type { Span } from "../lib/types";
import { sourceForKind, sourceGlyph, confidenceFor } from "../lib/vendetta";

/// A single send's full picture, stitched from:
/// - `pipeline`: synchronous PipelineTrace returned on SendMeta.
/// - `stream`: async StreamTrace emitted when the provider stream ends.
/// - `paranoid`: async ParanoidTrace emitted when the Qwen scan ends.
export interface TraceRecord {
  msg_id: string;
  conv_id: string;
  /// Wall-clock timestamp when the frontend received SendMeta. Not the
  /// backend send() entry time — it's a hair later because of IPC latency.
  ts: number;
  /// Original user input (pre-redaction). Kept client-side so the inspector
  /// can diff raw vs aliased; never round-trips back to backend.
  raw_text: string;
  /// Client-side block outcome (if the send was refused for containing a
  /// critical class like SSN or APIKEY). Backend sends blocked too.
  blocked: string | null;
  pipeline: PipelineTrace;
  stream?: StreamTrace;
  paranoid?: ParanoidTrace;
}

interface Props {
  traces: TraceRecord[];
  onClose: () => void;
  onClear: () => void;
}

export function DevInspector({ traces, onClose, onClear }: Props) {
  const [selectedId, setSelectedId] = useState<string | null>(traces[0]?.msg_id ?? null);
  const selected = useMemo(
    () => traces.find(t => t.msg_id === selectedId) ?? traces[0] ?? null,
    [traces, selectedId]
  );

  return (
    <div style={cx.overlay}>
      <div style={cx.shell}>
        <div style={cx.header}>
          <div style={cx.headerLeft}>
            <span style={cx.dot} />
            <span style={cx.title}>VENDETTA INSPECTOR</span>
            <span style={cx.subtitle}>{traces.length} send{traces.length === 1 ? "" : "s"} · ⌘⇧D to toggle</span>
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <button style={cx.btnGhost} onClick={onClear} title="Clear trace history">clear</button>
            <button style={cx.btnGhost} onClick={onClose}>close ×</button>
          </div>
        </div>

        <div style={cx.body}>
          <div style={cx.list}>
            {traces.length === 0 && (
              <div style={cx.empty}>
                <div style={cx.emptyTitle}>NO SENDS YET</div>
                <div style={cx.emptyText}>
                  Transmit a prompt (⌘↵) to capture a full pipeline trace here.
                  Every regex/NER ms, the merged span set, the exact aliased
                  payload that hits the provider, the TTFT, and the paranoid
                  scan will be broken down below.
                </div>
              </div>
            )}
            {traces.map(t => (
              <TraceRow
                key={t.msg_id}
                trace={t}
                active={t.msg_id === selected?.msg_id}
                onClick={() => setSelectedId(t.msg_id)}
              />
            ))}
          </div>

          <div style={cx.detail}>
            {selected ? <TraceDetail record={selected} /> : null}
          </div>
        </div>
      </div>
    </div>
  );
}

function TraceRow({ trace, active, onClick }: { trace: TraceRecord; active: boolean; onClick: () => void }) {
  const t = trace.pipeline;
  const time = new Date(trace.ts).toLocaleTimeString("en-US", { hour12: false });
  const totalMs = (trace.stream?.total_stream_ms ?? 0) + t.total_pre_dispatch_ms;
  const preview = trace.raw_text.slice(0, 60).replace(/\n/g, " ");

  return (
    <button onClick={onClick} style={{ ...cx.row, ...(active ? cx.rowActive : {}) }}>
      <div style={cx.rowHead}>
        <span style={cx.rowTs}>{time}</span>
        <span style={cx.rowModel}>{t.provider || "—"} · {t.model_id || "—"}</span>
        {trace.blocked && <span style={cx.rowBlocked}>BLOCKED</span>}
      </div>
      <div style={cx.rowPreview}>{preview}{trace.raw_text.length > 60 ? "…" : ""}</div>
      <div style={cx.rowChips}>
        <Chip label={`regex ${t.regex_ms}ms`} count={t.regex_spans_count} color="neon" />
        <Chip
          label={`ner ${t.ner_ms}ms`}
          count={t.ner_spans_count}
          color={t.ner_status === "ok" ? "teal" : "muted"}
          tooltip={t.ner_status}
        />
        {(t.structured_spans_count ?? 0) > 0 && (
          <Chip label="csv cols" count={t.structured_spans_count} color="neon"
            tooltip="column-driven hits from pasted tabular data" />
        )}
        {(t.custom_spans_count ?? 0) > 0 && (
          <Chip label="watchlist" count={t.custom_spans_count} color="neon"
            tooltip="custom watchlist terms" />
        )}
        {trace.stream && (
          <Chip
            label={`ttft ${trace.stream.ttft_ms ?? "—"}ms`}
            count={trace.stream.chunks}
            color="neon"
            tooltip={`${trace.stream.bytes}B across ${trace.stream.chunks} chunks`}
          />
        )}
        {trace.paranoid && (
          <Chip
            label={`paranoid ${trace.paranoid.ms}ms`}
            count={trace.paranoid.spans_found}
            color={trace.paranoid.timed_out ? "muted" : "violet"}
          />
        )}
        <Chip label={`total ${totalMs}ms`} color="muted" />
      </div>
    </button>
  );
}

function Chip({
  label, count, color, tooltip
}: {
  label: string;
  count?: number;
  color: "neon" | "teal" | "violet" | "muted";
  tooltip?: string;
}) {
  return (
    <span style={{ ...cx.chip, ...cx[`chip_${color}` as const] }} title={tooltip}>
      {label}
      {typeof count === "number" && <span style={cx.chipCount}>×{count}</span>}
    </span>
  );
}

function TraceDetail({ record }: { record: TraceRecord }) {
  const t = record.pipeline;
  const s = record.stream;
  const p = record.paranoid;

  /// Max ms for the timing bar chart — scales all bars so the longest stage
  /// hits the full width. TTFT dominates in typical sends because the
  /// provider round-trip is ~500–1500 ms.
  const maxMs = Math.max(
    t.regex_ms,
    t.ner_ms,
    t.merge_ms,
    t.alias_ms,
    s?.ttft_ms ?? 0,
    s?.total_stream_ms ?? 0,
    p?.ms ?? 0,
    1
  );

  const tokensPerSec = s && s.total_stream_ms > 0 && s.bytes > 0
    ? ((s.bytes / 4) / (s.total_stream_ms / 1000))
    : null;

  return (
    <div style={cx.detailInner}>
      <Section title="SUMMARY">
        <div style={cx.kvGrid}>
          <Kv k="msg_id" v={record.msg_id} mono />
          <Kv k="conv_id" v={record.conv_id} mono />
          <Kv k="provider" v={`${t.provider || "—"} · ${t.model_id || "—"}`} />
          <Kv k="text length" v={`${t.text_len} chars`} />
          <Kv k="paranoid mode" v={t.paranoid_enabled ? "ON" : "OFF"} />
          <Kv k="blocked" v={record.blocked ?? "no"} />
        </div>
      </Section>

      <Section title="TIMINGS">
        <Bar label="regex" ms={t.regex_ms} max={maxMs} color="#f2ff2b" />
        <Bar label="ner" ms={t.ner_ms} max={maxMs} color="#5eead4"
          extra={t.ner_status !== "ok" ? t.ner_status : undefined} />
        <Bar label="merge" ms={t.merge_ms} max={maxMs} color="#cbd5e1" />
        <Bar label="alias" ms={t.alias_ms} max={maxMs} color="#cbd5e1" />
        <Bar label="pre-dispatch total" ms={t.total_pre_dispatch_ms} max={maxMs} color="#a78bfa" />
        {s && (
          <>
            <Bar label="ttft" ms={s.ttft_ms ?? 0} max={maxMs} color="#fb7185"
              extra={s.ttft_ms === null ? "no first token" : undefined} />
            <Bar label="stream total" ms={s.total_stream_ms} max={maxMs} color="#fb7185" />
          </>
        )}
        {p && (
          <Bar label="paranoid" ms={p.ms} max={maxMs} color="#c084fc"
            extra={p.timed_out ? "timed out" : undefined} />
        )}
        {s && tokensPerSec !== null && (
          <div style={cx.hint}>≈ {tokensPerSec.toFixed(1)} tokens/sec · {s.chunks} chunks · {s.bytes} bytes</div>
        )}
        {t.ner_error && <div style={cx.errHint}>NER error: {t.ner_error}</div>}
        {s?.error && <div style={cx.errHint}>Stream error: {s.error}</div>}
        {p?.error && <div style={cx.errHint}>Paranoid error: {p.error}</div>}
      </Section>

      <Section title={`DETECTIONS — regex ${t.regex_spans_count} · structured ${t.structured_spans_count ?? 0} · custom ${t.custom_spans_count ?? 0} · ner ${t.ner_spans_count} · merged ${t.merged_spans_count}`}>
        <div style={cx.spanCols}>
          <SpanColumn title="regex only" spans={t.regex_spans} emptyHint="no regex hits" />
          <SpanColumn title="ner only" spans={t.ner_spans} emptyHint={t.ner_status !== "ok" ? `ner ${t.ner_status}` : "no ner hits"} />
        </div>
        <div style={cx.hint}>
          After merge: regex wins on overlap. Dropped NER spans aren't a bug —
          regex is authoritative for well-formed PII (emails, phones, SSNs).
        </div>
      </Section>

      <Section title="WIRE PAYLOAD — what the provider sees">
        <div style={cx.hint}>
          This is the aliased prompt that hit {t.provider || "the provider"}.
          Copy it into any LLM playground to reproduce the upstream behavior.
        </div>
        <CopyBlock text={t.aliased_prompt} placeholder="(empty)" maxLines={12} />
      </Section>

      <Section title="RAW INPUT — your original text">
        <CopyBlock text={record.raw_text} maxLines={8} />
      </Section>

      {s && (
        <>
          <Section title="RESPONSE (aliased) — what the model produced">
            <div style={cx.hint}>
              Pre-rehydration. If the model botched the ⟦…⟧ markers, you'll
              see it here: missing brackets, extra text inside them, rewrites.
            </div>
            <CopyBlock text={s.response_aliased} placeholder={s.error ?? "(empty — stream error)"} maxLines={12} />
          </Section>
          <Section title="RESPONSE (rehydrated) — what the user sees">
            <CopyBlock text={s.response_rehydrated} placeholder={s.error ?? "(empty)"} maxLines={12} />
          </Section>
        </>
      )}

      {p && p.spans_found === 0 && !p.error && !p.timed_out && (
        <Section title="PARANOID SCAN">
          <div style={cx.hint}>
            Qwen scanned the original (non-aliased) text in {p.ms}ms and found
            no semantically-sensitive content. Paranoid covers things like
            "layoffs", "legal hold" — phrases with no token signature.
          </div>
        </Section>
      )}
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={cx.section}>
      <div style={cx.sectionTitle}>{title}</div>
      {children}
    </div>
  );
}

function Kv({ k, v, mono }: { k: string; v: string; mono?: boolean }) {
  return (
    <>
      <div style={cx.kvK}>{k}</div>
      <div style={{ ...cx.kvV, ...(mono ? { fontFamily: "'JetBrains Mono', monospace" } : {}) }}>{v}</div>
    </>
  );
}

function Bar({ label, ms, max, color, extra }: { label: string; ms: number; max: number; color: string; extra?: string }) {
  const pct = max > 0 ? Math.min(100, (ms / max) * 100) : 0;
  return (
    <div style={cx.barRow}>
      <div style={cx.barLabel}>{label}</div>
      <div style={cx.barTrack}>
        <div style={{ ...cx.barFill, width: `${pct}%`, background: color, boxShadow: `0 0 8px ${color}` }} />
      </div>
      <div style={cx.barMs}>{ms}ms {extra && <span style={cx.barExtra}>· {extra}</span>}</div>
    </div>
  );
}

function SpanColumn({ title, spans, emptyHint }: { title: string; spans: Span[]; emptyHint: string }) {
  return (
    <div style={cx.spanCol}>
      <div style={cx.spanColTitle}>{title}</div>
      {spans.length === 0 ? (
        <div style={cx.spanEmpty}>{emptyHint}</div>
      ) : (
        spans.map((s, i) => {
          const conf = s.confidence ?? confidenceFor(s.kind);
          return (
            <div key={i} style={cx.spanRow}>
              <span style={cx.spanGlyph}>{sourceGlyph(sourceForKind(s.kind))}</span>
              <span style={cx.spanKind}>{s.kind}</span>
              <span style={cx.spanRaw}>{s.raw}</span>
              <span
                title="detection confidence"
                style={{ ...cx.spanPos, color: conf >= 0.95 ? "#7cffb2" : conf >= 0.85 ? "var(--neon)" : "#fbbf24" }}
              >{Math.round(conf * 100)}%</span>
              <span style={cx.spanPos}>[{s.start}–{s.end}]</span>
            </div>
          );
        })
      )}
    </div>
  );
}

function CopyBlock({ text, placeholder, maxLines }: { text: string; placeholder?: string; maxLines?: number }) {
  const [copied, setCopied] = useState(false);
  const display = text.length > 0 ? text : (placeholder ?? "");

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    } catch {}
  };

  return (
    <div style={cx.copyBlock}>
      <pre style={{
        ...cx.copyPre,
        maxHeight: maxLines ? `${maxLines * 1.5}em` : undefined
      }}>{display}</pre>
      <button onClick={copy} style={cx.copyBtn} disabled={text.length === 0}>
        {copied ? "✓ copied" : "copy"}
      </button>
    </div>
  );
}

const cx: Record<string, CSSProperties> = {
  overlay: {
    position: "fixed", inset: 0, background: "rgba(5, 6, 10, 0.82)",
    backdropFilter: "blur(18px)", zIndex: 200, display: "flex",
    alignItems: "center", justifyContent: "center", padding: 24,
  },
  shell: {
    width: "min(1280px, 100%)", maxHeight: "92vh",
    background: "rgba(10, 12, 20, 0.98)",
    border: "1px solid rgba(242,255,43,0.25)",
    borderRadius: 14,
    boxShadow: "0 0 60px rgba(242,255,43,0.15), 0 20px 60px rgba(0,0,0,0.6)",
    display: "flex", flexDirection: "column", overflow: "hidden",
  },
  header: {
    display: "flex", justifyContent: "space-between", alignItems: "center",
    padding: "12px 18px", borderBottom: "1px solid var(--line)",
    background: "rgba(242,255,43,0.04)",
  },
  headerLeft: { display: "flex", alignItems: "center", gap: 12 },
  dot: {
    width: 8, height: 8, borderRadius: 99, background: "var(--neon)",
    boxShadow: "0 0 10px var(--neon)", animation: "pulse 1.5s infinite",
  },
  title: {
    fontFamily: "'JetBrains Mono', monospace", fontSize: 13, letterSpacing: 3,
    color: "var(--neon)", fontWeight: 600,
  },
  subtitle: {
    fontFamily: "'JetBrains Mono', monospace", fontSize: 10, letterSpacing: 2,
    color: "var(--ink-3)",
  },
  btnGhost: {
    background: "transparent", border: "1px solid rgba(255,255,255,0.1)",
    color: "var(--ink-1)", padding: "4px 10px", fontSize: 11,
    fontFamily: "'JetBrains Mono', monospace", letterSpacing: 1,
    borderRadius: 4, cursor: "pointer",
  },
  body: { flex: 1, display: "flex", minHeight: 0 },
  list: {
    width: 360, borderRight: "1px solid var(--line)", overflowY: "auto",
    display: "flex", flexDirection: "column",
  },
  detail: { flex: 1, overflowY: "auto" },
  detailInner: { padding: "12px 18px 24px" },
  empty: { padding: "24px 18px", color: "var(--ink-3)" },
  emptyTitle: {
    fontFamily: "'JetBrains Mono', monospace", fontSize: 11, letterSpacing: 2,
    marginBottom: 8,
  },
  emptyText: { fontSize: 12, lineHeight: 1.6, color: "var(--ink-2)" },
  row: {
    textAlign: "left", background: "transparent", border: "none",
    borderBottom: "1px solid var(--line)",
    padding: "10px 14px", cursor: "pointer", color: "var(--ink-0)",
    display: "flex", flexDirection: "column", gap: 5,
  },
  rowActive: { background: "rgba(242,255,43,0.06)", borderLeft: "2px solid var(--neon)" },
  rowHead: { display: "flex", gap: 10, alignItems: "center", fontSize: 10 },
  rowTs: { fontFamily: "'JetBrains Mono', monospace", color: "var(--ink-3)", letterSpacing: 1 },
  rowModel: { fontFamily: "'JetBrains Mono', monospace", color: "var(--ink-1)" },
  rowBlocked: {
    marginLeft: "auto",
    fontFamily: "'JetBrains Mono', monospace", fontSize: 9, letterSpacing: 2,
    color: "#fb7185", padding: "2px 6px", borderRadius: 3,
    background: "rgba(251,113,133,0.12)",
  },
  rowPreview: {
    fontSize: 12, color: "var(--ink-1)",
    overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
  },
  rowChips: { display: "flex", flexWrap: "wrap", gap: 4 },
  chip: {
    fontFamily: "'JetBrains Mono', monospace", fontSize: 9, letterSpacing: 0.5,
    padding: "2px 6px", borderRadius: 3, border: "1px solid transparent",
  },
  chip_neon: {
    color: "#f2ff2b", background: "rgba(242,255,43,0.08)",
    borderColor: "rgba(242,255,43,0.3)",
  },
  chip_teal: {
    color: "#5eead4", background: "rgba(94,234,212,0.08)",
    borderColor: "rgba(94,234,212,0.3)",
  },
  chip_violet: {
    color: "#c084fc", background: "rgba(192,132,252,0.08)",
    borderColor: "rgba(192,132,252,0.3)",
  },
  chip_muted: {
    color: "var(--ink-2)", background: "rgba(255,255,255,0.03)",
    borderColor: "rgba(255,255,255,0.08)",
  },
  chipCount: { marginLeft: 4, opacity: 0.6 },
  section: {
    marginBottom: 18, padding: "12px 14px",
    background: "rgba(255,255,255,0.015)",
    border: "1px solid var(--line)", borderRadius: 8,
  },
  sectionTitle: {
    fontFamily: "'JetBrains Mono', monospace", fontSize: 10, letterSpacing: 2,
    color: "var(--neon)", marginBottom: 10, paddingBottom: 6,
    borderBottom: "1px solid rgba(242,255,43,0.15)",
  },
  kvGrid: {
    display: "grid", gridTemplateColumns: "max-content 1fr", gap: "6px 14px",
    fontSize: 12,
  },
  kvK: {
    fontFamily: "'JetBrains Mono', monospace", fontSize: 10, letterSpacing: 1,
    color: "var(--ink-3)", textTransform: "uppercase",
  },
  kvV: { color: "var(--ink-0)", wordBreak: "break-all" },
  barRow: {
    display: "grid",
    gridTemplateColumns: "140px 1fr 160px",
    alignItems: "center", gap: 10, marginBottom: 5,
  },
  barLabel: {
    fontFamily: "'JetBrains Mono', monospace", fontSize: 10, letterSpacing: 1,
    color: "var(--ink-2)",
  },
  barTrack: {
    height: 10, background: "rgba(255,255,255,0.04)", borderRadius: 4,
    overflow: "hidden",
  },
  barFill: { height: "100%", transition: "width 0.3s" },
  barMs: {
    fontFamily: "'JetBrains Mono', monospace", fontSize: 11,
    color: "var(--ink-1)", textAlign: "right",
  },
  barExtra: { color: "var(--ink-3)", marginLeft: 4 },
  hint: {
    fontSize: 11, color: "var(--ink-3)", marginTop: 6, lineHeight: 1.5,
    fontStyle: "italic",
  },
  errHint: {
    fontSize: 11, color: "#fb7185", marginTop: 6,
    fontFamily: "'JetBrains Mono', monospace",
  },
  spanCols: { display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 },
  spanCol: {
    border: "1px solid var(--line)", borderRadius: 6, padding: 8,
    background: "rgba(255,255,255,0.015)",
  },
  spanColTitle: {
    fontFamily: "'JetBrains Mono', monospace", fontSize: 10, letterSpacing: 2,
    color: "var(--ink-2)", marginBottom: 6,
  },
  spanEmpty: {
    fontFamily: "'JetBrains Mono', monospace", fontSize: 10,
    color: "var(--ink-3)",
  },
  spanRow: {
    display: "grid",
    gridTemplateColumns: "16px 90px 1fr 70px",
    gap: 8, alignItems: "center", fontSize: 11, padding: "3px 0",
    borderBottom: "1px dashed rgba(255,255,255,0.05)",
  },
  spanGlyph: { textAlign: "center", color: "var(--neon)" },
  spanKind: {
    fontFamily: "'JetBrains Mono', monospace", fontSize: 10,
    color: "var(--ink-2)", letterSpacing: 1,
  },
  spanRaw: { color: "var(--ink-0)", wordBreak: "break-all" },
  spanPos: {
    fontFamily: "'JetBrains Mono', monospace", fontSize: 10,
    color: "var(--ink-3)", textAlign: "right",
  },
  copyBlock: { position: "relative" },
  copyPre: {
    margin: 0, padding: "10px 12px",
    background: "rgba(0,0,0,0.35)",
    border: "1px solid rgba(255,255,255,0.08)", borderRadius: 4,
    fontFamily: "'JetBrains Mono', monospace", fontSize: 11, lineHeight: 1.5,
    color: "var(--ink-0)", whiteSpace: "pre-wrap", wordBreak: "break-word",
    overflowY: "auto",
  },
  copyBtn: {
    position: "absolute", top: 6, right: 6,
    background: "rgba(10,12,20,0.9)",
    border: "1px solid rgba(242,255,43,0.3)",
    color: "var(--neon)", padding: "3px 8px", fontSize: 10,
    fontFamily: "'JetBrains Mono', monospace", letterSpacing: 1,
    borderRadius: 3, cursor: "pointer",
  },
};
