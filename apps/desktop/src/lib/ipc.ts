import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Span, BlockReason, AuditEntry, AuditMetrics, AllModelStatus, ModelProgressEvent } from "./types";

export interface DetectResult { spans: Span[] }

/// Per-send instrumentation from the Rust `send()` pipeline. Delivered
/// inline with SendMeta so the DevInspector can render the full pre-dispatch
/// picture (regex/NER timings, merged spans, aliased payload) without any
/// extra round-trip.
export interface PipelineTrace {
  text_len: number;
  regex_ms: number;
  regex_spans_count: number;
  ner_ms: number;
  ner_spans_count: number;
  ner_status: "ok" | "timeout" | "not_loaded" | "error";
  ner_error: string | null;
  /// Matches from the user-defined custom watchlist (Settings → Watchlist).
  custom_spans_count: number;
  merge_ms: number;
  alias_ms: number;
  total_pre_dispatch_ms: number;
  merged_spans_count: number;
  aliased_prompt: string;
  provider: string;
  model_id: string;
  paranoid_enabled: boolean;
  regex_spans: Span[];
  ner_spans: Span[];
}

export interface StreamTrace {
  conv_id: string;
  msg_id: string;
  ttft_ms: number | null;
  total_stream_ms: number;
  chunks: number;
  bytes: number;
  response_aliased: string;
  response_rehydrated: string;
  error: string | null;
}

export interface ParanoidTrace {
  conv_id: string;
  msg_id: string;
  ms: number;
  spans_found: number;
  timed_out: boolean;
  error: string | null;
}

export interface SendMeta {
  assistant_msg_id: string;
  aliased_prompt: string;
  spans: Span[];
  blocked: BlockReason | null;
  trace: PipelineTrace;
}
export interface StreamChunk {
  conv_id: string; msg_id: string; delta: string; done: boolean; error: string | null;
}
export interface ConsensusColumn { model_id: string; msg_id: string }
export interface ConversationRow {
  id: string; title: string; model_id: string; created_at: string; shielded: boolean;
}
export interface MessageRow {
  id: string; conv_id: string; role: "user" | "assistant";
  text_raw: string; text_aliased: string; spans: Span[]; created_at: string;
}

export const ipc = {
  detect: (text: string, conv_id?: string) =>
    invoke<DetectResult>("detect", { text, convId: conv_id }),
  /// Regex + NER in one round-trip. Used by the composer's live-highlight
  /// loop (debounced) so names / orgs / codenames / locations show up
  /// in real time alongside regex hits.
  detectWithNer: (text: string) =>
    invoke<DetectResult>("detect_with_ner", { text }),
  send: (args: { conv_id: string; model_id: string; text: string }) =>
    invoke<SendMeta>("send", { args }),
  consensus: (args: { conv_id: string; model_ids: string[]; text: string }) =>
    invoke<ConsensusColumn[]>("consensus", { args }),
  listConversations: () => invoke<ConversationRow[]>("list_conversations"),
  loadConversation: (conv_id: string) => invoke<MessageRow[]>("load_conversation", { convId: conv_id }),
  newConversation: (title: string, model_id: string) =>
    invoke<string>("new_conversation", { args: { title, model_id } }),
  setApiKey: (provider: string, secret: string) =>
    invoke<{ storage: "keychain" | "file" }>("set_api_key", { args: { provider, secret } }),
  validateApiKey: (provider: string, secret?: string) =>
    invoke<{ ok: boolean; reason: string | null }>("validate_api_key", {
      args: { provider, secret: secret ?? null },
    }),
  hasApiKey: (provider: string) => invoke<boolean>("has_api_key", { provider }),
  listConfiguredProviders: () => invoke<string[]>("list_configured_providers"),
  listAudit: (limit = 50) => invoke<AuditEntry[]>("list_audit", { limit }),
  auditMetrics: () => invoke<AuditMetrics>("audit_metrics"),
};

export function onStreamChunk(cb: (c: StreamChunk) => void) {
  return listen<StreamChunk>("message://chunk", (e) => cb(e.payload));
}
export function onAuditNew(cb: () => void) {
  return listen("audit://new", () => cb());
}

export const modelsIpc = {
  status: () => invoke<AllModelStatus>("model_status"),
  download: (id: string) => invoke<void>("download_model", { args: { id } }),
  delete: (id: string) => invoke<void>("delete_model", { args: { id } }),
  setParanoid: (enabled: boolean) => invoke<void>("set_paranoid_mode", { args: { enabled } }),
  getParanoid: () => invoke<boolean>("get_paranoid_mode"),
};

/// Generic key/value settings backed by the SQLite settings table.
/// Values are opaque strings — caller picks JSON / raw / etc.
export const settingsIpc = {
  get: (key: string) => invoke<string | null>("get_setting", { key }),
  set: (key: string, value: string) =>
    invoke<void>("set_setting", { args: { key, value } }),
};

