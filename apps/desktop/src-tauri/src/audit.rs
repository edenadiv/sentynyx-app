use chrono::Utc;
use sha2::{Digest, Sha256};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub id: String,
    pub ts: String,
    pub kind: String,
    pub raw_hash: String,
    pub alias: String,
    pub action: String,
    pub prev_hash: String,
    pub sig: String,
}

pub fn hash_raw(raw: &str) -> String {
    let mut h = Sha256::new();
    h.update(raw.as_bytes());
    hex::encode(h.finalize())
}

pub fn sign(prev_hash: &str, ts: &str, kind: &str, raw_hash: &str, alias: &str, action: &str) -> String {
    let mut h = Sha256::new();
    h.update(prev_hash.as_bytes());
    h.update(ts.as_bytes());
    h.update(kind.as_bytes());
    h.update(raw_hash.as_bytes());
    h.update(alias.as_bytes());
    h.update(action.as_bytes());
    hex::encode(h.finalize())
}

pub fn make(prev_hash: &str, kind: &str, raw: &str, alias: &str, action: &str) -> AuditEntry {
    let ts = Utc::now().to_rfc3339();
    let raw_hash = hash_raw(raw);
    let sig = sign(prev_hash, &ts, kind, &raw_hash, alias, action);
    AuditEntry {
        id: uuid::Uuid::new_v4().to_string(),
        ts, kind: kind.to_string(), raw_hash, alias: alias.to_string(),
        action: action.to_string(), prev_hash: prev_hash.to_string(), sig,
    }
}
