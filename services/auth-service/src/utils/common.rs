// /pdf-bookstore/services/auth-service/src/utils/common.rs

use sha2::{Sha256, Digest};
use axum::http::HeaderMap;
use std::env;

/// Mengambil password pepper dari environment variable
pub fn get_pepper() -> String {
    env::var("PASSWORD_PEPPER")
        .unwrap_or_else(|_| "default_pepper_change_in_production".to_string())
}

/// Hash token menggunakan SHA256
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Memeriksa pattern mencurigakan dalam input untuk mencegah injection
pub fn contains_suspicious_patterns(input: &str) -> bool {
    let suspicious_patterns = [
        "<script", "javascript:", "data:", "vbscript:",
        "../../", "../", "./", "union select", "drop table",
        "insert into", "delete from", "update set", "--", "/*", "*/"
    ];
    
    let input_lower = input.to_lowercase();
    suspicious_patterns.iter().any(|pattern| input_lower.contains(pattern))
}

/// Extract device info dari headers untuk session tracking
pub fn extract_device_info(headers: &HeaderMap) -> Option<String> {
    headers.get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(|ua| {
            if ua.len() > 500 {
                format!("{}...", &ua[..497])
            } else {
                ua.to_string()
            }
        })
}