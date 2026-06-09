import type { CSSProperties } from "react";
import type { Model, ModelStatus } from "../lib/types";
import { modelStatusKind } from "../lib/types";

interface Props {
  model: Model;
  /// How many API-key-backed providers the user has configured. The backend
  /// ships this from `list_configured_providers`.
  configuredProviders: number;
  /// Current status of the Qwen GGUF (missing / downloading / ready).
  llmStatus: ModelStatus | null;
  onOpenSettings: () => void;
  onOpenModels: () => void;
  /// Launches the guided tour (the walkable tutorial).
  onStartTour?: () => void;
}

/// Shown in the empty-state above the Transcript until the user sends their
/// first message. Three live-status rows — no lies. Dismisses automatically
/// once `messages.length > 0` (the EmptyState stops rendering).
export function OnboardingCard({
  model, configuredProviders, llmStatus, onOpenSettings, onOpenModels, onStartTour,
}: Props) {
  const hasKey = configuredProviders > 0;
  const localReady = llmStatus !== null && modelStatusKind(llmStatus) === "ready";
  const localDownloading = llmStatus !== null && modelStatusKind(llmStatus) === "downloading";
  const isOllama = model.id.startsWith("ollama:");
  // Ollama + the bundled on-device model need no API key. An Ollama model is
  // only in the picker because it was discovered running, so it's send-ready.
  const usingLocal = model.id === "sentynyx-local" || isOllama;
  const canSend = hasKey || localReady || isOllama;

  return (
    <div style={cx.card}>
      <div style={cx.header}>
        <span style={cx.dot} />
        <span style={cx.title}>QUICK START</span>
        <span style={cx.subtle}>⌘⇧D for dev inspector · ⌘↵ to send</span>
      </div>

      <div style={cx.rows}>
        <Row
          state="ok"
          label={`Model`}
          value={`${model.name} · ${model.provider}`}
          hint="Swap via ⌘O"
        />

        <Row
          state={hasKey ? "ok" : (usingLocal ? "ok" : "warn")}
          label="Credentials"
          value={
            isOllama
              ? "Ollama (local) — no API key needed"
              : usingLocal
                ? "On-device — no API key needed"
                : hasKey
                  ? `${configuredProviders} provider${configuredProviders === 1 ? "" : "s"} configured`
                  : "Not configured"
          }
          hint={hasKey || usingLocal ? undefined : "Press ⌘, to add a key"}
          onClick={hasKey || usingLocal ? undefined : onOpenSettings}
        />

        <Row
          state={
            localReady ? "ok"
            : localDownloading ? "progress"
            : "neutral"
          }
          label="Sentynyx Local"
          value={
            localReady ? "Qwen 2.5 0.5B ready · on-device chat available"
            : localDownloading ? "Downloading…"
            : "Not installed — optional, use for fully-offline sends"
          }
          hint={localReady || localDownloading ? undefined : "Click to download"}
          onClick={localReady || localDownloading ? undefined : onOpenModels}
        />
      </div>

      <div style={{ ...cx.footer, ...(canSend ? {} : cx.footerWarn), display: "flex", alignItems: "center", gap: 10 }}>
        <span style={{ flex: 1 }}>
          {canSend
            ? <>Ready to transmit — just start typing, or press <kbd style={cx.kbd}>⌘↵</kbd> to send.</>
            : <>Add a provider key or install Sentynyx Local before sending.</>
          }
        </span>
        {onStartTour && (
          <button
            onClick={onStartTour}
            style={{
              background: "transparent", border: "1px solid rgba(242,255,43,0.4)",
              color: "var(--neon)", borderRadius: 6, padding: "4px 10px",
              fontSize: 10, letterSpacing: 1, cursor: "pointer", whiteSpace: "nowrap",
              fontFamily: "'JetBrains Mono', monospace",
            }}
          >TAKE THE 2-MIN TOUR →</button>
        )}
      </div>
    </div>
  );
}

type RowState = "ok" | "warn" | "progress" | "neutral";

function Row({ state, label, value, hint, onClick }: {
  state: RowState;
  label: string;
  value: string;
  hint?: string;
  onClick?: () => void;
}) {
  const glyph = state === "ok" ? "✓" : state === "warn" ? "◯" : state === "progress" ? "◐" : "◦";
  const color =
    state === "ok" ? "var(--neon)"
    : state === "warn" ? "#fb7185"
    : state === "progress" ? "var(--neon)"
    : "var(--ink-3)";
  return (
    <button
      onClick={onClick}
      disabled={!onClick}
      style={{ ...cx.row, cursor: onClick ? "pointer" : "default" }}
    >
      <span style={{ ...cx.glyph, color }}>{glyph}</span>
      <span style={cx.rowLabel}>{label}</span>
      <span style={cx.rowValue}>{value}</span>
      {hint && <span style={cx.rowHint}>{hint} →</span>}
    </button>
  );
}

const cx: Record<string, CSSProperties> = {
  card: {
    maxWidth: 640,
    margin: "0 auto 32px",
    background: "rgba(10, 12, 20, 0.6)",
    border: "1px solid rgba(242,255,43,0.15)",
    borderRadius: 12,
    backdropFilter: "blur(8px)",
    overflow: "hidden",
  },
  header: {
    display: "flex", alignItems: "center", gap: 10,
    padding: "10px 16px",
    borderBottom: "1px solid var(--line)",
    background: "rgba(242,255,43,0.04)",
  },
  dot: {
    width: 7, height: 7, borderRadius: 99,
    background: "var(--neon)", boxShadow: "0 0 10px var(--neon)",
    animation: "pulse 1.8s infinite",
  },
  title: {
    fontFamily: "'JetBrains Mono', monospace",
    fontSize: 11, letterSpacing: 3, color: "var(--neon)", fontWeight: 600,
  },
  subtle: {
    marginLeft: "auto",
    fontFamily: "'JetBrains Mono', monospace",
    fontSize: 9, letterSpacing: 1.5, color: "var(--ink-3)",
  },
  rows: { padding: "8px 6px" },
  row: {
    display: "grid",
    gridTemplateColumns: "22px 110px 1fr auto",
    alignItems: "center", gap: 10,
    width: "100%", padding: "8px 12px",
    background: "transparent", border: "none",
    color: "var(--ink-0)", textAlign: "left",
    borderRadius: 6,
    transition: "background 0.12s",
  },
  glyph: {
    fontSize: 14, fontFamily: "'JetBrains Mono', monospace",
    textAlign: "center",
  },
  rowLabel: {
    fontSize: 10, letterSpacing: 2, color: "var(--ink-3)",
    fontFamily: "'JetBrains Mono', monospace", textTransform: "uppercase",
  },
  rowValue: { fontSize: 13, color: "var(--ink-0)" },
  rowHint: {
    fontFamily: "'JetBrains Mono', monospace",
    fontSize: 10, letterSpacing: 1, color: "var(--neon)",
  },
  footer: {
    padding: "10px 16px",
    borderTop: "1px solid var(--line)",
    fontSize: 12, color: "var(--ink-1)",
    lineHeight: 1.5,
    background: "rgba(242,255,43,0.02)",
  },
  footerWarn: {
    color: "#ff99a8",
    background: "rgba(251,113,133,0.04)",
    borderTop: "1px solid rgba(251,113,133,0.2)",
  },
  kbd: {
    display: "inline-block",
    padding: "1px 6px",
    background: "rgba(0,0,0,0.4)",
    border: "1px solid rgba(255,255,255,0.1)",
    borderRadius: 3,
    fontFamily: "'JetBrains Mono', monospace",
    fontSize: 10,
    color: "var(--neon)",
  },
};
