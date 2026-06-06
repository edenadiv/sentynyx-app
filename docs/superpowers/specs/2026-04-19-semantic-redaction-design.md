# Semantic Redaction — Design Spec

- **Date:** 2026-04-19
- **Author:** Eden Adiv (via Claude Code brainstorming)
- **Status:** Approved, ready for implementation plan
- **Target release:** Sentynyx v0.2
- **Context:** v0.1 ships regex-only Vendetta. Fundraise demo needs a semantic moat — regex alone misses proper names not in the hardcoded whitelist, paraphrased codenames, non-English PII, and business-sensitive phrasing with no token patterns. "One shot" on investor demos: the feature must actually work.

---

## Goals

1. Catch sensitive content that regex structurally cannot: arbitrary proper names, organizations, and custom project codenames.
2. Offer an optional deeper "paranoid" pass that catches *semantic* sensitivity — tone and context, not just token patterns.
3. Preserve all existing regex guarantees. The ML layer is additive, never load-bearing for the critical classes (SSN, API key, email).
4. Keep end-to-end latency acceptable — target ≤80 ms added to the `send` critical path.
5. Demo cleanly with no internet dependency — the installer ships a working regex-only product; ML models are opt-in downloads.

## Non-goals (this spec)

- Training our own PII model. Deferred to v0.3 post-raise, with a real ML hire.
- Code signing, notarization, or signed installers. Separate spec when the product needs self-serve distribution.
- Knowledge Atlas, shared workspaces, role-based vaults, real agent tool-use. Separate specs.
- Replacing regex. Regex remains primary and authoritative on the classes it handles.
- Running NER on every keystroke. Keystroke detection stays regex-only.

## Scope

Add two new detection layers to the existing Vendetta pipeline:

- **Always-on encoder** (ONNX Runtime + GLiNER-small-v2.1 int8, ~80 MB) — runs in parallel with regex on every `send`, catches NER-class entities. Target inference: 50–80 ms CPU.
- **Opt-in "paranoid mode" LLM** (llama.cpp + Qwen-3-1.5B Q4_K_M GGUF, ~950 MB) — runs as a non-blocking background task after the main send. Catches semantic sensitivity that has no token pattern.

Both models are downloaded explicitly by the user on first run. Installer ships with zero ML models bundled.

---

## 1. Architecture

### Current flow (v0.1)

```
renderer → IPC send → regex.detect → alias → audit → provider.stream → rehydrate → emit
```

### New flow (v0.2)

```
renderer → IPC send
  → detectors (regex ∥ ner, tokio::join!)
  → merge_spans
  → alias → audit
  → provider.stream → rehydrate → emit
         │
         └─ (opt-in, non-blocking) → llm.paranoid_scan
              → if finds something → audit + toast
```

### New Rust modules

| File | Role |
|---|---|
| `src-tauri/src/detect/mod.rs` *(new)* | `Detector` trait + `merge_spans()`. Also re-exports `detect::regex` which wraps the existing `vendetta::detect`. |
| `src-tauri/src/detect/ner.rs` *(new)* | GLiNER inference via `ort` crate. Loads ONNX session lazily, runs inference, returns `Vec<Span>`. |
| `src-tauri/src/detect/llm.rs` *(new)* | Qwen-3-1.5B via `llama-cpp-2`. Opt-in paranoid scan, structured-output parsing with one retry. |
| `src-tauri/src/models.rs` *(new)* | Model download + SHA-256 verify + cached path resolution. Shared by ner/llm. |

### Modified files

| File | Change |
|---|---|
| `src-tauri/src/commands.rs` | `send` awaits both detectors via `tokio::join!`, merges, continues. New commands: `set_paranoid_mode`, `model_status`, `download_model`, `delete_model`. |
| `src-tauri/src/vendetta.rs` | `Kind` enum extends with `PERSON_NER`, `ORG_NER`, `CODENAME_NER`, `LOCATION_NER`, `EMPID_NER`. Suffix keeps audit and alias namespaces distinct from regex. |
| `src-tauri/src/lib.rs` | Register new commands, init `ModelRegistry` in `AppState`. |
| `src-tauri/Cargo.toml` | Add `ort` (ONNX), `tokenizers`, `llama-cpp-2`, `reqwest` features for streaming downloads. |

