use keyring::Entry;
use std::collections::HashMap;
use std::path::PathBuf;

const SERVICE: &str = "sentynyx";

fn env_key_for(provider: &str) -> Option<String> {
    let var = match provider {
        "openai"     => "OPENAI_API_KEY",
        "anthropic"  => "ANTHROPIC_API_KEY",
        "google"     => "GOOGLE_API_KEY",
        "xai"        => "XAI_API_KEY",
        "openrouter" => "OPENROUTER_API_KEY",
        _ => return None,
    };
    std::env::var(var).ok().filter(|s| !s.is_empty())
}

fn secrets_path() -> PathBuf {
    // Same resolution as models_root() — respects SENTYNYX_DATA_DIR for tests.
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
    base.join("secrets.json")
}

fn read_file_secrets() -> HashMap<String, String> {
    let p = secrets_path();
    std::fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str::<HashMap<String, String>>(&s).ok())
        .unwrap_or_default()
}

fn write_file_secrets(map: &HashMap<String, String>) -> std::io::Result<()> {
    let p = secrets_path();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(map)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(&p, json)?;
    // Best-effort permission hardening — fails silently on non-unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Outcome of a `set` call — lets callers surface "stored in plaintext file"
/// if the keychain wasn't reachable on a signed release build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetOutcome {
    /// Stored in the OS keychain (macOS Keychain / Windows Credential Manager / libsecret).
    Keychain,
    /// Stored in the plaintext (0600) JSON fallback at `<data_dir>/secrets.json`.
    /// Either because we're in a debug build (keychain can't be trusted) or the
    /// keychain rejected the write in release.
    FileFallback,
}

pub fn set(provider: &str, secret: &str) -> Result<SetOutcome, keyring::Error> {
    // Keys arrive from paste buffers — strip the whitespace that turns a
    // valid key into a mystifying 401.
    let secret = secret.trim();
    // Dev (unsigned) builds: keychain silently "succeeds" but doesn't persist,
    // so we write to the file and mirror-write to keychain as a no-op bonus.
    // Release builds: keychain is primary, file is a break-glass fallback for
    // when the user is on a Linux distro without a Secret Service daemon etc.
    if cfg!(debug_assertions) {
        write_file(provider, secret)?;
        let _ = Entry::new(SERVICE, provider).and_then(|e| e.set_password(secret));
        return Ok(SetOutcome::FileFallback);
    }

    // Release: try keychain first. On error, fall back to file and flag it.
    match Entry::new(SERVICE, provider).and_then(|e| e.set_password(secret)) {
        Ok(()) => {
            // Clear any stale file-fallback entry so reads don't race.
            let mut map = read_file_secrets();
            if map.remove(provider).is_some() {
                let _ = write_file_secrets(&map);
            }
            Ok(SetOutcome::Keychain)
        }
        Err(e) => {
            eprintln!("keys::set keychain error, falling back to file: {e}");
            write_file(provider, secret)?;
            Ok(SetOutcome::FileFallback)
        }
    }
}

fn write_file(provider: &str, secret: &str) -> Result<(), keyring::Error> {
    let mut map = read_file_secrets();
    map.insert(provider.to_string(), secret.to_string());
    write_file_secrets(&map).map_err(|e| keyring::Error::PlatformFailure(Box::new(e)))
}

pub fn get(provider: &str) -> Option<String> {
    // Env var always wins — easy override without mutating persisted state.
    if let Some(v) = env_key_for(provider) { return Some(v); }
    // In debug the file is the source of truth (keychain writes aren't reliable
    // for unsigned binaries). In release the keychain wins; file is fallback.
    if cfg!(debug_assertions) {
        if let Some(v) = read_file_secrets().remove(provider) { return Some(v); }
        return Entry::new(SERVICE, provider).ok().and_then(|e| e.get_password().ok());
    }
    if let Some(v) = Entry::new(SERVICE, provider).ok().and_then(|e| e.get_password().ok()) {
        return Some(v);
    }
    read_file_secrets().remove(provider)
}

pub fn has(provider: &str) -> bool {
    get(provider).is_some()
}
