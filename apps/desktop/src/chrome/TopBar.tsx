import { useEffect, useMemo, useRef, useState } from "react";
import { MODELS, PROVIDER_GLYPHS } from "../lib/models";
import { sx } from "./styles";
import type { Model, AllModelStatus } from "../lib/types";
import { modelStatusKind } from "../lib/types";
import { modelsIpc, onModelProgress, onModelReady, isTauri } from "../lib/ipc";

interface Props {
  model: Model;
  setModel: (m: Model) => void;
  /// Full model list (built-ins + discovered Ollama models). Defaults to MODELS.
  models?: Model[];
  onOpenVendetta: () => void;
  onOpenOrbital: () => void;
  onOpenCmd: () => void;
  onOpenConsensus: () => void;
  onOpenCompliance: () => void;
  onOpenAgent: () => void;
  onOpenSettings?: () => void;
  onOpenDev?: () => void;
  /// Loopback privacy-proxy port when it's running; null when off.
  proxyPort?: number | null;
}

export function TopBar(p: Props) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const h = (e: MouseEvent) => { if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false); };
    addEventListener("mousedown", h);
    return () => removeEventListener("mousedown", h);
  }, []);

  const [modelStatus, setModelStatus] = useState<AllModelStatus | null>(null);
  const [downloadPct, setDownloadPct] = useState<number | null>(null);

  useEffect(() => {
    if (!isTauri) return;
    let cancelled = false;
    modelsIpc.status().then(s => { if (!cancelled) setModelStatus(s); }).catch(() => {});
    const unProgress = onModelProgress(e => {
      if (e.id.startsWith("gliner")) setDownloadPct(e.percent);
    });
    const unReady = onModelReady(() => {
      setDownloadPct(null);
      modelsIpc.status().then(setModelStatus).catch(() => {});
    });
    return () => {
      cancelled = true;
      unProgress.then(u => u());
      unReady.then(u => u());
    };
  }, []);

  const nerReady = modelStatus !== null
    && modelStatusKind(modelStatus.ner) === "ready"
    && modelStatusKind(modelStatus.ner_tokenizer) === "ready";
  const paranoidReady = modelStatus !== null
    && modelStatusKind(modelStatus.llm) === "ready";

  const chipLabel =
    downloadPct !== null ? `◐ semantic ${downloadPct}%` :
    !modelStatus ? "◐ loading…" :
    nerReady && paranoidReady ? "◆◆ paranoid ready" :
    nerReady ? "◆ semantic ready" :
    "◐ semantic off";

  const chipColor =
    downloadPct !== null ? "var(--neon)" :
    nerReady ? "#7cffb2" :
    "#666";

  const models = p.models ?? MODELS;
  const grouped = useMemo(() => {
    const g: Record<string, Model[]> = {};
    for (const m of models) { (g[m.provider] ||= []).push(m); }
    return g;
  }, [models]);

  return (
    <div style={sx.topbar}>
      <div style={{ display:"flex", alignItems:"center", gap:14 }}>
        <div style={{ display:"flex", alignItems:"center", gap:8, fontSize:11, letterSpacing:2, color:"var(--ink-2)" }}>
          <span>WORKSPACE</span>
          <span style={sx.chevron}>›</span>
          <span style={{ color:"var(--ink-0)" }}>Personal</span>
          <span style={sx.chevron}>›</span>
          <span style={{ color:"var(--neon)" }}>Mission Control</span>
        </div>
      </div>

      <div ref={ref} style={{ position:"relative" }}>
        <button style={sx.modelPill} onClick={() => setOpen(o => !o)}>
          <span style={{ color: p.model.color, fontSize:14 }}>{PROVIDER_GLYPHS[p.model.provider]}</span>
          <span style={{ fontWeight:500 }}>{p.model.name}</span>
          <span style={sx.modelCtx}>{p.model.ctx}</span>
          <span style={{ opacity:0.5, marginLeft:4 }}>▾</span>
        </button>
        {open && (
          <div style={sx.modelMenu}>
            <div style={sx.modelMenuHead}>
              <span>Pick any model from any provider</span>
              <span style={{ fontSize:10, color:"var(--neon)" }}>⛨ Vendetta shields all</span>
            </div>
            <div style={{ maxHeight:420, overflow:"auto" }}>
              {Object.entries(grouped).map(([prov, list]) => (
                <div key={prov} style={{ padding:"10px 14px 4px" }}>
                  <div style={sx.provHead}>
                    <span style={{ color:list[0].color }}>{PROVIDER_GLYPHS[prov]}</span>
                    <span>{prov}</span>
                    <span style={{ flex:1, height:1, background:"var(--line)", marginLeft:8 }} />
                  </div>
                  {list.map(m => (
                    <button key={m.id}
                      onClick={() => { p.setModel(m); setOpen(false); }}
                      style={{ ...sx.modelRow, ...(m.id === p.model.id ? sx.modelRowActive : {}) }}>
                      <span style={{ flex:1, display:"flex", alignItems:"center", gap:8 }}>
                        <span style={{ width:6, height:6, borderRadius:99, background: m.id === p.model.id ? "var(--neon)" : "rgba(255,255,255,0.2)" }} />
                        <span>{m.name}</span>
                      </span>
                      <span style={sx.flash}>{m.flash}</span>
                      <span style={sx.modelCtxSm}>{m.ctx}</span>
                    </button>
                  ))}
                </div>
              ))}
            </div>
            <div style={sx.modelFoot}>
              <span>↑↓ navigate · ⏎ select</span>
              <span>+ Add provider</span>
            </div>
          </div>
        )}
      </div>

      <div style={{ display:"flex", alignItems:"center", gap:8 }}>
        <button style={sx.vendBtn} onClick={p.onOpenOrbital} title="Orbital picker">
          <span style={{ color:"var(--neon)", fontSize:12 }}>◎</span>
          <span>ORBITAL</span>
        </button>
        <button style={sx.vendBtn} onClick={p.onOpenConsensus} title="Multi-model consensus">
          <span style={{ color:"#c78bff", fontSize:12 }}>≡</span>
          <span>CONSENSUS</span>
        </button>
        <button style={sx.vendBtn} onClick={p.onOpenAgent} title="Agent mode">
          <span style={{ color:"#6effb0", fontSize:12 }}>⌥</span>
          <span>AGENT</span>
        </button>
        <button style={sx.vendBtn} onClick={p.onOpenCompliance} title="Compliance dashboard">
          <span style={{ color:"var(--neon)", fontSize:12 }}>⌖</span>
          <span>COMPLIANCE</span>
        </button>
        <button style={sx.vendBtn} onClick={p.onOpenVendetta}>
          <span style={sx.vendDot} />
          <span>VENDETTA</span>
          <span style={{ color:"var(--neon)" }}>ACTIVE</span>
        </button>
        {p.onOpenDev && (
          <button
            data-tour="dev-toggle"
            style={sx.iconBtn}
            onClick={p.onOpenDev}
            title="Dev inspector — per-send timings, wire payload, paranoid scan (⌘⇧D)"
          >⌘⇧D</button>
        )}
        <button style={sx.iconBtn} onClick={p.onOpenCmd} title="Command palette">⌘K</button>
        <button
          onClick={() => p.onOpenSettings?.()}
          title="Semantic detection status · click to manage models"
          style={{
            fontFamily: "JetBrains Mono, monospace",
            fontSize: 11, padding: "4px 10px", marginLeft: 8,
            background: "transparent", color: chipColor,
            border: `1px solid ${chipColor}`, borderRadius: 4, cursor: "pointer",
            letterSpacing: 0.5,
          }}
        >{chipLabel}</button>
        {p.proxyPort != null && (
          <button
            onClick={p.onOpenSettings}
            title={`Privacy proxy live — any OpenAI-compatible client can use http://127.0.0.1:${p.proxyPort}/v1`}
            style={{
              background: "rgba(124,255,178,0.08)", border: "1px solid rgba(124,255,178,0.35)",
              color: "#7cffb2", borderRadius: 4, padding: "3px 8px", fontSize: 10,
              fontFamily: "'JetBrains Mono', monospace", letterSpacing: 1, cursor: "pointer",
            }}
          >⇄ proxy :{p.proxyPort}</button>
        )}
      </div>
    </div>
  );
}