### Keystroke behavior — unchanged

Live composer highlighting stays regex-only. NER inference at ~50 ms would pile up behind typing cadence (~10 keys/sec). The encoder only runs at `send` time.

---

## 2. Components

### `detect/mod.rs`

```rust
#[async_trait]
pub trait Detector: Send + Sync {
    async fn detect(&self, text: &str) -> Result<Vec<Span>, DetectError>;
}

pub fn merge_spans(regex: Vec<Span>, ner: Vec<Span>) -> Vec<Span>;
```

`merge_spans` rules:
1. Index regex spans by `(start, end)`.
2. For each NER span: if it overlaps any regex span, drop it (regex wins on its turf — prevents double-aliasing `Sarah Chen` as both `{{person_01}}` and `{{person_NER_01}}`).
3. Keep non-overlapping NER spans with `_NER`-suffixed `Kind`.
4. Output sorted by `start`, same shape as today's `vendetta::detect` result.

### `detect/ner.rs`

- **Model:** GLiNER-small-v2.1, int8 ONNX. Chosen for: zero-shot custom labels (we pass our taxonomy at runtime — no training needed for `internal-project-codename`), active maintenance through 2024–2025, clean ONNX export.
- **Runtime:** `ort` crate (ONNX Runtime bindings). CPU execution provider on all platforms for v0.2 — Metal/CUDA acceleration is a v0.3 optimization.
- **Session lifecycle:** singleton `Session` via `once_cell::sync::OnceCell`. Lazy-initialized on first `detect()` call. Doesn't slow app startup.
- **Tokenizer:** `tokenizers` crate (HuggingFace). Tokenizer JSON downloaded alongside model.
- **Labels passed at inference:** `["person", "organization", "internal-project-codename", "location", "employee-id-code"]`.
- **Label → Kind mapping:** `person → PERSON_NER`, `organization → ORG_NER`, `internal-project-codename → CODENAME_NER`, `location → LOCATION_NER`, `employee-id-code → EMPID_NER`. Constant mapping table in `detect/ner.rs`, single source of truth.
- **Error handling:** Returns `Err(DetectError::ModelNotLoaded)` if model files missing. Caller falls back to regex-only cleanly.
- **Fallback model:** if GLiNER's ONNX export has issues on evaluation, swap to `Isotonic/deberta-v3-base-pii-identification` (narrower label set, rock-solid ONNX export). Evaluation decides.

### `detect/llm.rs`

- **Model:** Qwen-3-1.5B Q4_K_M GGUF. Chosen for: good structured-output reliability at its size, Apache 2.0 license, GGUF ecosystem support.
- **Runtime:** `llama-cpp-2` crate (llama.cpp bindings). Metal backend on macOS, CPU elsewhere.
- **Prompt:** single-shot, asks for JSON array of sensitive spans with `start`, `end`, `kind`, `reason` fields.
- **Parsing:** tolerant JSON parser; on invalid output retry once with a stricter prompt ("output ONLY a valid JSON array, no prose"); on second failure return empty spans (fail-closed).
- **Invocation:** from `commands::send`, spawned via `tokio::spawn` after regex+NER merge returns. Does **not** block the critical path. Results emit via a `paranoid://hit` event and append audit entries.

### `models.rs`

```rust
pub struct ModelSpec {
    pub id: &'static str,
    pub url: &'static str,          // HF direct URL, revision-pinned
    pub sha256: &'static str,
    pub size_bytes: u64,
}

pub async fn ensure_local(
    spec: &ModelSpec,
    progress_cb: impl Fn(u64, u64) + Send + 'static,
) -> Result<PathBuf, ModelError>;

pub fn local_path(spec: &ModelSpec) -> PathBuf;
pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), ModelError>;
```

Storage: `<app_data>/models/<id>/<file>`. Partial downloads saved as `<file>.partial`, atomically renamed on SHA match. URLs pinned by commit hash: `hf.co/{repo}/resolve/{commit}/{file}` — "latest" is never used.

