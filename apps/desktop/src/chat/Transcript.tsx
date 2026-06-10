import { useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import { Highlighted } from "./Composer";
import { PROVIDER_GLYPHS, SUGGESTIONS } from "../lib/models";
import type { Message, Model } from "../lib/types";

export interface TranscriptStats {
  /// Providers with working credentials (BYOK keys + a reachable Ollama).
  providers: number;
  /// Models currently routable (built-ins + discovered Ollama models).
  models: number;
  /// Real count from the local audit log.
  redactions24h: number;
  /// Mean time-to-first-token across this session's sends; null until data.
  meanTtftMs: number | null;
}

interface TProps { messages: Message[]; model: Model; stats?: TranscriptStats }

export function Transcript({ messages, model, stats }: TProps) {
  const scroller = useRef<HTMLDivElement>(null);
  useEffect(() => { if (scroller.current) scroller.current.scrollTop = scroller.current.scrollHeight; }, [messages]);

  return (
    <div ref={scroller} style={cx.transcript}>
      {messages.length === 0 && <EmptyState stats={stats} />}
      {messages.map((m, i) => (
        <MessageBlock key={m.id ?? i} m={m} model={model} glyph={PROVIDER_GLYPHS[model.provider]} />
      ))}
    </div>
  );
}

function EmptyState({ stats }: { stats?: TranscriptStats }) {
  return (
    <div style={cx.empty}>
      <div style={cx.emptyHalo} />
      <div style={{ position:"relative", textAlign:"center", maxWidth:640, margin:"0 auto" }}>
        <div style={{ fontSize:10, letterSpacing:6, color:"var(--neon)", fontFamily:"'JetBrains Mono',monospace", marginBottom:24 }}>
          ◦ ◦ ◦  SYSTEM READY  ◦ ◦ ◦
        </div>
        <h1 style={cx.emptyTitle}>
          What should we <em style={{ color:"var(--neon)", fontStyle:"italic", textShadow:"0 0 20px rgba(242,255,43,0.4)" }}>orchestrate</em> today?
        </h1>
        <div style={cx.emptySub}>
          Any model · any provider · zero sensitive data leaks the perimeter.
        </div>
        {stats && stats.providers === 0 && (
          <div style={{ marginTop: 10, fontSize: 12, color: "#ffb454", fontFamily: "'JetBrains Mono',monospace" }}>
            No providers configured yet — add an API key in Settings (⌘,), or install Ollama for zero-egress local models.
          </div>
        )}
        <div style={cx.suggest}>
          {SUGGESTIONS.map((s, i) => (
            <button key={i} style={cx.sugCard}>
              <div style={cx.sugK}>{s.k}</div>
              <div style={cx.sugT}>{s.t}</div>
              <div style={cx.sugArrow}>→</div>
            </button>
          ))}
        </div>
        <div style={cx.emptyStats}>
          <Stat label="Providers ready" value={stats ? String(stats.providers) : "—"} />
          <Stat label="Models routable" value={stats ? String(stats.models) : "—"} />
          <Stat label="Redactions / 24h" value={stats ? stats.redactions24h.toLocaleString() : "—"} accent />
          <Stat label="Mean TTFT" value={stats?.meanTtftMs != null ? `${stats.meanTtftMs}ms` : "—"} />
        </div>
      </div>
    </div>
  );
}

function Stat({ label, value, accent }: { label:string; value:string; accent?:boolean }) {
  return (
    <div style={cx.stat}>
      <div style={{ ...cx.statValue, color: accent ? "var(--neon)" : "var(--ink-0)" }}>{value}</div>
      <div style={cx.statLabel}>{label}</div>
    </div>
  );
}

function MessageBlock({ m, model, glyph }: { m: Message; model: Model; glyph: string }) {
  const [view, setView] = useState<"rehydrated"|"aliased">("rehydrated");

  if (m.role === "user") {
    return (
      <div style={cx.msgUser}>
        <div style={cx.msgUserHead}>
          <div style={cx.avatarU}>⛨</div>
          <div style={{ fontSize:11, color:"var(--ink-2)" }}>You</div>
          <div style={cx.msgMeta}>
            {m.spans && m.spans.length > 0 && (
              <span style={cx.msgShield}>
                <span style={{ color:"var(--neon)" }}>⛨</span>
                {m.spans.length} redacted · {model.provider} received aliased payload
              </span>
            )}
          </div>
        </div>
        <div style={cx.msgUserBody}>
          <Highlighted text={m.text} spans={m.spans || []} />
        </div>
      </div>
    );
  }

  return (
    <div style={cx.msgAssist}>
      <div style={cx.msgAssistHead}>
        <div style={{ ...cx.avatarA, background: model.color + "22", color: model.color, borderColor: model.color + "66" }}>
          {glyph}
        </div>
        <div style={{ fontSize:11, color:"var(--ink-2)" }}>{model.name}</div>
        <div style={cx.msgMeta}>
          <span style={{ fontFamily:"'JetBrains Mono',monospace", fontSize:10, color:"var(--ink-3)" }}>
            via Vendetta tunnel · re-hydrated
          </span>
          {m.aliasedPrompt && (
            <div style={cx.dualToggle}>
              <button onClick={() => setView("rehydrated")} style={{ ...cx.dualTab, ...(view === "rehydrated" ? cx.dualTabOn : {}) }}>You see</button>
              <button data-tour="modelsaw" onClick={() => setView("aliased")} style={{ ...cx.dualTab, ...(view === "aliased" ? cx.dualTabOn : {}) }}>Model saw</button>
            </div>
          )}
        </div>
      </div>
      <div style={{ ...cx.msgAssistBody, ...(view === "aliased" ? cx.msgAliased : {}) }}>
        {view === "rehydrated" ? (
          <>
            {m.error && <div style={{ color:"var(--danger)", marginBottom:8 }}>⚠ {m.error}</div>}
            {m.text.split("\n").map((ln, i) => <div key={i} style={{ marginBottom: ln ? 8 : 0 }}>{ln || "\u00A0"}</div>)}
            {m.streaming && <span className="caret" />}
          </>
        ) : (
          <div>
            <div style={{ fontSize:10, letterSpacing:2, color:"var(--neon)", fontFamily:"'JetBrains Mono',monospace", marginBottom:10 }}>
              ▸ UPSTREAM PAYLOAD · what {model.provider} actually received
            </div>
            <div style={{ fontFamily:"'JetBrains Mono',monospace", fontSize:12, lineHeight:1.7, color:"var(--ink-1)", whiteSpace:"pre-wrap" }}>
              {m.aliasedPrompt}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

const cx: Record<string, CSSProperties> = {
  transcript:{ flex:1, overflow:"auto", padding:"28px 24px 18px" },
  empty:{ position:"relative", minHeight:"60vh", display:"flex", alignItems:"center", justifyContent:"center", padding:"40px 20px" },
  emptyHalo:{ position:"absolute", left:"50%", top:"30%", transform:"translate(-50%,-50%)",
    width:700, height:700, borderRadius:"50%",
    background:"radial-gradient(circle, rgba(242,255,43,0.08), transparent 60%)",
    filter:"blur(20px)", pointerEvents:"none" },
  emptyTitle:{ fontFamily:"'Instrument Serif',serif", fontSize:56, fontWeight:400,
    lineHeight:1.05, margin:"0 0 14px", letterSpacing:"-0.02em" },
  emptySub:{ fontSize:14, color:"var(--ink-2)", marginBottom:32 },
  suggest:{ display:"grid", gridTemplateColumns:"repeat(2, 1fr)", gap:10, marginBottom:32 },
  sugCard:{ display:"flex", alignItems:"center", gap:12, padding:"14px 16px",
    background:"rgba(255,255,255,0.03)", border:"1px solid var(--line)", borderRadius:10,
    color:"var(--ink-1)", cursor:"pointer", textAlign:"left", transition:"all 0.15s" },
  sugK:{ fontSize:9, letterSpacing:2, color:"var(--neon)", fontFamily:"'JetBrains Mono',monospace",
    padding:"3px 7px", border:"1px solid rgba(242,255,43,0.3)", borderRadius:99 },
  sugT:{ flex:1, fontSize:13 },
  sugArrow:{ color:"var(--ink-3)", fontSize:14 },
  emptyStats:{ display:"grid", gridTemplateColumns:"repeat(4, 1fr)", gap:1,
    background:"var(--line)", border:"1px solid var(--line)", borderRadius:10, overflow:"hidden" },
  stat:{ padding:"14px 16px", background:"rgba(10,12,20,0.6)" },
  statValue:{ fontFamily:"'Instrument Serif',serif", fontSize:30, fontWeight:400, lineHeight:1 },
  statLabel:{ fontSize:9, letterSpacing:2, color:"var(--ink-3)", marginTop:4, fontFamily:"'JetBrains Mono',monospace" },

  msgUser:{ margin:"0 auto 24px", maxWidth:820 },
  msgUserHead:{ display:"flex", alignItems:"center", gap:10, marginBottom:8 },
  avatarU:{ width:24, height:24, borderRadius:6, background:"linear-gradient(135deg,#f2ff2b,#eaff48)",
    color:"#000", display:"flex", alignItems:"center", justifyContent:"center", fontWeight:700, fontSize:10 },
  msgMeta:{ marginLeft:"auto", display:"flex", alignItems:"center", gap:8 },
  msgShield:{ display:"inline-flex", alignItems:"center", gap:6, fontSize:10,
    color:"var(--neon)", fontFamily:"'JetBrains Mono',monospace", letterSpacing:1 },
  msgUserBody:{ padding:"14px 16px", background:"rgba(255,255,255,0.02)",
    border:"1px solid var(--line)", borderLeft:"2px solid var(--neon)", borderRadius:10,
    fontSize:14, lineHeight:1.6, color:"var(--ink-0)", whiteSpace:"pre-wrap", wordBreak:"break-word" },
  msgAssist:{ margin:"0 auto 28px", maxWidth:820 },
  msgAssistHead:{ display:"flex", alignItems:"center", gap:10, marginBottom:8 },
  avatarA:{ width:24, height:24, borderRadius:6, display:"flex", alignItems:"center",
    justifyContent:"center", fontWeight:600, fontSize:12, border:"1px solid" },
  msgAssistBody:{ fontSize:14.5, lineHeight:1.7, color:"var(--ink-0)", fontFamily:"'Inter',sans-serif" },
  msgAliased:{ background:"rgba(242,255,43,0.03)", border:"1px dashed rgba(242,255,43,0.3)",
    borderRadius:10, padding:14 },
  dualToggle:{ display:"flex", gap:2, padding:2, marginLeft:10,
    background:"rgba(255,255,255,0.04)", borderRadius:6, border:"1px solid var(--line)" },
  dualTab:{ padding:"3px 8px", background:"transparent", border:"none",
    borderRadius:4, fontSize:10, color:"var(--ink-2)", cursor:"pointer",
    fontFamily:"'JetBrains Mono',monospace", letterSpacing:1 },
  dualTabOn:{ background:"rgba(242,255,43,0.14)", color:"var(--neon)" },
};
