import { useEffect, useRef } from "react";
import type { CSSProperties } from "react";
import { detect } from "../lib/vendetta";
import { ipc, isTauri } from "../lib/ipc";
import { PROVIDER_GLYPHS } from "../lib/models";
import type { Model, Span } from "../lib/types";

interface Props {
  model: Model;
  onSend: (text: string, spans: Span[]) => void;
  spans: Span[];
  setSpans: (s: Span[]) => void;
  text: string;
  setText: (t: string) => void;
  onToggleVendetta: () => void;
  vendettaOpen: boolean;
}

/// Mirror of `detect::merge_spans` in the Rust backend: regex wins on
/// overlap, non-overlapping NER spans are kept, output sorted by start.
function mergeSpans(regex: Span[], ner: Span[]): Span[] {
  const out: Span[] = [...regex];
  for (const n of ner) {
    const overlaps = regex.some(r => r.start < n.end && n.start < r.end);
    if (!overlaps) out.push(n);
  }
  out.sort((a, b) => a.start - b.start);
  return out;
}

export function Highlighted({ text, spans }: { text: string; spans: Span[] }) {
  if (spans.length === 0) return <span>{text}</span>;
  const pieces: React.ReactNode[] = [];
  let cur = 0;
  spans.forEach((s, i) => {
    if (s.start > cur) pieces.push(<span key={"p" + i}>{text.slice(cur, s.start)}</span>);
    pieces.push(
      <span key={"h" + i} className={"vend-hit vend-" + s.kind}>
        <span className="vend-raw">{text.slice(s.start, s.end)}</span>
        <span className="vend-tag">{s.kind}</span>
      </span>
    );
    cur = s.end;
  });
  if (cur < text.length) pieces.push(<span key="tail">{text.slice(cur)}</span>);
  return <>{pieces}</>;
}

export function Composer({ model, onSend, spans, setSpans, text, setText, onToggleVendetta, vendettaOpen }: Props) {
  const taRef = useRef<HTMLTextAreaElement>(null);
  const mirrorRef = useRef<HTMLDivElement>(null);
  const latestText = useRef(text);
  // NER spans from the most recent backend response. Kept "sticky" across
  // keystrokes if their offsets still point at the same raw substring, so
  // names/orgs don't flicker off every time the user types another character.
  const lastNerSpans = useRef<Span[]>([]);

  // Instant client-side regex + carry-over NER on every keystroke. NER spans
  // whose offsets still line up with the live text are kept; ones that were
  // edited through are dropped. The next debounce cycle will refresh both.
  useEffect(() => {
    latestText.current = text;
    const regexSpans = detect(text);
    const validNer = lastNerSpans.current.filter(
      s => s.end <= text.length && text.slice(s.start, s.end) === s.raw
    );
    setSpans(mergeSpans(regexSpans, validNer));
  }, [text]);

  // Debounced server-side `detect_with_ner` — runs regex + NER in parallel on
  // the backend and upgrades the span list with names / orgs / codenames /
  // locations / employee IDs that regex can't template. Fires 350 ms after
  // the user stops typing; race-guarded so stale responses are dropped.
  useEffect(() => {
    if (!isTauri) return;
    const queriedText = text;
    if (!queriedText.trim()) { lastNerSpans.current = []; return; }
    const timer = setTimeout(async () => {
      try {
        const r = await ipc.detectWithNer(queriedText);
        if (latestText.current !== queriedText) return;
        lastNerSpans.current = r.spans.filter(s => s.kind.endsWith("_NER"));
        setSpans(r.spans);
      } catch {
        // Keep current highlights. Backend may be unavailable during
        // first-run model download or early startup.
      }
    }, 350);
    return () => clearTimeout(timer);
  }, [text]);

  const onScroll = () => {
    if (mirrorRef.current && taRef.current) {
      mirrorRef.current.scrollTop = taRef.current.scrollTop;
      mirrorRef.current.scrollLeft = taRef.current.scrollLeft;
    }
  };

  const submit = () => { if (!text.trim()) return; onSend(text, spans); };

  const onKey = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); submit(); }
  };

  return (
    <div style={cx.wrap}>
      <div style={{ ...cx.glow, opacity: spans.length > 0 ? 1 : 0 }} />

      <div data-tour="composer" style={{ ...cx.shell, borderColor: spans.length > 0 ? "rgba(242,255,43,0.4)" : "rgba(255,255,255,0.1)" }}>
        <div style={cx.liveStrip}>
          <div style={cx.liveLeft}>
            <span style={cx.liveDotY} />
            <span style={{ fontSize:10, letterSpacing:2, fontFamily:"'JetBrains Mono',monospace", color:"var(--ink-2)" }}>
              VENDETTA
            </span>
            <span style={{ fontSize:10, color: spans.length > 0 ? "var(--neon)" : "var(--ink-3)", fontFamily:"'JetBrains Mono',monospace" }}>
              {spans.length > 0
                ? `${spans.length} sensitive ${spans.length === 1 ? "token" : "tokens"} detected · will be anonymized before ${model.provider}`
                : "Listening · no PII in buffer"}
            </span>
          </div>
          <button data-tour="vendetta-toggle" onClick={onToggleVendetta} style={cx.livePanelBtn}>
            {vendettaOpen ? "HIDE" : "VIEW"} PANEL →
          </button>
        </div>

        <div style={cx.editorWrap}>
          <div ref={mirrorRef} style={cx.mirror} aria-hidden="true">
            <Highlighted text={text + "\u200b"} spans={spans} />
          </div>
          <textarea
            ref={taRef}
            value={text}
            onChange={e => setText(e.target.value)}
            onScroll={onScroll}
            onKeyDown={onKey}
            placeholder="Transmit to the fleet…  (⌘↵ to send)"
            style={cx.textarea}
            spellCheck={false}
          />
        </div>

        <div style={cx.toolbar}>
          <div style={{ display:"flex", alignItems:"center", gap:6 }}>
            <button style={cx.toolBtn} title="Attach"><span style={{ fontSize:14 }}>⏚</span></button>
            <button style={cx.toolBtn} title="Knowledge base"><span style={{ fontSize:12 }}>⌯</span><span>Atlas</span></button>
            <button style={cx.toolBtn} title="Tools"><span style={{ fontSize:12 }}>∷</span><span>Tools</span></button>
            <button style={cx.toolBtn} title="Voice"><span style={{ fontSize:12 }}>◉</span></button>
          </div>
          <div style={{ display:"flex", alignItems:"center", gap:10 }}>
            <div style={cx.routeInfo}>
              <span style={{ fontFamily:"'JetBrains Mono',monospace", fontSize:10, color:"var(--ink-3)", letterSpacing:1 }}>ROUTE</span>
              <span style={{ fontSize:11 }}>
                <span style={{ color:"var(--neon)" }}>⛨</span> Vendetta
                <span style={{ color:"var(--ink-3)", margin:"0 6px" }}>→</span>
                <span style={{ color: model.color }}>{PROVIDER_GLYPHS[model.provider]}</span> {model.name}
              </span>
            </div>
            <button data-tour="transmit" onClick={submit} style={{ ...cx.sendBtn, opacity: text.trim() ? 1 : 0.4 }}>
              <span>Transmit</span><span style={{ fontSize:14 }}>↗</span>
            </button>
          </div>
        </div>
      </div>

      <div style={cx.helperRow}>
        <span>{text.length} chars</span><span>·</span>
        <span>~{Math.ceil(text.length / 4)} tokens</span><span>·</span>
        <span>Payload egress is end-to-end scanned by the Vendetta engine</span>
      </div>
    </div>
  );
}

