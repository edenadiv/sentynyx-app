# Changelog

## v0.4.0 — Open source (unreleased)

The public open-source launch. Sentynyx becomes an auditable, local-first
privacy perimeter you can read, build, and verify — released under **AGPL-3.0**
on an **open-core** model (see `OPEN-CORE.md`).

### Added

- **Seasoning pass — every surface polished**: provider errors are now
  human (parsed JSON message + plain-language hint per status, instead of
  raw HTTP bodies); Google API keys moved out of URLs into headers; every
  overlay closes on Escape; the command palette lost its two vestigial
  fake entries (Knowledge Atlas, Switch Role) and gained real ones (Tune
  Detection, Audit Log); a live `⇄ proxy :4242` chip appears in the top
  bar when the privacy proxy is running and failed autostarts surface
  their reason in Settings; `/v1/models` on the proxy now lists live
  Ollama models; the empty state points at Settings when no provider is
  configured; pasted API keys are trimmed; a llama.cpp init failure now
  degrades to regex+NER instead of aborting the app; alias-map
  persistence failures fail the send instead of silently forking aliases;
  keyboard-focus rings and `prefers-reduced-motion` are respected; the
  first-run wizard quotes real model sizes (83 MB, 468 MB) and mentions
  detection packs; assorted copy de-jargoned (boot lines, onboarding,
  agent preview labeled "not functional yet").
- **Structured-data scanning** (now live in the composer too — column hits
  highlight as you paste, mirroring the engine): pasted CSV/TSV/semicolon/pipe tables get
  column-aware detection — headers like `ssn`, `email`, `card_number`,
  `salary`, `full_name` mark every cell in the column sensitive even when
  the bare value matches no pattern (undashed SSNs, arbitrary names).
  Checksum-invalid values under blocking headers alias as `custom` instead
  of hard-blocking; ragged rows are skipped so spans can never land at
  wrong offsets. Runs in the app pipeline, the proxy, and the eval gate.
- **Privacy proxy**: an OpenAI-compatible endpoint on `127.0.0.1` (Settings →
  Privacy proxy, default port 4242). Any tool with a `base_url` setting —
  Cursor, Continue, SDK scripts — gets the full perimeter: detection +
  aliasing + critical blocks on the way out, de-aliased responses on the way
  back, audit-chain entries (source `proxy`) throughout. Loopback-only by
  construction, zero new dependencies (hand-rolled HTTP/1.1 on tokio),
  `ollama:*` models stay zero-egress. The desktop app stops being the only
  place the perimeter exists.
- **VIN + Medicare MBI detection**: vehicle identification numbers are caught
  unanchored via the ISO 3779 check digit (identity pack); Medicare Beneficiary
  Identifiers via a `medicare:`/`mbi:` anchor plus the CMS per-position
  character classes (medical pack). 41 patterns total.
- **Generic credential detection (blocking)**: `password=…`, `"api_key": "…"`,
  `secret: …`-style assignments are caught by a context anchor plus a
  Shannon-entropy validator with a placeholder stoplist — `password=changeme`
  and `${ENV_VAR}` templating pass through, real high-entropy values block
  egress. 39th pattern, secrets pack (safety floor, not toggleable).
- **Windows source builds are CI-verified**: the ONNX Runtime win-x64 DLL
  (1.22.0, SHA-recorded) is now vendored and the previously-disabled
  `cargo-check-windows` job runs on every PR. The default Windows build is
  CPU local inference (no extra SDKs); GPU is the opt-in `windows-vulkan`
  feature (Vulkan SDK required), mirroring llama.cpp upstream's explicit
  backend selection.
- **OpenRouter provider**: one `sk-or-` key unlocks the open-model catalog —
  Llama 4 Maverick, DeepSeek V4 Pro, Mistral Large, Qwen3.7 Plus, and
  Command A ship in the picker (ids verified against the live catalog),
  routed via the `openrouter:` prefix through the same Vendetta pipeline.
  This honestly restores the Meta/Mistral/DeepSeek/Cohere tiles that v0.4.0
  removed as unwired demo scaffolding.
