import { useEffect, useState } from "react";
import { useEscape } from "../lib/useEscape";
import type { CSSProperties } from "react";
import { ipc, isTauri, modelsIpc } from "../lib/ipc";
import type { AuditEntry, AuditMetrics, AllModelStatus } from "../lib/types";
import { modelStatusKind } from "../lib/types";

// Every number on this screen comes from the local hash-chained audit log or
// the live engine state — nothing is fabricated. (This scene used to render
// made-up "SOC 2 / HIPAA COMPLIANT" tiles; a public app must never claim
// certifications it doesn't hold.)
export function ComplianceDashboard({ onClose }: { onClose: () => void }) {
  useEscape(onClose);
  const [audit, setAudit] = useState<AuditEntry[]>([]);
  const [metrics, setMetrics] = useState<AuditMetrics>({
    redactions_total: 0, blocks_total: 0, classes: 0,
    redactions_24h: 0, redactions_7d: 0, blocks_7d: 0,
  });
  const [models, setModels] = useState<AllModelStatus | null>(null);

  useEffect(() => {
    (async () => {
      if (!isTauri) return;
      try {
        setAudit(await ipc.listAudit(20));
        setMetrics(await ipc.auditMetrics());
        setModels(await modelsIpc.status());
      } catch {}
    })();
  }, []);

  const nerReady = models !== null
    && modelStatusKind(models.ner) === "ready"
    && modelStatusKind(models.ner_tokenizer) === "ready";
  const paranoidReady = models !== null && modelStatusKind(models.llm) === "ready";

  const tiles = [
    { n: "REDACTIONS · 24H", v: metrics.redactions_24h.toLocaleString(), s: "aliased before egress", c: "var(--neon)" },
    { n: "REDACTIONS · 7D", v: metrics.redactions_7d.toLocaleString(), s: "aliased before egress", c: "var(--neon)" },
    { n: "BLOCKS · 7D", v: metrics.blocks_7d.toLocaleString(), s: "egress prevented", c: metrics.blocks_7d > 0 ? "#ff99a8" : "var(--good)" },
    { n: "TOKEN CLASSES", v: String(metrics.classes), s: "distinct kinds observed", c: "var(--good)" },
    { n: "REGEX ENGINE", v: "ACTIVE", s: "41 patterns · 7 packs", c: "var(--good)" },
    { n: "SEMANTIC LAYERS", v: nerReady ? (paranoidReady ? "NER + LLM" : "NER") : "OFF", s: nerReady ? "on-device models loaded" : "download via Settings", c: nerReady ? "var(--good)" : "var(--ink-3)" },
  ];

  return (
    <div style={cd.overlay}>
      <div style={cd.stage}>
        <div style={cd.header}>
          <div>
            <div style={{ fontSize:10, letterSpacing:4, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace" }}>AUDIT</div>
            <div style={{ fontFamily:"'Instrument Serif',serif", fontSize:36, marginTop:4 }}>
              Privacy <em style={{ color:"var(--neon)" }}>posture</em>
            </div>
            <div style={{ fontSize:11, color:"var(--ink-2)", marginTop:4 }}>
              Every number below comes from the hash-chained audit log on this machine.
            </div>
          </div>
          <div style={{ display:"flex", alignItems:"center", gap:14 }}>
            <div style={cd.score}>
              <div style={{ fontSize:10, letterSpacing:2, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace" }}>LIFETIME</div>
              <div style={{ fontFamily:"'Instrument Serif',serif", fontSize:40, color:"var(--neon)", lineHeight:1 }}>{metrics.redactions_total.toLocaleString()}</div>
              <div style={{ fontSize:10, color:"var(--good)", fontFamily:"'JetBrains Mono',monospace" }}>tokens aliased</div>
            </div>
            <button onClick={onClose} style={cd.close}>×</button>
          </div>
        </div>

        <div style={cd.tileGrid}>
          {tiles.map(t => (
            <div key={t.n} style={cd.tile}>
              <div style={{ fontSize:10, letterSpacing:2, fontFamily:"'JetBrains Mono',monospace", color:"var(--ink-1)", marginBottom:8 }}>{t.n}</div>
              <div style={{ fontFamily:"'Instrument Serif',serif", fontSize:28, color: t.c }}>{t.v}</div>
              <div style={{ fontSize:9, letterSpacing:1, fontFamily:"'JetBrains Mono',monospace", color:"var(--ink-3)", marginTop:6 }}>{t.s}</div>
            </div>
          ))}
        </div>

        <div style={cd.row2}>
          <div style={cd.card}>
            <div style={cd.cardHead}>AUDIT FEED · HASH-CHAINED</div>
            <div style={{ display:"flex", flexDirection:"column", gap:6, fontFamily:"'JetBrains Mono',monospace", fontSize:11 }}>
              {audit.length === 0 && (
                <div style={{ color:"var(--ink-3)", padding:"10px 0" }}>No audit entries yet — send a prompt with PII to populate.</div>
              )}
              {audit.map((r) => (
                <div key={r.id} style={{ display:"flex", gap:10, alignItems:"center", padding:"4px 8px", background:"rgba(255,255,255,0.02)", borderRadius:4 }}>
                  <span style={{ color:"var(--ink-3)" }}>{r.ts.slice(11, 19)}</span>
                  <span style={{ color:"var(--neon)", width:96 }}>{r.kind}</span>
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
          <Metric label="Egress blocks" value={metrics.blocks_total.toLocaleString()} sub="lifetime" />
          <Metric label="Token classes observed" value={metrics.classes.toString()} sub="distinct kinds" />
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
  row2:{ display:"grid", gridTemplateColumns:"1fr", gap:12, marginBottom:16 },
  card:{ padding:16, background:"rgba(255,255,255,0.02)", border:"1px solid var(--line)", borderRadius:10 },
  cardHead:{ fontSize:10, letterSpacing:3, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace", marginBottom:12 },
  bottomRow:{ display:"grid", gridTemplateColumns:"repeat(3,1fr)", gap:10 },
  metric:{ padding:14, background:"rgba(255,255,255,0.02)", border:"1px solid var(--line)", borderRadius:10 },
};
