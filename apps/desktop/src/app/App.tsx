import { useEffect, useMemo, useRef, useState } from "react";
import { Starfield } from "../Starfield";
import { Sidebar } from "../chrome/Sidebar";
import { TopBar } from "../chrome/TopBar";
import { VendettaPanel } from "../chrome/VendettaPanel";
import { Composer } from "../chat/Composer";
import { Transcript } from "../chat/Transcript";
import { BootSequence } from "../scenes/BootSequence";
import { PolicyViolation } from "../scenes/PolicyViolation";
import { ThreatRadar } from "../scenes/ThreatRadar";
import { CommandPalette, type CmdKey } from "../scenes/CommandPalette";
import { OrbitalPicker } from "../scenes/OrbitalPicker";
import { XrayBeam } from "../scenes/XrayBeam";
import { ConsensusArena } from "../scenes/ConsensusArena";
import { ComplianceDashboard } from "../scenes/ComplianceDashboard";
import { AgentDAG } from "../scenes/AgentDAG";
import { TweaksPanel } from "../scenes/TweaksPanel";
import { SettingsPanel } from "../scenes/SettingsPanel";
import { ModelDownloadPanel } from "../scenes/ModelDownloadPanel";
import { FirstRunWizard } from "../scenes/FirstRunWizard";
import { ParanoidToast } from "../scenes/ParanoidToast";
import { UpdateToast } from "../scenes/UpdateToast";
import { DevInspector, type TraceRecord } from "../scenes/DevInspector";
import { GuidedTour } from "../scenes/GuidedTour";
import { OnboardingCard } from "../scenes/OnboardingCard";
import { AboutDialog } from "../scenes/AboutDialog";
import { MODELS, SAMPLE_CONVERSATIONS, ollamaModel } from "../lib/models";
import { ipc, isTauri, onStreamChunk, onAuditNew, modelsIpc, settingsIpc, onTraceStream, onTraceParanoid, onModelReady, ollamaIpc } from "../lib/ipc";
import { CRITICAL, detect as detectLocal, setCustomTerms, setDisabledPacks } from "../lib/vendetta";
import type { AllModelStatus, AuditMetrics, BlockReason, Conversation, Message, Model, Span, Tweaks } from "../lib/types";
import { modelStatusKind } from "../lib/types";

const DEFAULT_TWEAKS: Tweaks = {
  accent: "#f2ff2b", density: "comfy", starfield: true, scanAnim: true, defaultModelIdx: 3,
};

