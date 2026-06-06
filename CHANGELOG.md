# Changelog

## v0.4.0 — Open source (unreleased)

The public open-source launch. Sentynyx becomes an auditable, local-first
privacy perimeter you can read, build, and verify — released under **AGPL-3.0**
on an **open-core** model (see `OPEN-CORE.md`).

### Added

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