- **Guided-tour E2E + recorded demo**: `pnpm e2e` walks all 11 tour steps
  headlessly against the browser preview (Playwright) asserting each advances
  on its real event — including the SSN block — with zero page errors; runs
  in CI (`ci-e2e.yml`). `pnpm record:demo` regenerates the README demo GIF
  from the same flow. Fixed en route: the finale now auto-closes the Dev
  Inspector so the composer is clickable, and the ThreatRadar no longer
  throws in browser preview.
- **Per-detection confidence scores**: every span carries a [0,1] confidence —
  checksum-validated kinds 1.0, distinctive structural formats 0.95, anchored
  heuristics 0.85, unanchored heuristics 0.75; NER spans carry GLiNER's own
  model score. Shown in the Dev Inspector and color-graded next to every
  alias in the Vendetta panel.
- **Detection pack toggles** (Settings → Detection packs): switch off
  payment, identity, national-ID, medical, legal, or network/crypto
  detection if you never handle them. Core PII and secrets are the safety
  floor and cannot be disabled. A disabled pack neither aliases nor blocks.
- **International national-ID pack + infrastructure classes** (`vendetta.rs`).
  US ITIN, Canadian SIN (Luhn), UK NHS number (mod-11), UK National
  Insurance number (structure rules), Australian TFN (weighted mod-11),
  and Aadhaar (Verhoeff) — all context-anchored and checksum-validated,
  modeled on the Presidio/DLP entity catalogs. Plus MAC addresses and
  **credentialed database connection strings**
  (`postgres://user:pass@host`, mongodb/redis/amqp…), which carry live
  passwords and therefore **block egress** like API keys. Engine now
  spans **36 patterns across 7 packs**.
- **18 new detection classes across 6 industry packs** (`vendetta.rs`).
  Payment/banking: credit cards (Luhn + per-brand length validation,
  **blocks egress**), IBAN (mod-97, **blocks**), US routing/account numbers
  (ABA checksum), SWIFT/BIC, EIN. Secrets: expanded API-key formats
  (GitHub fine-grained, GitLab, Stripe live, Google, OAuth), JWTs,
  `-----BEGIN PRIVATE KEY-----` blocks (**blocks**). Identity: DOB,
  passport, driver's license (context-anchored + plausibility checks).
  Medical: MRN, NPI (checksum), DEA (checksum), insurance member IDs.
  Legal: case/docket numbers. Crypto/network: BTC/ETH wallets
  (Base58Check-validated), IPv6; IPv4 octets now range-validated.
- **Validator layer**: every checksum-able class is verified post-regex
  (Luhn, mod-97, ABA, NPI, DEA, date plausibility, Base58Check, IPv6
  parse) so "looks like a card number" never fires on a tracking number.
  Validators run at hit-collection time and are fully mirrored in the
  client-side preview so highlights always match the engine.
- **Custom watchlist** (Settings → Custom watchlist): your own sensitive
  terms — codenames, client names, hostnames — matched case-insensitively
  as whole words and aliased as `⟦custom_NN⟧`. Settings-backed, regex-
  escaped, capped at 200 terms, never blocks.
- **Per-kind block policies**: SSN, API keys, credit cards, IBAN, and
  private keys each get specific, accurate violation copy
  (`vendetta::block_policy`), replacing the old hardcoded SSN-only text.
- **Walkable guided tour**: an 11-step spotlight tutorial over the real
  app that advances on real events — live detection, the alias panel,
  a real transmit with the X-ray pass, the "Model saw" payload view, the
  Dev Inspector, and a deliberate SSN block. Auto-offered once after the
  first-run wizard; re-runnable from ⌘K. Replaces the pre-filled demo
  prompt.
- **Real metrics everywhere**: the sidebar, threat radar, empty-state
  stats, and the (renamed) "Privacy posture" dashboard now read live
  windowed counts (24h/7d) from the local hash-chained audit log and
  session time-to-first-token — every fabricated number is gone,
  including the placeholder SOC 2 / HIPAA "COMPLIANT" tiles, which a
  public app must never claim.