### Frontend additions

- `SettingsPanel` — new "Models" tab with toggles and status per model.
- `ModelDownloadPanel` — first-run modal with per-model progress bars and a "Continue with regex-only" escape hatch.
- TopBar status chip reflecting `model_status`.
- `VendettaPanel` — source-distinguishing glyphs on each chip.
- `Toast` component for paranoid-mode hits (new).

---

## 3. Data flow

### Worked example

User types:

> `Draft a memo to Project Helios for Sarah Chen at sarah.chen@halcyonlabs.com about the Q4 revenue of $42,500,000.`

and hits Transmit.

1. Renderer → `ipc.send({conv_id, model_id, text})`.
2. `commands::send` loads alias state for the conversation from SQLite.
3. Parallel detection via `tokio::join!`:
   - regex → `[email, money, codename (whitelist), name (whitelist)]`
   - NER → `[Sarah Chen (person), Project Helios (organization)]`
4. `merge_spans(regex, ner)` → NER spans overlap regex spans for both `Sarah Chen` and `Project Helios` → dropped. Final: regex's 4 spans. (Swap `Sarah Chen` for `Jamie Torres` and NER becomes the only detector that catches it.)
5. Critical class check — no SSN / API key → proceed. (If present, existing `BLOCK_EGRESS` path runs — unchanged.)
6. Alias using merged spans. Alias state persisted.
7. Audit entries — one per span, tagged with a new `source` column (`"regex" | "ner" | "llm"`).
8. Paranoid scan, if enabled — spawned via `tokio::spawn`, does NOT block.
9. Persist user message (`text_raw`, `text_aliased`, merged `spans`).
10. Dispatch to provider with aliased text.
11. Stream chunks → rehydrate via `vendetta::rehydrate_stream_with`, reverse map built from the merged spans so NER-aliased tokens rehydrate correctly too.
12. On stream completion, persist assistant message.

### Latency budget

| Stage | Target |
|---|---|
| regex detect | 1 ms |
| NER inference (GLiNER-small int8, CPU, ~128-tok prompt) | 50–80 ms |
| merge + alias + persist + audit | 5 ms |
| Provider TTFT (OpenAI / Anthropic) | 300–800 ms (network-bound) |
| **Total ML overhead on critical path** | **~60 ms** |

Paranoid LLM: 400–900 ms on Qwen-3-1.5B-Q4 on Apple Silicon. Async — never on the critical path.

### Schema changes

- `messages.spans_json` — already JSON, new `_NER` kinds flow through without a schema change.
- `audit` table — new column `source TEXT NOT NULL DEFAULT 'regex'`. Migration appends the column if missing.
- New table `settings(key TEXT PRIMARY KEY, value TEXT)` — stores `paranoid_mode` flag and any future global toggles.

---

## 4. Model distribution

### What ships in the installer

**Regex-only, no models bundled.** Installer stays ~25 MB. If the download fails during an investor demo, the app still works with the original regex product.

### Models

| Model | File | Size | Host |
|---|---|---|---|
| GLiNER-small-v2.1 (int8 ONNX) | `gliner-small-v2.1.int8.onnx` | ~80 MB | HuggingFace Hub, commit-pinned |
| GLiNER tokenizer | `tokenizer.json` | ~3 MB | HuggingFace Hub, commit-pinned |
| Qwen-3-1.5B Q4_K_M (GGUF) | `qwen3-1.5b-q4km.gguf` | ~950 MB | HuggingFace Hub, commit-pinned |

URLs and SHA-256 hashes hardcoded in `models.rs` as `ModelSpec` constants. Revision-pinned via `hf.co/{repo}/resolve/{commit}/{file}` — never "latest".

### First-run flow

1. App opens. Boot sequence plays.
2. Renderer requests `model_status` IPC.
3. Rust reports `{ner: "missing", llm: "missing"}`.
4. TopBar chip: `◐ semantic offline — using regex`.
5. User clicks chip or opens Settings → Models → clicks "Download GLiNER":
   - `ModelDownloadPanel` opens.
   - NER-only by default (80 MB ~8s on 100 Mbps home connection).
   - Checkbox: "Also download paranoid LLM (950 MB)" — unchecked by default.