export const cx: Record<string, CSSProperties> = {
  wrap:{ position:"relative", margin:"0 auto", width:"min(780px, 94%)", paddingBottom:18 },
  glow:{ position:"absolute", inset:"-40px -40px 0 -40px",
    background:"radial-gradient(60% 80% at 50% 100%, rgba(242,255,43,0.22), transparent 70%)",
    filter:"blur(30px)", transition:"opacity 0.4s", pointerEvents:"none" },
  shell:{ position:"relative", background:"rgba(10,12,20,0.82)", backdropFilter:"blur(20px)",
    border:"1px solid rgba(255,255,255,0.1)", borderRadius:16, transition:"border-color 0.3s",
    boxShadow:"0 10px 40px rgba(0,0,0,0.4)", overflow:"hidden" },
  liveStrip:{ display:"flex", justifyContent:"space-between", alignItems:"center",
    padding:"8px 14px", borderBottom:"1px solid var(--line)", background:"rgba(242,255,43,0.03)" },
  liveLeft:{ display:"flex", alignItems:"center", gap:10 },
  liveDotY:{ width:6, height:6, borderRadius:99, background:"var(--neon)",
    boxShadow:"0 0 8px var(--neon)", animation:"pulse 1.5s infinite" },
  livePanelBtn:{ background:"transparent", border:"none", color:"var(--neon)",
    fontSize:10, letterSpacing:2, fontFamily:"'JetBrains Mono',monospace", cursor:"pointer" },
  editorWrap:{ position:"relative", minHeight:110 },
  mirror:{ position:"absolute", inset:0, padding:"16px 18px",
    fontFamily:"Inter,system-ui,sans-serif", fontSize:15, lineHeight:1.6,
    color:"var(--ink-0)", pointerEvents:"none",
    whiteSpace:"pre-wrap", wordBreak:"break-word", overflow:"auto" },
  textarea:{ position:"relative", width:"100%", minHeight:110, maxHeight:260,
    padding:"16px 18px", background:"transparent", border:"none", outline:"none", resize:"none",
    fontFamily:"Inter,system-ui,sans-serif", fontSize:15, lineHeight:1.6,
    color:"transparent", caretColor:"var(--neon)" },
  toolbar:{ display:"flex", justifyContent:"space-between", alignItems:"center",
    padding:"8px 10px 10px 10px", borderTop:"1px solid var(--line)" },
  toolBtn:{ display:"inline-flex", alignItems:"center", gap:6, padding:"6px 10px",
    background:"transparent", border:"1px solid transparent",
    color:"var(--ink-2)", borderRadius:6, fontSize:11, cursor:"pointer" },
  routeInfo:{ display:"flex", alignItems:"center", gap:8, padding:"4px 10px",
    border:"1px solid var(--line)", borderRadius:99, background:"rgba(255,255,255,0.02)" },
  sendBtn:{ display:"inline-flex", alignItems:"center", gap:8, padding:"8px 14px",
    background:"var(--neon)", color:"#000", border:"none", borderRadius:8,
    fontSize:12, fontWeight:600, cursor:"pointer", boxShadow:"0 0 20px rgba(242,255,43,0.4)",
    transition:"all 0.15s" },
  helperRow:{ display:"flex", alignItems:"center", gap:8, justifyContent:"center",
    padding:"10px 0 0", fontSize:10, color:"var(--ink-3)",
    fontFamily:"'JetBrains Mono',monospace", letterSpacing:1 },
};
