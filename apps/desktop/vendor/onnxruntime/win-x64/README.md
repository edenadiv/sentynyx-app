# ONNX Runtime — Windows x86_64 vendored binary

The Rust `ort` crate (v2.0.0-rc.9, `load-dynamic`) opens a native ONNX
Runtime shared library at runtime via `dlopen` / `LoadLibrary`. We ship
that DLL in the installer so Sentynyx doesn't depend on the user's
`PATH` or a separately installed runtime.

**ABI-compatible version**: `1.22.0` (matches `ort-sys 2.0.0-rc.9`).

## How to populate this directory

1. Download the Windows x64 release from GitHub:
   https://github.com/microsoft/onnxruntime/releases/tag/v1.22.0
   Specifically: `onnxruntime-win-x64-1.22.0.zip` (~12 MB).

2. Unzip. The DLL lives at:
   `onnxruntime-win-x64-1.22.0/lib/onnxruntime.dll`

3. Copy the DLL into this directory:

   ```powershell
   Copy-Item onnxruntime-win-x64-1.22.0\lib\onnxruntime.dll `
     apps\desktop\vendor\onnxruntime\win-x64\onnxruntime.dll
   ```

4. Optionally commit it to the repo so Windows CI doesn't re-download
   every build. The DLL is ~13 MB — worth including in-tree for
   reproducibility. Git LFS is overkill at this size.

## Verification

The committed DLL is the unmodified upstream release artifact:

```
sha256(onnxruntime.dll) = 579b636403983254346a5c1d80bd28f1519cd1e284cd204f8d4ff41f8d711559
```

(from `onnxruntime-win-x64-1.22.0.zip` → `lib/onnxruntime.dll` at
https://github.com/microsoft/onnxruntime/releases/tag/v1.22.0 — verify a
fresh download against the same hash before replacing it.)

After copying, a fresh `cargo build` on Windows should no longer print
the `cargo:warning=Vendored ORT DLL not found ...` message. If it still
does, check the exact filename — it must be `onnxruntime.dll`, not
`onnxruntime-1.22.0.dll` or `libonnxruntime.dll`.

## Why not use NuGet?

NuGet packages the same binary behind a `Microsoft.ML.OnnxRuntime`
package that's straightforward to install but introduces a build-time
dependency on Visual Studio's NuGet restore. Shipping the DLL directly
is simpler for CI and for anyone cloning the repo to test — no extra
tooling beyond `git clone` and the standard Rust toolchain.

## Bundle behavior (known gap — needs first-Windows-build follow-up)

At `pnpm tauri build` time with `SENTYNYX_ORT_BUNDLED=1`, `build.rs`
embeds `ORT_DYLIB_PATH=onnxruntime.dll` — so at runtime, `ort` looks for
the DLL in the executable's directory. For this to actually resolve,
the DLL needs to land next to `Sentynyx.exe` in the install dir.

**Tauri 2 doesn't have a platform-scoped `resources` field for Windows
the way `bundle.macOS.frameworks` works on macOS.** Two options for
whoever does the first Windows build:

1. **WiX fragment** (MSI only). Create
   `apps/desktop/src-tauri/wix/onnxruntime.wxs` declaring a Component
   that pulls the DLL from `$(var.ProjectDir)..\..\vendor\onnxruntime\win-x64\onnxruntime.dll`
   into `[INSTALLFOLDER]`. Reference it in `tauri.conf.json`:
   ```json
   "wix": { "fragmentPaths": ["wix/onnxruntime.wxs"] }
   ```
2. **Pre-build copy** (works for both MSI + NSIS). Add a step to
   `release.yml` that copies the DLL into
   `apps/desktop/src-tauri/target/release/` before `tauri build`. WiX's
   `File Element` auto-discovery pattern picks it up, and NSIS includes
   everything in the target dir by default.

Option 2 is faster to implement; Option 1 is cleaner. Pick one once the
EV cert is in hand and a real Windows build is attempted.
