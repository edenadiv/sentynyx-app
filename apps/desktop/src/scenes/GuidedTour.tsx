import { useEffect, useRef, useState } from "react";
import type { CSSProperties, ReactNode } from "react";
import { ipc, isTauri } from "../lib/ipc";
import { MODELS, providerKey } from "../lib/models";
import type { AllModelStatus, Message, Model } from "../lib/types";
import { modelStatusKind } from "../lib/types";

// ---------------------------------------------------------------------------
// Guided tour — the walkable tutorial. A spotlight overlay that walks through
// the REAL app and advances on REAL events (spans detected, reply streamed,
// violation fired), never on blind Next-Next-Next. Anchors are `data-tour`
// attributes on existing elements; the scrim is four divs leaving a genuine
// DOM hole so the highlighted control stays natively clickable.
//
// z-index 290 on purpose: above VendettaPanel (50) / DevInspector (200) /
// SettingsPanel (260) / FirstRunWizard (280), but BELOW XrayBeam (300) — the
// transmit step hands the screen to that animation — and below CommandPalette
// (400) and PolicyViolation (500), which the block finale deliberately
// triggers.
// ---------------------------------------------------------------------------

/// Trips exactly EMAIL + PHONE + MONEY — pure regex, so the tour works on a
/// fresh install with zero models downloaded, and contains no blocking class.
const TOUR_SAMPLE =
  "Ask legal to email dana.reyes@example.com or call (415) 555-0142 about the $48,500 retainer.";

const SSN_FINALE = " My SSN is 123-45-6789.";

type StepId =
  | "intro" | "sample" | "highlights" | "panel" | "mapping" | "transmit"
  | "reply" | "modelsaw" | "inspector" | "block" | "done";

const STEPS: { id: StepId; anchor?: string; center?: boolean; quiet?: boolean }[] = [
  { id: "intro", center: true },
  { id: "sample", anchor: "composer" },
  { id: "highlights", anchor: "composer" },
  { id: "panel", anchor: "vendetta-toggle" },
  { id: "mapping", anchor: "vendetta-panel" },
  { id: "transmit", anchor: "transmit" },
  { id: "reply", quiet: true },
  { id: "modelsaw", anchor: "modelsaw" },
  { id: "inspector", anchor: "dev-toggle" },
  { id: "block", anchor: "composer" },
  { id: "done", center: true },
];

export interface GuidedTourProps {
  onExit: (completed: boolean) => void;
  spansCount: number;
  vendettaOpen: boolean;
  setVendettaOpen: (v: boolean) => void;
  devOpen: boolean;
  setDevOpen: (v: boolean) => void;
  violationActive: boolean;
  messages: Message[];
  draft: string;
  setDraft: (t: string) => void;
  xrayActive: boolean;
  model: Model;
  setModel: (m: Model) => void;
  configuredProviders: number;
  modelsStatus: AllModelStatus | null;
  onOpenSettings: () => void;
}

