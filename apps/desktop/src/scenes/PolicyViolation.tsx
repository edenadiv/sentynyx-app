import type { CSSProperties } from "react";
import { useEscape } from "../lib/useEscape";
import type { BlockReason } from "../lib/types";

interface Props {
  rule: BlockReason;
  /// Called when the user acknowledges / dismisses without action.
  onDismiss: () => void;
  /// Called if the user chooses "Remove and retry" — the App strips the
  /// critical span client-side and re-submits. Optional; button hidden if
  /// undefined.
  onRemoveAndRetry?: () => void;
  /// Called if the user chooses "Switch to Sentynyx Local" — the App
  /// flips the model to the on-device Qwen and re-submits the same prompt.
  /// Optional; button hidden if undefined.
  onSwitchToLocal?: () => void;
}

export function PolicyViolation({ rule, onDismiss, onRemoveAndRetry, onSwitchToLocal }: Props) {
  useEscape(onDismiss);
  // Stays until the user clicks something — no auto-dismiss, because a
  // policy block on an outbound payload is the kind of thing a user should
  // make a conscious choice about, not have wiped off their screen.
  return (
    <div style={pv.overlay}>
      <div style={pv.glitch} />
      <div style={pv.crack}>
        <svg width="100%" height="100%" viewBox="0 0 1000 600" preserveAspectRatio="none">
          <path d="M 0,300 L 300,280 L 320,320 L 450,290 L 470,350 L 700,310 L 720,260 L 1000,290"
            stroke="#ff3355" strokeWidth="2" fill="none" opacity="0.8" />
          <path d="M 200,0 L 210,100 L 190,180 L 220,280 L 200,400 L 230,500 L 210,600"
            stroke="#ff3355" strokeWidth="1" fill="none" opacity="0.5" />
          <path d="M 800,0 L 790,150 L 810,280 L 780,420 L 800,600"
            stroke="#ff3355" strokeWidth="1" fill="none" opacity="0.5" />
        </svg>
      </div>
      <div style={pv.modal}>
        <div style={pv.header}>
          <span style={pv.icon}>⚠</span>
          <div>
            <div style={pv.title}>TRANSMISSION HALTED</div>
            <div style={pv.sub}>POLICY VIOLATION · SEVERITY: CRITICAL</div>
          </div>
        </div>
        <div style={pv.body}>
          <div style={pv.ruleRow}><div style={pv.ruleTag}>RULE</div><div style={pv.ruleName}>{rule.rule}</div></div>
          <div style={pv.ruleRow}><div style={pv.ruleTag}>CLASS</div>
            <div style={{ color:"#ff99a8", fontFamily:"'JetBrains Mono',monospace" }}>{rule.class}</div>
          </div>
          <div style={pv.ruleRow}><div style={pv.ruleTag}>ACTION</div>
            <div style={{ color:"#fff", fontFamily:"'JetBrains Mono',monospace" }}>BLOCK_EGRESS + AUDIT_LOG</div>
          </div>
          <div style={pv.descBox}>{rule.desc}</div>
        </div>
        <div style={pv.foot}>
          <div style={{ fontSize:10, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace", letterSpacing:1 }}>
            Event logged to the local hash-chained audit log (⌘D)
          </div>
          <div style={{ display:"flex", gap:8 }}>
            {onSwitchToLocal && (
              <button onClick={onSwitchToLocal} style={pv.btnLocal} title="Run this prompt on-device with Sentynyx Local (Qwen 2.5 0.5B)">
                USE LOCAL
              </button>
            )}
            {onRemoveAndRetry && (
              <button onClick={onRemoveAndRetry} style={pv.btnSecondary}>
                REMOVE &amp; RETRY
              </button>
            )}
            <button onClick={onDismiss} style={pv.btn}>ACKNOWLEDGE</button>
          </div>
        </div>
      </div>
    </div>
  );
}

const pv: Record<string, CSSProperties> = {
  overlay:{ position:"fixed", inset:0, zIndex:500, background:"rgba(40,0,8,0.4)",
    backdropFilter:"blur(6px)", display:"flex", alignItems:"center", justifyContent:"center",
    animation:"shakeIn 0.5s" },
  glitch:{ position:"absolute", inset:0, pointerEvents:"none",
    background:"repeating-linear-gradient(0deg, transparent 0px, rgba(255,51,85,0.06) 2px, transparent 4px)",
    animation:"glitchV 0.15s infinite" },
  crack:{ position:"absolute", inset:0, pointerEvents:"none", opacity:0.7 },
  modal:{ position:"relative", width:520, background:"rgba(30,10,14,0.95)",
    border:"1px solid rgba(255,51,85,0.5)", borderRadius:12,
    boxShadow:"0 0 80px rgba(255,51,85,0.3), 0 20px 60px rgba(0,0,0,0.7)",
    animation:"modalIn 0.4s" },
  header:{ display:"flex", alignItems:"center", gap:14, padding:"20px 22px",
    borderBottom:"1px solid rgba(255,51,85,0.2)" },
  icon:{ fontSize:32, color:"#ff3355", animation:"pulse 0.8s infinite" },
  title:{ fontSize:18, fontWeight:700, letterSpacing:4, color:"#fff", fontFamily:"'JetBrains Mono',monospace" },
  sub:{ fontSize:10, letterSpacing:3, color:"#ff99a8", marginTop:4, fontFamily:"'JetBrains Mono',monospace" },
  body:{ padding:20 },
  ruleRow:{ display:"flex", gap:14, alignItems:"center", marginBottom:10 },
  ruleTag:{ width:70, fontSize:9, letterSpacing:2, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace" },
  ruleName:{ color:"#fff", fontSize:13 },
  descBox:{ marginTop:14, padding:12, background:"rgba(255,51,85,0.06)",
    border:"1px solid rgba(255,51,85,0.2)", borderRadius:8,
    fontSize:12, color:"var(--ink-1)", lineHeight:1.6 },
  foot:{ display:"flex", justifyContent:"space-between", alignItems:"center",
    padding:"14px 22px", borderTop:"1px solid rgba(255,51,85,0.2)" },
  btn:{ padding:"8px 16px", background:"#ff3355", color:"#fff", border:"none",
    borderRadius:6, fontSize:11, fontWeight:700, letterSpacing:2, cursor:"pointer",
    fontFamily:"'JetBrains Mono',monospace", boxShadow:"0 0 20px rgba(255,51,85,0.4)" },
  btnSecondary:{ padding:"8px 16px", background:"transparent", color:"#ff99a8",
    border:"1px solid rgba(255,153,168,0.5)", borderRadius:6, fontSize:11, fontWeight:700,
    letterSpacing:2, cursor:"pointer", fontFamily:"'JetBrains Mono',monospace" },
  btnLocal:{ padding:"8px 16px", background:"transparent", color:"#f2ff2b",
    border:"1px solid rgba(242,255,43,0.5)", borderRadius:6, fontSize:11, fontWeight:700,
    letterSpacing:2, cursor:"pointer", fontFamily:"'JetBrains Mono',monospace",
    boxShadow:"0 0 12px rgba(242,255,43,0.2)" },
};
