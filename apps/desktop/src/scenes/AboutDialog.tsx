import { useEffect, useState } from "react";
import type { CSSProperties } from "react";
import { systemIpc, type SystemStats } from "../lib/ipc";
import type { TraceRecord } from "./DevInspector";
import type { AllModelStatus } from "../lib/types";
import { modelStatusKind } from "../lib/types";

interface Props {
  onClose: () => void;
  /// Most-recent send trace — used to show stage timings so the About
  /// dialog doubles as a health-at-a-glance panel. `undefined` on first
  /// launch before any send.
  lastTrace?: TraceRecord;
  /// Current GGUF + ONNX model status so admins see what's loaded in RAM.
  modelsStatus: AllModelStatus | null;
}

/// Compact about-the-process dialog: version + live RSS + uptime + model
/// load state + most-recent send breakdown + keybinding cheat sheet.
/// Refreshes system stats every 2 s while open.
export function AboutDialog({ onClose, lastTrace, modelsStatus }: Props) {
  const [stats, setStats] = useState<SystemStats | null>(null);

  useEffect(() => {
    let mounted = true;
    const pull = () => {
      systemIpc.stats()
        .then(s => { if (mounted) setStats(s); })
        .catch(() => {});
    };
    pull();
    const iv = setInterval(pull, 2000);
    return () => { mounted = false; clearInterval(iv); };
  }, []);

  useEffect(() => {
    const h = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [onClose]);

  return (
    <div style={cx.overlay} onClick={onClose}>
      <div style={cx.modal} onClick={e => e.stopPropagation()}>
        <div style={cx.header}>
          <div style={cx.logoBox}>
            <span style={cx.logoDot} />
            <span style={cx.logo}>SENTYNYX</span>
          </div>
          <button style={cx.closeBtn} onClick={onClose}>×</button>
        </div>

        <div style={cx.body}>
          <Section title="BUILD">
            <Kv k="version" v={stats?.version ?? "…"} mono />
            <Kv k="pid" v={stats?.pid?.toString() ?? "…"} mono />
            <Kv k="uptime" v={stats ? fmtUptime(stats.uptime_sec) : "…"} />
            <Kv k="rss" v={stats ? `${stats.rss_mb} MB` : "…"} accent />
          </Section>

          <Section title="MODELS LOADED">
            {modelsStatus ? (
              <>
                <ModelRow name="GLiNER (NER)" status={modelStatusKind(modelsStatus.ner)} detail="~280 MB · ONNX" />
                <ModelRow name="GLiNER tokenizer" status={modelStatusKind(modelsStatus.ner_tokenizer)} detail="~2 MB" />
                <ModelRow name="Qwen 2.5 0.5B (paranoid + local)" status={modelStatusKind(modelsStatus.llm)} detail="~470 MB · GGUF Q4_K_M" />
              </>
            ) : (
              <div style={cx.hint}>Loading model status…</div>
            )}
          </Section>

          <Section title="LAST SEND">
            {lastTrace ? (
              <>
                <Kv k="msg_id" v={lastTrace.msg_id.slice(0, 8) + "…"} mono />
                <Kv k="model" v={`${lastTrace.pipeline.provider} · ${lastTrace.pipeline.model_id}`} />
                <Kv k="text" v={`${lastTrace.pipeline.text_len} chars`} />
                <Kv k="regex" v={`${lastTrace.pipeline.regex_ms} ms · ${lastTrace.pipeline.regex_spans_count} span(s)`} mono />
                <Kv k="ner" v={`${lastTrace.pipeline.ner_ms} ms · ${lastTrace.pipeline.ner_spans_count} span(s) · ${lastTrace.pipeline.ner_status}`} mono />
                <Kv k="merge+alias" v={`${lastTrace.pipeline.merge_ms + lastTrace.pipeline.alias_ms} ms`} mono />
                <Kv k="pre-dispatch" v={`${lastTrace.pipeline.total_pre_dispatch_ms} ms total`} mono accent />
                {lastTrace.stream && (
                  <>
                    <Kv k="ttft" v={`${lastTrace.stream.ttft_ms ?? "—"} ms`} mono />
                    <Kv k="stream total" v={`${lastTrace.stream.total_stream_ms} ms · ${lastTrace.stream.chunks} chunks · ${lastTrace.stream.bytes} bytes`} mono />
                  </>
                )}
                {lastTrace.paranoid && (
                  <Kv k="paranoid" v={`${lastTrace.paranoid.ms} ms · ${lastTrace.paranoid.spans_found} hit(s)${lastTrace.paranoid.timed_out ? " · TIMED OUT" : ""}`} mono />
                )}
              </>
            ) : (
              <div style={cx.hint}>No sends yet. Transmit a prompt and reopen to see the full breakdown.</div>
            )}
          </Section>

          <Section title="KEYBINDINGS">
            <div style={cx.kbGrid}>
              <Kb k="⌘↵" v="Transmit" />
              <Kb k="⌘K" v="Command palette" />
              <Kb k="⌘O" v="Orbital model picker" />
              <Kb k="⌘M" v="Consensus arena" />
              <Kb k="⌘D" v="Compliance dashboard" />
              <Kb k="⌘⇧D" v="Dev inspector" />
              <Kb k="⌘⇧I" v="About (this dialog)" />
              <Kb k="⌘," v="Settings" />
              <Kb k="ESC" v="Dismiss" />
            </div>
          </Section>
        </div>

        <div style={cx.footer}>
          Privacy perimeter for every LLM. © 2026 Sentynyx · built with Tauri, Rust, React.
        </div>
      </div>
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

function Kv({ k, v, mono, accent }: { k: string; v: string; mono?: boolean; accent?: boolean }) {
  return (
    <div style={cx.kvRow}>
      <div style={cx.kvK}>{k}</div>
      <div style={{
        ...cx.kvV,
        ...(mono ? { fontFamily: "'JetBrains Mono', monospace" } : {}),
        ...(accent ? { color: "var(--neon)" } : {}),
      }}>{v}</div>
    </div>
  );
}

function ModelRow({ name, status, detail }: { name: string; status: string; detail: string }) {
  const glyph = status === "ready" ? "●" : status === "downloading" ? "◐" : status === "error" ? "⚠" : "○";
  const color = status === "ready" ? "var(--neon)"
    : status === "downloading" ? "#fbbf24"
    : status === "error" ? "#fb7185"
    : "var(--ink-3)";
  const label = status === "ready" ? "loaded" : status;
  return (
    <div style={cx.kvRow}>
      <div style={{ ...cx.kvK, color }}>{glyph} {label}</div>
      <div style={cx.kvV}>
        {name} <span style={cx.modelDetail}>{detail}</span>
      </div>
    </div>
  );
}

function Kb({ k, v }: { k: string; v: string }) {
  return (
    <div style={cx.kbRow}>
      <kbd style={cx.kbd}>{k}</kbd>
      <span style={cx.kbV}>{v}</span>
    </div>
  );
}

function fmtUptime(sec: number): string {
  if (sec < 60) return `${sec}s`;
  if (sec < 3600) return `${Math.floor(sec / 60)}m ${sec % 60}s`;
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  return `${h}h ${m}m`;
}

const cx: Record<string, CSSProperties> = {
  overlay: {
    position: "fixed", inset: 0, zIndex: 400,
    background: "rgba(5, 6, 10, 0.78)", backdropFilter: "blur(14px)",
    display: "flex", alignItems: "center", justifyContent: "center",
    animation: "fadeIn 0.2s",
  },
  modal: {
    width: "min(580px, 94%)", maxHeight: "88vh",
    background: "rgba(10, 12, 20, 0.98)",
    border: "1px solid rgba(242,255,43,0.2)",
    borderRadius: 14,
    boxShadow: "0 0 50px rgba(242,255,43,0.12), 0 18px 48px rgba(0,0,0,0.6)",
    display: "flex", flexDirection: "column", overflow: "hidden",
    animation: "modalIn 0.25s",
  },
  header: {
    display: "flex", justifyContent: "space-between", alignItems: "center",
    padding: "12px 18px",
    borderBottom: "1px solid var(--line)",
    background: "rgba(242,255,43,0.04)",
  },
  logoBox: { display: "flex", alignItems: "center", gap: 12 },
  logoDot: {
    width: 10, height: 10, borderRadius: 99, background: "var(--neon)",
    boxShadow: "0 0 14px var(--neon)", animation: "pulse 1.6s infinite",
  },
  logo: {
    fontFamily: "'JetBrains Mono', monospace",
    fontSize: 14, fontWeight: 700, letterSpacing: 5,
    color: "var(--neon)",
  },
  closeBtn: {
    background: "transparent", border: "none", color: "var(--ink-2)",
    fontSize: 22, cursor: "pointer", lineHeight: 1,
    padding: "0 6px",
  },
  body: { flex: 1, overflowY: "auto", padding: "12px 18px 6px" },
  section: {
    marginBottom: 14, padding: "10px 12px",
    background: "rgba(255,255,255,0.015)",
    border: "1px solid var(--line)",
    borderRadius: 8,
  },
  sectionTitle: {
    fontFamily: "'JetBrains Mono', monospace",
    fontSize: 10, letterSpacing: 2.5, color: "var(--neon)",
    marginBottom: 8, paddingBottom: 6,
    borderBottom: "1px solid rgba(242,255,43,0.12)",
  },
  kvRow: {
    display: "grid", gridTemplateColumns: "120px 1fr",
    gap: 10, padding: "3px 0",
    fontSize: 12,
  },
  kvK: {
    fontFamily: "'JetBrains Mono', monospace",
    fontSize: 10, letterSpacing: 1.5,
    color: "var(--ink-3)", textTransform: "uppercase",
  },
  kvV: { color: "var(--ink-0)", wordBreak: "break-all" },
  modelDetail: {
    fontFamily: "'JetBrains Mono', monospace",
    fontSize: 10, color: "var(--ink-3)", marginLeft: 6,
  },
  kbGrid: {
    display: "grid", gridTemplateColumns: "1fr 1fr", gap: "4px 14px",
  },
  kbRow: {
    display: "flex", alignItems: "center", gap: 10,
    padding: "3px 0", fontSize: 11,
  },
  kbd: {
    display: "inline-block", minWidth: 32,
    padding: "2px 7px",
    background: "rgba(0,0,0,0.4)",
    border: "1px solid rgba(255,255,255,0.1)",
    borderRadius: 3,
    fontFamily: "'JetBrains Mono', monospace",
    fontSize: 10, color: "var(--neon)",
    textAlign: "center",
  },
  kbV: { color: "var(--ink-1)" },
  hint: { fontSize: 11, color: "var(--ink-3)", fontStyle: "italic" },
  footer: {
    padding: "8px 18px 10px",
    borderTop: "1px solid var(--line)",
    fontSize: 10, color: "var(--ink-3)",
    fontFamily: "'JetBrains Mono', monospace",
    letterSpacing: 1,
    textAlign: "center",
  },
};
