import { useEffect, useState } from "react";
import type { CSSProperties } from "react";
import { ipc, modelsIpc, settingsIpc, onModelProgress, onModelReady } from "../lib/ipc";
import { modelStatusKind } from "../lib/types";
import type { AllModelStatus } from "../lib/types";

type Step = "welcome" | "key" | "models" | "done";

type ProviderId = "openai" | "anthropic" | "google" | "xai";

const PROVIDERS: { id: ProviderId; name: string; hint: string }[] = [
  { id: "openai",    name: "OpenAI",    hint: "sk-…" },
  { id: "anthropic", name: "Anthropic", hint: "sk-ant-…" },
  { id: "google",    name: "Google",    hint: "AIza…" },
  { id: "xai",       name: "xAI",       hint: "xai-…" },
];

interface Props {
  onClose: () => void;
}

/// Three-step onboarding: welcome → one API key (any provider) → optional
/// NER/paranoid model downloads. On completion we flip the `first_run_seen`
/// settings key so we don't fire again.
export function FirstRunWizard({ onClose }: Props) {
  const [step, setStep] = useState<Step>("welcome");
  const [provider, setProvider] = useState<ProviderId>("openai");
  const [keyDraft, setKeyDraft] = useState("");
  const [keyStatus, setKeyStatus] = useState<string | null>(null);
  const [keyBusy, setKeyBusy] = useState(false);
  const [status, setStatus] = useState<AllModelStatus | null>(null);
  const [downloadingId, setDownloadingId] = useState<string | null>(null);
  const [pct, setPct] = useState(0);

  useEffect(() => {
    modelsIpc.status().then(setStatus).catch(() => {});
    const unP = onModelProgress(e => {
      setDownloadingId(e.id);
      setPct(e.percent);
    });
    const unR = onModelReady(() => {
      setDownloadingId(null);
      setPct(0);
      modelsIpc.status().then(setStatus).catch(() => {});
    });
    return () => { unP.then(u => u()); unR.then(u => u()); };
  }, []);

  const finish = async () => {
    try { await settingsIpc.set("first_run_seen", "1"); } catch {}
    onClose();
  };

  const saveKey = async () => {
    if (!keyDraft.trim()) {
      setKeyStatus("paste a key first");
      return;
    }
    setKeyBusy(true);
    setKeyStatus("validating…");
    try {
      const check = await ipc.validateApiKey(provider, keyDraft);
      if (!check.ok) {
        setKeyStatus(`✗ ${check.reason ?? "validation failed"}`);
        setKeyBusy(false);
        return;
      }
      await ipc.setApiKey(provider, keyDraft);
      setKeyStatus("✓ saved");
      setKeyDraft("");
      setStep("models");
    } catch (e) {
      setKeyStatus(`error: ${String(e)}`);
    } finally {
      setKeyBusy(false);
    }
  };

  const nerReady = status
    && modelStatusKind(status.ner) === "ready"
    && modelStatusKind(status.ner_tokenizer) === "ready";
  const llmReady = status && modelStatusKind(status.llm) === "ready";

  return (
    <div style={fw.overlay}>
      <div style={fw.card}>
        {step === "welcome" && (
          <>
            <div style={fw.eyebrow}>SENTYNYX · FIRST RUN</div>
            <div style={fw.title}>Welcome.</div>
            <p style={fw.body}>
              Sentynyx wraps every prompt in the <strong>Vendetta perimeter</strong> — a realtime
              privacy filter that catches sensitive tokens (emails, SSNs, API keys, names) and
              replaces them with opaque aliases <em>before</em> the payload reaches a third-party
              model. You see the real values; the model only ever sees the aliases.
            </p>
            <p style={fw.body}>
              Two quick steps: one API key so we have something to send to, and the optional ML
              models that power semantic detection. Three minutes and you're running.
            </p>
            <div style={fw.footer}>
              <button onClick={finish} style={fw.link}>Skip setup</button>
              <button onClick={() => setStep("key")} style={fw.primary}>Continue →</button>
            </div>
          </>
        )}

        {step === "key" && (
          <>
            <div style={fw.eyebrow}>STEP 1 OF 2</div>
            <div style={fw.title}>Add one API key.</div>
            <p style={fw.body}>
              Sentynyx doesn't bill you — you bring your own provider key. Pick whichever you
              already have; you can add others later in Settings (⌘,).
            </p>
            <div style={fw.providerRow}>
              {PROVIDERS.map(p => (
                <button
                  key={p.id}
                  onClick={() => setProvider(p.id)}
                  style={{
                    ...fw.providerChip,
                    borderColor: provider === p.id ? "var(--neon)" : "rgba(255,255,255,0.15)",
                    color: provider === p.id ? "var(--neon)" : "var(--ink-2)",
                  }}
                >{p.name}</button>
              ))}
            </div>
            <input
              type="password"
              value={keyDraft}
              onChange={e => setKeyDraft(e.target.value)}
              onKeyDown={e => { if (e.key === "Enter" && !keyBusy) saveKey(); }}
              placeholder={PROVIDERS.find(p => p.id === provider)?.hint}
              style={fw.input}
              autoFocus
            />
            {keyStatus && (
              <div style={{
                fontSize: 11, marginTop: 6,
                color: keyStatus.startsWith("✓") ? "#7cffb2"
                     : keyStatus.startsWith("✗") || keyStatus.startsWith("error") ? "#ff99a8"
                     : "var(--ink-3)",
              }}>{keyStatus}</div>
            )}
            <div style={fw.footer}>
              <button onClick={() => setStep("welcome")} style={fw.link}>← Back</button>
              <div style={{ display: "flex", gap: 10 }}>
                <button onClick={() => setStep("models")} style={fw.link}>Skip this step</button>
                <button onClick={saveKey} disabled={keyBusy || !keyDraft.trim()} style={{
                  ...fw.primary,
                  opacity: keyBusy || !keyDraft.trim() ? 0.5 : 1,
                }}>{keyBusy ? "…" : "Save & continue →"}</button>
              </div>
            </div>
          </>
        )}

        {step === "models" && (
          <>
            <div style={fw.eyebrow}>STEP 2 OF 2</div>
            <div style={fw.title}>Download the semantic models.</div>
            <p style={fw.body}>
              Regex catches the obvious PII. The <strong>NER model</strong> (GLiNER, ~600 MB)
              catches names, orgs, and custom codenames regex can't. The <strong>paranoid LLM</strong>
              {" "}(Qwen 2.5 0.5B, ~470 MB, optional) does a deeper semantic sweep for anything the
              earlier layers missed. All inference runs locally — nothing about these models
              leaves your machine.
            </p>

            <ModelRow
              id="gliner-small-v2.1"
              label="Semantic NER (GLiNER)"
              sizeMb={611}
              required
              ready={Boolean(nerReady)}
              downloadingId={downloadingId}
              pct={pct}
            />
            <ModelRow
              id="paranoid-llm"
              label="Paranoid LLM (Qwen 2.5 0.5B)"
              sizeMb={468}
              required={false}
              ready={Boolean(llmReady)}
              downloadingId={downloadingId}
              pct={pct}
            />

            <div style={fw.footer}>
              <button onClick={() => setStep("key")} style={fw.link}>← Back</button>
              <button onClick={finish} style={fw.primary}>
                {nerReady ? "Launch Sentynyx →" : "Continue with regex-only →"}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function ModelRow({
  id, label, sizeMb, required, ready, downloadingId, pct,
}: {
  id: string;
  label: string;
  sizeMb: number;
  required: boolean;
  ready: boolean;
  downloadingId: string | null;
  pct: number;
}) {
  const isDownloading = downloadingId === id;
  return (
    <div style={fw.modelRow}>
      <div style={{ flex: 1 }}>
        <div style={{ fontSize: 14 }}>
          {label}
          {!required && <span style={fw.optional}>optional</span>}
        </div>
        <div style={fw.modelMeta}>
          {sizeMb} MB · {ready ? "ready" : isDownloading ? `${pct}% downloading` : "not installed"}
        </div>
      </div>
      {ready ? (
        <span style={fw.readyBadge}>✓</span>
      ) : isDownloading ? (
        <span style={fw.progressBadge}>{pct}%</span>
      ) : (
        <button
          onClick={() => modelsIpc.download(id).catch(() => {})}
          style={fw.smallBtn}
        >Download</button>
      )}
    </div>
  );
}

const fw: Record<string, CSSProperties> = {
  overlay: {
    position: "fixed", inset: 0, zIndex: 280,
    background: "rgba(5,6,10,0.92)", backdropFilter: "blur(8px)",
    display: "flex", alignItems: "center", justifyContent: "center",
  },
  card: {
    width: "min(580px, 92vw)", padding: 32,
    background: "rgba(10,12,20,0.96)",
    border: "1px solid rgba(242,255,43,0.25)", borderRadius: 14,
    boxShadow: "0 0 80px rgba(242,255,43,0.15), 0 24px 80px rgba(0,0,0,0.6)",
    fontFamily: "Inter, sans-serif", color: "#e5e9f0",
  },
  eyebrow: {
    fontSize: 10, letterSpacing: 4, color: "var(--neon)",
    fontFamily: "'JetBrains Mono', monospace", marginBottom: 8,
  },
  title: {
    fontFamily: "'Instrument Serif', serif", fontSize: 36,
    lineHeight: 1.1, marginBottom: 14,
  },
  body: { fontSize: 14, color: "#b8bcc8", lineHeight: 1.6, marginBottom: 18 },
  providerRow: { display: "flex", gap: 8, marginBottom: 12, flexWrap: "wrap" },
  providerChip: {
    padding: "6px 14px", fontSize: 11, letterSpacing: 2,
    fontFamily: "'JetBrains Mono', monospace",
    background: "transparent", border: "1px solid",
    borderRadius: 4, cursor: "pointer",
  },
  input: {
    width: "100%", padding: "10px 12px", fontSize: 13,
    background: "rgba(0,0,0,0.4)", border: "1px solid rgba(255,255,255,0.12)",
    borderRadius: 6, color: "#e5e9f0", fontFamily: "'JetBrains Mono', monospace",
    outline: "none",
  },
  footer: {
    display: "flex", justifyContent: "space-between", alignItems: "center",
    marginTop: 24, paddingTop: 18, borderTop: "1px solid rgba(255,255,255,0.06)",
  },
  link: {
    padding: "6px 10px", fontSize: 12,
    background: "transparent", color: "#9ba3b4",
    border: "none", cursor: "pointer",
  },
  primary: {
    padding: "8px 16px", fontSize: 13, fontWeight: 600,
    background: "var(--neon, #f2ff2b)", color: "#000",
    border: "none", borderRadius: 6, cursor: "pointer",
  },
  modelRow: {
    display: "flex", alignItems: "center", justifyContent: "space-between",
    padding: "12px 0", borderBottom: "1px solid rgba(255,255,255,0.06)",
  },
  modelMeta: {
    fontSize: 11, color: "#9ba3b4",
    fontFamily: "'JetBrains Mono', monospace", marginTop: 2,
  },
  optional: {
    marginLeft: 8, fontSize: 10, color: "#9ba3b4",
    fontFamily: "'JetBrains Mono', monospace",
  },
  smallBtn: {
    padding: "5px 12px", fontSize: 11,
    background: "var(--neon, #f2ff2b)", color: "#000",
    border: "none", borderRadius: 4, cursor: "pointer",
  },
  readyBadge: {
    color: "#7cffb2", fontSize: 18, fontWeight: 700, padding: "0 6px",
  },
  progressBadge: {
    color: "var(--neon, #f2ff2b)", fontSize: 11,
    fontFamily: "'JetBrains Mono', monospace", padding: "0 8px",
  },
};