- **Eval corpus v2**: 165 fixtures (+59) with explicit negative cases per
  class (Luhn-fail numbers, tracking numbers, MAC addresses, timestamps,
  git SHAs); critical zero-miss gate extended to all five blocking
  classes; `EVAL_DEBUG=1` prints per-fixture FP/FN traces. Measured:
  precision 0.898, recall 0.851, p99 17 ms, 0 critical misses.

- **Ollama provider** (`apps/desktop/src-tauri/src/providers/ollama.rs`). Run any
  locally-hosted model via Ollama's `/api/chat` (NDJSON streaming), no API key.
  Installed models are discovered at runtime (`ollama_list_models` →
  `GET /api/tags`) and merged into the picker under an "Ollama" group. A new
  `ollama_health` command powers the Settings → Local models panel with a
  connection check + installed-model list.
- **Egress-aware redaction for Ollama.** A **loopback** base URL
  (localhost / 127.0.0.1 / ::1) is treated as zero-egress — prompts run on-device
  and are sent raw, like Sentynyx Local. A **remote** base URL is detected
  (`ollama_host_is_local`, fail-closed) and aliased + paranoid-scanned like any
  cloud provider. Covered by unit tests.
- **Open-core feature gate.** All team-tier cloud code (audit sync `cloud.rs`,
  Sentry telemetry `telemetry.rs`, and the `team_*` IPC surface) now lives behind
  a `team-cloud` Cargo feature that is **off by default**. The public build
  compiles with no Sentry and no Ed25519 signing crates at all
  (`cargo tree --no-default-features` confirms); those deps are `optional` and
  pulled in only by `team-cloud`.
- **`build_info` capability command.** The renderer asks the binary which
  optional surfaces exist and hides the Settings → Team and → Telemetry sections
  in the public build, so the UI can never drift from what's actually compiled in.
- **Open-source project files**: `LICENSE` (AGPL-3.0), rewritten `README.md`,
  `TUTORIAL.md`, `OPEN-CORE.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`,
  `CLA.md`, `NOTICE` (third-party attributions), GitHub issue/PR templates, and a
  rewritten `SECURITY.md`.
- **Install + distribution**: `scripts/install.sh` (one-line macOS installer from
  GitHub Releases), a Homebrew cask template (`packaging/homebrew/sentynyx.rb`),
  a GitHub Pages workflow that publishes the interactive web demo, and
  `scripts/cut-public-repo.sh` (produces a clean-history public repo from the
  monorepo with automated leak verification).
- **CI**: `ci-feature-matrix.yml` builds + tests both the public (default) and
  commercial (`team-cloud`) configurations so the gate can't rot.

### Changed

- **Honest model picker.** Removed the four un-wired provider tiles
  (Meta / Mistral / DeepSeek / Cohere) that produced "no provider" errors. The
  picker now shows only models that actually work: 9 cloud models across 4 BYOK
  providers, the bundled on-device model, plus any discovered Ollama models.
- **App metadata** reframed from "AI OS for Business" to the accurate
  "local-first privacy perimeter for LLMs". Version → `0.4.0`.
- **Auto-update** ships off for the open-source release
  (`createUpdaterArtifacts: false`; updater endpoint points at GitHub Releases),
  so no signing key is required to build a public release.

### Notes

- Detection model weights still download on first launch from Hugging Face
  (SHA-verified); no large blobs are committed.
- The commercial team-tier backend, admin dashboard, and the model-improvement
  roadmap are developed in a separate private repository and are intentionally
  excluded from the public repo (see `OPEN-CORE.md`).

## v0.3.2 — 2026-04-24

The "raise-ready" cycle. Turns the v0.3 technical bedrock into an investor-
facing company: Team-tier audit sync shipping client + server, an admin
dashboard suitable for slide 6 of the deck, a full pitch stack (HTML deck,
financial model, competitor matrix, exec summary), first-batch outreach
templates, security disclosure policy, and a data-room structure
ready to populate the moment the SOC2 Type I report lands.

### Added

