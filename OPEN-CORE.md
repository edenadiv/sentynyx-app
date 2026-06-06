# Open-core: what's open, what's commercial, and why

Sentynyx is **open-core**. The entire product you run on your own machine — the privacy engine, the app, all three model modes — is free and open source under [AGPL-3.0](LICENSE). A separate set of **team/enterprise** features, which only make sense as a hosted multi-user service, is developed under a commercial license.

We're explicit about the line so you always know what you're getting and contributors know where the boundary is.

## The boundary

| ✅ Open source (this repo, AGPL-3.0) | 💼 Commercial (separate, proprietary) |
| --- | --- |
| The whole desktop app | Hosted team backend (centralized audit storage) |
| **Vendetta** engine: regex + semantic NER + paranoid LLM | Org-wide admin dashboard & compliance reporting |
| BYOK for OpenAI / Anthropic / Google / xAI | SSO / SCIM / role-based vaults |
| **Ollama** + on-device local models | The per-org model fine-tune loop |
| Local SQLite + hash-chained audit log | Centralized policy distribution |
| Reproducible detection benchmark + corpus | SLAs, support, managed deployment |

If you run Sentynyx for yourself or your own use, **you never need the commercial tier**. The open app is complete and fully functional on its own.

## How it's wired in this codebase

There is one codebase. The commercial client code lives behind a Cargo feature flag, `team-cloud`, that is **off by default**:

```bash
cargo build                       # public, open-source build (this is what ships)
cargo build --features team-cloud # commercial build (Sentynyx-internal)
```

With the feature off:
- the cloud audit-sync client and Sentry telemetry are **not compiled in at all** (verify: `cargo tree --no-default-features` shows no `sentry` and no `ed25519-dalek`),
- the Team and Telemetry sections don't appear in Settings (the UI asks the binary what it supports via the `build_info` command — the binary is the source of truth),
- no data ever leaves your machine.

The public release binaries are built **without** `team-cloud`. There is no hidden phone-home.

## Why open source a commercial product?

Because Sentynyx's core claim is *"your sensitive data never leaves your machine."* A closed-source privacy tool asks you to take that on faith. An open one lets you read [`vendetta.rs`](apps/desktop/src-tauri/src/vendetta.rs) and the [Dev Inspector](TUTORIAL.md#4-verify-it-yourself-the-whole-point) and **verify it**. Open source isn't a giveaway here — it's the most credible form of the product itself.

The commercial tier funds the work and covers the things that only matter at company scale (multi-user governance, compliance, the learning loop) — none of which you'd run on your own laptop.

## Why AGPL-3.0?

AGPL is a strong, OSI-approved, FSF-endorsed open-source license. It guarantees the freedoms you'd expect (use, study, modify, share) **and** closes the "SaaS loophole": anyone who offers a modified Sentynyx as a network service must also publish their changes. That keeps the project and its ecosystem open. As the copyright holder, the maintainers retain the right to also offer the code under a commercial license — which is what makes the open-core model work.

Questions about licensing for a specific use case? Open a discussion or email the address in [SECURITY.md](SECURITY.md).
