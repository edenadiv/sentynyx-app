# binaries/

Staging directory for Tauri's `externalBin` sidecar copies. Tauri looks for
target-triple-suffixed binaries here at bundle time, e.g.
`sentynyx-ner-aarch64-apple-darwin` or `sentynyx-ner-universal-apple-darwin`.

The sidecar itself is defined as a `[[bin]]` target in `Cargo.toml` and lives
at `target/<profile>/sentynyx-ner` after `cargo build --bin sentynyx-ner`.

## Pre-release step

Before `pnpm tauri build` (or the release.yml CI job) do:

```bash
cargo build --release --bin sentynyx-ner --target aarch64-apple-darwin
cargo build --release --bin sentynyx-ner --target x86_64-apple-darwin
lipo -create -output binaries/sentynyx-ner-universal-apple-darwin \
    target/aarch64-apple-darwin/release/sentynyx-ner \
    target/x86_64-apple-darwin/release/sentynyx-ner
```

Or, for a non-universal single-arch build, place the appropriate binary as
`binaries/sentynyx-ner-<target-triple>`. Tauri's bundler copies the matching
file into `Sentynyx.app/Contents/MacOS/sentynyx-ner` and signs it.

## Dev mode

In `pnpm tauri dev` / `cargo run`, the main binary finds the sidecar via
`resolve_sidecar_path()` (in `src/detect/ner_sidecar.rs`) which walks up from
the current executable's directory and tries `./sentynyx-ner` and
`../sentynyx-ner`. This works out-of-the-box once the sidecar is built by
`cargo build --bin sentynyx-ner`.
