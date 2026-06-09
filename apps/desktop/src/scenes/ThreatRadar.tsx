import { useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import { isTauri, onAuditNew } from "../lib/ipc";

type Ping = { angle: number; r: number; t: number; kind: string };

export function ThreatRadar({ redactionsDay, blocksWeek }: { redactionsDay: number; blocksWeek: number }) {
  const [sweep, setSweep] = useState(0);
  const [pings, setPings] = useState<Ping[]>([]);
  const rafRef = useRef(0);

  useEffect(() => {
    let running = true;
    const loop = () => {
      if (!running) return;
      setSweep(s => (s + 2) % 360);
      rafRef.current = requestAnimationFrame(loop);
    };
    loop();
    return () => { running = false; cancelAnimationFrame(rafRef.current); };
  }, []);

  useEffect(() => {
    if (!isTauri) return; // browser preview has no Tauri event bridge
    const unsub = onAuditNew(() => {
      setPings(p => [...p.slice(-8), {
        angle: Math.random() * 360, r: 10 + Math.random() * 45,
        t: performance.now(), kind: ["PII","KEY","FIN","ID"][Math.floor(Math.random()*4)],
      }]);
    });
    return () => { unsub.then(u => u()); };
  }, []);

  const now = performance.now();
  const livePings = pings.filter(p => now - p.t < 3000);

  return (
    <div style={tr.hud}>
      <div style={tr.head}>
        <span style={tr.dot} />
        <span>THREAT RADAR</span>
        <span style={{ marginLeft:"auto", color:"var(--neon)" }}>{livePings.length} ACTIVE</span>
      </div>
      <div style={tr.scope}>
        <svg viewBox="-60 -60 120 120" width="120" height="120">
          <defs>
            <radialGradient id="sweep">
              <stop offset="0%" stopColor="var(--neon)" stopOpacity="0.4"/>
              <stop offset="100%" stopColor="var(--neon)" stopOpacity="0"/>
            </radialGradient>
          </defs>
          <circle r="55" fill="rgba(242,255,43,0.02)" stroke="rgba(242,255,43,0.2)"/>
          <circle r="35" fill="none" stroke="rgba(242,255,43,0.1)"/>
          <circle r="15" fill="none" stroke="rgba(242,255,43,0.1)"/>
          <line x1="-55" y1="0" x2="55" y2="0" stroke="rgba(242,255,43,0.1)"/>
          <line x1="0" y1="-55" x2="0" y2="55" stroke="rgba(242,255,43,0.1)"/>
          <g transform={`rotate(${sweep})`}>
            <path d="M 0,0 L 55,0 A 55,55 0 0,1 27,47 z" fill="url(#sweep)"/>
          </g>
          {livePings.map((p, i) => {
            const x = Math.cos(p.angle * Math.PI/180) * p.r;
            const y = Math.sin(p.angle * Math.PI/180) * p.r;
            const age = (now - p.t)/3000;
            return (
              <g key={i}>
                <circle cx={x} cy={y} r={2 + age*4} fill="none" stroke="var(--neon)" strokeWidth="0.5" opacity={1-age}/>
                <circle cx={x} cy={y} r="1.5" fill="var(--neon)" opacity={1-age}/>
              </g>
            );
          })}
        </svg>
      </div>
      <div style={tr.stats}>
        <div><span style={{ color:"var(--neon)" }}>{redactionsDay.toLocaleString()}</span> redactions · 24h</div>
        <div><span style={{ color:"var(--neon)" }}>{blocksWeek.toLocaleString()}</span> blocked · 7d</div>
      </div>
    </div>
  );
}

const tr: Record<string, CSSProperties> = {
  hud:{ position:"fixed", right:20, top:80, width:180, zIndex:30,
    background:"rgba(10,12,20,0.85)", backdropFilter:"blur(12px)",
    border:"1px solid rgba(242,255,43,0.2)", borderRadius:10, padding:10,
    fontFamily:"'JetBrains Mono',monospace" },
  head:{ display:"flex", alignItems:"center", gap:6, fontSize:9, letterSpacing:2,
    color:"var(--ink-2)", marginBottom:6 },
  dot:{ width:6, height:6, borderRadius:99, background:"var(--neon)",
    boxShadow:"0 0 8px var(--neon)", animation:"pulse 1.2s infinite" },
  scope:{ display:"flex", justifyContent:"center" },
  stats:{ fontSize:9, color:"var(--ink-2)", marginTop:6, display:"flex", flexDirection:"column", gap:2 },
};
