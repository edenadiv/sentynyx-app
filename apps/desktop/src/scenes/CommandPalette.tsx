import { useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";

export type CmdKey = "orbital"|"consensus"|"compliance"|"agent"|"newchat"|"toggle-v"|"policy"|"audit"|"atlas"|"role"|"settings"|"tour";

const ALL: { k: CmdKey; t: string; g: string; sh?: string }[] = [
  { k:"tour",       t:"Take the guided tour",         g:"HELP" },
  { k:"orbital",    t:"Open Orbital Picker",          g:"FLEET", sh:"⌘O" },
  { k:"consensus",  t:"Start Multi-Model Consensus",  g:"FLEET", sh:"⌘M" },
  { k:"compliance", t:"Open Compliance Dashboard",    g:"VIEW",  sh:"⌘D" },
  { k:"agent",      t:"Run Agent (Tool-chain)",       g:"EXEC",  sh:"⌘G" },
  { k:"newchat",    t:"New Transmission",             g:"CHAT",  sh:"⌘N" },
  { k:"toggle-v",   t:"Toggle Vendetta Panel",        g:"VIEW",  sh:"⌘V" },
  { k:"settings",   t:"API Keys & Providers",         g:"ADMIN", sh:"⌘," },
  { k:"policy",     t:"Edit Policy Rules",            g:"ADMIN" },
  { k:"audit",      t:"Open Audit Log",               g:"ADMIN" },
  { k:"atlas",      t:"Knowledge Atlas",              g:"DATA"  },
  { k:"role",       t:"Switch Role · Legal / Eng / HR",g:"ADMIN" },
];

export function CommandPalette({ open, onClose, onAction }: { open: boolean; onClose: () => void; onAction: (k: CmdKey) => void }) {
  const [q, setQ] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => { if (open && inputRef.current) inputRef.current.focus(); }, [open]);
  useEffect(() => {
    const h = (e: KeyboardEvent) => { if (open && e.key === "Escape") onClose(); };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [open]);
  if (!open) return null;

  const filtered = ALL.filter(x => x.t.toLowerCase().includes(q.toLowerCase()));

  return (
    <div style={cp.overlay} onClick={onClose}>
      <div style={cp.box} onClick={e => e.stopPropagation()}>
        <div style={cp.inputRow}>
          <span style={{ color:"var(--neon)", fontSize:14 }}>⌘</span>
          <input ref={inputRef} value={q} onChange={e => setQ(e.target.value)}
            placeholder="Type a command or search…" style={cp.input} />
          <span style={{ fontSize:10, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace" }}>ESC</span>
        </div>
        <div style={cp.list}>
          {filtered.map(x => (
            <button key={x.k} onClick={() => { onAction(x.k); onClose(); }} style={cp.row}>
              <span style={cp.rowG}>{x.g}</span>
              <span style={{ flex:1 }}>{x.t}</span>
              {x.sh && <span style={cp.sh}>{x.sh}</span>}
            </button>
          ))}
          {filtered.length === 0 && (
            <div style={{ padding:20, textAlign:"center", color:"var(--ink-3)", fontSize:12 }}>No match.</div>
          )}
        </div>
      </div>
    </div>
  );
}

const cp: Record<string, CSSProperties> = {
  overlay:{ position:"fixed", inset:0, zIndex:400,
    background:"rgba(5,6,10,0.6)", backdropFilter:"blur(12px)",
    display:"flex", alignItems:"flex-start", justifyContent:"center",
    paddingTop:"12vh", animation:"fadeIn 0.15s" },
  box:{ width:"min(600px, 94vw)", background:"rgba(10,12,20,0.95)",
    border:"1px solid rgba(242,255,43,0.3)", borderRadius:12,
    boxShadow:"0 0 60px rgba(242,255,43,0.15), 0 20px 80px rgba(0,0,0,0.7)",
    overflow:"hidden" },
  inputRow:{ display:"flex", alignItems:"center", gap:12, padding:"14px 18px",
    borderBottom:"1px solid var(--line)" },
  input:{ flex:1, background:"transparent", border:"none", outline:"none",
    color:"#fff", fontSize:16, fontFamily:"Inter" },
  list:{ maxHeight:360, overflow:"auto", padding:6 },
  row:{ display:"flex", alignItems:"center", gap:12, width:"100%", padding:"10px 12px",
    background:"transparent", border:"none", borderRadius:6,
    color:"var(--ink-1)", cursor:"pointer", fontSize:13, textAlign:"left" },
  rowG:{ fontSize:9, letterSpacing:2, color:"var(--neon)", width:56,
    fontFamily:"'JetBrains Mono',monospace" },
  sh:{ fontSize:10, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace",
    border:"1px solid var(--line)", padding:"2px 6px", borderRadius:4 },
};
