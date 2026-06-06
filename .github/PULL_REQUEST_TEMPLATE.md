<!-- Thanks for contributing! See CONTRIBUTING.md. -->

## What & why

<!-- What does this change, and why? Link any related issue (e.g. Closes #123). -->

## How I verified

<!-- Commands run + result. For behavior changes, include output. -->

- [ ] `cargo test` (in `apps/desktop/src-tauri`) passes
- [ ] `cargo build` (public, default features) compiles clean
- [ ] `pnpm build` (in `apps/desktop`) type-checks + builds
- [ ] If I changed gated code: `cargo build --features team-cloud` also compiles
- [ ] If I changed a detector: ran `cargo run --bin eval -- compare` and the F1 gate is green (numbers below)

## Privacy invariant

- [ ] This change does **not** cause raw user text, PII, or secrets to leave the device in the open-source build.

## Checklist

- [ ] I read [CONTRIBUTING.md](../blob/main/CONTRIBUTING.md) and agree to the [CLA](../blob/main/CLA.md)
- [ ] New keyboard shortcuts (if any) are added to `docs/ux/shortcuts.md`
- [ ] Docs updated if behavior/usage changed