- **Client-side audit sync** (`apps/desktop/src-tauri/src/cloud.rs` — new).
  Periodic 5-minute flush of locally-persisted audit events to
  `api.sentynyx.com/audit`. Envelope is raw JSON bytes (no canonical
  serialization) with a random 16-byte nonce; signature is Ed25519 over
  those exact bytes; upload is a two-field `{envelope, signature}` payload
  the Worker verifies against a per-team pubkey. Nonces are replay-
  checked against `seen_nonces(team_id, nonce)` with a freshness window.
  Events are idempotent by `event_id`, and the client flips `uploaded_at`
  only on `{ok:true}` so retries don't double-count.
- **Team-tier Settings panel** (`apps/desktop/src/scenes/SettingsPanel.tsx`).
  Three-step onboarding: (1) **Generate key** — Ed25519 keypair minted
  locally, private half → keychain, public base64 copy-button → admin
  pastes into the Worker's `/admin/teams`. (2) **Team config** — `team_id`
  + `member_email` saved via `team_configure` IPC. (3) **Enable + sync** —
  gated off until 1+2 complete. "Sync now" force-flushes and shows the
  outcome inline.
- **Admin dashboard** (`apps/admin/`). Zero-dependency HTML/CSS/JS SPA.
  Bearer-token auth against the Worker's admin endpoints, renders a stats
  row (total events, unique members, last 24 h), by-kind horizontal bars,
  and a top-members leaderboard. Auto-refreshes every 60 s while the tab
  is foregrounded. This is the screenshot that anchors deck slide 6.
- **Team API plumbing** (`apps/api/src/index.ts`). Two-field envelope
  wire format (verifies against raw signed bytes — no canonicalization
  mismatch, closing bug_003). Replay protection via `seen_nonces` UNIQUE
  index. SHA-256 XOR constant-time admin-token compare (bug_010). Empty-
  batch short-circuit before SQL (bug_012). `IS NOT NULL` filter on
  `member_email` in the rollup (bug_017).
- **Wire-contract round-trip harness** (`apps/api/tests/wire/`). Rust
  binary signs a canonical envelope with a deterministic `[0x01; 32]`
  seed; Node verifier imports the Worker's `verifyEd25519Bytes` helper
  and asserts the signature validates. Round-trip runs in
  `ci-wire-contracts.yml` — any accidental re-introduction of canonical-
  JSON assumptions fails the PR.
- **Demo seeder** (`apps/api/scripts/seed_demo_data.mjs`). Node 18+
  script using WebCrypto Ed25519 to generate a team, mint a signing
  keypair, and POST ~200 audit events across three fake members and
  fourteen kinds over seven days. Makes the admin dashboard screenshot-
  ready without needing a running desktop client.
- **Pitch kit** (`docs/pitch/`). Twelve-slide HTML deck (`deck.html`) with
  scroll-snap nav, 24-month financial model, competitor matrix vs
  Presidio / AWS Comprehend PII / Google DLP / Skyflow / Private AI,
  data-room outline (9 folders), and this release's **executive
  summary** (`executive-summary.md`, 2-page PDF-ready).
- **Outreach templates** (`docs/outreach/`). Investor emails for seven
  stages (cold-intro, warm-intro-via-angel, partner-meeting-ask,
  post-meeting follow-up, term-sheet-received, term-sheet-declined,
  announcement). Cold-outreach batch 1 (three email templates × five
  prospects) with `{{vars}}` for personalization. Five-email waitlist
  drip. Press kit for media.
- **Public FAQ** (`apps/site/faq.html`). 42 questions across 8 categories
  (product, privacy, security, pricing, operations, compliance,
  business, technical). Linked from nav and footer across all site
  pages. Cross-referenced from `docs/outreach/waitlist-drip.md`.
- **Second blog** (`docs/blog/v0.3.1-from-regex-to-ner.md`). Deep-dive
  on the merge rules — how regex + NER spans get combined without
  double-counting, when each detector wins, why we chose GLiNER over
  DeBERTa-v3-small. Publishable standalone after the v0.3 benchmark
  post lands.
- **SOC2 Type I control narrative** (`docs/legal/soc2-control-narrative.md`).
  Drata-friendly CC1.1–CC9.2 control descriptions with evidence
  pointers. Ready for the week-7 auditor kickoff.
