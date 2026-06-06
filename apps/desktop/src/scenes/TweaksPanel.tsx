import { MODELS } from "../lib/models";
import type { Tweaks } from "../lib/types";

interface Props { tweaks: Tweaks; update: <K extends keyof Tweaks>(k: K, v: Tweaks[K]) => void; onClose: () => void }

export function TweaksPanel({ tweaks, update, onClose }: Props) {
  const accents = [
    { c:"#f2ff2b", n:"Electric" },
    { c:"#b3ff3c", n:"Toxic" },
    { c:"#ff3cc1", n:"Fuchsia" },
    { c:"#3cf0ff", n:"Plasma" },
    { c:"#ff7a2b", n:"Ember" },
    { c:"#ffffff", n:"White" },
  ];
  return (
    <div style={{
      position:"fixed", right:20, bottom:20, width:300, zIndex:200,
      background:"rgba(10,12,20,0.94)", backdropFilter:"blur(20px)",
      border:"1px solid rgba(255,255,255,0.12)", borderRadius:12,
      boxShadow:"0 20px 60px rgba(0,0,0,0.6)", padding:16, color:"#fff"
    }}>
      <div style={{ display:"flex", justifyContent:"space-between", alignItems:"center", marginBottom:14 }}>
        <div style={{ fontSize:11, letterSpacing:3, fontFamily:"'JetBrains Mono',monospace", color:"var(--ink-2)" }}>TWEAKS</div>
        <button onClick={onClose} style={{ background:"transparent", border:"1px solid var(--line)", color:"var(--ink-1)", borderRadius:6, width:24, height:24, cursor:"pointer" }}>×</button>
      </div>

      <div style={{ marginBottom:14 }}>
        <div style={{ fontSize:10, letterSpacing:2, color:"var(--ink-3)", marginBottom:8, fontFamily:"'JetBrains Mono',monospace" }}>NEON ACCENT</div>
        <div style={{ display:"flex", gap:6, flexWrap:"wrap" }}>
          {accents.map(a => (
            <button key={a.c} onClick={() => update("accent", a.c)} title={a.n} style={{
              width:28, height:28, borderRadius:6,
              background: a.c, cursor:"pointer",
              border: tweaks.accent === a.c ? "2px solid #fff" : "1px solid rgba(255,255,255,0.1)",
              boxShadow: tweaks.accent === a.c ? `0 0 12px ${a.c}` : "none"
            }} />
          ))}
        </div>
      </div>

      <div style={{ marginBottom:14 }}>
        <div style={{ fontSize:10, letterSpacing:2, color:"var(--ink-3)", marginBottom:8, fontFamily:"'JetBrains Mono',monospace" }}>DEFAULT MODEL</div>
        <select value={tweaks.defaultModelIdx} onChange={e => update("defaultModelIdx", +e.target.value)}
          style={{ width:"100%", padding:8, background:"rgba(255,255,255,0.03)",
            border:"1px solid var(--line)", color:"#fff", borderRadius:6, fontSize:12 }}>
          {MODELS.map((m, i) => (
            <option key={m.id} value={i} style={{ background:"#10131c" }}>{m.provider} — {m.name}</option>
          ))}
        </select>
      </div>

      <div style={{ display:"flex", gap:10, marginBottom:10 }}>
        <label style={{ display:"flex", alignItems:"center", gap:6, fontSize:11, color:"var(--ink-1)", cursor:"pointer" }}>
          <input type="checkbox" checked={tweaks.starfield} onChange={e => update("starfield", e.target.checked)} />
          Starfield
        </label>
        <label style={{ display:"flex", alignItems:"center", gap:6, fontSize:11, color:"var(--ink-1)", cursor:"pointer" }}>
          <input type="checkbox" checked={tweaks.scanAnim} onChange={e => update("scanAnim", e.target.checked)} />
          Scan animation
        </label>
      </div>
    </div>
  );
}
