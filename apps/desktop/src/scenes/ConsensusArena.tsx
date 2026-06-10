import { useEffect, useRef, useState } from "react";
import { useEscape } from "../lib/useEscape";
import type { CSSProperties } from "react";
import { MODELS, PROVIDER_GLYPHS } from "../lib/models";
import { ipc, onStreamChunk, isTauri } from "../lib/ipc";
import type { Model } from "../lib/types";

interface Props { prompt: string; convId: string; onClose: () => void }

export function ConsensusArena({ prompt, convId, onClose }: Props) {
  useEscape(onClose);
  const models = [MODELS[3], MODELS[0], MODELS[6]];
  const [text, setText] = useState<string[]>(["", "", ""]);
  const [done, setDone] = useState<boolean[]>([false, false, false]);
  const msgIds = useRef<string[]>(["", "", ""]);

  useEffect(() => {
    let unsub: (() => void) | null = null;
    (async () => {
      if (isTauri && prompt.trim()) {
        try {
          const cols = await ipc.consensus({ conv_id: convId, model_ids: models.map(m => m.id), text: prompt });
          msgIds.current = cols.map(c => c.msg_id);
        } catch (e) {
          setText(() => [`error: ${String(e)}`, "", ""]);
          setDone([true, true, true]);
        }
        const p = onStreamChunk(c => {
          const idx = msgIds.current.indexOf(c.msg_id);
          if (idx === -1) return;
          setText(t => { const n=[...t]; n[idx] += c.delta; return n; });
          if (c.done) setDone(d => { const n=[...d]; n[idx] = true; return n; });
        });
        unsub = () => { p.then(u => u()); };
      } else {
        const targets = models.map((m, i) => [
          `${m.name} sees the Vendetta-aliased payload and returns a ${["structured memo","rapid bullet list","narrative exec summary"][i]}.`,
          `Projected revenue signals ${["stable growth","aggressive uplift","mixed recovery"][i]}. Recommend: ${["board review","follow-up session","roll-forward memo"][i]}.`,
        ].join("\n\n"));
        const ivs = targets.map((tgt, i) => {
          let j = 0;
          return setInterval(() => {
            j += Math.ceil(1 + Math.random()*2);
            setText(prev => { const n=[...prev]; n[i] = tgt.slice(0, j); return n; });
            if (j >= tgt.length) setDone(d => { const n=[...d]; n[i] = true; return n; });
          }, 18);
        });
        unsub = () => ivs.forEach(clearInterval);
      }
    })();
    return () => { unsub?.(); };
  }, []);

  const allDone = done.every(Boolean);

  return (
    <div style={ca.overlay}>
      <div style={ca.stage}>
        <div style={ca.header}>
          <div>
            <div style={{ fontSize:10, letterSpacing:4, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace" }}>MULTI-MODEL</div>
            <div style={{ fontFamily:"'Instrument Serif',serif", fontSize:32, marginTop:4 }}>
              Consensus <em style={{ color:"var(--neon)" }}>arena</em>
            </div>
            <div style={{ fontSize:11, color:"var(--ink-2)", marginTop:4 }}>One prompt · three models · one truth</div>
          </div>
          <button onClick={onClose} style={ca.close}>×</button>
        </div>

        <div style={ca.grid}>
          {models.map((m, i) => (
            <div key={m.id} style={{ ...ca.col, borderColor: m.color + "44" }}>
              <div style={ca.colHead}>
                <span style={{ color: m.color, fontSize:16 }}>{PROVIDER_GLYPHS[m.provider]}</span>
                <span style={{ fontSize:12, fontWeight:600 }}>{m.name}</span>
                <span style={{ marginLeft:"auto", fontSize:9, fontFamily:"'JetBrains Mono',monospace",
                  color: done[i] ? "var(--good)" : "var(--neon)" }}>
                  {done[i] ? "✓ DONE" : "STREAMING"}
                </span>
              </div>
              <div style={ca.colBody}>
                {text[i]}{!done[i] && <span className="caret" />}
              </div>
            </div>
          ))}
        </div>

        {allDone && (
          <div style={ca.synth}>
            <div style={ca.synthHead}>
              <span style={{ color:"var(--neon)", fontSize:14 }}>✦</span>
              <span style={{ fontSize:12, letterSpacing:3, fontFamily:"'JetBrains Mono',monospace", color:"var(--neon)" }}>SENTYNYX SYNTHESIS</span>
            </div>
            <div style={ca.synthBody}>
              All three models responded through the Vendetta perimeter; only aliased payloads reached upstream.
              Compare columns above for agreement and disagreement markers.
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

const ca: Record<string, CSSProperties> = {
  overlay:{ position:"fixed", inset:0, zIndex:250,
    background:"radial-gradient(ellipse at center, rgba(5,6,10,0.85), rgba(5,6,10,0.98))",
    backdropFilter:"blur(14px)",
    display:"flex", alignItems:"center", justifyContent:"center", animation:"fadeIn 0.3s" },
  stage:{ width:"min(1180px, 96vw)", maxHeight:"92vh",
    display:"flex", flexDirection:"column",
    background:"rgba(10,12,20,0.95)",
    border:"1px solid rgba(255,255,255,0.1)", borderRadius:14,
    padding:24, overflow:"auto" },
  header:{ display:"flex", justifyContent:"space-between", alignItems:"flex-end", marginBottom:20 },
  close:{ width:36, height:36, borderRadius:8, background:"rgba(255,255,255,0.05)",
    border:"1px solid var(--line)", color:"var(--ink-1)", fontSize:20, cursor:"pointer" },
  grid:{ display:"grid", gridTemplateColumns:"repeat(3,1fr)", gap:12 },
  col:{ padding:14, background:"rgba(255,255,255,0.02)",
    border:"1px solid", borderRadius:10, minHeight:260 },
  colHead:{ display:"flex", alignItems:"center", gap:8, marginBottom:10, paddingBottom:10,
    borderBottom:"1px solid var(--line)" },
  colBody:{ fontSize:12, lineHeight:1.6, color:"var(--ink-1)", whiteSpace:"pre-wrap" },
  synth:{ marginTop:16, padding:16, borderRadius:10,
    background:"linear-gradient(180deg, rgba(242,255,43,0.08), rgba(242,255,43,0.02))",
    border:"1px solid rgba(242,255,43,0.3)", animation:"fadeIn 0.4s" },
  synthHead:{ display:"flex", alignItems:"center", gap:8, marginBottom:10 },
  synthBody:{ fontSize:13, lineHeight:1.7, color:"var(--ink-0)" },
};
