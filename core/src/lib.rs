pub mod epub;
pub mod i18n;
pub mod library;
pub mod sharing;

use std::time::{SystemTime, UNIX_EPOCH};

/// Current time as seconds since UNIX epoch.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Sanitize a string for use as a filename, with length limit.
pub fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .take(200)
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c > '\x7F' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let sanitized = sanitized.trim_matches('.').to_string();
    if sanitized.is_empty() || sanitized.contains("..") {
        format!("book_{}", now_secs())
    } else {
        sanitized
    }
}

pub fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

pub fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| format!("base64 decode: {e}"))
}
