import { useEffect, useState } from "react";
import type { CSSProperties } from "react";

const lines = [
  "BOOTING SENTYNYX KERNEL...",
  "HANDSHAKE · OPENAI · ANTHROPIC · GOOGLE · xAI · META",
  "INITIALIZING VENDETTA PERIMETER",
  "LOADING POLICY ENGINE · 147 RULES",
  "MOUNTING KNOWLEDGE ATLAS · 2.4TB",
  "TELEMETRY ONLINE · CRYPTO SEAL VERIFIED",
  "FLEET LINK ESTABLISHED",
];

export function BootSequence({ onDone }: { onDone: () => void }) {
  const [step, setStep] = useState(0);
  useEffect(() => {
    if (step >= lines.length) { const t = setTimeout(onDone, 500); return () => clearTimeout(t); }
    const t = setTimeout(() => setStep(s => s + 1), step === 0 ? 400 : 280);
    return () => clearTimeout(t);
  }, [step]);

  return (
    <div style={bs.overlay}>
      <div style={bs.ring1} /><div style={bs.ring2} /><div style={bs.ring3} />
      <div style={bs.core} />
      <div style={bs.content}>
        <div style={bs.wordmark}>
          {"SENTYNYX".split("").map((c, i) => (
            <span key={i} style={{ ...bs.letter, animationDelay: i * 0.05 + "s" }}>{c}</span>
          ))}
        </div>
        <div style={bs.tagline}>THE AI OPERATING SYSTEM FOR BUSINESS</div>
        <div style={bs.console}>
          {lines.slice(0, step).map((l, i) => (
            <div key={i} style={bs.line}>
              <span style={{ color:"var(--neon)" }}>▸</span> {l}
              <span style={{ color:"var(--neon)", marginLeft:8 }}>OK</span>
            </div>
          ))}
          {step < lines.length && (
            <div style={{ ...bs.line, opacity:0.6 }}>
              <span style={{ color:"var(--neon)" }}>▸</span> {lines[step]}<span className="caret" />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

const bs: Record<string, CSSProperties> = {
  overlay:{ position:"fixed", inset:0, zIndex:10000,
    background:"radial-gradient(ellipse at center, #0a0c14, #05060a 70%)",
    display:"flex", alignItems:"center", justifyContent:"center", animation:"fadeIn 0.3s" },
  ring1:{ position:"absolute", width:400, height:400, borderRadius:"50%",
    border:"1px solid rgba(242,255,43,0.15)", animation:"spin 8s linear infinite" },
  ring2:{ position:"absolute", width:560, height:560, borderRadius:"50%",
    border:"1px dashed rgba(242,255,43,0.1)", animation:"spin 20s linear reverse infinite" },
  ring3:{ position:"absolute", width:200, height:200, borderRadius:"50%",
    border:"1px solid rgba(242,255,43,0.3)", animation:"spin 4s linear infinite" },
  core:{ position:"absolute", width:16, height:16, borderRadius:"50%",
    background:"var(--neon)", boxShadow:"0 0 60px var(--neon), 0 0 120px var(--neon)",
    animation:"pulse 1s infinite" },
  content:{ position:"relative", textAlign:"center", zIndex:2, marginTop:120 },
  wordmark:{ fontFamily:"'JetBrains Mono',monospace", fontSize:42, fontWeight:700, letterSpacing:12, color:"#fff" },
  letter:{ display:"inline-block", animation:"bootLetter 0.5s both", textShadow:"0 0 20px rgba(242,255,43,0.5)" },
  tagline:{ fontSize:10, letterSpacing:6, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace", marginTop:8 },
  console:{ marginTop:36, fontFamily:"'JetBrains Mono',monospace", fontSize:11,
    color:"var(--ink-1)", textAlign:"left", minWidth:380, maxWidth:480 },
  line:{ margin:"4px 0", display:"flex", alignItems:"center", gap:8,
    animation:"bootLine 0.3s both", justifyContent:"flex-start" },
};
