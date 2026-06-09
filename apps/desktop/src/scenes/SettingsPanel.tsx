import { useEffect, useState } from "react";
import type { CSSProperties } from "react";
import { ipc, isTauri, modelsIpc, onModelProgress, onModelReady, dataIpc, telemetryIpc, teamIpc, settingsIpc, buildInfoIpc, ollamaIpc, type TeamStatus, type SyncOutcome, type BuildInfo, type OllamaHealth } from "../lib/ipc";
import { setCustomTerms, setDisabledPacks, TOGGLEABLE_PACKS } from "../lib/vendetta";
import type { AllModelStatus } from "../lib/types";
import { modelStatusKind } from "../lib/types";

const PROVIDERS: { id: "openai"|"anthropic"|"google"|"xai"; name: string; hint: string }[] = [
  { id:"openai",    name:"OpenAI",    hint:"sk-…" },
  { id:"anthropic", name:"Anthropic", hint:"sk-ant-…" },
  { id:"google",    name:"Google",    hint:"AIza… (Gemini)" },
  { id:"xai",       name:"xAI",       hint:"xai-… (Grok)" },
];

export function SettingsPanel({ onClose }: { onClose: () => void }) {
  const [configured, setConfigured] = useState<string[]>([]);
  const [drafts, setDrafts] = useState<Record<string,string>>({});
  const [status, setStatus] = useState<Record<string,string>>({});

  useEffect(() => {
    if (!isTauri) return;
    ipc.listConfiguredProviders().then(setConfigured).catch(()=>{});
  }, []);

  // Which optional surfaces this binary supports. In the public open-source
  // build, team-cloud + telemetry are compiled out, so we hide those sections.
  const [caps, setCaps] = useState<BuildInfo | null>(null);
  useEffect(() => {
    if (!isTauri) { setCaps({ team_cloud: false, telemetry_available: false, version: "" }); return; }
    buildInfoIpc.get().then(setCaps)
      .catch(() => setCaps({ team_cloud: false, telemetry_available: false, version: "" }));
  }, []);

  const [modelStatus, setModelStatus] = useState<AllModelStatus | null>(null);
  const [paranoid, setParanoid] = useState(false);
  const [downloadingId, setDownloadingId] = useState<string | null>(null);
  const [pct, setPct] = useState(0);

  useEffect(() => {
    modelsIpc.status().then(setModelStatus).catch(() => {});
    modelsIpc.getParanoid().then(setParanoid).catch(() => {});
    const unP = onModelProgress(e => { setDownloadingId(e.id); setPct(e.percent); });
    const unR = onModelReady(() => {
      setDownloadingId(null); setPct(0);
      modelsIpc.status().then(setModelStatus).catch(() => {});
    });
    return () => { unP.then(u => u()); unR.then(u => u()); };
  }, []);

  const toggleParanoid = async (v: boolean) => {
    await modelsIpc.setParanoid(v);
    setParanoid(v);
  };

  const deleteAndRefresh = async (id: string) => {
    await modelsIpc.delete(id);
    const s = await modelsIpc.status();
    setModelStatus(s);
  };

  const btnPrimary: CSSProperties = {
    padding: "6px 12px", fontSize: 12, background: "var(--neon, #f2ff2b)",
    color: "#000", border: "none", borderRadius: 4, cursor: "pointer",
  };
  const btnDanger: CSSProperties = {
    padding: "6px 12px", fontSize: 12, background: "transparent",
    color: "#ff6b9d", border: "1px solid #ff6b9d", borderRadius: 4, cursor: "pointer",
  };
  const rowStyle: CSSProperties = {
    padding: 12, background: "#0a0d14", border: "1px solid rgba(255,255,255,0.06)",
    borderRadius: 4, marginBottom: 12,
  };

  const save = async (p: string) => {
    if (!isTauri) { setStatus(s => ({ ...s, [p]: "stored locally only (browser preview)" })); return; }
    const secret = drafts[p]?.trim();
    if (!secret) return;
    try {
      setStatus(s => ({ ...s, [p]: "validating…" }));
      const check = await ipc.validateApiKey(p, secret);
      if (!check.ok) {
        setStatus(s => ({ ...s, [p]: `✗ ${check.reason ?? "validation failed"}` }));
        return;
      }
      const result = await ipc.setApiKey(p, secret);
      const storage = result.storage === "keychain"
        ? "saved to OS keychain"
        : "saved to protected file (unsigned build)";
      setStatus(s => ({ ...s, [p]: `✓ ${storage}` }));
      setConfigured(await ipc.listConfiguredProviders());
      setDrafts(d => ({ ...d, [p]: "" }));
    } catch (e) {
      setStatus(s => ({ ...s, [p]: `error: ${String(e)}` }));
    }
  };

  return (
    <div style={st.overlay} onClick={onClose}>
      <div style={st.stage} onClick={e => e.stopPropagation()}>
        <div style={st.header}>
          <div>
            <div style={{ fontSize:10, letterSpacing:4, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace" }}>ADMIN</div>
            <div style={{ fontFamily:"'Instrument Serif',serif", fontSize:32, marginTop:4 }}>
              API keys &amp; <em style={{ color:"var(--neon)" }}>providers</em>
            </div>
            <div style={{ fontSize:11, color:"var(--ink-2)", marginTop:4 }}>
              Stored in your OS keychain. Never leaves this device. Never reaches the renderer.
            </div>
          </div>
          <button onClick={onClose} style={st.close}>×</button>
        </div>
        <div style={st.list}>
          {PROVIDERS.map(p => (
            <div key={p.id} style={st.row}>
              <div style={{ flex:1 }}>
                <div style={{ fontSize:14, fontWeight:600 }}>{p.name}</div>
                <div style={{ fontSize:10, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace" }}>{p.hint}</div>
                {status[p.id] && <div style={{ fontSize:10, color:"var(--neon)", marginTop:4 }}>{status[p.id]}</div>}
              </div>
              <input
                type="password"
                placeholder={configured.includes(p.id) ? "●●●●●●●● (configured)" : "paste key"}
                value={drafts[p.id] || ""}
                onChange={e => setDrafts(d => ({ ...d, [p.id]: e.target.value }))}
                style={st.input}
              />
              <button onClick={() => save(p.id)} style={st.saveBtn}>
                {configured.includes(p.id) ? "Update" : "Save"}
              </button>
            </div>
          ))}
        </div>
        <section style={{ marginTop: 32 }}>
          <h3 style={{ fontFamily: "Instrument Serif, serif", fontSize: 20, margin: "0 0 12px" }}>
            Models
          </h3>

          <div style={rowStyle}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div>
                <div>Semantic NER (GLiNER)</div>
                <div style={{ fontSize: 11, color: "#9ba3b4", fontFamily: "JetBrains Mono, monospace", marginTop: 2 }}>
                  Status: {modelStatus ? modelStatusKind(modelStatus.ner) : "…"}
                  {downloadingId === "gliner-small-v2.1" && ` · ${pct}%`}
                </div>
              </div>
              <div style={{ display: "flex", gap: 8 }}>
                {modelStatus && modelStatusKind(modelStatus.ner) !== "ready" && (
                  <button onClick={() => modelsIpc.download("gliner-small-v2.1")} style={btnPrimary}>Download</button>
                )}
                {modelStatus && modelStatusKind(modelStatus.ner) === "ready" && (
                  <button onClick={() => deleteAndRefresh("gliner-small-v2.1")} style={btnDanger}>Delete</button>
                )}
              </div>
            </div>
          </div>

          <div style={rowStyle}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div>
                <div>Paranoid mode (Qwen 2.5 0.5B)</div>
                <div style={{ fontSize: 11, color: "#9ba3b4", fontFamily: "JetBrains Mono, monospace", marginTop: 2 }}>
                  Deep semantic scan · ~500ms per send · {modelStatus ? modelStatusKind(modelStatus.llm) : "…"}
                  {downloadingId === "paranoid-llm" && ` · ${pct}%`}
                </div>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                {modelStatus && modelStatusKind(modelStatus.llm) !== "ready" ? (
                  <button onClick={() => modelsIpc.download("paranoid-llm")} style={btnPrimary}>Download (468 MB)</button>
                ) : (
                  <>
                    <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12 }}>
                      <input type="checkbox" checked={paranoid} onChange={e => toggleParanoid(e.target.checked)} />
                      Enabled
                    </label>
                    <button onClick={() => deleteAndRefresh("paranoid-llm")} style={btnDanger}>Delete</button>
                  </>
                )}
              </div>
            </div>
          </div>
        </section>

        <PacksSection />
        <WatchlistSection />
        <DataSection />
        <OllamaSection />
        {caps?.telemetry_available && <TelemetrySection />}

        {caps?.team_cloud && <TeamSection />}

        <div style={st.foot}>
          <span style={{ fontSize:10, color:"var(--ink-3)", fontFamily:"'JetBrains Mono',monospace", letterSpacing:1 }}>
            Vendetta rewrites every outbound payload before it reaches these endpoints.
          </span>
        </div>
      </div>
    </div>
  );
}

function TelemetrySection() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri) return;
    telemetryIpc.get().then(setEnabled).catch(() => setEnabled(false));
  }, []);

  const toggle = async (v: boolean) => {
    setStatus("saving…");
    try {
      await telemetryIpc.set(v);
      setEnabled(v);
      setStatus(v ? "✓ telemetry on" : "✓ telemetry off");
    } catch (e) {
      setStatus(`✗ ${String(e)}`);
    }
  };

  return (
    <section style={{ marginTop: 32 }}>
      <h3 style={{ fontFamily: "Instrument Serif, serif", fontSize: 20, margin: "0 0 12px" }}>Telemetry</h3>
      <div style={{
        padding: 12, background: "#0a0d14",
        border: "1px solid rgba(255,255,255,0.06)", borderRadius: 4,
      }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div style={{ flex: 1, paddingRight: 12 }}>
            <div>Anonymous crash &amp; event reporting</div>
            <div style={{ fontSize: 11, color: "#9ba3b4", fontFamily: "'JetBrains Mono', monospace", marginTop: 2 }}>
              Off by default. Sends panic backtraces + named events (app.launched, send.succeeded, send.error)
              to our Sentry project. Never sends prompt text, API keys, aliases, or provider responses.
              All payloads containing ⟦ / ⟧ brackets are dropped client-side as a safety net.
            </div>
            {status && (
              <div style={{ fontSize: 11, color: status.startsWith("✓") ? "#7cffb2" : "#ff99a8", marginTop: 6 }}>
                {status}
              </div>
            )}
          </div>
          <label style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <input
              type="checkbox"
              checked={enabled === true}
              onChange={e => toggle(e.target.checked)}
              disabled={enabled === null}
            />
            <span style={{ fontSize: 12 }}>Enable</span>
          </label>
        </div>
      </div>
    </section>
  );
}

