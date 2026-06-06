// Build script: point `ort` (load-dynamic) at a real ONNX Runtime dylib.
//
// The matching ABI for `ort-sys 2.0.0-rc.9` is ONNX Runtime 1.22.0. That
// universal-binary dylib is vendored at:
//   apps/desktop/vendor/onnxruntime/mac-universal/libonnxruntime.1.22.0.dylib
//
// At build time we set `ORT_DYLIB_PATH` to an absolute path pointing at the
// vendored file. `ort` embeds that string into the binary and dlopens it at
// runtime.
//
// At bundle/release time (Tauri `pnpm tauri build`) we want the dylib to be
// loaded from the app bundle — `.app/Contents/Frameworks/libonnxruntime.1.22.0.dylib`.
// Setting `SENTYNYX_ORT_BUNDLED=1` before invoking the build flips the embedded
// path to `@executable_path/../Frameworks/libonnxruntime.1.22.0.dylib`, which
// macOS resolves relative to the main executable inside the `.app`.
//
// Windows and Linux builds are out of scope for Phase 1 — their vendored
// shared libraries land in `vendor/onnxruntime/{win-x64,linux-x64}/` in a
// later phase.

use std::path::PathBuf;

fn main() {
    tauri_build::build();

    println!("cargo:rerun-if-env-changed=SENTYNYX_ORT_BUNDLED");
    println!("cargo:rerun-if-env-changed=ORT_DYLIB_PATH");

    #[cfg(target_os = "macos")]
    {
        if std::env::var_os("SENTYNYX_ORT_BUNDLED").is_some() {
            // Runtime-resolved against the .app bundle's executable directory.
            println!(
                "cargo:rustc-env=ORT_DYLIB_PATH=@executable_path/../Frameworks/libonnxruntime.1.22.0.dylib"
            );
            return;
        }

        // Dev / test / non-bundled. Use the vendored dylib so cargo test and
        // pnpm tauri dev both work without requiring Homebrew.
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let vendored = manifest_dir
            .join("..")
            .join("vendor")
            .join("onnxruntime")
            .join("mac-universal")
            .join("libonnxruntime.1.22.0.dylib");

        if vendored.exists() {
            println!(
                "cargo:rustc-env=ORT_DYLIB_PATH={}",
                vendored.display()
            );
        } else {
            // Vendored file missing — last-ditch fallback to Homebrew so devs
            // who haven't run the vendor step still get a working build. Emit
            // a warning so this doesn't silently stay broken in CI.
            println!("cargo:warning=Vendored ORT dylib not found at {}. Falling back to /opt/homebrew/lib — release builds will break.", vendored.display());
            println!("cargo:rustc-env=ORT_DYLIB_PATH=/opt/homebrew/lib/libonnxruntime.dylib");
        }
    }

    #[cfg(target_os = "linux")]
    {
        // libonnxruntime.so is expected to be either vendored in
        // vendor/onnxruntime/linux-x64/ or installed as libonnxruntime-dev.
        // Phase 3 work will wire the vendored path in explicitly.
        if std::env::var_os("ORT_DYLIB_PATH").is_none() {
            println!("cargo:rustc-env=ORT_DYLIB_PATH=libonnxruntime.so.1.22.0");
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Mirror the macOS flow: SENTYNYX_ORT_BUNDLED=1 at `tauri build` time
        // makes ort() dlopen `onnxruntime.dll` next to the executable inside
        // the .msi/.exe install. Dev / test falls back to the vendored DLL at
        // vendor/onnxruntime/win-x64/onnxruntime.dll so `cargo test` works
        // on a fresh clone without the user setting anything.
        if std::env::var_os("SENTYNYX_ORT_BUNDLED").is_some() {
            println!("cargo:rustc-env=ORT_DYLIB_PATH=onnxruntime.dll");
            return;
        }
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let vendored = manifest_dir
            .join("..")
            .join("vendor")
            .join("onnxruntime")
            .join("win-x64")
            .join("onnxruntime.dll");
        if vendored.exists() {
            println!("cargo:rustc-env=ORT_DYLIB_PATH={}", vendored.display());
        } else {
            // Graceful dev warning — cargo check still succeeds, but any
            // runtime ORT call will fail until the DLL is vendored. See
            // apps/desktop/vendor/onnxruntime/win-x64/README.md.
            println!("cargo:warning=Vendored ORT DLL not found at {}. Follow apps/desktop/vendor/onnxruntime/win-x64/README.md to populate.", vendored.display());
            println!("cargo:rustc-env=ORT_DYLIB_PATH=onnxruntime.dll");
        }
    }
}
