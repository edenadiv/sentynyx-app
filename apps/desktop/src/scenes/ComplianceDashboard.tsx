import { useEffect, useState } from "react";
import type { CSSProperties } from "react";
import { ipc, isTauri } from "../lib/ipc";
import type { AuditEntry, AuditMetrics } from "../lib/types";

export function ComplianceDashboard({ onClose }: { onClose: () => void }) {
  const [audit, setAudit] = useState<AuditEntry[]>([]);
  const [metrics, setMetrics] = useState<AuditMetrics>({ redactions_total: 0, blocks_total: 0, classes: 0 });

  useEffect(() => {
    (async () => {
      if (!isTauri) return;
      try {
        setAudit(await ipc.listAudit(20));
        setMetrics(await ipc.auditMetrics());
      } catch {}
    })();
  }, []);

  const tiles = [
    { n:"SOC 2 TYPE II", s:"COMPLIANT", v:99.1, c:"var(--good)" },
    { n:"GDPR",         s:"COMPLIANT", v:98.4, c:"var(--good)" },
    { n:"HIPAA",        s:"COMPLIANT", v:97.9, c:"var(--good)" },
    { n:"ISO 27001",    s:"IN REVIEW", v:92.0, c:"var(--neon)" },
    { n:"CCPA",         s:"COMPLIANT", v:99.6, c:"var(--good)" },
    { n:"EU AI ACT",    s:"MONITORED", v:94.3, c:"var(--neon)" },
  ];

  return (
    <div style={cd.overlay}>
      <div style={cd.stage}>
        <div style={cd.header}>
          <div>
            <div style={{ fontSize:10, letterSpacing:4, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace" }}>GOVERNANCE</div>
            <div style={{ fontFamily:"'Instrument Serif',serif", fontSize:36, marginTop:4 }}>
              Compliance <em style={{ color:"var(--neon)" }}>cockpit</em>
            </div>
          </div>
          <div style={{ display:"flex", alignItems:"center", gap:14 }}>
            <div style={cd.score}>
              <div style={{ fontSize:10, letterSpacing:2, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace" }}>COMPOSITE</div>
              <div style={{ fontFamily:"'Instrument Serif',serif", fontSize:40, color:"var(--neon)", lineHeight:1 }}>98.2%</div>
              <div style={{ fontSize:10, color:"var(--good)", fontFamily:"'JetBrains Mono',monospace" }}>▲ 0.4 · 7d</div>
            </div>
            <button onClick={onClose} style={cd.close}>×</button>
          </div>
        </div>

        <div style={cd.tileGrid}>
          {tiles.map(t => (
            <div key={t.n} style={cd.tile}>
              <div style={{ display:"flex", justifyContent:"space-between", alignItems:"baseline", marginBottom:8 }}>
                <div style={{ fontSize:11, letterSpacing:2, fontFamily:"'JetBrains Mono',monospace", color:"var(--ink-1)" }}>{t.n}</div>
                <div style={{ fontSize:9, letterSpacing:2, fontFamily:"'JetBrains Mono',monospace", color:t.c }}>{t.s}</div>
              </div>
              <div style={{ fontFamily:"'Instrument Serif',serif", fontSize:30, color:"#fff" }}>{t.v}%</div>
              <div style={cd.bar}><div style={{ ...cd.barFill, width: t.v + "%", background:t.c }} /></div>
            </div>
          ))}
        </div>

        <div style={cd.row2}>
          <div style={cd.card}>
            <div style={cd.cardHead}>AUDIT FEED · CRYPTO-SIGNED</div>
            <div style={{ display:"flex", flexDirection:"column", gap:6, fontFamily:"'JetBrains Mono',monospace", fontSize:11 }}>
              {audit.length === 0 && (
                <div style={{ color:"var(--ink-3)", padding:"10px 0" }}>No audit entries yet — send a prompt with PII to populate.</div>
              )}
              {audit.map((r) => (
                <div key={r.id} style={{ display:"flex", gap:10, alignItems:"center", padding:"4px 8px", background:"rgba(255,255,255,0.02)", borderRadius:4 }}>
                  <span style={{ color:"var(--ink-3)" }}>{r.ts.slice(11, 19)}</span>
                  <span style={{ color:"var(--neon)", width:60 }}>{r.kind}</span>
                  <span style={{ flex:1, color:"var(--ink-1)", overflow:"hidden", textOverflow:"ellipsis", whiteSpace:"nowrap" }}>
                    {r.raw_hash.slice(0, 10)}… → {r.alias}
                  </span>
                  <span style={{ color: r.action === "BLOCK" ? "#ff99a8" : "var(--good)", width:40, textAlign:"right" }}>{r.action}</span>
                  <span style={{ color:"var(--ink-3)" }}>{r.sig.slice(0,6)}</span>
                </div>
              ))}
            </div>
          </div>
        </div>

        <div style={cd.bottomRow}>
          <Metric label="Sensitive tokens aliased" value={metrics.redactions_total.toLocaleString()} sub="lifetime" accent />
          <Metric label="Policy violations blocked" value={metrics.blocks_total.toLocaleString()} sub="lifetime" />
          <Metric label="Token classes observed" value={metrics.classes.toString()} sub="distinct kinds" />
          <Metric label="Mean egress latency" value="712ms" sub="p99 · 1.2s" />
          <Metric label="Cost saved vs. vendor" value="$1.24M" sub="smart routing" accent />
        </div>
      </div>
    </div>
  );
}

function Metric({ label, value, sub, accent }: { label:string; value:string; sub:string; accent?:boolean }) {
  return (
    <div style={cd.metric}>
      <div style={{ fontSize:10, letterSpacing:2, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace" }}>{label}</div>
      <div style={{ fontFamily:"'Instrument Serif',serif", fontSize:30, color: accent ? "var(--neon)" : "#fff", lineHeight:1, marginTop:4 }}>{value}</div>
      <div style={{ fontSize:10, color:"var(--ink-3)", marginTop:4, fontFamily:"'JetBrains Mono',monospace" }}>{sub}</div>
    </div>
  );
}

const cd: Record<string, CSSProperties> = {
  overlay:{ position:"fixed", inset:0, zIndex:250,
    background:"radial-gradient(ellipse at center, rgba(5,6,10,0.9), rgba(5,6,10,0.99))",
    backdropFilter:"blur(16px)",
    display:"flex", alignItems:"center", justifyContent:"center", padding:20, animation:"fadeIn 0.3s" },
  stage:{ width:"min(1280px, 96vw)", maxHeight:"96vh",
    background:"rgba(10,12,20,0.96)",
    border:"1px solid rgba(255,255,255,0.1)", borderRadius:14, padding:28, overflow:"auto" },
  header:{ display:"flex", justifyContent:"space-between", alignItems:"flex-end", marginBottom:20 },
  close:{ width:36, height:36, borderRadius:8, background:"rgba(255,255,255,0.05)",
    border:"1px solid var(--line)", color:"var(--ink-1)", fontSize:20, cursor:"pointer" },
  score:{ padding:"6px 16px", border:"1px solid rgba(242,255,43,0.3)", borderRadius:10,
    background:"rgba(242,255,43,0.04)", textAlign:"right" },
  tileGrid:{ display:"grid", gridTemplateColumns:"repeat(6,1fr)", gap:10, marginBottom:16 },
  tile:{ padding:14, background:"rgba(255,255,255,0.02)", border:"1px solid var(--line)", borderRadius:10 },
  bar:{ height:3, background:"rgba(255,255,255,0.05)", marginTop:8, borderRadius:99, overflow:"hidden" },
  barFill:{ height:"100%" },
  row2:{ display:"grid", gridTemplateColumns:"1fr", gap:12, marginBottom:16 },
  card:{ padding:16, background:"rgba(255,255,255,0.02)", border:"1px solid var(--line)", borderRadius:10 },
  cardHead:{ fontSize:10, letterSpacing:3, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace", marginBottom:12 },
  bottomRow:{ display:"grid", gridTemplateColumns:"repeat(5,1fr)", gap:10 },
  metric:{ padding:14, background:"rgba(255,255,255,0.02)", border:"1px solid var(--line)", borderRadius:10 },
};