function DataSection() {
  const [exportStatus, setExportStatus] = useState<string | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState(false);
  const [deleteStatus, setDeleteStatus] = useState<string | null>(null);

  const doExport = async () => {
    setExportStatus("exporting…");
    try {
      const r = await dataIpc.export();
      setExportStatus(`✓ ${r.files.length} file${r.files.length === 1 ? "" : "s"} written to ${r.dest}`);
    } catch (e) {
      setExportStatus(`✗ ${String(e)}`);
    }
  };

  const doDelete = async () => {
    setDeleteStatus("deleting…");
    try {
      await dataIpc.deleteAll();
      setDeleteStatus("✓ All local data cleared. Quit and relaunch Sentynyx.");
    } catch (e) {
      setDeleteStatus(`✗ ${String(e)}`);
    }
  };

  return (
    <section style={{ marginTop: 32 }}>
      <h3 style={{ fontFamily: "Instrument Serif, serif", fontSize: 20, margin: "0 0 12px" }}>Data</h3>

      <div style={{
        padding: 12, background: "#0a0d14",
        border: "1px solid rgba(255,255,255,0.06)", borderRadius: 4, marginBottom: 12,
      }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div style={{ flex: 1, paddingRight: 12 }}>
            <div>Export local data</div>
            <div style={{ fontSize: 11, color: "#9ba3b4", fontFamily: "'JetBrains Mono', monospace", marginTop: 2 }}>
              Copies the SQLite database and API-key fallback file into ~/Downloads/sentynyx-export-&lt;timestamp&gt;/. Models are excluded.
            </div>
            {exportStatus && (
              <div style={{ fontSize: 11, color: exportStatus.startsWith("✓") ? "#7cffb2" : "#ff99a8", marginTop: 6 }}>
                {exportStatus}
              </div>
            )}
          </div>
          <button
            onClick={doExport}
            style={{
              padding: "6px 12px", fontSize: 12, background: "var(--neon, #f2ff2b)",
              color: "#000", border: "none", borderRadius: 4, cursor: "pointer",
            }}
          >Export</button>
        </div>
      </div>

      <div style={{
        padding: 12, background: "#0a0d14",
        border: "1px solid rgba(255,51,85,0.2)", borderRadius: 4,
      }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div style={{ flex: 1, paddingRight: 12 }}>
            <div>Delete all local data</div>
            <div style={{ fontSize: 11, color: "#9ba3b4", fontFamily: "'JetBrains Mono', monospace", marginTop: 2 }}>
              Removes the Sentynyx app data directory (conversations, audit log, downloaded models, API keys in file + keychain). Irreversible.
            </div>
            {deleteStatus && (
              <div style={{ fontSize: 11, color: deleteStatus.startsWith("✓") ? "#7cffb2" : "#ff99a8", marginTop: 6 }}>
                {deleteStatus}
              </div>
            )}
          </div>
          {!deleteConfirm ? (
            <button
              onClick={() => setDeleteConfirm(true)}
              style={{
                padding: "6px 12px", fontSize: 12, background: "transparent",
                color: "#ff6b9d", border: "1px solid #ff6b9d", borderRadius: 4, cursor: "pointer",
              }}
            >Delete…</button>
          ) : (
            <div style={{ display: "flex", gap: 6 }}>
              <button
                onClick={() => setDeleteConfirm(false)}
                style={{
                  padding: "6px 12px", fontSize: 12, background: "transparent",
                  color: "#9ba3b4", border: "1px solid #2a3040", borderRadius: 4, cursor: "pointer",
                }}
              >Cancel</button>
              <button
                onClick={doDelete}
                style={{
                  padding: "6px 12px", fontSize: 12, background: "#ff3355",
                  color: "#fff", border: "none", borderRadius: 4, cursor: "pointer",
                  fontWeight: 600,
                }}
              >Yes, delete everything</button>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

function PacksSection() {
  const [disabled, setDisabled] = useState<Set<string>>(new Set());
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri) return;
    settingsIpc.get("disabled_packs").then(v => {
      if (!v) return;
      try { setDisabled(new Set(JSON.parse(v) as string[])); } catch {}
    }).catch(() => {});
  }, []);

  const toggle = async (id: string) => {
    const next = new Set(disabled);
    if (next.has(id)) next.delete(id); else next.add(id);
    setDisabled(next);
    const list = [...next];
    setDisabledPacks(list); // live highlights update immediately
    if (!isTauri) return;
    try {
      await settingsIpc.set("disabled_packs", JSON.stringify(list));
      setStatus("✓ saved");
    } catch (e) {
      setStatus(`✗ ${String(e)}`);
    }
  };

  return (
    <section style={{ marginTop: 32 }}>
      <h3 style={{ fontFamily: "Instrument Serif, serif", fontSize: 20, margin: "0 0 12px" }}>
        Detection packs
      </h3>
      <div style={{
        padding: 12, background: "#0a0d14",
        border: "1px solid rgba(255,255,255,0.06)", borderRadius: 4,
      }}>
        <div style={{ fontSize: 11, color: "#9ba3b4", lineHeight: 1.5, marginBottom: 10 }}>
          Switch off categories you never handle to reduce noise. Core PII
          (emails, phones, SSNs…) and secrets (API keys, private keys,
          connection strings) are the safety floor and stay on.
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8 }}>
          {TOGGLEABLE_PACKS.map(p => (
            <label key={p.id} style={{
              display: "flex", alignItems: "flex-start", gap: 8, padding: "8px 10px",
              background: "rgba(255,255,255,0.02)", border: "1px solid rgba(255,255,255,0.06)",
              borderRadius: 4, cursor: "pointer",
            }}>
              <input
                type="checkbox"
                checked={!disabled.has(p.id)}
                onChange={() => toggle(p.id)}
                style={{ marginTop: 2 }}
              />
              <span>
                <span style={{ fontSize: 12, display: "block" }}>{p.name}</span>
                <span style={{ fontSize: 9, color: "#9ba3b4", fontFamily: "'JetBrains Mono', monospace" }}>{p.hint}</span>
              </span>
            </label>
          ))}
        </div>
        {status && (
          <div style={{
            fontSize: 11, marginTop: 8,
            color: status.startsWith("✓") ? "#7cffb2" : "#ff99a8",
            fontFamily: "'JetBrains Mono', monospace",
          }}>{status}</div>
        )}
      </div>
    </section>
  );
}

function WatchlistSection() {
  const [text, setText] = useState("");
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri) return;
    settingsIpc.get("custom_watchlist").then(v => {
      if (!v) return;
      try {
        const terms = JSON.parse(v) as string[];
        setText(terms.join("\n"));
      } catch { /* malformed stored value — start fresh */ }
    }).catch(() => {});
  }, []);

  const terms = text.split("\n").map(t => t.trim()).filter(t => t.length >= 2);

  const save = async () => {
    setStatus("saving…");
    try {
      const capped = terms.slice(0, 200);
      if (isTauri) {
        await settingsIpc.set("custom_watchlist", JSON.stringify(capped));
      }
      setCustomTerms(capped); // live highlights update without restart
      setStatus(`✓ ${capped.length} term${capped.length === 1 ? "" : "s"} active`);
    } catch (e) {
      setStatus(`✗ ${String(e)}`);
    }
  };

  return (
    <section style={{ marginTop: 32 }}>
      <h3 style={{ fontFamily: "Instrument Serif, serif", fontSize: 20, margin: "0 0 12px" }}>
        Custom watchlist
      </h3>
      <div style={{
        padding: 12, background: "#0a0d14",
        border: "1px solid rgba(255,255,255,0.06)", borderRadius: 4,
      }}>
        <div style={{ fontSize: 11, color: "#9ba3b4", lineHeight: 1.5, marginBottom: 10 }}>
          Your own sensitive terms — project codenames, client names, internal hostnames.
          One per line. Matched case-insensitively as whole words, aliased as{" "}
          <code style={st.codeInline}>⟦custom_NN⟧</code> before egress. Never blocks a send.
        </div>
        <textarea
          value={text}
          onChange={e => setText(e.target.value)}
          placeholder={"Project Nightfall\nAcme Corp\nvault-prod-7"}
          spellCheck={false}
          style={{
            ...st.input, width: "100%", minHeight: 96, resize: "vertical",
            lineHeight: 1.6, boxSizing: "border-box" as const,
          }}
        />
        <div style={{ display: "flex", alignItems: "center", gap: 10, marginTop: 8 }}>
          <button onClick={save} style={st.saveBtn}>Save watchlist</button>
          <span style={{ fontSize: 10, color: "#9ba3b4", fontFamily: "'JetBrains Mono', monospace" }}>
            {terms.length} term{terms.length === 1 ? "" : "s"} · max 200
          </span>
          {status && (
            <span style={{
              fontSize: 11,
              color: status.startsWith("✓") ? "#7cffb2" : status.startsWith("✗") ? "#ff99a8" : "#9ba3b4",
              fontFamily: "'JetBrains Mono', monospace",
            }}>{status}</span>
          )}
        </div>
      </div>
    </section>
  );
}

