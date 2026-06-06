# vendor/

Third-party native libraries shipped with the Sentynyx app bundle.

## onnxruntime/

ONNX Runtime 1.22.0 — the dynamic library that `ort` (our Rust ONNX crate)
dlopens at runtime to execute the GLiNER NER model.

The version (1.22.0) is pinned by `ort-sys 2.0.0-rc.9` in `src-tauri/Cargo.toml`.
Upgrading `ort`/`ort-sys` may require upgrading this dylib in lockstep — check
`ort-sys/build.rs` for the expected `ONNXRUNTIME_VERSION` constant.

### mac-universal/libonnxruntime.1.22.0.dylib

Universal (arm64 + x86_64) Mach-O dylib, ~70 MB. Built by `lipo`-combining the
official `onnxruntime-osx-arm64-1.22.0.tgz` and `onnxruntime-osx-x86_64-1.22.0.tgz`
release tarballs from https://github.com/microsoft/onnxruntime/releases/tag/v1.22.0.

### mac-arm64/libonnxruntime.1.22.0.dylib

Native arm64 dylib kept alongside the universal one for any downstream tooling
that needs a thin binary (e.g. size-constrained sidecars). Redundant at bundle
time — Tauri only copies the universal file, per `tauri.conf.json` `bundle.macOS.frameworks`.

## How to refresh

```bash
# Pin the version you want and run from repo root.
ORT_VERSION="1.22.0"
curl -LO "https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-osx-arm64-${ORT_VERSION}.tgz"
curl -LO "https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-osx-x86_64-${ORT_VERSION}.tgz"
tar -xzf onnxruntime-osx-arm64-${ORT_VERSION}.tgz
tar -xzf onnxruntime-osx-x86_64-${ORT_VERSION}.tgz

mkdir -p apps/desktop/vendor/onnxruntime/mac-arm64
cp onnxruntime-osx-arm64-${ORT_VERSION}/lib/libonnxruntime.${ORT_VERSION}.dylib \
   apps/desktop/vendor/onnxruntime/mac-arm64/

mkdir -p apps/desktop/vendor/onnxruntime/mac-universal
lipo -create -output \
  apps/desktop/vendor/onnxruntime/mac-universal/libonnxruntime.${ORT_VERSION}.dylib \
  onnxruntime-osx-arm64-${ORT_VERSION}/lib/libonnxruntime.${ORT_VERSION}.dylib \
  onnxruntime-osx-x86_64-${ORT_VERSION}/lib/libonnxruntime.${ORT_VERSION}.dylib

rm -rf onnxruntime-osx-*-${ORT_VERSION}*
```

Then update the embedded version in `src-tauri/build.rs` (`ORT_DYLIB_PATH`) and
`src-tauri/tauri.conf.json` (`bundle.macOS.frameworks`).

## Licensing

ONNX Runtime is MIT-licensed. No attribution in the UI is required, but a note
in the app's About/Credits screen is considered polite.