- **Incident-response playbook** (`docs/security/incident-response.md`).
  Four severity classes, escalation tree, external-comms template.
- **Admin-token rotation runbook** (`docs/eng/admin-token-rotation.md`).
  Quarterly rotation SOP for `ADMIN_AUTH_TOKEN` with zero-downtime
  dual-valid window.
- **Demo-recording day checklist** (`docs/launch/demo-recording-day.md`).
  Capture-day SOP with audio, machine-state, screen-resolution, and
  on-camera rehearsal steps.
- **HN submission playbook** (`docs/launch/hn-submission.md`). Submission
  timing, title variants, comment-response strategy. Paired with the
  benchmark blog.
- **Security disclosure policy** (`SECURITY.md`). Vulnerability
  disclosure process with 72-h acknowledge / 30-d fix SLA, PGP
  key reference, and safe-harbor language. Supports SOC2 CC1.1
  evidence alongside the per-release changelog format.

### Changed

- **Eval corpus** (`apps/desktop/src-tauri/eval/prompts.json`). Expanded
  from 80 → 106 prompts with +26 semantic cases across `CODENAME_NER`,
  `EMPID_NER`, `LOCATION_NER`, `ORG_NER`, `PERSON_NER`. Current head-to-
  head: regex F1 0.637 → regex + NER F1 0.853 (+0.22 headline).

### Fixed

- **Paranoid mutex starvation** (`apps/desktop/src-tauri/src/detect/llm.rs`).
  Paranoid timeout was counting cold-load time against the 5 s budget,
  so the first post-idle send logged 17 s "elapsed" despite the timeout
  firing normally. Lowered `MAX_NEW_TOKENS` to 128 and added a
  background Qwen warmup alongside NER warmup so cold-load happens
  before the first user-facing send.

### Known limitations

- **NER sidecar not re-enabled in production.** ORT Metal init hangs
  when spawned inside the Tauri tokio runtime. Root cause is probably
  a libdispatch deadlock with ORT's CoreML provider chain; fallback is
  in-process `NerDetector` with `catch_unwind`. Sidecar binary stays
  committed as forward work for v0.4.0.
- **CF Worker not deployed.** Everything above works against `wrangler
  dev`. First real deploy lands in v0.3.3 (week 4) when the first
  pilot is ready to onboard.

## v0.3.1 — 2026-04-21

Institutional-practice hardening from the v0.3 ultrareview. Eleven
review findings; each is patched on its originating phase branch, and
this release turns the lessons into **four CI gates** plus **three docs**
that prevent the same class of bug from recurring.

### Added

- **CI: `ci-lints.yml`.** clippy (Rust), eslint-plugin-security (TS),
  tsc --noEmit, `cargo test --lib`. Blocks PRs on any failure.
- **CI: `ci-site-links.yml`.** `lychee` walks every internal link in
  `apps/site/**/*.html`. Zero-404 gate. Catches the next time somebody
  adds a nav item and forgets to add the page.
- **CI: `ci-bundle-dryrun.yml`.** `tauri build --skip-sign --dry-run`
  on macOS + Windows. Detects missing-asset bugs (like bug_001's
  absent `icon.ico`) before they reach a release branch.
- **CI: `ci-wire-contracts.yml`.** Scaffolds the Rust-signs → TS-verifies
  and TS-signs → Rust-verifies round-trip harness. Full implementation
  ships with the audit-sync client in v0.3.2.
- **`docs/eng/new-endpoint-checklist.md`.** Day-one security checklist
  for any new auth/authz surface: auth, replay, rate-limit, idempotency,
  empty-case, error-shape. Referenced by every new endpoint PR.
- **`docs/ux/shortcuts.md`.** Keybinding registry — single source of
  truth for both the keybinding handler AND any hint strings. Grep-
  enforced by CI (planned).
- **`docs/eng/keybinding-adr-template.md`.** Any new single-modifier
  shortcut must pass the compose-surface collision check (Slack,
  Notion, GDocs, browser) before landing. Template captures the
  decision.

### Fixed (review findings, cross-branch)