function OllamaSection() {
  const [baseUrl, setBaseUrl] = useState("http://localhost:11434");
  const [models, setModels] = useState<string[]>([]);
  const [status, setStatus] = useState<string | null>(null);

  const check = async () => {
    if (!isTauri) { setStatus("browser preview — run the app to reach Ollama"); return; }
    setStatus("checking…");
    try {
      const h: OllamaHealth = await ollamaIpc.health();
      if (h.reachable) {
        const m = await ollamaIpc.listModels().catch(() => []);
        setModels(m);
        setStatus(`✓ reachable · ${h.model_count} model${h.model_count === 1 ? "" : "s"} installed`);
      } else {
        setModels([]);
        setStatus("✗ not reachable — start it with `ollama serve`");
      }
    } catch (e) {
      setStatus(`✗ ${String(e)}`);
    }
  };

  const save = async () => {
    const url = baseUrl.trim() || "http://localhost:11434";
    setStatus("saving…");
    try {
      await settingsIpc.set("ollama_base_url", url);
      await check();
    } catch (e) { setStatus(`✗ ${String(e)}`); }
  };

  useEffect(() => {
    if (!isTauri) return;
    settingsIpc.get("ollama_base_url")
      .then(v => { if (v) setBaseUrl(v); })
      .catch(() => {})
      .finally(check);
  }, []);

  const t = baseUrl.toLowerCase();
  const isLoopback = t.includes("localhost") || t.includes("127.0.0.1") || t.includes("[::1]") || t.includes("://::1");

  return (
    <section style={{ marginTop: 32 }}>
      <h3 style={{ fontFamily: "Instrument Serif, serif", fontSize: 20, margin: "0 0 12px" }}>
        Local models · Ollama
      </h3>
      <div style={{
        padding: 12, background: "#0a0d14",
        border: "1px solid rgba(255,255,255,0.06)", borderRadius: 4,
      }}>
        <div style={{ fontSize: 11, color: "#9ba3b4", lineHeight: 1.5, marginBottom: 10 }}>
          Run any open model fully on your machine via{" "}
          <a href="https://ollama.com" target="_blank" rel="noreferrer" style={{ color: "var(--neon)" }}>Ollama</a>.
          Install it, run <code style={st.codeInline}>ollama pull llama3.2</code>, and the model appears in the
          picker (⌘O). No API key needed.
        </div>

        <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center" }}>
          <input
            placeholder="http://localhost:11434"
            value={baseUrl}
            onChange={e => setBaseUrl(e.target.value)}
            style={{ ...st.input, minWidth: 240 }}
          />
          <button onClick={save} style={st.saveBtn}>Save</button>
          <button onClick={check} style={st.ghostBtn}>Check connection</button>
        </div>

        {status && (
          <div style={{
            fontSize: 11, marginTop: 8,
            color: status.startsWith("✓") ? "#7cffb2" : status.startsWith("✗") ? "#ff99a8" : "#9ba3b4",
            fontFamily: "'JetBrains Mono', monospace",
          }}>{status}</div>
        )}

        {models.length > 0 && (
          <div style={{ marginTop: 10 }}>
            {models.map(m => (
              <span key={m} style={{
                display: "inline-block", margin: "2px 6px 2px 0", padding: "2px 8px",
                border: "1px solid rgba(255,255,255,0.1)", borderRadius: 99,
                fontSize: 10, color: "#c9d2e3", fontFamily: "'JetBrains Mono', monospace",
              }}>{m}</span>
            ))}
          </div>
        )}

        <div style={{
          marginTop: 10, fontSize: 10, lineHeight: 1.5,
          color: isLoopback ? "#7cffb2" : "#fbbf24",
          fontFamily: "'JetBrains Mono', monospace",
        }}>
          {isLoopback
            ? "⛨ Loopback address — prompts run on-device with zero egress, so they're sent raw (no aliasing needed)."
            : "⚠ Remote address — that's network egress, so prompts are aliased + scanned by Vendetta like any cloud provider."}
        </div>
      </div>
    </section>
  );
}

