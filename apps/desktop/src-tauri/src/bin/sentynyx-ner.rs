//! sentynyx-ner — NER sidecar process
//!
//! Runs GLiNER ONNX inference inside an isolated child process so that a
//! panic in `spm_precompiled` (transitive tokenizer dep, occasionally
//! hits index-out-of-bounds on odd inputs) can only crash *this* process,
//! not the main Sentynyx app. The parent (`NerSidecarDetector`) respawns
//! us on EOF and falls back to regex-only for the in-flight request.
//!
//! Protocol: line-framed JSON over stdin/stdout.
//!   request:  {"id": u64, "text": String}
//!   response: {"id": u64, "spans": [Span, ...]}
//!         or: {"id": u64, "error": String}
//!
//! The sidecar is a single-purpose, single-threaded process. No tokio
//! runtime — we call into `run_inference` synchronously so inference can
//! complete without any .await yielding dance.

use std::io::{BufRead, Write};

use sentynyx_lib::detect::ner::run_inference_sync;

#[derive(serde::Deserialize)]
struct Request {
    id: u64,
    text: String,
}

#[derive(serde::Serialize)]
struct SuccessResponse {
    id: u64,
    spans: Vec<sentynyx_lib::vendetta::Span>,
}

#[derive(serde::Serialize)]
struct ErrorResponse<'a> {
    id: u64,
    error: &'a str,
}

fn main() {
    // `ort` reads ORT_DYLIB_PATH from the runtime environment. Our build.rs
    // sets it via `cargo:rustc-env=ORT_DYLIB_PATH=…` so `option_env!` picks
    // up the vendored absolute path in dev (or @executable_path/... in
    // release). If the parent already set it in the child env, that wins.
    if std::env::var_os("ORT_DYLIB_PATH").is_none() {
        if let Some(path) = option_env!("ORT_DYLIB_PATH") {
            std::env::set_var("ORT_DYLIB_PATH", path);
        }
    }

    // Tokio's `Command` piped stdio sets non-blocking on the child's fds.
    // std::io::stdin() (our reader) expects blocking reads; without this
    // fixup, ORT's background threads end up flipping stdin to non-blocking
    // via poll()-style shims and read_line hangs forever. Safe because we
    // own both fds exclusively in this process.
    #[cfg(unix)]
    unsafe {
        use std::os::unix::io::AsRawFd;
        for fd in [
            std::io::stdin().as_raw_fd(),
            std::io::stdout().as_raw_fd(),
            std::io::stderr().as_raw_fd(),
        ] {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            if flags >= 0 {
                libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK);
            }
        }
    }

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();

    let mut line = String::new();
    loop {
        line.clear();
        let bytes = match stdin.lock().read_line(&mut line) {
            Ok(0) => break, // EOF, parent closed stdin
            Ok(n) => n,
            Err(e) => {
                eprintln!("[sentynyx-ner] stdin read error: {e}");
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }

        let req: Request = match serde_json::from_str(line.trim()) {
            Ok(r) => r,
            Err(e) => {
                write_response(
                    &mut stdout,
                    &serde_json::to_string(&ErrorResponse {
                        id: 0,
                        error: &format!("bad request ({bytes} bytes): {e}"),
                    })
                    .unwrap_or_else(|_| String::from("{\"id\":0,\"error\":\"bad request\"}")),
                );
                continue;
            }
        };

        // Inference runs synchronously on the main (and only) thread. Panics
        // inside spm_precompiled / tokenizers are caught here so one bad input
        // can't take down the sidecar loop.
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_inference_sync(&req.text)));
        let response = match result {
            Ok(Ok(spans)) => serde_json::to_string(&SuccessResponse { id: req.id, spans })
                .unwrap_or_else(|_| format!("{{\"id\":{},\"spans\":[]}}", req.id)),
            Ok(Err(e)) => serde_json::to_string(&ErrorResponse {
                id: req.id,
                error: &e,
            })
            .unwrap_or_else(|_| format!("{{\"id\":{},\"error\":\"inference error\"}}", req.id)),
            Err(pe) => {
                let msg = pe
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| pe.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "tokenizer/inference panicked".to_string());
                serde_json::to_string(&ErrorResponse {
                    id: req.id,
                    error: &format!("panic: {msg}"),
                })
                .unwrap_or_else(|_| format!("{{\"id\":{},\"error\":\"panic\"}}", req.id))
            }
        };

        write_response(&mut stdout, &response);
    }

    // _exit to skip ORT destructors (same race as main — see commit 85deabe).
    unsafe { libc::_exit(0) }
}

fn write_response<W: Write>(w: &mut W, s: &str) {
    let _ = writeln!(w, "{}", s);
    let _ = w.flush();
}