- **bug_001 — Missing `icon.ico`** (phase4). Ran `pnpm tauri icon` on
  the source PNG. ICO + ICNS + Windows Store tiles committed.
- **bug_002 — Wrong shortcut in hint** (phase1). Copy reworded.
- **bug_003 — Non-canonical JSON signing** (phase5). Replaced
  canonical-JSON assumption with a two-field `{envelope, signature}`
  wire format where `envelope` is raw bytes; server verifies signature
  against those exact bytes, never re-serializes.
- **bug_006 — Paranoid JSON truncation** (phase1). Brace-walking
  salvage for truncated arrays; tests for each malformed-output class.
- **bug_010 — Timing-unsafe admin compare** (phase5). SHA-256 XOR
  constant-time compare.
- **bug_012 — Empty events 500** (phase5). Short-circuit at handler top.
- **bug_013 — No Pricing nav link** (phase6). Link added to nav + footer.
- **bug_014 — ⌘I italic collision** (phase1). Swapped to ⌘⇧I.
- **bug_017 — NULL `member_email` rollup** (phase5). Reject-at-write
  + `IS NOT NULL` filter on the stats query.
- **bug_021 — No replay protection on /audit** (phase5). `seen_nonces`
  table + freshness window.
- **bug_023 — DEMO_DRAFT vs NAME regex** (phase1). Pre-filled draft
  now capitalizes "Sarah Chen" so the NAME regex fires on first launch.

## v0.3.0 — 2026-04-21

Shipped across six independently-reviewable phase branches. Two
headline shifts: the product gets **demo-grade polish** and the
repository grows a **public-facing surface** (landing page, pricing,
benchmark blog, Team-tier API scaffold, Windows bundle scaffold).

### Added

- **Phase 1 — demo polish.** OnboardingCard in the empty state with
  live model / credentials / local-sentynyx rows. About dialog (⌘⇧I)
  with RSS / uptime / version / model state / last-trace timings.
  First-send pre-fill that triggers all three detectors. Settings
  parity — accent colors, density, starfield, scan animation, default
  model persisted. Misc copy + hotkey fixes from an end-to-end demo
  run-through.
- **Phase 2 — reproducible benchmark.** `cargo run --bin eval --
  compare` runs the engine against a fixture corpus under three
  configs (regex-only, regex+NER, +paranoid) and emits precision /
  recall / F1 / p99 numbers. Presidio head-to-head script
  (`docs/blog/scripts/presidio_compare.py`) runs the same prompts
  through Microsoft Presidio for an apples-to-apples comparison.
  First blog draft at `docs/blog/v0.3-we-rebuilt-pii-redaction.md`
  with real numbers populated (F1 0.853 vs Presidio's 0.642 on the
  v0.3 corpus).
- **Phase 3 — landing site.** `apps/site/` static HTML + CSS + JS
  (`index.html`, `pricing.html`, `styles.css`, `pricing.css`,
  `main.js`, `compare.json`). Starfield canvas, hero + features
  + benchmark + waitlist sections, responsive down to 380 px wide.
  Numbers in the benchmark table hydrate from `compare.json`
  post-JS so crawlers see static fallback. Demo-video slot wired
  but points at a TK placeholder until the 60-second cut is
  recorded post-v0.3.1.
- **Phase 4 — Windows bundle scaffold.** `Cargo.toml` Vulkan target
  for `llama-cpp-2` on Windows. `build.rs` emits `ORT_DYLIB_PATH`
  for `libonnxruntime.dll`. `tauri.conf.json` Windows bundle with
  NSIS installer, icon set, and SmartScreen-compliant metadata.
  Does NOT sign — that blocks on the EV cert arriving.
- **Phase 5 — Team-tier API scaffold.** `apps/api/` Cloudflare Worker
  with D1 migrations (`0001_init.sql`, `0002_seen_nonces.sql`).
  Endpoints: `GET /health`, `POST /admin/teams`, `POST /audit`,
  `GET /admin/teams/:id/stats`. Envelope + signature wire format
  post-review fixes. Not yet deployed — `wrangler dev` only.
