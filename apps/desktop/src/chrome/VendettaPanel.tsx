import { useMemo } from "react";
import { sx } from "./styles";
import type { Span } from "../lib/types";
import { confidenceFor, sourceForKind, sourceGlyph, sourceTooltip } from "../lib/vendetta";

interface Props {
  spans: Span[];
  open: boolean;
  onClose: () => void;
  aliasMode: "mask" | "alias" | "raw";
  setAliasMode: (m: "mask" | "alias" | "raw") => void;
}

export function VendettaPanel({ spans, open, onClose, aliasMode, setAliasMode }: Props) {
  const byKind = useMemo(() => {
    const g: Record<string, Span[]> = {};
    for (const s of spans) { (g[s.kind] ||= []).push(s); }
    return g;
  }, [spans]);

  const kinds = Object.keys(byKind);
  const total = spans.length;

  return (
    <div data-tour="vendetta-panel" style={{ ...sx.vPanel, transform: open ? "translateX(0)" : "translateX(110%)" }}>
      <div style={sx.vHead}>
        <div style={{ display:"flex", alignItems:"center", gap:10 }}>
          <div style={sx.vBadge}>
            <span style={sx.vBadgeDot} />V
          </div>
          <div>
            <div style={{ fontSize:14, fontWeight:600, letterSpacing:1 }}>VENDETTA</div>
            <div style={{ fontSize:10, color:"var(--ink-3)", letterSpacing:2 }}>REDACTION ENGINE</div>
          </div>
        </div>
        <button style={sx.iconBtn} onClick={onClose}>×</button>
      </div>

      <div style={sx.vStats}>
        <div style={sx.vStat}>
          <div style={sx.vStatNum}>{total}</div>
          <div style={sx.vStatLbl}>detected</div>
        </div>
        <div style={sx.vStat}>
          <div style={sx.vStatNum}>{kinds.length}</div>
          <div style={sx.vStatLbl}>classes</div>
        </div>
        <div style={sx.vStat}>
          <div style={{ ...sx.vStatNum, color:"var(--good)" }}>{total > 0 ? "ON" : "IDLE"}</div>
          <div style={sx.vStatLbl}>shield</div>
        </div>
      </div>
      <div style={{
        padding: "6px 14px", fontSize: 9.5, color: "var(--ink-3)",
        fontFamily: "'JetBrains Mono', monospace", letterSpacing: 0.5,
        borderTop: "1px solid rgba(255,255,255,0.05)",
      }}>
        hover a token for source + confidence — 100% checksum-validated · 95% structural · 85% context-anchored · 75% heuristic
      </div>

      <div style={{ padding:"0 18px" }}>
        <div style={sx.aliasToggle}>
          <button onClick={() => setAliasMode("mask")} style={{ ...sx.aliasTab, ...(aliasMode === "mask" ? sx.aliasTabOn : {}) }}>Masked</button>
          <button onClick={() => setAliasMode("alias")} style={{ ...sx.aliasTab, ...(aliasMode === "alias" ? sx.aliasTabOn : {}) }}>Aliased</button>
          <button onClick={() => setAliasMode("raw")} style={{ ...sx.aliasTab, ...(aliasMode === "raw" ? sx.aliasTabOn : {}) }}>Raw (internal)</button>
        </div>
      </div>

      <div style={sx.vList}>
        {total === 0 && (
          <div style={sx.vEmpty}>
            <div style={{ fontSize:11, letterSpacing:2, color:"var(--ink-3)", marginBottom:8 }}>NO SENSITIVE DATA</div>
            <div style={{ fontSize:12, color:"var(--ink-2)", lineHeight:1.6 }}>
              Start typing in the prompt box. Vendetta scans every keystroke for PII, API keys, project codenames, employee IDs, financials and more.
            </div>
          </div>
        )}
        {kinds.map(k => (
          <div key={k} style={sx.vGroup}>
            <div style={sx.vGroupHead}>
              <span style={sx.vDotNeon} />
              <span>{k}</span>
              <span style={{ opacity:0.4 }}>×{byKind[k].length}</span>
            </div>
            {byKind[k].map((s, i) => {
              const conf = s.confidence ?? confidenceFor(s.kind);
              return (
                <div key={i} style={sx.vItem}>
                  <div style={sx.vItemRaw}>{aliasMode === "mask" ? "•".repeat(Math.min(s.raw.length, 10)) : s.raw}</div>
                  <div style={sx.vItemArrow}>→</div>
                  <div style={sx.vItemAlias}>
                    <span title={`${sourceTooltip(sourceForKind(s.kind))} · ${Math.round(conf * 100)}% confidence`}>
                      <span style={{ opacity: 0.6, marginRight: 4 }}>{sourceGlyph(sourceForKind(s.kind))}</span>
                      {s.alias}
                      <span style={{
                        marginLeft: 6, fontSize: 9, opacity: 0.7,
                        color: conf >= 0.95 ? "#7cffb2" : conf >= 0.85 ? "var(--neon)" : "#fbbf24",
                      }}>{Math.round(conf * 100)}%</span>
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        ))}
      </div>

      <div style={sx.vFoot}>
        <div style={{ fontSize:10, color:"var(--ink-3)", lineHeight:1.5 }}>
          Outbound payload is rewritten before the API call. Responses are re-hydrated locally — the model never sees the raw values.
        </div>
      </div>
    </div>
  );
}