export interface ExportResult { dest: string; files: string[] }

export const dataIpc = {
  export: () => invoke<ExportResult>("export_data"),
  deleteAll: () => invoke<void>("delete_all_data"),
};

export const telemetryIpc = {
  get: () => invoke<boolean>("get_telemetry_enabled"),
  set: (enabled: boolean) =>
    invoke<void>("set_telemetry_enabled", { args: { enabled } }),
};

export interface BuildInfo {
  team_cloud: boolean;
  telemetry_available: boolean;
  version: string;
}

/// Compile-time feature flags reported by the running binary. The binary is the
/// source of truth for which IPC commands actually exist, so the UI gates
/// optional surfaces (Team, Telemetry) on this rather than a build-time define.
export const buildInfoIpc = {
  get: () => invoke<BuildInfo>("build_info"),
};

export interface OllamaHealth {
  reachable: boolean;
  base_url: string;
  model_count: number;
}

/// Local model hosting via Ollama (https://ollama.com). No API key — only a
/// base URL stored under the `ollama_base_url` setting (default
/// http://localhost:11434). A loopback server is treated as zero-egress, so
/// prompts to it skip aliasing; a remote base URL is aliased like any cloud
/// provider (the Rust side makes the authoritative host-aware decision).
export const ollamaIpc = {
  listModels: () => invoke<string[]>("ollama_list_models"),
  health: () => invoke<OllamaHealth>("ollama_health"),
};

export interface SystemStats {
  rss_mb: number;
  uptime_sec: number;
  version: string;
  pid: number;
}

/// Process stats for the About dialog — RSS in MB, uptime seconds, pid,
/// Cargo crate version. Backed by the `sysinfo` crate on the Rust side.
export const systemIpc = {
  stats: () => invoke<SystemStats>("system_stats"),
};

// ---------------------------------------------------------------------------
// Team-tier audit sync (Phase 5 wiring)
// ---------------------------------------------------------------------------

export interface TeamStatus {
  enabled: boolean;
  configured: boolean;
  team_id: string | null;
  member_email: string | null;
  endpoint: string;
  last_upload_at: number | null; // unix seconds
  pending_count: number;
  has_signing_key: boolean;
}

export interface SyncOutcome {
  attempted: number;
  uploaded: number;
  skipped_replay: number;
  error: string | null;
}

/// Admin + user APIs for the Team-tier audit-sync feature.
///
/// Onboarding flow (admin-side):
///   1. `generateSigningKey()` → base64 public key. Admin pastes this
///      into the CF Worker's POST /admin/teams call (out of band).
///   2. Worker returns a team_id.
///   3. `configure({ team_id, member_email })` — stores config locally.
///   4. `setEnabled(true)` — flips the opt-in switch; background sync
///      task picks up from the next tick (runs every 5 min).
///
/// User-side: `status()` + `uploadNow()` for the Settings → Team panel.
export const teamIpc = {
  status: () => invoke<TeamStatus>("team_status"),
  generateSigningKey: () =>
    invoke<{ public_key: string }>("team_generate_signing_key"),
  configure: (args: { team_id: string; member_email: string; endpoint?: string }) =>
    invoke<void>("team_configure", { args }),
  setEnabled: (enabled: boolean) =>
    invoke<void>("team_set_enabled", { args: { enabled } }),
  uploadNow: () => invoke<SyncOutcome>("team_upload_now"),
};

export function onModelProgress(cb: (e: ModelProgressEvent) => void) {
  return listen<ModelProgressEvent>("model://progress", (e) => cb(e.payload));
}

export function onModelReady(cb: (id: string) => void) {
  return listen<{ id: string }>("model://ready", (e) => cb(e.payload.id));
}

export interface ParanoidHit {
  conv_id: string;
  count: number;
  spans: { start: number; end: number; kind: string; raw: string; alias: string }[];
}

export function onParanoidHit(cb: (h: ParanoidHit) => void) {
  return listen<ParanoidHit>("paranoid://hit", (e) => cb(e.payload));
}

/// Fires when the provider stream for a send() completes (or errors). Carries
/// TTFT, total stream ms, chunk/byte counts, and BOTH the raw aliased
/// response (what the model sent) and the rehydrated output (what the user
/// saw). DevInspector joins this with the SendMeta trace by msg_id.
export function onTraceStream(cb: (t: StreamTrace) => void) {
  return listen<StreamTrace>("vendetta://trace-stream", (e) => cb(e.payload));
}

/// Fires when the paranoid Qwen scan for a send() completes. Always fires
/// when paranoid was enabled for the send, regardless of whether spans were
/// found — so the inspector can show paranoid cost even on empty scans.
export function onTraceParanoid(cb: (t: ParanoidTrace) => void) {
  return listen<ParanoidTrace>("vendetta://trace-paranoid", (e) => cb(e.payload));
}

/** Detect whether we're running inside Tauri (IPC available) or just a browser preview. */
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