- **Phase 6 — pricing + pitch.** `apps/site/pricing.html` with
  three tiers (Personal free, Team $15/seat/mo, Enterprise custom)
  and an FAQ. `docs/pitch/deck-outline.md` — twelve-slide deck
  outline. `docs/outreach/loi-template.md` — cold design-partner
  email for the first pilot outreach batch.

## v0.2.0 — unreleased (feat/semantic-redaction)

The "consumer-RAM deliverable MVP" cycle. Lands the semantic detection layer,
brings the app into a shippable state (signing pipeline, vendored ORT, first-
run onboarding, auto-update, telemetry), and adds user data controls (export
+ delete-all).

### Added

- **Semantic NER detection.** Every prompt now runs through GLiNER-small-v2.1
  (ONNX, ~600 MB) alongside the existing regex engine. Regex wins on overlap;
  NER adds spans for arbitrary names, orgs, codenames, locations, and employee
  IDs that regex can't template. Runs in 50–150 ms on CPU.
- **Paranoid LLM pass.** Opt-in background scan via Qwen 2.5 0.5B-Instruct
  Q4_K_M (~470 MB, Metal-accelerated on Apple Silicon). Catches semantic
  sensitivity ("layoffs", "legal hold") that has no token signature.
  Structured-output prompt + tolerant JSON parser; panic-safe.
- **Alias format switched to `⟦...⟧`** (U+27E6 / U+27E7, mathematical double
  brackets). The old `{{...}}` Handlebars-style syntax caused LLM responses
  to describe aliased tokens as "template variables," producing incoherent
  re-hydrated output. The new format is opaque to LLM priors.
- **Vendored ONNX Runtime 1.22.0** universal dylib (arm64 + x86_64 via lipo).
  Removes the Homebrew `libonnxruntime.dylib` dependency for running the app
  — a required step for distribution.
- **First-run onboarding wizard**: three-step welcome → API key (with live
  `validate_api_key` green-check feedback) → model download. Persisted via
  the new `first_run_seen` settings key so it fires once per install.
- **API key validation**: lightweight `GET /models` probe per provider before
  save. Shows `✓ saved to OS keychain` / `✗ 401 invalid API key` inline
  rather than failing on first Transmit.
- **Settings persistence**: generic `get_setting` / `set_setting` IPC backed
  by the SQLite `settings` table. Tweaks (accent color, density, starfield,
  scan animation, default model) and alias display mode now survive restart.
- **PolicyViolation "Remove & Retry"**: strips every occurrence of the
  blocked critical span client-side so the user can re-send without
  retyping. Violation screen no longer auto-dismisses.
- **Smart send error classification**: "no API key" opens Settings
  automatically; 429 says retry in ~30s; model-not-routed points to ⌘O.
- **Auto-update plumbing**: `tauri-plugin-updater` + `UpdateToast` scene.
  Needs the Ed25519 pubkey populated in `tauri.conf.json` (currently a
  placeholder — generate with `tauri signer generate`) and an endpoint at
  `releases.sentynyx.com` to activate.
- **Opt-in telemetry**: `sentry-rust` integration, off by default. Hard
  guardrail drops any event whose JSON contains the alias brackets `⟦` or
  `⟧` so redacted content can never leak through crash reports. Enabled via
  `SENTYNYX_SENTRY_DSN` env var + the in-app toggle.
- **Data export + delete-all**: Settings → Data tab. Export copies
  `sentynyx.db` and `secrets.json` into `~/Downloads/sentynyx-export-<ts>/`.
  Delete-all nukes the app data directory and keychain entries (requires
  confirmation). GDPR-aligned.
- **Configurable timeouts**: NER (default 500 ms) and paranoid LLM
  (default 5000 ms) read from `ner_timeout_ms` / `paranoid_timeout_ms`
  settings keys.
- **macOS signing + notarization pipeline**: `.github/workflows/release.yml`
  runs `xcrun notarytool submit --wait` and `stapler staple` on a universal
  DMG build. Requires the six Apple secrets documented in
  `apps/desktop/src-tauri/RELEASE.md`.
- **Split eval gates**: `fast` mode (regex + NER) hard-fails PRs on
  regression; `full` mode (adds paranoid) is report-only until the paranoid
  prompt is tuned. CI runs both; artifacts uploaded.
