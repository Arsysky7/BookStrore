// /pdf-bookstore/services/auth-service/src/middleware/auth.rs

use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::Json,
    middleware::Next,
};
use uuid::Uuid;

use crate::{
    AppState,
    models::ErrorResponse,
};

/// Middleware untuk validasi JWT token pada protected endpoints
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<axum::response::Response, (StatusCode, Json<ErrorResponse>)> {
    let path = req.uri().path().to_string();
    
    // Skip auth untuk public endpoints
    if is_public_endpoint(&path) {
        return Ok(next.run(req).await);
    }

    // Extract dan validate JWT token
    let token = extract_bearer_token(&req)?;
    
    // Verify token menggunakan shared JWT service
    let claims = state.jwt_service.verify_token(&token)
        .map_err(|e| {
            tracing::warn!("JWT verification failed: {}", e);
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse::new("Token tidak valid atau expired", Some("INVALID_TOKEN")))
            )
        })?;

    // Check admin access untuk admin routes
    if path.contains("/admin/") && claims.role != "admin" {
        tracing::warn!("Non-admin user {} attempted admin access: {}", claims.sub, path);
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse::new("Admin access required", Some("INSUFFICIENT_PRIVILEGES")))
        ));
    }

    // Inject user information ke request extensions
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse::new("Invalid user ID in token", Some("INVALID_TOKEN")))
        ))?;

    req.extensions_mut().insert(user_id);
    req.extensions_mut().insert(claims.role.clone());
    req.extensions_mut().insert(token.to_string());
    req.extensions_mut().insert(state.jwt_service.clone());

    // Log admin access untuk security monitoring
    if path.contains("/admin/") {
        tracing::info!("Admin access: {} by user {} (role: {})", path, user_id, claims.role);
    }

    Ok(next.run(req).await)
}

/// Helper untuk check apakah endpoint public (no auth required)
fn is_public_endpoint(path: &str) -> bool {
    let public_paths = [
        "/health",
        "/api/auth/register",
        "/api/auth/login",
        "/api/auth/password-reset/request",
        "/api/auth/password-reset/confirm",
        "/api/auth/email/verify",
    ];
    
    public_paths.iter().any(|&public_path| path == public_path)
}

/// Helper untuk extract bearer token dari request header
fn extract_bearer_token(req: &Request) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let auth_header = req
        .headers()
        .get("authorization")
        .ok_or_else(|| {
            tracing::debug!("Authorization header not found");
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse::new("Missing authorization header", Some("MISSING_AUTH_HEADER")))
            )
        })?
        .to_str()
        .map_err(|_| {
            tracing::debug!("Invalid authorization header encoding");
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse::new("Invalid authorization header encoding", Some("INVALID_HEADER_ENCODING")))
            )
        })?;

    if !auth_header.starts_with("Bearer ") {
        tracing::debug!("Authorization header missing Bearer prefix");
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse::new("Authorization header must start with 'Bearer '", Some("INVALID_AUTH_FORMAT")))
        ));
    }

    let token = auth_header.strip_prefix("Bearer ").unwrap();
    Ok(token.to_string())
}