export function App() {
  const [tweaks, setTweaks] = useState<Tweaks>(DEFAULT_TWEAKS);
  const [tweaksOpen, setTweaksOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [model, setModel] = useState<Model>(MODELS[DEFAULT_TWEAKS.defaultModelIdx]);
  const [conversations, setConversations] = useState<Conversation[]>(SAMPLE_CONVERSATIONS);
  const [activeConvo, setActiveConvo] = useState<string>(SAMPLE_CONVERSATIONS[0].id);
  const [messages, setMessages] = useState<Message[]>([]);
  const [draft, setDraft] = useState("");
  const [spans, setSpans] = useState<Span[]>([]);
  const [vendettaOpen, setVendettaOpen] = useState(true);
  const [aliasMode, setAliasMode] = useState<"mask"|"alias"|"raw">("alias");
  const [orbitalOpen, setOrbitalOpen] = useState(false);
  const [xraying, setXraying] = useState<null | { text: string; spans: Span[] }>(null);
  const [showBoot, setShowBoot] = useState(true);
  const [cmdOpen, setCmdOpen] = useState(false);
  const [consensusOpen, setConsensusOpen] = useState(false);
  const [complianceOpen, setComplianceOpen] = useState(false);
  const [agentOpen, setAgentOpen] = useState(false);
  const [violation, setViolation] = useState<BlockReason | null>(null);
  const [modelPanelOpen, setModelPanelOpen] = useState(false);
  const [firstRunOpen, setFirstRunOpen] = useState(false);
  const [devOpen, setDevOpen] = useState(false);
  /// Rolling buffer of per-send traces — capped at 50 so we don't hold
  /// unbounded provider responses in memory while the user explores.
  const [traces, setTraces] = useState<TraceRecord[]>([]);
  /// Live state backing the OnboardingCard — number of providers with keys
  /// and current status of the GGUF / ONNX model files. Empty-state UI only.
  const [configuredProviders, setConfiguredProviders] = useState<number>(0);
  const [modelsStatus, setModelsStatus] = useState<AllModelStatus | null>(null);
  const [aboutOpen, setAboutOpen] = useState(false);
  /// The walkable guided tour (auto-offered once after the wizard; ⌘K → tour).
  const [tourOpen, setTourOpen] = useState(false);
  /// Models discovered on the local Ollama server (if running), merged into the
  /// picker alongside the built-in MODELS.
  const [ollamaModels, setOllamaModels] = useState<Model[]>([]);
  const allModels = useMemo(() => [...MODELS, ...ollamaModels], [ollamaModels]);
  /// Live numbers from the local hash-chained audit log — the ONLY source for
  /// every stat the chrome displays. No fabricated metrics anywhere.
  const [auditStats, setAuditStats] = useState<AuditMetrics | null>(null);

  const pendingTransmit = useRef<null | { text: string; spans: Span[] }>(null);

  // Load conversations from SQLite on startup when running in Tauri.
  useEffect(() => {
    if (!isTauri) return;
    (async () => {
      try {
        const rows = await ipc.listConversations();
        if (rows.length > 0) {
          setConversations(rows.map(r => ({
            id: r.id, title: r.title, time: timeAgo(r.created_at), shield: r.shielded,
          })));
          setActiveConvo(rows[0].id);
        } else {
          // Seed a first real conversation so send() has a valid conv_id.
          const id = await ipc.newConversation("Q4 board memo — draft", model.id);
          setConversations([{ id, title:"Q4 board memo — draft", time:"now", shield:true }]);
          setActiveConvo(id);
        }
      } catch (e) { console.warn("load conversations failed", e); }

      // First-run wizard wins over the bare download panel. It runs once
      // per machine (persisted via the `first_run_seen` settings key).
      try {
        const [firstRunSeen, tutorialDone, modelsStatus] = await Promise.all([
          settingsIpc.get("first_run_seen"),
          settingsIpc.get("tutorial_done"),
          modelsIpc.status(),
        ]);
        const anyModelMissing =
          modelStatusKind(modelsStatus.ner) === "missing"
          || modelStatusKind(modelsStatus.ner_tokenizer) === "missing";
        if (!firstRunSeen) {
          setFirstRunOpen(true);
        } else if (anyModelMissing) {
          setModelPanelOpen(true);
        } else if (!tutorialDone) {
          // Wizard done, models handled, tour never taken — offer it. The
          // tour's intro card IS the offer; declining persists tutorial_done.
          setTourOpen(true);
        }
      } catch {}
    })();
  }, []);

  // Subscribe to streaming chunks.
  useEffect(() => {
    if (!isTauri) return;
    const p = onStreamChunk(c => {
      setMessages(ms => ms.map(m => {
        if (m.id !== c.msg_id) return m;
        if (c.error) return { ...m, streaming: false, error: c.error };
        return { ...m, text: m.text + c.delta, streaming: !c.done };
      }));
    });
    return () => { p.then(u => u()); };
  }, []);

  // Global keybindings. ⌘⇧D is the dev inspector; plain ⌘D is compliance.
  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") { e.preventDefault(); setCmdOpen(o => !o); }
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "m") { e.preventDefault(); setConsensusOpen(true); }
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "d") { e.preventDefault(); setDevOpen(o => !o); return; }
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "d") { e.preventDefault(); setComplianceOpen(true); }
      // ⌘⇧I for About — plain ⌘I is the universal italic shortcut in every
      // compose surface (Slack, Notion, GDocs). Using the shifted variant
      // stays consistent with ⌘⇧D (dev inspector) and doesn't trip users
      // mid-typing.
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "i") { e.preventDefault(); setAboutOpen(o => !o); return; }
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "g") { e.preventDefault(); setAgentOpen(true); }
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "o") { e.preventDefault(); setOrbitalOpen(true); }
      if ((e.metaKey || e.ctrlKey) && e.key === ",") { e.preventDefault(); setSettingsOpen(true); }
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, []);

  // Subscribe to the async trace streams so the DevInspector keeps growing
  // even when it's closed — you can open it after a send and see everything.
  useEffect(() => {
    if (!isTauri) return;
    const p1 = onTraceStream(st => {
      setTraces(ts => ts.map(r => r.msg_id === st.msg_id ? { ...r, stream: st } : r));
    });
    const p2 = onTraceParanoid(pt => {
      setTraces(ts => ts.map(r => r.msg_id === pt.msg_id ? { ...r, paranoid: pt } : r));
    });
    return () => { p1.then(u => u()); p2.then(u => u()); };
  }, []);

  // Hydrate OnboardingCard state on mount + refresh when a model finishes
  // downloading (so the "Sentynyx Local ready" row flips live).
  useEffect(() => {
    if (!isTauri) return;
    const refresh = () => {
      ipc.listConfiguredProviders().then(ps => setConfiguredProviders(ps.length)).catch(() => {});
      modelsIpc.status().then(setModelsStatus).catch(() => {});
    };
    refresh();
    const u = onModelReady(refresh);
    return () => { u.then(fn => fn()); };
  }, []);

  // Re-check configured providers whenever Settings closes — the guided
  // tour's transmit step (and the onboarding card) watch this to unblock
  // the moment a key is added.
  useEffect(() => {
    if (!isTauri || settingsOpen) return;
    ipc.listConfiguredProviders().then(ps => setConfiguredProviders(ps.length)).catch(() => {});
  }, [settingsOpen]);

  // Discover models on the local Ollama server (if running) and merge them into
  // the picker. Silent no-op when Ollama isn't installed/running.
  useEffect(() => {
    if (!isTauri) return;
    ollamaIpc.listModels()
      .then(names => setOllamaModels(names.map(ollamaModel)))
      .catch(() => setOllamaModels([]));
  }, []);

  // Audit metrics drive the sidebar/radar/empty-state numbers. Refresh on
  // every audit insert so the counters tick live as redactions happen.
  useEffect(() => {
    if (!isTauri) return;
    const refresh = () => ipc.auditMetrics().then(setAuditStats).catch(() => {});
    refresh();
    const u = onAuditNew(refresh);
    return () => { u.then(fn => fn()); };
  }, []);

  // Mean time-to-first-token across this session's sends (null until data).
  const meanTtftMs = useMemo(() => {
    const vals = traces
      .map(t => t.stream?.ttft_ms)
      .filter((v): v is number => typeof v === "number");
    if (vals.length === 0) return null;
    return Math.round(vals.reduce((a, b) => a + b, 0) / vals.length);
  }, [traces]);

  // Hydrate the custom watchlist + pack toggles into the client-side
  // highlighter so the composer's live preview matches the engine from the
  // first keystroke.
  useEffect(() => {
    if (!isTauri) return;
    settingsIpc.get("custom_watchlist").then(v => {
      if (!v) return;
      try { setCustomTerms(JSON.parse(v) as string[]); } catch {}
    }).catch(() => {});
    settingsIpc.get("disabled_packs").then(v => {
      if (!v) return;
      try { setDisabledPacks(JSON.parse(v) as string[]); } catch {}
    }).catch(() => {});
  }, []);

  useEffect(() => {
    document.documentElement.style.setProperty("--neon", tweaks.accent);
  }, [tweaks.accent]);

  // Hydrate tweaks + alias mode from SQLite on mount. Any missing keys
  // fall back to the DEFAULT_TWEAKS values already in state.
  useEffect(() => {
    if (!isTauri) return;
    (async () => {
      try {
        const [tweaksRaw, aliasRaw] = await Promise.all([
          settingsIpc.get("tweaks"),
          settingsIpc.get("alias_mode"),
        ]);
        if (tweaksRaw) {
          try {
            const parsed = JSON.parse(tweaksRaw) as Partial<Tweaks>;
            setTweaks(t => ({ ...t, ...parsed }));
          } catch { /* corrupt stored tweaks — fall through to defaults */ }
        }
        if (aliasRaw === "mask" || aliasRaw === "alias" || aliasRaw === "raw") {
          setAliasMode(aliasRaw);
        }
      } catch (e) { console.warn("settings hydrate failed", e); }
    })();
  }, []);

  const updateTweak = <K extends keyof Tweaks>(k: K, v: Tweaks[K]) => {
    setTweaks(t => {
      const next = { ...t, [k]: v };
      if (isTauri) {
        settingsIpc.set("tweaks", JSON.stringify(next)).catch(() => {});
      }
      return next;
    });
  };

  // Persist aliasMode changes.
  useEffect(() => {
    if (!isTauri) return;
    settingsIpc.set("alias_mode", aliasMode).catch(() => {});
  }, [aliasMode]);

  const runCmd = (k: CmdKey) => {
    if (k === "tour") setTourOpen(true);
    if (k === "orbital") setOrbitalOpen(true);
    if (k === "consensus") setConsensusOpen(true);
    if (k === "compliance") setComplianceOpen(true);
    if (k === "agent") setAgentOpen(true);
    if (k === "settings") setSettingsOpen(true);
    if (k === "toggle-v") setVendettaOpen(o => !o);
    if (k === "newchat") newTransmission();
  };

  const newTransmission = async () => {
    setMessages([]);
    setDraft("");
    if (isTauri) {
      try {
        const id = await ipc.newConversation("New transmission", model.id);
        setConversations(cs => [{ id, title:"New transmission", time:"now" }, ...cs]);
        setActiveConvo(id);
      } catch {}
    }
  };

  const send = (text: string, sp: Span[], modelOverride?: Model) => {
    const m = modelOverride ?? model;
    // Ollama is treated as local here (optimistic — no critical-class block in
    // the client). The Rust side makes the authoritative host-aware decision:
    // a remote Ollama base URL is still aliased + blocked server-side.
    const isLocal = m.id === "sentynyx-local" || m.id.startsWith("ollama:");
    console.log("[Sentynyx] send() fired", { textLen: text.length, spans: sp.length, model: m.id, activeConvo, isLocal });
    // Local never leaves the machine — critical egress rules don't apply.
    if (!isLocal) {
      const crit = sp.find(s => CRITICAL[s.kind]);
      if (crit) {
        console.log("[Sentynyx] local critical class -> violation", crit.kind);
        const c = CRITICAL[crit.kind]!;
        setViolation({ kind: crit.kind, rule: c.name, class: c.class, desc: c.desc });
        return;
      }
    }
    pendingTransmit.current = { text, spans: sp };
    if (sp.length === 0) {
      // Nothing to redact — skip the X-ray animation entirely and dispatch
      // immediately. The X-ray beam only tells a story when there are spans
      // to alias; on a clean prompt it's a 2-second loading screen for
      // nothing.
      console.log("[Sentynyx] no spans — skipping XrayBeam");
      actuallyTransmit();
    } else {
      setXraying({ text, spans: sp });
      console.log("[Sentynyx] xraying state set — XrayBeam should render");
    }
  };

  const actuallyTransmit = async () => {
    console.log("[Sentynyx] actuallyTransmit fired (XrayBeam done)");
    const pt = pendingTransmit.current;
    pendingTransmit.current = null;
    setXraying(null);
    if (!pt) return;

    if (!isTauri) {
      // Browser preview: fake reply.
      const userMsg: Message = { role:"user", text: pt.text, spans: pt.spans };
      const asst: Message = { id: String(Date.now()), role:"assistant", text:"",
        streaming: true, spans: pt.spans,
        aliasedPrompt: pt.spans.reduce((s, x) => s.replace(x.raw, x.alias), pt.text) };
      setMessages(ms => [...ms, userMsg, asst]);
      setDraft("");
      const reply = `Routed through the Vendetta perimeter. ${pt.spans.length} sensitive tokens were aliased before the payload reached ${model.provider}. Response re-hydrated locally.\n\n(Browser preview — run the Tauri app for live provider streaming.)`;
      let i = 0;
      const iv = setInterval(() => {
        i += 3;
        setMessages(ms => ms.map(m => m.id === asst.id ? { ...m, text: reply.slice(0, i), streaming: i < reply.length } : m));
        if (i >= reply.length) clearInterval(iv);
      }, 20);
      return;
    }

    try {
      const meta = await ipc.send({ conv_id: activeConvo, model_id: model.id, text: pt.text });

      // Capture the synchronous half of the trace. Stream + paranoid arrive
      // via events and update this record in place.
      const record: TraceRecord = {
        msg_id: meta.assistant_msg_id || `blocked-${Date.now()}`,
        conv_id: activeConvo,
        ts: Date.now(),
        raw_text: pt.text,
        blocked: meta.blocked ? meta.blocked.kind : null,
        pipeline: meta.trace,
      };
      setTraces(ts => [record, ...ts].slice(0, 50));

      if (meta.blocked) {
        setViolation({
          kind: meta.blocked.kind, rule: meta.blocked.rule,
          class: meta.blocked.class, desc: meta.blocked.desc,
        });
        return;
      }
      const userMsg: Message = { role:"user", text: pt.text, spans: meta.spans };
      const asst: Message = {
        id: meta.assistant_msg_id, role:"assistant", text:"", streaming:true,
        spans: meta.spans, aliasedPrompt: meta.aliased_prompt,
      };
      setMessages(ms => [...ms, userMsg, asst]);
      setDraft("");
    } catch (e) {
      const err = String(e);
      // Classify common errors so users see an actionable next step, not a raw
      // provider error string. Keep the original reason appended so debugging
      // is still possible.
      let friendly = err;
      if (err.includes("no API key")) {
        friendly = `No API key configured for ${model.provider}. Press ⌘, to add one.`;
        // Auto-open Settings to shorten the recovery path.
        setSettingsOpen(true);
      } else if (err.includes("401") || err.includes("Unauthorized")) {
        friendly = `${model.provider} rejected the stored API key (401). Press ⌘, to rotate it.`;
        setSettingsOpen(true);
      } else if (err.includes("429")) {
        friendly = `${model.provider} rate-limited this send. Wait ~30s and try again.`;
      } else if (err.includes("no provider for model")) {
        friendly = `This model isn't wired up yet. Pick a GPT / Claude / Gemini / Grok model via ⌘O.`;
      } else if (err.toLowerCase().includes("local model not loaded")) {
        friendly = `The on-device Qwen model isn't installed yet. Opening Models panel — click Download.`;
        setModelPanelOpen(true);
      } else if (err.toLowerCase().includes("timeout") || err.toLowerCase().includes("connection")) {
        friendly = `Network problem reaching ${model.provider}. Check your internet or retry.`;
      }
      setMessages(ms => [...ms, {
        id: String(Date.now()), role:"assistant", text:"", streaming:false,
        error: friendly, spans: pt.spans,
      }]);
    }
  };

  return (
    <>
      <Starfield />
      <div style={{ display:"flex", flex:1, minHeight:0 }}>
        <Sidebar
          activeId={activeConvo}
          conversations={conversations}
          redactionsWeek={auditStats?.redactions_7d ?? 0}
          blocksWeek={auditStats?.blocks_7d ?? 0}
          onSelect={async (id) => {
            setActiveConvo(id);
            if (isTauri) {
              try {
                const rows = await ipc.loadConversation(id);
                setMessages(rows.map(r => ({
                  id: r.id, role: r.role, text: r.text_raw, spans: r.spans,
                  aliasedPrompt: r.role === "assistant" ? r.text_aliased : undefined,
                })));
              } catch {}
            }
          }}
          onNew={newTransmission}
          onOpenSettings={() => setSettingsOpen(true)}
        />
        <div style={{ flex:1, display:"flex", flexDirection:"column", minWidth:0, position:"relative" }}>
          <TopBar model={model} setModel={setModel} models={allModels}
            onOpenVendetta={() => setVendettaOpen(o => !o)}
            onOpenOrbital={() => setOrbitalOpen(true)}
            onOpenCmd={() => setCmdOpen(true)}
            onOpenConsensus={() => setConsensusOpen(true)}
            onOpenCompliance={() => setComplianceOpen(true)}
            onOpenAgent={() => setAgentOpen(true)}
            onOpenSettings={() => setSettingsOpen(true)}
            onOpenDev={() => setDevOpen(o => !o)} />
          <div style={{ flex:1, display:"flex", flexDirection:"column", minHeight:0,
            marginRight: vendettaOpen ? 360 : 0, transition:"margin 0.35s" }}>
            {messages.length === 0 && isTauri && (
              <div style={{ padding: "20px 24px 0" }}>
                <OnboardingCard
                  model={model}
                  configuredProviders={configuredProviders}
                  llmStatus={modelsStatus?.llm ?? null}
                  onOpenSettings={() => setSettingsOpen(true)}
                  onOpenModels={() => setModelPanelOpen(true)}
                  onStartTour={() => setTourOpen(true)}
                />
              </div>
            )}
            <Transcript messages={messages} model={model} stats={{
              providers: configuredProviders + (ollamaModels.length > 0 ? 1 : 0),
              models: allModels.length,
              redactions24h: auditStats?.redactions_24h ?? 0,
              meanTtftMs,
            }} />
            <Composer
              model={model}
              onSend={send}
              spans={spans}
              setSpans={setSpans}
              text={draft}
              setText={setDraft}
              onToggleVendetta={() => setVendettaOpen(o => !o)}
              vendettaOpen={vendettaOpen} />
          </div>
        </div>
      </div>
      <VendettaPanel
        spans={spans}
        open={vendettaOpen}
        onClose={() => setVendettaOpen(false)}
        aliasMode={aliasMode}
        setAliasMode={setAliasMode} />
      {tweaksOpen && <TweaksPanel tweaks={tweaks} update={updateTweak} onClose={() => setTweaksOpen(false)} />}
      {settingsOpen && <SettingsPanel onClose={() => setSettingsOpen(false)} />}
      {orbitalOpen && <OrbitalPicker model={model} setModel={setModel} models={allModels} onClose={() => setOrbitalOpen(false)} />}
      {xraying && <XrayBeam text={xraying.text} spans={xraying.spans} model={model} onDone={actuallyTransmit} />}
      {showBoot && <BootSequence onDone={() => setShowBoot(false)} />}
      {violation && <PolicyViolation
        rule={violation}
        onDismiss={() => setViolation(null)}
        onRemoveAndRetry={() => {
          // Strip every occurrence of the offending critical class from the
          // current draft, then close the violation and let the user hit
          // Transmit again. We don't auto-resend — the user should see what
          // changed first.
          const critKind = violation.kind;
          const stripped = spans
            .filter(s => s.kind === critKind)
            .reduce((text, s) => text.split(s.raw).join("[REDACTED]"), draft);
          setDraft(stripped);
          setSpans(spans.filter(s => s.kind !== critKind));
          setViolation(null);
        }}
        onSwitchToLocal={() => {
          // Flip model to the on-device Qwen and re-submit the same prompt.
          // The modelOverride on send() skips the client-side critical check
          // synchronously (React state isn't updated yet at this point);
          // by the time actuallyTransmit fires after the xray animation,
          // the re-render has completed and `model` closure is local.
          const localModel = MODELS.find(m => m.id === "sentynyx-local");
          if (!localModel) { setViolation(null); return; }
          setModel(localModel);
          setViolation(null);
          send(draft, spans, localModel);
        }}
      />}
      {cmdOpen && <CommandPalette open={cmdOpen} onClose={() => setCmdOpen(false)} onAction={runCmd} />}
      {consensusOpen && <ConsensusArena prompt={draft} convId={activeConvo} onClose={() => setConsensusOpen(false)} />}
      {complianceOpen && <ComplianceDashboard onClose={() => setComplianceOpen(false)} />}
      {agentOpen && <AgentDAG onClose={() => setAgentOpen(false)} />}
      {firstRunOpen && <FirstRunWizard onClose={() => {
        setFirstRunOpen(false);
        // Chain into the guided tour the first time through. Its intro card
        // is the offer — skipping persists tutorial_done and never nags again.
        if (!isTauri) { setTourOpen(true); return; }
        settingsIpc.get("tutorial_done")
          .then(d => { if (!d) setTourOpen(true); })
          .catch(() => {});
      }} />}
      {modelPanelOpen && <ModelDownloadPanel onClose={() => setModelPanelOpen(false)} />}
      {tourOpen && <GuidedTour
        onExit={() => {
          setTourOpen(false);
          if (isTauri) settingsIpc.set("tutorial_done", "1").catch(() => {});
        }}
        spansCount={spans.length}
        vendettaOpen={vendettaOpen}
        setVendettaOpen={setVendettaOpen}
        devOpen={devOpen}
        setDevOpen={setDevOpen}
        violationActive={violation !== null}
        messages={messages}
        draft={draft}
        setDraft={setDraft}
        xrayActive={xraying !== null}
        model={model}
        setModel={setModel}
        configuredProviders={configuredProviders}
        modelsStatus={modelsStatus}
        onOpenSettings={() => setSettingsOpen(true)}
      />}
      {!showBoot && <ThreatRadar redactionsDay={auditStats?.redactions_24h ?? 0} blocksWeek={auditStats?.blocks_7d ?? 0} />}
      <ParanoidToast />
      <UpdateToast />
      {devOpen && (
        <DevInspector
          traces={traces}
          onClose={() => setDevOpen(false)}
          onClear={() => setTraces([])}
        />
      )}
      {aboutOpen && (
        <AboutDialog
          onClose={() => setAboutOpen(false)}
          lastTrace={traces[0]}
          modelsStatus={modelsStatus}
        />
      )}
    </>
  );
}

function timeAgo(iso: string): string {
  const d = new Date(iso);
  const mins = Math.max(0, Math.floor((Date.now() - d.getTime()) / 60000));
  if (mins < 1) return "now";
  if (mins < 60) return `${mins}m`;
  if (mins < 60 * 24) return `${Math.floor(mins/60)}h`;
  return d.toLocaleDateString(undefined, { month:"short", day:"numeric" });
}
// detectLocal re-exported to avoid pruning by tsc
void detectLocal;
