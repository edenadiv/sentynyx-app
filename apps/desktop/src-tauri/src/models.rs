use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub struct ModelSpec {
    pub id: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModelStatus {
    Missing,
    Downloading { percent: u32 },
    Ready,
    Error { msg: String },
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("io error: {0}")] Io(#[from] std::io::Error),
    #[error("http error: {0}")] Http(String),
    #[error("sha256 mismatch (expected {expected}, got {actual})")]
    Sha { expected: String, actual: String },
    #[error("size mismatch (expected {expected}, got {actual})")]
    Size { expected: u64, actual: u64 },
    #[error("cancelled")] Cancelled,
}

pub const GLINER_SMALL: ModelSpec = ModelSpec {
    id: "gliner-small-v2.1",
    file: "model.onnx",
    // Note: URL uses underscore (onnx-community/gliner_small-v2.1 not gliner-small-v2.1).
    // The `-v2.1` part uses a hyphen but the repo name uses an underscore before "small".
    url: "https://huggingface.co/onnx-community/gliner_small-v2.1/resolve/main/onnx/model.onnx",
    sha256: "874bc905cf3537a3bca91a55c3d99c78dbc571ebc2d8e209eb0aadd63e9d3948",
    size_bytes: 611_293_061,
};

pub const GLINER_TOKENIZER: ModelSpec = ModelSpec {
    id: "gliner-small-v2.1",
    file: "tokenizer.json",
    url: "https://huggingface.co/onnx-community/gliner_small-v2.1/resolve/main/tokenizer.json",
    sha256: "677203884d026e721115cf0daccf70ec4239545a13d6619e3e66d7151e0c9ce3",
    size_bytes: 8_657_198,
};

/// Default paranoid LLM. Qwen 2.5 0.5B Instruct Q4_K_M GGUF from the official
/// Qwen org. Chosen for consumer-RAM (~468 MB on disk, ~640 MB peak RSS via
/// llama.cpp Metal on Apple Silicon) AND for llama-cpp-2 compatibility —
/// SmolLM2's tensor layout triggered a SIGABRT in llama-cpp-2 0.1.144, while
/// the Qwen2 family is rock-solid on this runtime.
///
/// The id string remains generic ("paranoid-llm") so future model swaps
/// don't require a data-directory rename.
pub const PARANOID_LLM: ModelSpec = ModelSpec {
    id: "paranoid-llm",
    file: "qwen2.5-0.5b-instruct-q4_k_m.gguf",
    url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf",
    sha256: "74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db",
    size_bytes: 491_400_032,
};

/// Legacy alias. Kept so downstream code paths that still reference the old
/// name compile during the migration. Remove once all call sites use PARANOID_LLM.
pub const QWEN3_1_5B_Q4: ModelSpec = PARANOID_LLM;

pub fn models_root() -> PathBuf {
    let base = if let Some(d) = std::env::var_os("SENTYNYX_DATA_DIR") {
        PathBuf::from(d)
    } else {
        #[cfg(target_os = "macos")]
        { PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Library/Application Support/Sentynyx") }
        #[cfg(target_os = "windows")]
        { PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join("Sentynyx") }
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        { PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share/sentynyx") }
    };
    base.join("models")
}

pub fn local_path(spec: &ModelSpec) -> PathBuf {
    models_root().join(spec.id).join(spec.file)
}

pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), ModelError> {
    let mut f = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    let actual = hex::encode(hasher.finalize());
    if actual != expected {
        return Err(ModelError::Sha { expected: expected.into(), actual });
    }
    Ok(())
}

pub fn status(spec: &ModelSpec) -> ModelStatus {
    // Fast path — file presence + size check only. SHA verification happens at
    // load time (NerDetector::load, ParanoidDetector::load, ensure_local) where
    // it actually matters. Hashing a 600MB+ file on every UI status query would
    // block the webview for seconds per call.
    let p = local_path(spec);
    let meta = match std::fs::metadata(&p) {
        Ok(m) => m,
        Err(_) => return ModelStatus::Missing,
    };
    if !meta.is_file() { return ModelStatus::Missing; }
    let size = meta.len();
    // Reject obviously-wrong sizes (partial download left behind, etc.)
    // Allow ±1% slack in case of silent upstream nudges.
    let lo = spec.size_bytes.saturating_sub(spec.size_bytes / 100);
    let hi = spec.size_bytes.saturating_add(spec.size_bytes / 100);
    if size < lo || size > hi {
        return ModelStatus::Error {
            msg: format!("size mismatch (have {size}, expected {})", spec.size_bytes),
        };
    }
    ModelStatus::Ready
}

use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

pub async fn ensure_local<F>(spec: &ModelSpec, progress: F) -> Result<PathBuf, ModelError>
where F: Fn(u64, u64) + Send + 'static
{
    // Guard against downloading with a placeholder SHA. Returns an error rather
    // than panicking so a stray call path can't take down a tokio worker. The
    // UI surfaces this as "cannot download, model SHA not yet recorded".
    if spec.sha256 == "REPLACE_WITH_ACTUAL_SHA_AT_IMPLEMENTATION_TIME" {
        return Err(ModelError::Http(format!(
            "ModelSpec {} still has placeholder SHA — update the constant in models.rs before enabling this model",
            spec.id
        )));
    }

    let final_path = local_path(spec);
    if final_path.exists() {
        if verify_sha256(&final_path, spec.sha256).is_ok() {
            return Ok(final_path);
        }
        let _ = std::fs::remove_file(&final_path);
    }

    if let Some(parent) = final_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let partial = final_path.with_extension(
        format!("{}.partial", final_path.extension().and_then(|s| s.to_str()).unwrap_or(""))
    );

    let resume_from = if partial.exists() {
        std::fs::metadata(&partial).map(|m| m.len()).unwrap_or(0)
    } else { 0 };

    let client = reqwest::Client::builder()
        .build().map_err(|e| ModelError::Http(e.to_string()))?;
    let mut req = client.get(spec.url);
    if resume_from > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={}-", resume_from));
    }
    let resp = req.send().await.map_err(|e| ModelError::Http(e.to_string()))?;
    let status_code = resp.status();
    if !status_code.is_success() && status_code.as_u16() != 206 {
        return Err(ModelError::Http(format!("http {}", status_code)));
    }

    let total = spec.size_bytes;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true).append(true).open(&partial).await?;
    let mut downloaded = resume_from;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ModelError::Http(e.to_string()))?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        progress(downloaded, total);
    }
    file.flush().await?;
    file.sync_all().await?;
    drop(file);

    if let Err(e) = verify_sha256(&partial, spec.sha256) {
        let _ = std::fs::remove_file(&partial);
        return Err(e);
    }

    let final_size = std::fs::metadata(&partial)?.len();
    if final_size != total {
        let _ = std::fs::remove_file(&partial);
        return Err(ModelError::Size { expected: total, actual: final_size });
    }

    std::fs::rename(&partial, &final_path)?;
    Ok(final_path)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn verify_sha256_matches_correct_hash() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("x.bin");
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(b"hello").unwrap();
        let hash = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert!(verify_sha256(&p, hash).is_ok());
    }

    #[test]
    fn verify_sha256_rejects_mismatched_hash() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("x.bin");
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(b"hello").unwrap();
        let bad = "0000000000000000000000000000000000000000000000000000000000000000";
        assert!(matches!(verify_sha256(&p, bad), Err(ModelError::Sha { .. })));
    }

    /// Mutex serializing tests that mutate `SENTYNYX_DATA_DIR`. `cargo test` runs
    /// tests in parallel and `std::env::set_var` is process-global; without this
    /// guard, env-var-using tests race each other.
    pub(crate) static ENV_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn status_reports_missing_when_file_absent() {
        let _g = ENV_GUARD.lock().unwrap();
        let dir = tempdir().unwrap();
        std::env::set_var("SENTYNYX_DATA_DIR", dir.path());
        let s = status(&GLINER_SMALL);
        assert_eq!(s, ModelStatus::Missing);
        std::env::remove_var("SENTYNYX_DATA_DIR");
    }

    use tokio::io::AsyncWriteExt;

    /// Local HTTP server that streams a fixed body with Range support.
    /// Spawned per test, random port.
    async fn spawn_range_server(body: Vec<u8>) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
        use tokio::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else { break; };
                let body = body.clone();
                tokio::spawn(async move {
                    use tokio::io::AsyncReadExt;
                    let mut buf = vec![0u8; 2048];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let range = req.lines().find_map(|l| {
                        let ll = l.to_ascii_lowercase();
                        ll.strip_prefix("range: bytes=").map(|s| {
                            // Return the portion from the original line after "range: bytes="
                            &l[l.len() - s.len()..]
                        })
                    });
                    let (start, status) = if let Some(r) = range {
                        let start: usize = r.split('-').next().unwrap_or("0").parse().unwrap_or(0);
                        (start, "206 Partial Content")
                    } else { (0, "200 OK") };
                    let slice = &body[start..];
                    let response = format!(
                        "HTTP/1.1 {}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n\r\n",
                        status, slice.len()
                    );
                    sock.write_all(response.as_bytes()).await.ok();
                    sock.write_all(slice).await.ok();
                    sock.shutdown().await.ok();
                });
            }
        });
        (addr, handle)
    }

    #[tokio::test]
    async fn ensure_local_downloads_full_file_when_absent() {
        let _g = ENV_GUARD.lock().unwrap();
        let body = b"hello world".to_vec();
        let expected_sha = {
            let mut h = Sha256::new(); h.update(&body); hex::encode(h.finalize())
        };
        let (addr, _srv) = spawn_range_server(body.clone()).await;
        let dir = tempdir().unwrap();
        std::env::set_var("SENTYNYX_DATA_DIR", dir.path());

        let spec = ModelSpec {
            id: "testmodel", file: "x.bin",
            url: Box::leak(format!("http://{}/x.bin", addr).into_boxed_str()),
            sha256: Box::leak(expected_sha.clone().into_boxed_str()),
            size_bytes: body.len() as u64,
        };
        let out = ensure_local(&spec, |_, _| {}).await.unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), body);
        std::env::remove_var("SENTYNYX_DATA_DIR");
    }

    #[tokio::test]
    async fn ensure_local_resumes_partial_download() {
        let _g = ENV_GUARD.lock().unwrap();
        let body = b"abcdefghij".to_vec();
        let expected_sha = {
            let mut h = Sha256::new(); h.update(&body); hex::encode(h.finalize())
        };
        let (addr, _srv) = spawn_range_server(body.clone()).await;
        let dir = tempdir().unwrap();
        std::env::set_var("SENTYNYX_DATA_DIR", dir.path());

        // Pre-populate a partial file
        let dummy = ModelSpec {
            id: "resume", file: "y.bin", url: "", sha256: "", size_bytes: 0,
        };
        let partial = local_path(&dummy).with_extension("bin.partial");
        std::fs::create_dir_all(partial.parent().unwrap()).unwrap();
        std::fs::write(&partial, &body[..3]).unwrap();

        let spec = ModelSpec {
            id: "resume", file: "y.bin",
            url: Box::leak(format!("http://{}/y.bin", addr).into_boxed_str()),
            sha256: Box::leak(expected_sha.clone().into_boxed_str()),
            size_bytes: body.len() as u64,
        };
        let out = ensure_local(&spec, |_, _| {}).await.unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), body);
        std::env::remove_var("SENTYNYX_DATA_DIR");
    }

    #[tokio::test]
    async fn ensure_local_rejects_sha_mismatch() {
        let _g = ENV_GUARD.lock().unwrap();
        let body = b"hello".to_vec();
        let (addr, _srv) = spawn_range_server(body.clone()).await;
        let dir = tempdir().unwrap();
        std::env::set_var("SENTYNYX_DATA_DIR", dir.path());

        let spec = ModelSpec {
            id: "badsha", file: "z.bin",
            url: Box::leak(format!("http://{}/z.bin", addr).into_boxed_str()),
            sha256: "0000000000000000000000000000000000000000000000000000000000000000",
            size_bytes: body.len() as u64,
        };
        assert!(matches!(ensure_local(&spec, |_, _| {}).await, Err(ModelError::Sha { .. })));
        // Partial file should be cleaned up on sha mismatch
        assert!(!local_path(&spec).with_extension("bin.partial").exists());
        std::env::remove_var("SENTYNYX_DATA_DIR");
    }
}