const st: Record<string, CSSProperties> = {
  overlay:{ position:"fixed", inset:0, zIndex:260,
    background:"radial-gradient(ellipse at center, rgba(5,6,10,0.85), rgba(5,6,10,0.98))",
    backdropFilter:"blur(12px)",
    display:"flex", alignItems:"center", justifyContent:"center", animation:"fadeIn 0.2s" },
  stage:{ width:"min(720px, 96vw)", background:"rgba(10,12,20,0.96)",
    border:"1px solid rgba(255,255,255,0.1)", borderRadius:14, padding:24 },
  header:{ display:"flex", justifyContent:"space-between", alignItems:"flex-end", marginBottom:18 },
  close:{ width:36, height:36, borderRadius:8, background:"rgba(255,255,255,0.05)",
    border:"1px solid var(--line)", color:"var(--ink-1)", fontSize:20, cursor:"pointer" },
  list:{ display:"flex", flexDirection:"column", gap:10 },
  row:{ display:"flex", alignItems:"center", gap:10, padding:"12px 14px",
    background:"rgba(255,255,255,0.02)", border:"1px solid var(--line)", borderRadius:10 },
  input:{ flex:1, padding:"8px 10px", background:"rgba(255,255,255,0.03)",
    border:"1px solid var(--line)", borderRadius:6, color:"#fff",
    fontFamily:"'JetBrains Mono',monospace", fontSize:12, outline:"none" },
  saveBtn:{ padding:"8px 14px", background:"var(--neon)", color:"#000",
    border:"none", borderRadius:6, fontSize:12, fontWeight:600, cursor:"pointer" },
  foot:{ paddingTop:14, borderTop:"1px solid var(--line)", marginTop:14 },
};

