<div align="center">

# Sentynyx

**A local-first privacy perimeter for using any LLM without leaking sensitive data.**

Every prompt is routed through the **Vendetta** engine — a real-time PII / sensitive-info detector that aliases emails, phones, SSNs, API keys, project codenames, employee IDs, money values, names, and more into opaque tokens **before** the payload ever leaves your machine. The model only ever sees `⟦email_01⟧`; you see the real values, re-hydrated locally in the response.

Bring your own API keys (OpenAI · Anthropic · Google · xAI), run models fully offline via **Ollama**, or use the bundled on-device model. Your raw text never touches our servers — there are no servers. Don't trust us: [read `vendetta.rs`](apps/desktop/src-tauri/src/vendetta.rs).

[Quick start](#quick-start) · [Tutorial](TUTORIAL.md) · [Live demo](#live-demo) · [How it works](#how-it-works) · [Open-core](OPEN-CORE.md) · [Contributing](CONTRIBUTING.md)

`AGPL-3.0` · Tauri 2 + Rust + React · macOS today, Windows/Linux from source

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE) ![Platform: macOS](https://img.shields.io/badge/macOS-supported-black) ![Windows | Linux: from source](https://img.shields.io/badge/Windows%20%7C%20Linux-from%20source-lightgrey)

</div>

---

## Why

Pasting company data into ChatGPT/Claude/Gemini leaks it to a third party. Generic redaction tools (Presidio, Purview, DLP) run in *their* cloud and can't be audited by you. Sentynyx runs the redaction **on your side of the wire**, in a Rust core below the UI, and ships the source so the privacy claim is verifiable rather than promised.

```
your keystrokes → IPC → Rust Vendetta (alias) → Rust router → provider
                                  ↑ runs locally; raw text never egresses
```

## Quick start

> Prebuilt macOS app, or build from source on any platform. **First launch downloads ~1.1 GB of detection models** (GLiNER + a small LLM) from Hugging Face — one time, SHA-verified.

### Install (macOS)

```bash
# One-line installer — downloads the latest signed release into /Applications
curl -fsSL https://raw.githubusercontent.com/edenadiv/sentynyx-app/main/scripts/install.sh | bash
```

or with Homebrew:

```bash
brew install --cask edenadiv/tap/sentynyx
```

or grab the `.dmg` from [Releases](https://github.com/edenadiv/sentynyx-app/releases).

### Build from source (macOS / Windows / Linux)

```bash
git clone https://github.com/edenadiv/sentynyx-app.git
cd sentynyx/apps/desktop
pnpm install
./scripts/stage-sidecar.sh    # builds the NER sidecar binary
pnpm tauri dev                # dev window with hot reload
pnpm tauri build              # → .dmg / .exe / .AppImage
```

Linux build deps: see [Build details](#build-details). Full walkthrough in **[TUTORIAL.md](TUTORIAL.md)**.

## Three ways to run a model

| Mode | What it is | Egress | Setup |
| --- | --- | --- | --- |
| **BYOK cloud** | Your own OpenAI / Anthropic / Google / xAI key. 9 models. | Aliased payload only | Settings (⌘,) → paste key → stored in OS keychain |
| **Ollama** 🆕 | Any model you've `ollama pull`-ed, running locally. | **Zero** (loopback) | Install [Ollama](https://ollama.com), `ollama pull llama3.2`, it auto-appears in the picker |
| **Sentynyx Local** | Bundled on-device model (Qwen 2.5). | **Zero** | Download once from Settings → Models |

With a **localhost Ollama** server (or Sentynyx Local), nothing leaves the machine, so prompts are sent raw — no aliasing needed. Point Ollama at a **remote** host and Sentynyx automatically treats it as egress: the prompt is aliased and scanned like any cloud provider. (The decision is made in Rust, fail-closed — see `ollama_host_is_local` in [`commands.rs`](apps/desktop/src-tauri/src/commands.rs).)

Keys live in your OS keychain (macOS Keychain / Windows Credential Manager / libsecret) and never reach the renderer.

## Live demo

No install required — the UI runs in your browser with simulated streaming:

```bash
open Sentynyx.html        # zero-build, self-contained interactive demo
```

or visit the hosted demo at **https://edenadiv.github.io/sentynyx-app/**. Type a prompt with an email or `123-45-6789` and watch the Vendetta panel light up.

## How it works

Three detection layers run on every send and merge into one alias map:

1. **Regex** ([`vendetta.rs`](apps/desktop/src-tauri/src/vendetta.rs)) — emails, phones, SSNs, IPs, API keys, URLs, addresses, money. Fast, deterministic.
2. **Semantic NER** ([`detect/ner.rs`](apps/desktop/src-tauri/src/detect/ner.rs)) — GLiNER-small (ONNX) for arbitrary names, orgs, codenames, locations, employee IDs that regex can't enumerate.
3. **Paranoid LLM** ([`detect/llm.rs`](apps/desktop/src-tauri/src/detect/llm.rs)) — a small local model catching semantic sensitivity ("layoffs", "legal hold") with no token signature.

Regex wins on overlap; non-overlapping NER spans are kept ([`merge_spans`](apps/desktop/src-tauri/src/detect/mod.rs)). Aliases are stable per conversation and use `⟦…⟧` math brackets so the model doesn't mistake them for template variables. Streaming responses are re-hydrated across token boundaries. SSNs and API keys trigger **BLOCK_EGRESS** — the request is never made.

Everything persists to local SQLite with a SHA-256 hash-chained audit log. Reproducible benchmark: regex+NER **F1 0.853 vs Presidio 0.642** on the 106-prompt corpus ([`eval/`](apps/desktop/src-tauri/eval)).

## What's real vs. roadmap

**Real:** Vendetta engine + re-hydration, streaming for 4 cloud providers (9 models), Ollama (any local model), bundled on-device model, SQLite persistence, hash-chained audit log, policy-violation block, consensus arena, compliance cockpit, local-only telemetry-free operation.

**Roadmap:** real agent tool-use, knowledge-atlas ingest, custom visual policy rules, voice with redacted transcription, Windows/Linux signed binaries, OpenRouter provider. See [Issues](https://github.com/edenadiv/sentynyx-app/issues).

## Repo layout

```
apps/desktop/            # the app
  src/                   # React renderer (TypeScript)
  src-tauri/             # Rust core — Vendetta, detectors, router, providers, store, audit, keychain
    src/providers/       # openai · anthropic · google · xai · ollama · local
    eval/                # reproducible detection benchmark + corpus
Sentynyx.html            # zero-build interactive web demo
scripts/                 # install.sh, public-repo helpers
```

## Build details

The NER sidecar (`apps/desktop/src-tauri/binaries/sentynyx-ner-*`) is built by `apps/desktop/scripts/stage-sidecar.sh` (it's git-ignored). Detection model weights download on first launch from Hugging Face and are SHA-verified — no large blobs in the repo.

Linux build deps:

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev
```

Tests:

```bash
cd apps/desktop/src-tauri && cargo test        # Rust unit tests
cd apps/desktop && pnpm build                  # type-check + bundle
```

## Privacy & security

- Raw prompt text, conversation contents, and PII **never leave the device** in the open-source build. There is no telemetry compiled in.
- API keys live in the OS keychain, never in the renderer, never logged.
- Found a vulnerability? See [SECURITY.md](SECURITY.md).

## License & contributing

Sentynyx is **[AGPL-3.0](LICENSE)** — free and open, with a copyleft that keeps network-hosted derivatives open too. Some advanced team/enterprise features are developed separately under a commercial license; see **[OPEN-CORE.md](OPEN-CORE.md)** for exactly what's open vs. commercial and why.

Contributions welcome — start with **[CONTRIBUTING.md](CONTRIBUTING.md)** and the [Code of Conduct](CODE_OF_CONDUCT.md). By contributing you agree to the [CLA](CLA.md).
