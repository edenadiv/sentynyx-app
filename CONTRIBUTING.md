# Contributing to Sentynyx

Thanks for helping build an auditable privacy layer for LLMs. This guide gets you from clone to merged PR.

## Ground rules

- Be excellent to each other — see [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
- By submitting a contribution you agree to the [Contributor License Agreement](CLA.md). A bot will ask you to sign on your first PR.
- The privacy invariant is sacred: **raw user text, PII, and secrets must never leave the device** in the open-source build. Any change that risks this needs a very good reason and explicit review.

## Development setup

```bash
git clone https://github.com/edenadiv/sentynyx-app.git
cd sentynyx-app/apps/desktop
pnpm install
./scripts/stage-sidecar.sh     # builds the git-ignored NER sidecar binary
pnpm tauri dev                 # run the app with hot reload
```

Prerequisites: [Rust](https://rustup.rs) (stable), [pnpm](https://pnpm.io), Node 18+, and on Linux the GTK/WebKit deps in the [README](README.md#build-details). Detection model weights download on first launch — no setup needed.

## Project layout

```
apps/desktop/src/             React/TypeScript renderer
  app/App.tsx                 top-level state + send() flow
  chrome/, scenes/, chat/     UI
  lib/ipc.ts                  every Tauri command wrapper lives here
  lib/models.ts               built-in model registry
apps/desktop/src-tauri/src/   Rust core
  vendetta.rs                 regex detection, aliasing, re-hydration
  detect/                     ner.rs (GLiNER), llm.rs (paranoid), mod.rs (merge)
  providers/                  one file per provider; implement the Provider trait
  router.rs                   model id → provider
  commands.rs                 #[tauri::command] IPC surface
  keys.rs, store.rs, audit.rs keychain, SQLite, hash-chained log
  eval/                       reproducible detection benchmark
```

## Tests & checks (run before pushing)

```bash
cd apps/desktop/src-tauri
cargo test                       # Rust unit tests (incl. the host-classifier security tests)
cargo build                      # public build must compile clean
cargo build --features team-cloud  # commercial build must also compile (if you touched gated code)

cd apps/desktop
pnpm build                       # tsc --noEmit + vite build
pnpm e2e                         # headless walkthrough of the guided tour (Playwright)
```

CI runs lints (clippy, eslint-security, tsc), the detection eval gate, and a bundle dry-run. The eval gate **fails the PR on a redaction-quality regression** — if you change a detector, run `cargo run --bin eval -- compare` and include the numbers.

## How to… (common contributions)

### Add a new cloud provider
1. Create `apps/desktop/src-tauri/src/providers/<name>.rs` implementing the `Provider` trait (`stream(...)`). Model `openai.rs` for SSE or `ollama.rs` for NDJSON.
2. Register it in `providers/mod.rs` and add the model-id match arm in `router.rs`.
3. Add the key plumbing in `keys.rs` (`env_key_for`) + `commands.rs` (`validate_api_key`, `list_configured_providers`).
4. Add models to `apps/desktop/src/lib/models.ts` and a provider row in `SettingsPanel.tsx`.

### Add a detector or detection class
Work in `vendetta.rs` (regex/aliasing) or `detect/`. **Add eval fixtures** to `eval/prompts.json` and keep the F1 gate green.

### Touch team-cloud (commercial) code
Anything behind `#[cfg(feature = "team-cloud")]` is the commercial surface. Most contributions won't touch it. If you do, make sure **both** `cargo build` and `cargo build --features team-cloud` compile.

## Pull requests

- Branch from `main`; keep PRs focused.
- Use clear commit messages (we loosely follow Conventional Commits: `feat:`, `fix:`, `docs:`…).
- Describe what you changed and how you verified it. Include test output for behavior changes.
- New keyboard shortcuts must pass the compose-surface collision check (see `docs/eng/keybinding-adr-template.md`) and be added to `docs/ux/shortcuts.md` (CI enforces this).

## Security

Don't open public issues for vulnerabilities — follow [SECURITY.md](SECURITY.md).

## License

Contributions are accepted under [AGPL-3.0](LICENSE) per the [CLA](CLA.md), which also lets the maintainers include accepted contributions in the commercial build. See [OPEN-CORE.md](OPEN-CORE.md).
