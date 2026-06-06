import { useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import { MODELS, PROVIDER_GLYPHS } from "../lib/models";
import type { Model } from "../lib/types";

export function OrbitalPicker({ model, setModel, onClose, models = MODELS }: { model: Model; setModel: (m: Model) => void; onClose: () => void; models?: Model[] }) {
  const [hover, setHover] = useState<string | null>(null);
  const [rot, setRot] = useState(0);
  const rafRef = useRef(0);

  useEffect(() => {
    let running = true;
    const loop = () => { if (!running) return; setRot(r => r + 0.05); rafRef.current = requestAnimationFrame(loop); };
    loop();
    return () => { running = false; cancelAnimationFrame(rafRef.current); };
  }, []);

  const orbits = [
    { r: 120, ms: models.slice(0, 5),  speed: 1.0 },
    { r: 200, ms: models.slice(5, 10), speed: 0.6 },
    { r: 280, ms: models.slice(10),    speed: 0.3 },
  ];

  return (
    <div style={op.overlay} onClick={onClose}>
      <div style={op.stage} onClick={e => e.stopPropagation()}>
        <div style={op.header}>
          <div>
            <div style={{ fontSize:10, letterSpacing:4, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace" }}>FLEET</div>
            <div style={{ fontFamily:"'Instrument Serif',serif", fontSize:34, marginTop:4 }}>
              Choose your <em style={{ color:"var(--neon)" }}>orbit</em>
            </div>
          </div>
          <button onClick={onClose} style={op.close}>×</button>
        </div>

        <div style={op.galaxy}>
          <svg style={op.orbits} viewBox="-320 -320 640 640">
            {orbits.map((o, i) => (
              <circle key={i} cx="0" cy="0" r={o.r} fill="none"
                stroke="rgba(242,255,43,0.15)" strokeWidth="0.5" strokeDasharray="2 6" />
            ))}
            <circle cx="0" cy="0" r="55" fill="url(#coreGlow)" />
            <defs>
              <radialGradient id="coreGlow">
                <stop offset="0%" stopColor="#f2ff2b" stopOpacity="0.8"/>
                <stop offset="50%" stopColor="#f2ff2b" stopOpacity="0.2"/>
                <stop offset="100%" stopColor="#f2ff2b" stopOpacity="0"/>
              </radialGradient>
            </defs>
          </svg>

          <div style={op.core}>
            <div style={{ fontSize:9, letterSpacing:3, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace" }}>VENDETTA</div>
            <div style={{ fontSize:12, color:"var(--neon)", fontFamily:"'JetBrains Mono',monospace", marginTop:2 }}>PERIMETER</div>
            <div style={op.coreDot} />
          </div>

          {orbits.map((o) =>
            o.ms.map((m, mi) => {
              const total = o.ms.length;
              const angle = (rot * o.speed) + (mi * (360 / total));
              const rad = (angle * Math.PI) / 180;
              const x = Math.cos(rad) * o.r;
              const y = Math.sin(rad) * o.r;
              const active = m.id === model.id;
              const isHover = hover === m.id;
              return (
                <button
                  key={m.id}
                  onMouseEnter={() => setHover(m.id)}
                  onMouseLeave={() => setHover(null)}
                  onClick={() => { setModel(m); setTimeout(onClose, 300); }}
                  style={{
                    ...op.sat,
                    transform:`translate(calc(-50% + ${x}px), calc(-50% + ${y}px))`,
                    borderColor: active ? "var(--neon)" : (isHover ? m.color : "rgba(255,255,255,0.15)"),
                    background: active ? "rgba(242,255,43,0.15)" : "rgba(10,12,20,0.8)",
                    boxShadow: active ? "0 0 24px rgba(242,255,43,0.6)" : (isHover ? `0 0 16px ${m.color}66` : "none"),
                    zIndex: isHover || active ? 10 : 1,
                  }}>
                  <span style={{ color:m.color, fontSize:16 }}>{PROVIDER_GLYPHS[m.provider]}</span>
                  {(isHover || active) && (
                    <div style={op.satLabel}>
                      <div style={{ fontSize:11, fontWeight:600 }}>{m.name}</div>
                      <div style={{ fontSize:9, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace", marginTop:2 }}>
                        {m.provider} · {m.ctx} · {m.flash}
                      </div>
                    </div>
                  )}
                  {active && <div style={op.dockRing} />}
                </button>
              );
            })
          )}

          {(() => {
            const m = models.find(x => x.id === model.id);
            if (!m) return null;
            const idx = models.indexOf(m);
            const oi = idx < 5 ? 0 : idx < 10 ? 1 : 2;
            const o = orbits[oi];
            const idxInOrbit = o.ms.findIndex(x => x.id === m.id);
            const angle = (rot * o.speed) + (idxInOrbit * (360 / o.ms.length));
            const rad = (angle * Math.PI) / 180;
            const x = Math.cos(rad) * o.r;
            const y = Math.sin(rad) * o.r;
            return (
              <svg style={op.beam} viewBox="-320 -320 640 640">
                <line x1="0" y1="0" x2={x} y2={y}
                  stroke="var(--neon)" strokeWidth="1" opacity="0.5" strokeDasharray="3 3" />
              </svg>
            );
          })()}
        </div>

        <div style={op.footer}>
          <div style={{ display:"flex", gap:16, fontSize:11, color:"var(--ink-2)", fontFamily:"'JetBrains Mono',monospace" }}>
            <span><span style={{ color:"var(--neon)" }}>●</span> Active: {model.name}</span>
            <span>·</span>
            <span>{models.length} models · {new Set(models.map(m => m.provider)).size} providers</span>
          </div>
          <div style={{ fontSize:10, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace", letterSpacing:1 }}>
            All satellites route through Vendetta before contact
          </div>
        </div>
      </div>
    </div>
  );
}

const op: Record<string, CSSProperties> = {
  overlay:{ position:"fixed", inset:0, zIndex:200,
    background:"radial-gradient(ellipse at center, rgba(5,6,10,0.85), rgba(5,6,10,0.98))",
    backdropFilter:"blur(12px)",
    display:"flex", alignItems:"center", justifyContent:"center", animation:"fadeIn 0.3s ease-out" },
  stage:{ position:"relative", width:760, maxWidth:"94vw", height:700, maxHeight:"94vh",
    display:"flex", flexDirection:"column" },
  header:{ display:"flex", justifyContent:"space-between", alignItems:"flex-end", padding:"0 20px 20px" },
  close:{ width:36, height:36, borderRadius:8, background:"rgba(255,255,255,0.05)",
    border:"1px solid var(--line)", color:"var(--ink-1)", fontSize:20, cursor:"pointer" },
  galaxy:{ flex:1, position:"relative", display:"flex", alignItems:"center", justifyContent:"center" },
  orbits:{ position:"absolute", width:640, height:640 },
  beam:{ position:"absolute", width:640, height:640, pointerEvents:"none" },
  core:{ position:"absolute", top:"50%", left:"50%", transform:"translate(-50%,-50%)",
    textAlign:"center", width:110, height:110,
    border:"1px solid rgba(242,255,43,0.5)", borderRadius:"50%",
    display:"flex", flexDirection:"column", alignItems:"center", justifyContent:"center",
    background:"radial-gradient(circle, rgba(242,255,43,0.08), transparent 70%)" },
  coreDot:{ position:"absolute", width:8, height:8, borderRadius:99,
    background:"var(--neon)", boxShadow:"0 0 16px var(--neon)",
    bottom:-4, left:"50%", transform:"translateX(-50%)", animation:"pulse 1.5s infinite" },
  sat:{ position:"absolute", top:"50%", left:"50%",
    width:44, height:44, borderRadius:"50%",
    display:"flex", alignItems:"center", justifyContent:"center",
    border:"1px solid rgba(255,255,255,0.15)", background:"rgba(10,12,20,0.8)",
    backdropFilter:"blur(8px)", cursor:"pointer",
    transition:"border-color 0.2s, box-shadow 0.2s, background 0.2s" },
  satLabel:{ position:"absolute", top:"calc(100% + 10px)", left:"50%", transform:"translateX(-50%)",
    whiteSpace:"nowrap", padding:"6px 10px",
    background:"rgba(5,6,10,0.95)", border:"1px solid var(--line)",
    borderRadius:6, pointerEvents:"none", zIndex:20 },
  dockRing:{ position:"absolute", inset:-8, borderRadius:"50%",
    border:"1px solid var(--neon)", animation:"pulse 1.5s infinite" },
  footer:{ display:"flex", justifyContent:"space-between", alignItems:"center",
    padding:20, borderTop:"1px solid var(--line)" },
};
