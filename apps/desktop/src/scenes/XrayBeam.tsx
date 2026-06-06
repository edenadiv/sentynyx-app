import { useEffect, useState } from "react";
import type { CSSProperties } from "react";
import { PROVIDER_GLYPHS } from "../lib/models";
import type { Model, Span } from "../lib/types";

type Phase = "scan" | "excise" | "launch" | "done";

export function XrayBeam({ text, spans, model, onDone }: { text: string; spans: Span[]; model: Model; onDone: () => void }) {
  const [phase, setPhase] = useState<Phase>("scan");
  const [progress, setProgress] = useState(0);

  useEffect(() => {
    let raf = 0;
    const start = performance.now();
    const total = 2600;
    const tick = (t: number) => {
      const p = Math.min(1, (t - start) / total);
      setProgress(p);
      if (p < 0.45) setPhase("scan");
      else if (p < 0.75) setPhase("excise");
      else if (p < 1) setPhase("launch");
      else { setPhase("done"); onDone(); return; }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);

  return (
    <div style={xb.overlay}>
      <div style={xb.frame}>
        <span style={{ ...xb.corner, top:12, left:12, borderTop:"2px solid var(--neon)", borderLeft:"2px solid var(--neon)" }} />
        <span style={{ ...xb.corner, top:12, right:12, borderTop:"2px solid var(--neon)", borderRight:"2px solid var(--neon)" }} />
        <span style={{ ...xb.corner, bottom:12, left:12, borderBottom:"2px solid var(--neon)", borderLeft:"2px solid var(--neon)" }} />
        <span style={{ ...xb.corner, bottom:12, right:12, borderBottom:"2px solid var(--neon)", borderRight:"2px solid var(--neon)" }} />

        <div style={xb.topBar}>
          <div style={{ display:"flex", alignItems:"center", gap:10 }}>
            <span style={xb.dot} />
            <span style={{ fontSize:10, letterSpacing:3, fontFamily:"'JetBrains Mono',monospace", color:"var(--neon)" }}>
              VENDETTA · {phase.toUpperCase()}
            </span>
          </div>
          <div style={{ fontSize:10, fontFamily:"'JetBrains Mono',monospace", color:"var(--ink-2)" }}>
            {(progress * 100).toFixed(0)}% · {spans.length} tokens
          </div>
        </div>

        <div style={xb.content}>
          <TextScan text={text} spans={spans} phase={phase} progress={progress} />
        </div>

        <div style={xb.footBar}>
          <div style={{ fontSize:10, fontFamily:"'JetBrains Mono',monospace", color:"var(--ink-3)", letterSpacing:1 }}>
            {phase === "scan" && "▸ PERIMETER SWEEP · scanning all tokens"}
            {phase === "excise" && "▸ EXCISING · sensitive tokens aliased"}
            {phase === "launch" && `▸ LAUNCH · routing to ${model.provider}`}
          </div>
          <div style={{ display:"flex", alignItems:"center", gap:8, fontSize:10, fontFamily:"'JetBrains Mono',monospace" }}>
            <span style={{ color: model.color }}>{PROVIDER_GLYPHS[model.provider]}</span>
            <span style={{ color:"var(--ink-1)" }}>{model.name}</span>
          </div>
        </div>
        <div style={xb.progressBar}>
          <div style={{ ...xb.progressFill, width: `${progress * 100}%` }} />
        </div>
      </div>
    </div>
  );
}

function TextScan({ text, spans, phase, progress }: { text: string; spans: Span[]; phase: Phase; progress: number }) {
  const scanPos = phase === "scan" ? (progress / 0.45) : 1;
  return (
    <div style={xb.scanWrap}>
      <div style={xb.textReveal}>
        {renderAnimatedText(text, spans, phase)}
      </div>
      {phase === "scan" && (
        <div style={{ ...xb.scanLine, top: `${scanPos * 100}%` }} />
      )}
      {(phase === "excise" || phase === "launch") && (
        <div style={xb.indexOverlay}>
          {spans.slice(0, 8).map((s, i) => (
            <div key={i} style={{ ...xb.indexChip, animation: `chipIn 0.4s ${i*0.06}s both` }}>
              <span style={{ color:"var(--ink-3)", fontSize:9 }}>{s.kind}</span>
              <span style={{ color:"var(--ink-3)" }}>→</span>
              <span style={{ color:"var(--neon)" }}>{s.alias}</span>
            </div>
          ))}
          {spans.length > 8 && (
            <div style={{ ...xb.indexChip, color:"var(--ink-2)" }}>+{spans.length - 8} more</div>
          )}
        </div>
      )}
      {phase === "launch" && (
        <div style={xb.launchFx}>
          {Array.from({ length:24 }).map((_, i) => (
            <span key={i} style={{ ...xb.particle,
              left: `${Math.random() * 100}%`, top: `${30 + Math.random() * 40}%`,
              animation: `launchUp ${0.6 + Math.random()*0.4}s ${Math.random()*0.3}s ease-out forwards` }} />
          ))}
        </div>
      )}
    </div>
  );
}

function renderAnimatedText(text: string, spans: Span[], phase: Phase) {
  if (!spans.length) return <span style={{ whiteSpace:"pre-wrap" }}>{text}</span>;
  const parts: React.ReactNode[] = [];
  let cur = 0;
  spans.forEach((s, i) => {
    if (s.start > cur) parts.push(<span key={"p" + i}>{text.slice(cur, s.start)}</span>);
    if (phase === "scan") {
      parts.push(<span key={"h" + i} style={{ background:"rgba(242,255,43,0.3)" }}>{text.slice(s.start, s.end)}</span>);
    } else if (phase === "excise") {
      parts.push(
        <span key={"h" + i} style={{ display:"inline-block",
          animation: `excise 0.5s ${i*0.04}s forwards`, position:"relative" }}>
          <span style={{ background:"rgba(242,255,43,0.4)", padding:"1px 4px", borderRadius:3 }}>
            {text.slice(s.start, s.end)}
          </span>
        </span>
      );
    } else {
      parts.push(
        <span key={"h" + i} style={{ color:"var(--neon)",
          background:"rgba(242,255,43,0.1)", padding:"1px 4px", borderRadius:3,
          fontFamily:"'JetBrains Mono',monospace", fontSize:"0.9em",
          animation:"aliasIn 0.4s forwards" }}>
          {s.alias}
        </span>
      );
    }
    cur = s.end;
  });
  if (cur < text.length) parts.push(<span key="tail">{text.slice(cur)}</span>);
  return <span style={{ whiteSpace:"pre-wrap", lineHeight:1.8 }}>{parts}</span>;
}

const xb: Record<string, CSSProperties> = {
  overlay:{ position:"fixed", inset:0, zIndex:300,
    background:"rgba(5,6,10,0.88)", backdropFilter:"blur(20px)",
    display:"flex", alignItems:"center", justifyContent:"center", animation:"fadeIn 0.2s ease-out" },
  frame:{ position:"relative", width:"min(860px, 92vw)", maxHeight:"84vh",
    background:"rgba(10,12,20,0.96)", border:"1px solid rgba(242,255,43,0.3)", borderRadius:12,
    boxShadow:"0 0 60px rgba(242,255,43,0.2), 0 20px 80px rgba(0,0,0,0.6)",
    display:"flex", flexDirection:"column", overflow:"hidden" },
  corner:{ position:"absolute", width:14, height:14, pointerEvents:"none" },
  topBar:{ display:"flex", justifyContent:"space-between", alignItems:"center",
    padding:"14px 20px", borderBottom:"1px solid var(--line)" },
  dot:{ width:8, height:8, borderRadius:99, background:"var(--neon)",
    boxShadow:"0 0 12px var(--neon)", animation:"pulse 1s infinite" },
  content:{ flex:1, overflow:"auto", padding:"24px 28px" },
  footBar:{ display:"flex", justifyContent:"space-between", alignItems:"center",
    padding:"12px 20px", borderTop:"1px solid var(--line)", background:"rgba(242,255,43,0.03)" },
  progressBar:{ height:2, background:"rgba(255,255,255,0.05)" },
  progressFill:{ height:"100%", background:"var(--neon)",
    boxShadow:"0 0 8px var(--neon)", transition:"width 0.05s linear" },
  scanWrap:{ position:"relative", minHeight:180 },
  textReveal:{ fontSize:15, lineHeight:1.8, color:"var(--ink-0)", fontFamily:"'Inter',sans-serif" },
  scanLine:{ position:"absolute", left:-12, right:-12, height:2,
    background:"linear-gradient(90deg, transparent, var(--neon), transparent)",
    boxShadow:"0 0 16px var(--neon)", transition:"top 0.05s linear" },
  indexOverlay:{ marginTop:24, paddingTop:16, borderTop:"1px dashed var(--line)",
    display:"flex", flexWrap:"wrap", gap:6 },
  indexChip:{ display:"inline-flex", alignItems:"center", gap:6,
    padding:"4px 10px", background:"rgba(255,255,255,0.03)",
    border:"1px solid var(--line)", borderRadius:99,
    fontSize:10, fontFamily:"'JetBrains Mono',monospace", opacity:0 },
  launchFx:{ position:"absolute", inset:0, pointerEvents:"none" },
  particle:{ position:"absolute", width:3, height:3, borderRadius:99,
    background:"var(--neon)", boxShadow:"0 0 6px var(--neon)" },
};
