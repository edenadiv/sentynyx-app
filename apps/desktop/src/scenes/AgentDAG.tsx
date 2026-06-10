import { useEffect, useState } from "react";
import { useEscape } from "../lib/useEscape";
import type { CSSProperties } from "react";

export function AgentDAG({ onClose }: { onClose: () => void }) {
  useEscape(onClose);
  const [step, setStep] = useState(0);
  const nodes: { id:string; label:string; x:number; y:number; kind:"user"|"vend"|"llm"|"tool"; model?:string }[] = [
    { id:"prompt", label:"USER PROMPT",         x:80,  y:140, kind:"user" },
    { id:"vend1",  label:"VENDETTA · ingress",  x:240, y:140, kind:"vend" },
    { id:"plan",   label:"PLANNER",             x:400, y:140, kind:"llm", model:"Claude Opus 4.5" },
    { id:"search", label:"tool: web_search",    x:560, y:60,  kind:"tool" },
    { id:"db",     label:"tool: query_db",      x:560, y:140, kind:"tool" },
    { id:"code",   label:"tool: execute_python",x:560, y:220, kind:"tool" },
    { id:"synth",  label:"SYNTHESIZER",         x:720, y:140, kind:"llm", model:"GPT-5" },
    { id:"vend2",  label:"VENDETTA · egress",   x:880, y:140, kind:"vend" },
    { id:"user",   label:"RESPONSE",            x:1040,y:140, kind:"user" },
  ];
  const edges: [string, string][] = [
    ["prompt","vend1"],["vend1","plan"],
    ["plan","search"],["plan","db"],["plan","code"],
    ["search","synth"],["db","synth"],["code","synth"],
    ["synth","vend2"],["vend2","user"],
  ];

  useEffect(() => {
    if (step >= edges.length) return;
    const t = setTimeout(() => setStep(s => s + 1), 450);
    return () => clearTimeout(t);
  }, [step]);

  const activeEdges = edges.slice(0, step);
  const activeNodes = new Set<string>(["prompt"]);
  activeEdges.forEach(([a,b]) => { activeNodes.add(a); activeNodes.add(b); });

  const nodeColor = (k: string, active: boolean) =>
    !active ? "#2a2f3e" : (k === "vend" ? "var(--neon)" : k === "tool" ? "#6effb0" : k === "llm" ? "#c78bff" : "#fff");

  return (
    <div style={ag.overlay}>
      <div style={ag.stage}>
        <div style={ag.header}>
          <div>
            <div style={{ fontSize:10, letterSpacing:4, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace" }}>
              AGENT MODE · <span style={{ color:"#ffb454", border:"1px solid rgba(255,180,84,0.4)", borderRadius:3, padding:"1px 6px" }}>⚠ CONCEPT PREVIEW — NOT FUNCTIONAL YET</span>
            </div>
            <div style={{ fontFamily:"'Instrument Serif',serif", fontSize:32, marginTop:4 }}>
              Tool-chain <em style={{ color:"var(--neon)" }}>execution</em>
            </div>
            <div style={{ fontSize:11, color:"var(--ink-2)", marginTop:4 }}>Vendetta wraps every hop · in & out</div>
          </div>
          <button onClick={onClose} style={ag.close}>×</button>
        </div>

        <svg viewBox="0 0 1120 280" style={{ width:"100%", height:400 }}>
          <defs>
            <marker id="arrow" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="8" markerHeight="8" orient="auto">
              <path d="M 0 0 L 10 5 L 0 10 z" fill="var(--neon)" />
            </marker>
          </defs>
          {edges.map(([a, b], i) => {
            const na = nodes.find(n => n.id === a)!;
            const nb = nodes.find(n => n.id === b)!;
            const active = i < step;
            return (
              <g key={a + "-" + b}>
                <line x1={na.x + 60} y1={na.y} x2={nb.x - 60} y2={nb.y}
                  stroke={active ? "var(--neon)" : "#2a2f3e"}
                  strokeWidth={active ? 1.5 : 1}
                  strokeDasharray={active ? "" : "3 4"}
                  markerEnd={active ? "url(#arrow)" : ""}
                  style={active ? { filter:"drop-shadow(0 0 4px var(--neon))" } : {}}
                />
              </g>
            );
          })}
          {nodes.map(n => {
            const active = activeNodes.has(n.id);
            const c = nodeColor(n.kind, active);
            return (
              <g key={n.id} transform={`translate(${n.x},${n.y})`}>
                <rect x="-60" y="-22" width="120" height="44" rx="8"
                  fill={active ? "rgba(10,12,20,0.9)" : "rgba(10,12,20,0.5)"}
                  stroke={c} strokeWidth="1" />
                <text x="0" y="-2" fontSize="10" fontFamily="JetBrains Mono" fill={c} textAnchor="middle" letterSpacing="1">
                  {n.label}
                </text>
                {n.model && (
                  <text x="0" y="12" fontSize="8" fontFamily="JetBrains Mono" fill="#8b91a6" textAnchor="middle">
                    {n.model}
                  </text>
                )}
              </g>
            );
          })}
        </svg>

        <div style={ag.foot}>
          <div style={{ display:"flex", gap:20, fontSize:10, fontFamily:"'JetBrains Mono',monospace", letterSpacing:1, color:"var(--ink-2)" }}>
            <Legend c="var(--neon)" t="VENDETTA" />
            <Legend c="#c78bff" t="LLM" />
            <Legend c="#6effb0" t="TOOL" />
            <Legend c="#fff" t="I/O" />
          </div>
          <div style={{ fontSize:10, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace", letterSpacing:1 }}>
            {step >= edges.length ? "PREVIEW · concept animation — real tool execution is on the roadmap" : `animating step ${step}/${edges.length}`}
          </div>
        </div>
      </div>
    </div>
  );
}

function Legend({ c, t }: { c: string; t: string }) {
  return (
    <span style={{ display:"inline-flex", alignItems:"center", gap:6 }}>
      <span style={{ width:8, height:8, borderRadius:99, background:c, boxShadow:`0 0 6px ${c}` }} />
      <span>{t}</span>
    </span>
  );
}

const ag: Record<string, CSSProperties> = {
  overlay:{ position:"fixed", inset:0, zIndex:250,
    background:"radial-gradient(ellipse at center, rgba(5,6,10,0.88), rgba(5,6,10,0.98))",
    backdropFilter:"blur(14px)",
    display:"flex", alignItems:"center", justifyContent:"center", padding:20, animation:"fadeIn 0.3s" },
  stage:{ width:"min(1180px, 96vw)", background:"rgba(10,12,20,0.96)",
    border:"1px solid rgba(255,255,255,0.1)", borderRadius:14, padding:28 },
  header:{ display:"flex", justifyContent:"space-between", alignItems:"flex-end", marginBottom:20 },
  close:{ width:36, height:36, borderRadius:8, background:"rgba(255,255,255,0.05)",
    border:"1px solid var(--line)", color:"var(--ink-1)", fontSize:20, cursor:"pointer" },
  foot:{ display:"flex", justifyContent:"space-between", alignItems:"center", marginTop:8,
    paddingTop:16, borderTop:"1px solid var(--line)" },
};