6. Download runs via `reqwest` with HTTP Range (resumable). Partial file saved as `.partial`.
7. On completion: SHA-256 verify. On mismatch: delete, retry once, then error with "Continue with regex-only" button.
8. On success: atomic rename, ONNX session loads lazily on next `send`, chip flips to `◆ semantic ready`.

### Explicit vs background download

**Explicit, never automatic.** Reasoning:
- 950 MB auto-download on first launch is hostile UX and a surprise bandwidth bill on metered connections.
- "Enable semantic detection" is a product feature the user turns on — frames NER as intentional, not magical-background.
- Investors love the toggle: sovereignty over your own models reads well.

**Exception:** if `paranoid_mode=true` is already in settings but the model file is gone (e.g., user deleted from disk), the app silently resumes an interrupted download on startup — assumes prior intent.

### Integrity

- SHA-256 check at every session load (not just post-download). Catches disk corruption.
- On mismatch at load time: file deleted, chip flips to "missing", app degrades to regex-only until the user re-downloads.
- Model updates ship via app updates — `ModelSpec` constants bump in a new Sentynyx release. No in-app "check for model update" — ties model version to app version, one thing to audit.

### Failure behaviors

| Scenario | Behavior |
|---|---|
| No internet at first launch | Regex-only works; chip offers retry |
| Download interrupted | Resumes via HTTP Range on next attempt |
| HF Hub rate-limit (429) | Exponential backoff, max 5 attempts |
| Disk full mid-download | Abort, delete `.partial`, clear error |
| SHA mismatch | Delete, retry once, fail to regex-only with error chip |
| Model file externally deleted | Detected on load, chip → "missing", one-click re-download |

---

## 5. UX surface

### TopBar status chip (new, rightmost)

| State | Chip | Color |
|---|---|---|
| No models, no download attempted | `◐ semantic off` | neutral grey |
| Download running | `◐ semantic 42%` | neon yellow |
| NER ready, LLM off | `◆ semantic ready` | mint green |
| NER ready, LLM ready + paranoid on | `◆◆ paranoid active` | neon yellow |
| Download failed | `✕ semantic error` | danger pink |

Click → opens Settings → Models tab.

### ModelDownloadPanel (first-run modal; also reachable from Settings)

One row per model: name, size, status (Not installed / Downloading / Ready / Error), action button (Download / Cancel / Re-verify / Delete). Global "Continue with regex only" at bottom — dismisses, can re-open from Settings.

### SettingsPanel — new "Models" tab

Three sections:

1. **Semantic detection (NER)** — on/off toggle, model status, disk usage, "Delete model" button.
2. **Paranoid mode (LLM)** — same shape. Toggle disabled if LLM model missing. Warning text: "Adds ~500 ms to each send for deeper semantic detection."
3. **Debug** — last detection stats: spans caught per detector across the last 20 sends, p50/p95 latencies. Demo gold.

### VendettaPanel source glyphs

Each chip gets a tiny prefix glyph:

- `∎ {{email_01}}` — regex (filled square)
- `◆ {{person_NER_01}}` — encoder (diamond)
- `✦ {{person_LLM_01}}` — paranoid LLM (star)

Hover tooltip names the detector. Makes the ML story legible in the UI during demos.

### Paranoid hit toast

When paranoid scan finds spans post-send, bottom-right toast: `✦ Paranoid scan: found 2 additional sensitive spans — aliased retroactively.` Auto-dismiss 6 s. (Clicking to open a diff view is a stretch goal, not MVP.)

### Composer — unchanged

Keystroke highlighting stays regex-only. No visible change until `send`.

### Transcript — unchanged

User still sees rehydrated (original) values. Dual-view ("Model saw") shows the aliased version with NER/LLM aliases woven in naturally.

---

## 6. Failure modes & testing

### Failure mode matrix

