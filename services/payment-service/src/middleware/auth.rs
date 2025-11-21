// /pdf-bookstore/services/payment-service/src/middleware/auth.rs

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{Json, Response},
};
use sha2::Digest;
use uuid::Uuid;
use std::env;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};

use crate::{
    AppState,
    models::{ErrorResponse, CachedToken},
};

lazy_static::lazy_static! {
    static ref TOKEN_CACHE: Arc<RwLock<HashMap<String, CachedToken>>> = 
        Arc::new(RwLock::new(HashMap::new()));
}

/// Middleware untuk verify JWT token dari auth service
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let path = req.uri().path().to_string();
    
    // Skip auth untuk public endpoints
    if is_public_endpoint(&path) {
        return Ok(next.run(req).await);
    }

    // ===== GATEWAY SUPPORT =====
    
    // Kalau ada header dari gateway, trust itu
    if let Some(_gateway_header) = req.headers().get("X-Gateway-Request") {
        if let Some(user_id_header) = req.headers().get("X-User-Id") {
            if let Ok(user_id_str) = user_id_header.to_str() {
                if let Ok(user_id) = Uuid::parse_str(user_id_str) {
                    // Role bisa optional, default ke "customer"
                    let user_role = req.headers()
                        .get("X-User-Role")
                        .and_then(|h| h.to_str().ok())
                        .unwrap_or("customer")
                        .to_string();

                    // Email bisa optional
                    let user_email = req.headers()
                        .get("X-User-Email")
                        .and_then(|h| h.to_str().ok())
                        .unwrap_or("")
                        .to_string();

                    tracing::debug!("✓ Gateway auth: user={}, role={}, path={}", 
                        user_id, user_role, path);

                    // Check admin access
                    if path.contains("/admin/") && user_role != "admin" {
                        tracing::warn!("Non-admin user {} mencoba akases admin via gateway: {}", user_id, path);
                        return Err((
                            StatusCode::FORBIDDEN,
                            Json(ErrorResponse {
                                success: false,
                                message: "Akses admin diperlukan".to_string(),
                                error_code: Some("INSUFFICIENT_PRIVILEGES".to_string()),
                                details: None,
                            })
                        ));
                    }
                    // Insert ke request extensions
                    req.extensions_mut().insert(user_id);
                    req.extensions_mut().insert(user_role.clone());
                    if !user_email.is_empty() {
                        req.extensions_mut().insert(user_email.clone());
                    }

                    // gateway verified, Skip JWT verification
                    return Ok(next.run(req).await);
                }
            }
        }
    }

    // ===== END GATEWAY SUPPORT =====
    // ===== JWT TOKEN VERIFICATION (FALLBACK) =====
    
    // Extract Authorization header
    tracing::debug!("Direct JWT verification for path: {}", path);
    
    // Extract Authorization header
    let auth_header = req.headers()
        .get("authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| {
            if header.starts_with("Bearer ") {
                Some(header.strip_prefix("Bearer ").unwrap().to_string())
            } else {
                None
            }
        });
    
    let token = match auth_header {
        Some(token) => token,
        None => {
            tracing::debug!("Request ke {} ditolak: missing authorization header", path);
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    success: false,
                    message: "Authorization header diperlukan".to_string(),
                    error_code: Some("MISSING_TOKEN".to_string()),
                    details: None,
                })
            ));
        }
    };
    
    // Check cache dulu untuk performance
    let token_hash = format!("{:x}", sha2::Sha256::digest(token.as_bytes()));
    {
        let cache = TOKEN_CACHE.read().await;
        if let Some(cached) = cache.get(&token_hash) {
            if cached.expires > Instant::now() {
                tracing::debug!("✓ Using cached token for user: {}", cached.user_id);
                
                // Use cached data
                req.extensions_mut().insert(cached.user_id);
                req.extensions_mut().insert(cached.role.clone());
                req.extensions_mut().insert(cached.email.clone());
                req.extensions_mut().insert(token.clone());
                
                // Check admin access from cache
                if path.contains("/admin/") && cached.role != "admin" {
                    return Err((
                        StatusCode::FORBIDDEN,
                        Json(ErrorResponse {
                            success: false,
                            message: "Akses admin diperlukan".to_string(),
                            error_code: Some("INSUFFICIENT_PRIVILEGES".to_string()),
                            details: None,
                        })
                    ));
                }
                
                return Ok(next.run(req).await);
            } else {
                tracing::debug!("Cache expired for token hash: {}", &token_hash[..8]);
            }
        }
    }

    // Kalau ga ada di cache, verify ke auth service
    let auth_service_url = env::var("AUTH_SERVICE_URL")
        .unwrap_or_else(|_| "http://localhost:3001".to_string());

    tracing::debug!("Verifying token with auth service: {}", auth_service_url);

    let verify_response = state.http_client
        .get(format!("{}/api/auth/verify", auth_service_url))
        .header("Authorization", format!("Bearer {}", token))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Gagal kontak auth service: {}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    success: false,
                    message: "Auth service tidak tersedia".to_string(),
                    error_code: Some("AUTH_SERVICE_ERROR".to_string()),
                    details: Some(serde_json::json!({
                        "error": e.to_string()
                    })),
                })
            )
        })?;

    if !verify_response.status().is_success() {
        tracing::warn!("Token verification failed with status: {}", verify_response.status());
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                success: false,
                message: "Token tidak valid atau expired".to_string(),
                error_code: Some("INVALID_TOKEN".to_string()),
                details: None,
            })
        ));
    }

    // Parse response
    let auth_data: serde_json::Value = verify_response.json().await
        .map_err(|e| {
            tracing::error!("Gagal parse auth response: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: "Gagal parse response auth service".to_string(),
                    error_code: Some("AUTH_PARSE_ERROR".to_string()),
                    details: None,
                })
            )
        })?;

    // Extract user info dengan proper error handling
    let user_data = auth_data["user"].as_object()
        .ok_or_else(|| (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                success: false,
                message: "Invalid token response".to_string(),
                error_code: Some("INVALID_TOKEN_RESPONSE".to_string()),
                details: None,
            })
        ))?;

    let user_id_str = user_data["id"].as_str()
        .ok_or_else(|| (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                success: false,
                message: "Invalid user ID dalam token".to_string(),
                error_code: Some("INVALID_USER_ID".to_string()),
                details: None,
            })
        ))?;

    // Parse UUID dengan proper error handling
    let user_id = Uuid::parse_str(user_id_str)
        .map_err(|_| (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                success: false,
                message: "Format user ID tidak valid".to_string(),
                error_code: Some("INVALID_UUID".to_string()),
                details: None,
            })
        ))?;

    let user_role = user_data["role"].as_str()
        .unwrap_or("customer")
        .to_string();

    let user_email = user_data["email"].as_str()
        .unwrap_or("")
        .to_string();

    tracing::debug!("✓ Token verified: user={}, role={}", user_id, user_role);

    // Insert ke cache untuk performance
    {
        let mut cache = TOKEN_CACHE.write().await;
        cache.insert(token_hash.clone(), CachedToken {
            user_id,
            role: user_role.clone(),
            email: user_email.clone(),
            expires: Instant::now() + Duration::from_secs(300), // 5 minutes cache
        });
        
        tracing::debug!("Token cached: {} (expires in 5 minutes)", &token_hash[..8]);
    }

    // Check admin access
    if path.contains("/admin/") && user_role != "admin" {
        tracing::warn!("Non-admin user {} mencoba akses admin: {}", user_id, path);
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                success: false,
                message: "Akses admin diperlukan".to_string(),
                error_code: Some("INSUFFICIENT_PRIVILEGES".to_string()),
                details: None,
            })
        ));
    }

    // Add to request extensions
    req.extensions_mut().insert(user_id);
    req.extensions_mut().insert(user_role.clone());
    req.extensions_mut().insert(user_email.clone());
    req.extensions_mut().insert(token);
    
    // Log admin access untuk audit
    if path.contains("/admin/") {
        tracing::info!("Admin access: {} by user {} (role: {})", path, user_id, user_role);
        
        if let Err(e) = log_admin_access(&state, user_id, &path).await {
            tracing::warn!("Gagal log admin access: {}", e);
        }
    }
    
    Ok(next.run(req).await)
}

/// Log admin access ke database untuk audit trail
async fn log_admin_access(
    state: &AppState,
    user_id: Uuid,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query!(
        r#"
        INSERT INTO audit_logs (user_id, action, resource_type, details)
        VALUES ($1, 'ADMIN_ACCESS', 'api', $2)
        "#,
        user_id,
        serde_json::json!({ "path": path })
    )
    .execute(state.repository.get_pool())
    .await?;
    
    Ok(())
}

/// Check apakah endpoint public (tidak perlu auth)
fn is_public_endpoint(path: &str) -> bool {
    let public_paths = [
        "/health",
        "/api/webhook",
        "/api/webhooks",
        "/api/csrf-token", 
    ];
    
    public_paths.iter().any(|&public_path| path.starts_with(public_path))
}

/// Clean expired tokens dari cache (call periodically)
pub async fn clean_token_cache() {
    let mut cache = TOKEN_CACHE.write().await;
    let now = Instant::now();
    cache.retain(|_, token| token.expires > now);
    tracing::debug!("Token cache cleaned, remaining: {}", cache.len());
}