- **NER sidecar infrastructure (disabled)**: `sentynyx-ner` binary, line-
  framed JSON IPC, respawn-on-crash. Would isolate `spm_precompiled`
  panics to a child process. Currently inert because ORT's Metal init
  hangs when the child is spawned from a parent with a live tokio runtime;
  root cause isolated but not yet fixed. Swap `NerDetector` →
  `NerSidecarDetector` in `lib.rs:AppState` when the hang is resolved.

### Changed

- **Keychain behavior splits on build profile.** Release builds write the
  OS keychain first and fall back to a 0600-permissioned JSON file on
  failure. Debug builds write the file primary because unsigned-binary
  keychain writes silently succeed without persisting on macOS.
- **`is_critical` extended to block APIKEY** (was SSN-only). The frontend
  `CRITICAL` map was out of sync; now matches. Affected live prompts with
  the `DEMO_DRAFT` API key — violation flash fires consistently now.
- **`append_audit_for_spans` takes a `source` parameter** (`"regex" | "ner"
  | "llm"`). Schema: `audit.source` column added via idempotent ALTER.
- **`Kind` enum extended** with `PERSON_NER`, `ORG_NER`, `CODENAME_NER`,
  `LOCATION_NER`, `EMPID_NER`. The `_NER` suffix keeps alias namespaces
  distinct so regex-caught `Sarah Chen` and NER-caught `Sarah Chen` don't
  collide in the conversation alias map.
- **Paranoid LLM shared via `AppState`** (previously constructed per
  `send()`, which duplicated the 500 MB GGUF load on every request).

### Fixed

- Tauri `.setup()` runtime panic: swapped `tokio::spawn` → `tauri::
  async_runtime::spawn` so background tasks spawn after Tauri's tokio
  runtime boots.
- `rehydrate_stream_with` stream buffering works for multi-codepoint
  alias markers (`⟦` and `⟧` are each three UTF-8 bytes).
- Process teardown SIGABRT: leak the `LlamaBackend` instance and call
  `libc::_exit(0)` in the `eval` binary so the C++ static destructor
  chain doesn't race ORT's Metal cleanup on exit.
- llama.cpp sampler index: use `-1` (batch-relative "last logits") instead
  of the absolute position, which was `>n_batch-1` after the first
  iteration and would GGML_ASSERT-fail mid-inference.

### Infrastructure

- New Rust crate deps: `ort`, `tokenizers`, `ndarray`, `llama-cpp-2` (only
  on macOS via `[target]` gate), `tauri-plugin-updater`, `tauri-plugin-
  process`, `sentry`, `libc`.
- New npm deps: `@tauri-apps/plugin-updater`, `@tauri-apps/plugin-process`.
- `apps/desktop/vendor/onnxruntime/mac-universal/libonnxruntime.1.22.0.dylib`
  shipped as a bundle resource.
- `apps/desktop/scripts/stage-sidecar.sh` — convenience wrapper that
  builds `sentynyx-ner` and copies it into `src-tauri/binaries/` with the
  target-triple suffix Tauri's `externalBin` system expects.

### Known limitations

- NER sidecar is not active (ORT Metal init hangs when spawned from a
  Tauri-owned tokio runtime). Tracked as a blocker for activating the
  `spm_precompiled` panic isolation.
- Windows + Linux CI matrix is disabled; the Cargo.toml gates the Metal
  feature on `target_os = "macos"` so builds can at least link on other
  platforms, but the vendored `libonnxruntime.dll` / `.so` aren't
  committed yet.
- Auto-updater ships in a no-op state (pubkey placeholder). Must run
  `tauri signer generate` + wire up a release host before the first
  public release.
- Licensing layer deliberately omitted pending a paid-tier business
  decision.

## v0.1.0 — 2026-04-19

Initial Tauri + Rust + React app with regex-based Vendetta perimeter.
Streaming OpenAI / Anthropic / Google / xAI provider dispatch, SQLite-
persisted conversation + audit chain, first-run boot sequence animation,
orbital model picker, consensus arena, compliance cockpit.