| Failure | Detection | Response |
|---|---|---|
| Model file missing | Check at Rust startup + on-demand | Chip → `semantic off`, regex-only |
| ONNX session init fails | Try/catch at first inference | Log, cache failure, regex-only until app restart |
| GLiNER inference panics or hangs >500 ms | `tokio::time::timeout(500ms, infer)` | Timeout → drop NER spans for this send; regex proceeds |
| GLiNER returns invalid span offsets | Validate `start < end ≤ text.len()` | Drop invalid spans silently |
| Paranoid LLM returns invalid JSON | Parse, retry once with stricter prompt | Second failure → drop, don't alert |
| LLM crash mid-stream | Task isolation (`tokio::spawn`) | Log, doesn't affect completed main send |
| Alias collision between regex + NER | Enforced by `_NER` suffix on `Kind` | — |
| Two sends race on alias state | Existing mutex on store | — |
| SHA-256 mismatch on model load | Check at every session init | Delete file, chip → error, never use a corrupt model |
| Rehydration miss on an alias | Already handled — `replace` is no-op on miss | Already robust |

**Key invariant:** a failure in the ML layer MUST NOT suppress a regex detection. Regex is load-bearing; NER/LLM are additive.

### Testing strategy

**Unit tests** (new, per module):
- `merge_spans` — overlap rules, empty inputs, identical spans, partial overlaps, span-within-span.
- `detect::ner::parse_output` — valid, empty, malformed, offset-out-of-bounds.
- `detect::llm::parse_json_spans` — valid, missing fields, wrong types, empty array, garbage prose.
- `models::verify_sha256` — matching, mismatching, partial file.

**Integration tests** (`src-tauri/tests/`):
- `test_send_with_ner_unavailable` — regex still works, response streams.
- `test_send_with_mock_ner` — stub detector returns canned spans; verify merge + alias + rehydration round-trip.
- `test_paranoid_task_failure_does_not_affect_main_send` — paranoid task panics; main send completes normally.

**Eval harness** (new, `src-tauri/eval/`) — *the "one shot" insurance*:

Fixed 100-prompt JSON file:
- 40 prompts with PII mix (emails + names + codenames + SSNs + API keys).
- 30 prompts with *semantic-only* sensitivity (e.g., "my manager Jamie mentioned layoffs") — no regex targets.
- 20 benign prompts (zero expected detections → false-positive check).
- 10 adversarial prompts (near-PII strings designed to trip the model).

`cargo run --bin eval` runs the full detector stack against this set and prints:

```
Regex:        P=100.0% R=99.2%   (baseline, unchanged)
NER:          P=87.4%  R=81.1%   (target: P>80, R>70)
Merged:       P=94.2%  R=95.8%
Latency p95:  78 ms    p99: 124 ms
Critical recall (SSN/APIKEY): 100.0%  ← GATE
```

**Go/no-go gate** (blocks shipping NER enabled by default):
- Critical recall (SSN / API key) < 100%, **or**
- Merged precision < 85%, **or**
- p99 latency > 200 ms

If any gate fails → NER ships disabled-by-default behind a settings flag. App still demos. *This is the guardrail that prevents "one shot" from becoming "zero shot."*

**LLM paranoid mode has no ship gate** — it's opt-in, off by default, user-toggled. Flakiness is acceptable because the feature is labeled experimental and never on the critical send path. If the eval harness shows the LLM is broken, we ship with the toggle hidden behind a dev flag rather than blocking the release.

**CI integration:** eval runs in GitHub Actions on every PR touching `detect/`. Report uploaded as a job artifact. Regressions block merge.

---

## Out of scope / follow-ups

- Metal / CUDA acceleration for ONNX Runtime (v0.3 optimization).
- Multilingual PII detection beyond what GLiNER-multi already covers (v0.3).
- Fine-tuning a custom PII model with proprietary training data (v0.3, post-raise).
- User-editable custom entity labels (v0.3 — power-user config surface).
- Clickable diff view for paranoid hits (post-MVP polish).
- Model A/B evaluation tooling beyond the fixed 100-prompt eval set.
- Code signing, notarization, and self-serve distribution — separate spec when needed.
