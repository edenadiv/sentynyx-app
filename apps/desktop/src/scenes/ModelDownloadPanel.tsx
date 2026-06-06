import { useEffect, useState } from "react";
import { modelsIpc, onModelProgress, onModelReady } from "../lib/ipc";
import type { AllModelStatus, ModelStatus } from "../lib/types";
import { modelStatusKind } from "../lib/types";

interface Props { onClose: () => void }

type RowState = "idle" | "downloading" | "ready" | "error";

interface Row {
  id: string;
  label: string;
  sizeMb: number;
  optional: boolean;
  state: RowState;
  percent: number;
  error: string | null;
}

const INITIAL: Row[] = [
  { id: "gliner-small-v2.1", label: "Semantic NER (GLiNER)", sizeMb: 80, optional: false, state: "idle", percent: 0, error: null },
  { id: "gliner-small-v2.1-tokenizer", label: "NER tokenizer", sizeMb: 3, optional: false, state: "idle", percent: 0, error: null },
  { id: "paranoid-llm", label: "Paranoid LLM (Qwen 2.5 0.5B)", sizeMb: 468, optional: true, state: "idle", percent: 0, error: null },
];

function rowStateFromStatus(s: ModelStatus): { state: RowState; error: string | null } {
  const kind = modelStatusKind(s);
  if (kind === "ready") return { state: "ready", error: null };
  if (kind === "downloading") return { state: "downloading", error: null };
  if (kind === "error") {
    const msg = typeof s === "object" && "error" in s ? s.error.msg : "unknown";
    return { state: "error", error: msg };
  }
  return { state: "idle", error: null };
}

export function ModelDownloadPanel({ onClose }: Props) {
  const [rows, setRows] = useState<Row[]>(INITIAL);
  const [includeLlm, setIncludeLlm] = useState(false);

  useEffect(() => {
    modelsIpc.status().then((s: AllModelStatus) => {
      setRows(rs => rs.map(r => {
        const status =
          r.id === "gliner-small-v2.1" ? s.ner :
          r.id === "gliner-small-v2.1-tokenizer" ? s.ner_tokenizer :
          s.llm;
        const { state, error } = rowStateFromStatus(status);
        return { ...r, state, error };
      }));
    }).catch(() => {});

    const unP = onModelProgress(e => {
      setRows(rs => rs.map(r => r.id === e.id
        ? { ...r, state: "downloading", percent: e.percent }
        : r));
    });
    const unR = onModelReady(id => {
      setRows(rs => rs.map(r => r.id === id
        ? { ...r, state: "ready", percent: 100, error: null }
        : r));
    });
    return () => { unP.then(u => u()); unR.then(u => u()); };
  }, []);

  const start = async (id: string) => {
    setRows(rs => rs.map(r => r.id === id ? { ...r, state: "downloading", error: null } : r));
    try {
      await modelsIpc.download(id);
    } catch (e) {
      setRows(rs => rs.map(r => r.id === id
        ? { ...r, state: "error", error: String(e) }
        : r));
    }
  };

  const startAll = async () => {
    const targets = rows.filter(r => r.state !== "ready" && (!r.optional || includeLlm));
    for (const r of targets) await start(r.id);
  };

  return (
    <div style={{
      position: "fixed", inset: 0, background: "rgba(5,6,10,0.88)",
      display: "flex", alignItems: "center", justifyContent: "center", zIndex: 90,
    }}>
      <div style={{
        width: 560, padding: 32, background: "#0a0d14",
        border: "1px solid rgba(242,255,43,0.25)", borderRadius: 8,
        fontFamily: "Inter, sans-serif", color: "#e5e9f0",
      }}>
        <div style={{ fontFamily: "Instrument Serif, serif", fontSize: 28, marginBottom: 8 }}>
          Enable semantic detection
        </div>
        <div style={{ fontSize: 13, color: "#9ba3b4", marginBottom: 20 }}>
          Downloads run from HuggingFace Hub. Files are SHA-256 verified and stored in your app data directory.
        </div>

        {rows.map(r => (
          <div key={r.id} style={{
            display: "flex", alignItems: "center", justifyContent: "space-between",
            padding: "12px 0", borderBottom: "1px solid rgba(255,255,255,0.05)",
          }}>
            <div>
              <div style={{ fontSize: 14 }}>
                {r.label}
                {r.optional && <span style={{ color: "#9ba3b4", fontSize: 11, marginLeft: 8 }}>optional</span>}
              </div>
              <div style={{ fontSize: 11, color: "#9ba3b4", fontFamily: "JetBrains Mono, monospace", marginTop: 2 }}>
                {r.sizeMb} MB · {r.state === "downloading" ? `${r.percent}%` : r.state}
                {r.error && <span style={{ color: "#ff6b9d", marginLeft: 8 }}>{r.error}</span>}
              </div>
            </div>
            <button
              onClick={() => start(r.id)}
              disabled={r.state === "downloading" || r.state === "ready"}
              style={{
                padding: "6px 14px", fontSize: 12,
                background: r.state === "ready" ? "transparent" : "var(--neon, #f2ff2b)",
                color: r.state === "ready" ? "#7cffb2" : "#000",
                border: "none", borderRadius: 4, cursor: "pointer",
                opacity: r.state === "downloading" ? 0.5 : 1,
              }}
            >{r.state === "ready" ? "✓ ready" : r.state === "downloading" ? `${r.percent}%` : "download"}</button>
          </div>
        ))}

        <label style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 16, fontSize: 12, color: "#9ba3b4" }}>
          <input type="checkbox" checked={includeLlm} onChange={e => setIncludeLlm(e.target.checked)} />
          Include paranoid LLM in "download all" (468 MB)
        </label>

        <div style={{ display: "flex", gap: 12, marginTop: 24, justifyContent: "flex-end" }}>
          <button onClick={onClose} style={{
            padding: "8px 16px", fontSize: 13, background: "transparent",
            color: "#9ba3b4", border: "1px solid #2a3040", borderRadius: 4, cursor: "pointer",
          }}>Continue with regex only</button>
          <button onClick={startAll} style={{
            padding: "8px 16px", fontSize: 13, background: "var(--neon, #f2ff2b)",
            color: "#000", border: "none", borderRadius: 4, cursor: "pointer",
          }}>Download all</button>
        </div>
      </div>
    </div>
  );
}