function TeamSection() {
  const [status, setStatusData] = useState<TeamStatus | null>(null);
  const [teamId, setTeamId] = useState("");
  const [memberEmail, setMemberEmail] = useState("");
  const [newPubKey, setNewPubKey] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [lastSync, setLastSync] = useState<SyncOutcome | null>(null);

  const refresh = async () => {
    if (!isTauri) return;
    try {
      const s = await teamIpc.status();
      setStatusData(s);
      if (s.team_id) setTeamId(s.team_id);
      if (s.member_email) setMemberEmail(s.member_email);
    } catch (e) {
      setMsg(`✗ status: ${String(e)}`);
    }
  };

  useEffect(() => { refresh(); }, []);

  const doGenerate = async () => {
    setMsg("generating…");
    try {
      const r = await teamIpc.generateSigningKey();
      setNewPubKey(r.public_key);
      setMsg("✓ key generated. Paste the pubkey into your CF Worker's POST /admin/teams request.");
      await refresh();
    } catch (e) { setMsg(`✗ ${String(e)}`); }
  };

  const doSave = async () => {
    if (!teamId.trim() || !memberEmail.trim()) {
      setMsg("✗ team_id and email required");
      return;
    }
    setMsg("saving…");
    try {
      await teamIpc.configure({ team_id: teamId.trim(), member_email: memberEmail.trim() });
      setMsg("✓ team config saved");
      await refresh();
    } catch (e) { setMsg(`✗ ${String(e)}`); }
  };

  const toggleEnabled = async (v: boolean) => {
    setMsg(v ? "enabling…" : "disabling…");
    try {
      await teamIpc.setEnabled(v);
      setMsg(v ? "✓ audit sync on" : "✓ audit sync off");
      await refresh();
    } catch (e) { setMsg(`✗ ${String(e)}`); }
  };

  const doSyncNow = async () => {
    setMsg("syncing…");
    try {
      const out = await teamIpc.uploadNow();
      setLastSync(out);
      setMsg(out.error
        ? `⚠ ${out.uploaded}/${out.attempted} uploaded · ${out.error}`
        : `✓ ${out.uploaded}/${out.attempted} uploaded`);
      await refresh();
    } catch (e) { setMsg(`✗ ${String(e)}`); }
  };

  const copy = (s: string) => { navigator.clipboard.writeText(s).catch(() => {}); };
  const fmtTs = (unix: number | null) => unix
    ? new Date(unix * 1000).toLocaleString(undefined, { dateStyle: "short", timeStyle: "short" })
    : "never";

  return (
    <section style={{ marginTop: 32 }}>
      <h3 style={{ fontFamily: "Instrument Serif, serif", fontSize: 20, margin: "0 0 12px" }}>Team</h3>
      <div style={{
        padding: 12, background: "#0a0d14",
        border: "1px solid rgba(255,255,255,0.06)", borderRadius: 4,
      }}>
        {/* live status strip */}
        {status && (
          <div style={{
            display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 10,
            padding: "8px 0", marginBottom: 10,
            borderBottom: "1px solid rgba(255,255,255,0.05)",
            fontFamily: "'JetBrains Mono', monospace", fontSize: 10,
          }}>
            <div>
              <div style={{ color: "#9ba3b4", letterSpacing: 2, marginBottom: 2 }}>SYNC</div>
              <div style={{ color: status.enabled ? "#7cffb2" : "#9ba3b4" }}>
                {status.enabled ? "ON" : "OFF"}
              </div>
            </div>
            <div>
              <div style={{ color: "#9ba3b4", letterSpacing: 2, marginBottom: 2 }}>CONFIGURED</div>
              <div style={{ color: status.configured ? "#7cffb2" : "#ff99a8" }}>
                {status.configured ? "YES" : "NO"}
              </div>
            </div>
            <div>
              <div style={{ color: "#9ba3b4", letterSpacing: 2, marginBottom: 2 }}>PENDING</div>
              <div style={{ color: status.pending_count > 0 ? "var(--neon)" : "#9ba3b4" }}>
                {status.pending_count}
              </div>
            </div>
            <div>
              <div style={{ color: "#9ba3b4", letterSpacing: 2, marginBottom: 2 }}>LAST SYNC</div>
              <div style={{ color: "#fff" }}>{fmtTs(status.last_upload_at)}</div>
            </div>
          </div>
        )}

        {/* step 1: generate signing key */}
        <div style={{ marginBottom: 14 }}>
          <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 4 }}>
            1 · Signing key {status?.has_signing_key && <span style={{ color: "#7cffb2", fontSize: 10, fontWeight: 400 }}>✓ stored in keychain</span>}
          </div>
          <div style={{ fontSize: 11, color: "#9ba3b4", marginBottom: 6, lineHeight: 1.5 }}>
            Ed25519 keypair. Private key stays in your macOS keychain; public key is what your admin paste into
            <code style={st.codeInline}> POST /admin/teams </code> on your Cloudflare Worker.
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <button onClick={doGenerate} style={st.saveBtn}>
              {status?.has_signing_key ? "Regenerate" : "Generate"} key
            </button>
            {newPubKey && (
              <button onClick={() => copy(newPubKey)} style={st.ghostBtn}>Copy pubkey</button>
            )}
          </div>
          {newPubKey && (
            <pre style={st.code}>{newPubKey}</pre>
          )}
        </div>

        {/* step 2: configure team id + email */}
        <div style={{ marginBottom: 14 }}>
          <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 4 }}>2 · Team config</div>
          <div style={{ fontSize: 11, color: "#9ba3b4", marginBottom: 6 }}>
            Admin registers the pubkey with <code style={st.codeInline}>POST /admin/teams</code> and receives a
            <code style={st.codeInline}> team_id</code>. Paste it here along with your email.
          </div>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <input
              placeholder="team_id (from /admin/teams response)"
              value={teamId}
              onChange={e => setTeamId(e.target.value)}
              style={{ ...st.input, minWidth: 240 }}
            />
            <input
              placeholder="you@company.com"
              value={memberEmail}
              onChange={e => setMemberEmail(e.target.value)}
              style={{ ...st.input, minWidth: 200 }}
            />
            <button onClick={doSave} style={st.saveBtn}>Save</button>
          </div>
        </div>

        {/* step 3: enable + sync */}
        <div style={{ marginBottom: 8 }}>
          <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 4 }}>3 · Enable audit sync</div>
          <div style={{ display: "flex", gap: 10, alignItems: "center", flexWrap: "wrap" }}>
            <label style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <input
                type="checkbox"
                checked={status?.enabled === true}
                disabled={!status?.configured || !status?.has_signing_key}
                onChange={e => toggleEnabled(e.target.checked)}
              />
              <span style={{ fontSize: 12 }}>
                Sync audit events to <code style={st.codeInline}>{status?.endpoint ?? "api.sentynyx.com/audit"}</code>
              </span>
            </label>
            <button
              onClick={doSyncNow}
              disabled={!status?.enabled || !status?.configured}
              style={{
                ...st.ghostBtn,
                opacity: (!status?.enabled || !status?.configured) ? 0.4 : 1,
                cursor: (!status?.enabled || !status?.configured) ? "not-allowed" : "pointer",
              }}
            >Sync now</button>
          </div>
        </div>

        {msg && (
          <div style={{
            fontSize: 11, marginTop: 10,
            color: msg.startsWith("✓") ? "#7cffb2" :
                   msg.startsWith("⚠") ? "#fbbf24" :
                   msg.startsWith("✗") ? "#ff99a8" : "#9ba3b4",
            fontFamily: "'JetBrains Mono', monospace",
          }}>{msg}</div>
        )}

        {lastSync && lastSync.attempted > 0 && (
          <div style={{
            marginTop: 8, fontSize: 10, color: "#9ba3b4",
            fontFamily: "'JetBrains Mono', monospace",
          }}>
            last batch: {lastSync.attempted} attempted · {lastSync.uploaded} uploaded ·{" "}
            {lastSync.skipped_replay} server-deduped
          </div>
        )}
      </div>
    </section>
  );
}

// Extended style tokens used by TeamSection — appended as Object.assign so the
// original `st` const stays readable above.
Object.assign(st, {
  ghostBtn: {
    padding: "8px 14px",
    background: "transparent",
    color: "var(--ink-1)",
    border: "1px solid rgba(255,255,255,0.15)",
    borderRadius: 6,
    fontSize: 12,
    cursor: "pointer",
    fontFamily: "'JetBrains Mono', monospace",
  },
  code: {
    marginTop: 8,
    padding: 10,
    background: "rgba(0,0,0,0.4)",
    border: "1px solid rgba(255,255,255,0.08)",
    borderRadius: 4,
    fontFamily: "'JetBrains Mono', monospace",
    fontSize: 11,
    color: "var(--neon)",
    wordBreak: "break-all",
    whiteSpace: "pre-wrap",
  },
  codeInline: {
    fontFamily: "'JetBrains Mono', monospace",
    fontSize: 10,
    background: "rgba(0,0,0,0.35)",
    padding: "1px 5px",
    borderRadius: 3,
    color: "var(--neon)",
  },
});