export function GuidedTour(p: GuidedTourProps) {
  const [idx, setIdx] = useState(0);
  const [rect, setRect] = useState<DOMRect | null>(null);
  const [sendable, setSendable] = useState<boolean | null>(null);
  const msgBaseline = useRef(0);
  // The panel step closes the (default-open) panel on entry via App state —
  // an async update. Without this latch the advance check would still see the
  // stale `vendettaOpen === true` in the same commit and skip the step.
  const panelClosedSeen = useRef(false);
  const step = STEPS[idx];

  const next = () => setIdx(i => Math.min(i + 1, STEPS.length - 1));
  const exit = (completed: boolean) => p.onExit(completed);

  // ---- step entry side-effects -------------------------------------------
  useEffect(() => {
    if (step.id === "panel") {
      panelClosedSeen.current = !p.vendettaOpen;
      if (p.vendettaOpen) {
        // The panel defaults open — close it so the user's click is a real one.
        p.setVendettaOpen(false);
      }
    }
    if (step.id === "transmit") {
      msgBaseline.current = p.messages.length;
    }
    if (step.id === "block" && p.devOpen) {
      // The Dev Inspector is a full-screen overlay that would cover the
      // composer the finale points at — close it on the user's behalf.
      p.setDevOpen(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [idx]);

  // ---- sendability for the transmit step (re-checked as config changes) ---
  useEffect(() => {
    if (step.id !== "transmit") return;
    let cancelled = false;
    (async () => {
      const ok = await checkSendable(p.model, p.modelsStatus);
      if (!cancelled) setSendable(ok);
    })();
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [idx, p.model.id, p.configuredProviders, p.modelsStatus]);

  // ---- event-driven advancement -------------------------------------------
  useEffect(() => {
    switch (step.id) {
      case "sample":
        if (p.spansCount > 0 && p.draft.length > 0) next();
        break;
      case "panel":
        if (!p.vendettaOpen) panelClosedSeen.current = true;
        else if (panelClosedSeen.current) next();
        break;
      case "transmit": {
        const last = p.messages[p.messages.length - 1];
        if (p.messages.length > msgBaseline.current && last?.role === "assistant") next();
        break;
      }
      case "reply": {
        const last = [...p.messages].reverse().find(m => m.role === "assistant");
        if (last && !last.streaming && !last.error) next();
        break;
      }
      case "inspector":
        if (p.devOpen) next();
        break;
      case "block":
        if (p.violationActive) next();
        break;
      default: break;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [idx, p.spansCount, p.draft, p.vendettaOpen, p.messages, p.devOpen, p.violationActive]);

  // ---- delegated click advance for the "Model saw" tab --------------------
  useEffect(() => {
    if (step.id !== "modelsaw") return;
    const h = (e: MouseEvent) => {
      const el = e.target as Element | null;
      if (el?.closest?.('[data-tour="modelsaw"]')) next();
    };
    document.addEventListener("click", h, true);
    return () => document.removeEventListener("click", h, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [idx]);

  // ---- Esc exits -----------------------------------------------------------
  useEffect(() => {
    const h = (e: KeyboardEvent) => { if (e.key === "Escape") exit(false); };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ---- anchor tracking (rAF; follows the VendettaPanel slide etc.) --------
  useEffect(() => {
    if (!step.anchor) { setRect(null); return; }
    let raf = 0;
    let prev: DOMRect | null = null;
    const tick = () => {
      const els = document.querySelectorAll(`[data-tour="${step.anchor}"]`);
      // Last match matters: "Model saw" exists once per assistant message.
      const el = els[els.length - 1] as HTMLElement | undefined;
      const r = el?.getBoundingClientRect() ?? null;
      const moved = !prev || !r
        || Math.abs(r.top - prev.top) > 1 || Math.abs(r.left - prev.left) > 1
        || Math.abs(r.width - prev.width) > 1 || Math.abs(r.height - prev.height) > 1;
      if (moved) {
        prev = r;
        setRect(r);
      }
      raf = requestAnimationFrame(tick);
    };
    tick();
    return () => cancelAnimationFrame(raf);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [idx]);

  const lastAssistant = [...p.messages].reverse().find(m => m.role === "assistant");
  const replyFailed = step.id === "reply" && !!lastAssistant?.error && !lastAssistant?.streaming;
  const quiet = step.quiet || (step.id === "transmit" && p.xrayActive);
  const localReady = p.modelsStatus !== null && modelStatusKind(p.modelsStatus.llm) === "ready";

  // ---- card content per step ----------------------------------------------
  const card = cardFor(step.id, {
    next,
    exit,
    insertSample: () => p.setDraft(TOUR_SAMPLE),
    addSsn: () => p.setDraft((p.draft || TOUR_SAMPLE) + SSN_FINALE),
    openSettings: p.onOpenSettings,
    useLocal: () => {
      const local = MODELS.find(m => m.id === "sentynyx-local");
      if (local) p.setModel(local);
    },
    provider: p.model.provider,
    sendable,
    localReady,
    replyFailed,
  });

  if (quiet) {
    return (
      <div style={gt.chip}>
        <span style={gt.chipDot} />
        <span>{card.chip ?? card.title}</span>
        <button style={gt.chipSkip} onClick={() => exit(false)}>skip</button>
      </div>
    );
  }

  return (
    <div style={gt.root}>
      {step.center || !rect ? (
        <div style={gt.fullScrim} />
      ) : (
        <Scrims rect={rect} />
      )}
      {!step.center && rect && <GlowRing rect={rect} />}
      <Card rect={step.center ? null : rect} title={card.title} step={idx} total={STEPS.length}>
        {card.body}
        <div style={gt.btnRow}>
          {card.buttons}
          <button style={gt.skipBtn} onClick={() => exit(false)}>Skip tour</button>
        </div>
      </Card>
    </div>
  );
}

async function checkSendable(model: Model, status: AllModelStatus | null): Promise<boolean> {
  if (!isTauri) return true; // browser preview fakes the stream
  if (model.id === "sentynyx-local") {
    return status !== null && modelStatusKind(status.llm) === "ready";
  }
  if (model.id.startsWith("ollama:")) return true; // only listed when reachable
  const pk = providerKey(model.id);
  if (!pk) return false;
  try { return await ipc.hasApiKey(pk); } catch { return false; }
}

// ---------------------------------------------------------------------------
// Step copy
// ---------------------------------------------------------------------------

interface CardCtx {
  next: () => void;
  exit: (completed: boolean) => void;
  insertSample: () => void;
  addSsn: () => void;
  openSettings: () => void;
  useLocal: () => void;
  provider: string;
  sendable: boolean | null;
  localReady: boolean;
  replyFailed: boolean;
}

function cardFor(id: StepId, c: CardCtx): { title: string; body: ReactNode; buttons?: ReactNode; chip?: string } {
  switch (id) {
    case "intro":
      return {
        title: "Welcome to the perimeter",
        body: (
          <p style={gt.p}>
            Everything you type is scanned <b>locally</b> and sensitive values are
            swapped for aliases <b>before</b> anything leaves this machine.
            Seven quick stops — about two minutes, using the real app.
          </p>
        ),
        buttons: <button style={gt.primaryBtn} onClick={c.next}>Start the tour</button>,
      };
    case "sample":
      return {
        title: "1 · Put something sensitive in the composer",
        body: (
          <p style={gt.p}>
            Insert a sample prompt with an email, a phone number, and a money
            amount — or type your own. Detection runs on every keystroke.
          </p>
        ),
        buttons: <button style={gt.primaryBtn} onClick={c.insertSample}>Insert sample prompt</button>,
      };
    case "highlights":
      return {
        title: "2 · Live detection",
        body: (
          <p style={gt.p}>
            The highlighted tokens are what Vendetta caught — colors mark the
            category (hover one for its class). Nothing has been sent anywhere yet;
            this is all local.
          </p>
        ),
        buttons: <button style={gt.primaryBtn} onClick={c.next}>Next</button>,
      };
    case "panel":
      return {
        title: "3 · Open the Vendetta panel",
        body: (
          <p style={gt.p}>
            Click <b>VIEW PANEL</b> to see every detected value and the alias
            that will replace it on the wire.
          </p>
        ),
      };
    case "mapping":
      return {
        title: "4 · Raw → alias",
        body: (
          <p style={gt.p}>
            Each value gets a stable alias like <code style={gt.code}>⟦email_01⟧</code>.
            The model only ever sees the alias column. Same value, same alias —
            so conversations stay coherent.
          </p>
        ),
        buttons: <button style={gt.primaryBtn} onClick={c.next}>Next</button>,
      };
    case "transmit":
      if (c.sendable === false) {
        return {
          title: "5 · One thing first — pick where to send",
          body: (
            <p style={gt.p}>
              Sentynyx needs a working model to send to. Add an API key (⌘,),
              pick a different model (⌘O){c.localReady ? ", or use the on-device model" : ""}.
              The tour will continue automatically once you transmit.
            </p>
          ),
          buttons: (
            <>
              <button style={gt.primaryBtn} onClick={c.openSettings}>Open Settings</button>
              {c.localReady && <button style={gt.ghostBtn} onClick={c.useLocal}>Use Sentynyx Local</button>}
            </>
          ),
        };
      }
      return {
        title: "5 · Transmit",
        body: (
          <p style={gt.p}>
            Hit <b>Transmit</b> and watch the X-ray pass: scan → excise → launch.
            What leaves the machine is the aliased payload, never your raw text.
          </p>
        ),
        chip: "Transmitting — watch the X-ray pass…",
      };
    case "reply":
      if (c.replyFailed) {
        return {
          title: "Send failed",
          body: (
            <p style={gt.p}>
              The provider rejected the send — fix the key in Settings (⌘,) and
              hit Transmit again. The tour picks up automatically.
            </p>
          ),
          chip: "Send failed — fix the key (⌘,) and Transmit again",
        };
      }
      return {
        title: "Streaming",
        body: null,
        chip: "Reply streaming in — re-hydrated locally as it arrives…",
      };
    case "modelsaw":
      return {
        title: `6 · What did ${c.provider} actually receive?`,
        body: (
          <p style={gt.p}>
            You read the real values — the provider never did. Click{" "}
            <b>Model saw</b> on the reply to inspect the exact aliased payload
            that went over the wire.
          </p>
        ),
      };
    case "inspector":
      return {
        title: "7 · The proof — Dev Inspector",
        body: (
          <p style={gt.p}>
            Press <code style={gt.code}>⌘⇧D</code> for per-send telemetry:
            regex/NER timings, the exact wire payload, time-to-first-token.
            Copy that payload into any LLM playground to verify it yourself.
          </p>
        ),
      };
    case "block":
      return {
        title: "Finale · Some things never leave",
        body: (
          <p style={gt.p}>
            Aliasing isn't always enough — SSNs, card numbers, and private keys
            are <b>blocked outright</b>. Add an SSN and hit Transmit to see the
            perimeter refuse.
          </p>
        ),
        buttons: (
          <>
            <button style={gt.primaryBtn} onClick={c.addSsn}>Add a fake SSN for me</button>
            <button style={gt.ghostBtn} onClick={c.next}>Skip finale</button>
          </>
        ),
      };
    case "done":
      return {
        title: "That's the perimeter",
        body: (
          <p style={gt.p}>
            Detection packs, your own watchlist, BYOK cloud models, local models
            via Ollama — all behind the same rule: <b>nothing sensitive leaves
            raw</b>. Re-run this tour anytime from ⌘K → "Take the guided tour".
            The signed audit log lives behind ⌘D.
          </p>
        ),
        buttons: <button style={gt.primaryBtn} onClick={() => c.exit(true)}>Finish</button>,
      };
  }
}

// ---------------------------------------------------------------------------
// Spotlight machinery
// ---------------------------------------------------------------------------

const SCRIM = "rgba(5,6,10,0.72)";

function Scrims({ rect }: { rect: DOMRect }) {
  const pad = 6;
  const top = Math.max(0, rect.top - pad);
  const left = Math.max(0, rect.left - pad);
  const right = Math.min(window.innerWidth, rect.right + pad);
  const bottom = Math.min(window.innerHeight, rect.bottom + pad);
  const base: CSSProperties = { position: "fixed", background: SCRIM, pointerEvents: "auto", zIndex: 290 };
  return (
    <>
      <div style={{ ...base, top: 0, left: 0, right: 0, height: top }} />
      <div style={{ ...base, top: bottom, left: 0, right: 0, bottom: 0 }} />
      <div style={{ ...base, top, left: 0, width: left, height: bottom - top }} />
      <div style={{ ...base, top, left: right, right: 0, height: bottom - top }} />
    </>
  );
}

function GlowRing({ rect }: { rect: DOMRect }) {
  return (
    <div
      style={{
        position: "fixed",
        top: rect.top - 8, left: rect.left - 8,
        width: rect.width + 16, height: rect.height + 16,
        border: "1px solid var(--neon)", borderRadius: 12,
        boxShadow: "0 0 24px rgba(242,255,43,0.45), inset 0 0 16px rgba(242,255,43,0.12)",
        pointerEvents: "none", zIndex: 291,
        transition: "all 0.15s ease-out",
      }}
    />
  );
}

function Card({ rect, title, step, total, children }: {
  rect: DOMRect | null; title: string; step: number; total: number; children: ReactNode;
}) {
  const W = 380;
  let style: CSSProperties;
  if (!rect) {
    style = { position: "fixed", left: "50%", top: "50%", transform: "translate(-50%,-50%)", zIndex: 292 };
  } else {
    const below = rect.bottom + 16;
    const fitsBelow = below + 220 < window.innerHeight;
    const topPos = fitsBelow ? below : undefined;
    const bottomPos = fitsBelow ? undefined : window.innerHeight - rect.top + 16;
    const left = Math.min(Math.max(12, rect.left + rect.width / 2 - W / 2), window.innerWidth - W - 12);
    style = { position: "fixed", top: topPos, bottom: bottomPos, left, zIndex: 292 };
  }
  return (
    <div style={{ ...gt.card, ...style, width: W }}>
      <div style={gt.cardHead}>
        <span style={gt.cardEyebrow}>GUIDED TOUR</span>
        <span style={gt.cardStep}>{step + 1} / {total}</span>
      </div>
      <div style={gt.cardTitle}>{title}</div>
      {children}
    </div>
  );
}

const gt: Record<string, CSSProperties> = {
  root: { position: "fixed", inset: 0, zIndex: 290, pointerEvents: "none" },
  fullScrim: { position: "fixed", inset: 0, background: SCRIM, pointerEvents: "auto", zIndex: 290 },
  card: {
    background: "rgba(10,12,20,0.97)", border: "1px solid rgba(242,255,43,0.4)",
    borderRadius: 12, padding: "16px 18px", pointerEvents: "auto",
    boxShadow: "0 0 50px rgba(242,255,43,0.12), 0 20px 60px rgba(0,0,0,0.7)",
    animation: "modalIn 0.25s ease-out",
  },
  cardHead: { display: "flex", justifyContent: "space-between", marginBottom: 8 },
  cardEyebrow: { fontSize: 9, letterSpacing: 3, color: "var(--neon)", fontFamily: "'JetBrains Mono',monospace" },
  cardStep: { fontSize: 9, letterSpacing: 1, color: "var(--ink-3)", fontFamily: "'JetBrains Mono',monospace" },
  cardTitle: { fontFamily: "'Instrument Serif',serif", fontSize: 22, marginBottom: 6, color: "var(--ink-0)" },
  p: { fontSize: 13, lineHeight: 1.6, color: "var(--ink-1)", margin: "0 0 4px" },
  code: {
    fontFamily: "'JetBrains Mono',monospace", fontSize: 11,
    background: "rgba(0,0,0,0.4)", padding: "1px 5px", borderRadius: 3, color: "var(--neon)",
  },
  btnRow: { display: "flex", alignItems: "center", gap: 8, marginTop: 12, flexWrap: "wrap" },
  primaryBtn: {
    padding: "8px 14px", background: "var(--neon)", color: "#000", border: "none",
    borderRadius: 6, fontSize: 12, fontWeight: 600, cursor: "pointer",
  },
  ghostBtn: {
    padding: "8px 14px", background: "transparent", color: "var(--ink-1)",
    border: "1px solid rgba(255,255,255,0.18)", borderRadius: 6, fontSize: 12, cursor: "pointer",
  },
  skipBtn: {
    marginLeft: "auto", padding: "8px 10px", background: "transparent",
    border: "none", color: "var(--ink-3)", fontSize: 11, cursor: "pointer",
    fontFamily: "'JetBrains Mono',monospace", letterSpacing: 1,
  },
  chip: {
    position: "fixed", right: 20, bottom: 20, zIndex: 292,
    display: "flex", alignItems: "center", gap: 10,
    background: "rgba(10,12,20,0.95)", border: "1px solid rgba(242,255,43,0.35)",
    borderRadius: 99, padding: "8px 14px", fontSize: 11, color: "var(--ink-1)",
    fontFamily: "'JetBrains Mono',monospace", pointerEvents: "auto",
    boxShadow: "0 0 24px rgba(242,255,43,0.12)",
  },
  chipDot: {
    width: 7, height: 7, borderRadius: 99, background: "var(--neon)",
    boxShadow: "0 0 8px var(--neon)", animation: "pulse 1.2s infinite",
  },
  chipSkip: {
    background: "transparent", border: "none", color: "var(--ink-3)",
    fontSize: 10, cursor: "pointer", letterSpacing: 1,
    fontFamily: "'JetBrains Mono',monospace",
  },
};
