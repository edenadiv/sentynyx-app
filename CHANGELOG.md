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
- **Structured-data scanning**: pasted CSV/TSV/semicolon/pipe tables get
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

## Earlier

Sentynyx was developed privately before v0.4.0; this public changelog begins at
the open-source launch.
