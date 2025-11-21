// /pdf-bookstore/services/auth-service/src/api/handlers/oauth.rs

use axum::{
    extract::{State, ConnectInfo, Query},
    http::{StatusCode, HeaderMap},
    response::{Json, Redirect},
};
use serde::Deserialize;
use std::net::SocketAddr;
use uuid::Uuid;
use utoipa;

use crate::{
    AppState,
    models::*,
    utils::{extract_device_info, hash_token, get_pepper},
    services::oauth_service::OAuthService,
    db::UserRepository,
};

#[derive(Debug, Deserialize)]
pub struct GoogleAuthQuery {
    pub code: String,
    pub state: String,
}

/// Handler untuk memulai Google OAuth flow
/// POST /api/auth/oauth/google
#[utoipa::path(
    post,
    path = "/api/auth/oauth/google",
    request_body = OAuthStateRequest,
    responses(
        (status = 200, description = "OAuth URL generated", body = OAuthStateResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
    ),
    tag = "oauth"
)]
pub async fn start_google_oauth(
    State(_state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<OAuthStateRequest>,
) -> Result<Json<OAuthStateResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if social login is enabled
    if !std::env::var("ENABLE_SOCIAL_LOGIN").unwrap_or_else(|_| "false".to_string()).parse::<bool>().unwrap_or(false) {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse::new("Social login is disabled", Some("SOCIAL_LOGIN_DISABLED")))
        ));
    }

    if !std::env::var("ENABLE_SOCIAL_GOOGLE").unwrap_or_else(|_| "false".to_string()).parse::<bool>().unwrap_or(false) {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse::new("Google login is disabled", Some("GOOGLE_LOGIN_DISABLED")))
        ));
    }

    // Initialize OAuth service
    let mut oauth_service = match OAuthService::new() {
        Ok(service) => service,
        Err(e) => {
            tracing::error!("Failed to initialize OAuth service: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("OAuth service not available", Some("OAUTH_INIT_ERROR")))
            ));
        }
    };

    // Generate OAuth URL
    let device_fingerprint = extract_device_info(&headers);
    let oauth_response = oauth_service.generate_auth_url(
        &request.provider,
        request.redirect_uri,
        device_fingerprint,
    ).await.map_err(|e| {
        tracing::error!("Failed to generate OAuth URL: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Failed to generate OAuth URL", Some("OAUTH_URL_ERROR")))
        )
    })?;

    Ok(Json(oauth_response))
}

/// Handler untuk Google OAuth callback
/// GET /api/auth/oauth/google/callback
#[utoipa::path(
    get,
    path = "/api/auth/oauth/google/callback",
    params(
        ("code" = String, Query, description = "Authorization code from Google"),
        ("state" = String, Query, description = "State parameter for CSRF protection"),
    ),
    responses(
        (status = 302, description = "Redirect to frontend with token"),
        (status = 400, description = "Invalid OAuth response", body = ErrorResponse),
    ),
    tag = "oauth"
)]
pub async fn google_oauth_callback(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(query): Query<GoogleAuthQuery>,
) -> Result<Redirect, (StatusCode, Json<ErrorResponse>)> {
    // Check if social login is enabled
    if !std::env::var("ENABLE_SOCIAL_LOGIN").unwrap_or_else(|_| "false".to_string()).parse::<bool>().unwrap_or(false) {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse::new("Social login is disabled", Some("SOCIAL_LOGIN_DISABLED")))
        ));
    }

    // Initialize OAuth service
    let mut oauth_service = match OAuthService::new() {
        Ok(service) => service,
        Err(e) => {
            tracing::error!("Failed to initialize OAuth service: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("OAuth service not available", Some("OAUTH_INIT_ERROR")))
            ));
        }
    };

    // Exchange code for user info
    let device_fingerprint = extract_device_info(&headers);
    let user_info = oauth_service.exchange_code(
        &query.code,
        &query.state,
        device_fingerprint,
    ).await.map_err(|e| {
        tracing::error!("Failed to exchange OAuth code: {}", e);
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("Failed to exchange OAuth code", Some("OAUTH_EXCHANGE_ERROR")))
        )
    })?;

    // Verify email domain if needed (optional business logic)
    if !user_info.verified_email {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("Email not verified with Google", Some("EMAIL_NOT_VERIFIED")))
        ));
    }

    // Create or update user
    let user = oauth_service.create_or_update_user(&user_info, &state.db).await.map_err(|e| {
        tracing::error!("Failed to create/update user: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Failed to process user", Some("USER_PROCESSING_ERROR")))
        )
    })?;

    // Generate tokens (similar to verify_otp handler)
    let user_repository = UserRepository::new(get_pepper().as_bytes());
    let token_pair = state.jwt_service.generate_token_pair(&user).map_err(|e| {
        tracing::error!("Failed to generate token pair: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Token generation failed", Some("TOKEN_ERROR")))
        )
    })?;

    // Store refresh token
    let token_hash = hash_token(&token_pair.refresh_token);
    let expires_at = chrono::Utc::now() + chrono::Duration::days(7);
    let device_fingerprint = extract_device_info(&headers).unwrap_or_default();

    if let Err(e) = sqlx::query!(
        r#"
        INSERT INTO refresh_tokens (user_id, token_hash, device_fingerprint, expires_at)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (token_hash) DO NOTHING
        "#,
        user.id,
        token_hash,
        Some(device_fingerprint.clone()),
        expires_at
    )
    .execute(&state.db)
    .await {
        tracing::warn!("Failed to store refresh token: {}", e);
    }

    // Create session
    let session_info = crate::db::SessionInfo {
        device_info: Some(device_fingerprint),
        ip_address: Some(addr.ip()),
    };

    let session_token = user_repository.update_last_login_with_session(
        &state.db,
        user.id,
        session_info,
    ).await.unwrap_or_else(|_| Uuid::new_v4().to_string());

    // Log successful OAuth login
    log_security_event(
        &state.db,
        Some(user.id),
        "GOOGLE_OAUTH_LOGIN_SUCCESS",
        serde_json::json!({
            "provider": "google",
            "email": user_info.email,
            "ip": addr.ip().to_string(),
            "user_agent": extract_device_info(&headers)
        }),
        true
    ).await;

    // Redirect to frontend with tokens
    let frontend_url = std::env::var("FRONTEND_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    let redirect_url = format!(
        "{}?access_token={}&refresh_token={}&session_token={}&login_success=true",
        frontend_url,
        urlencoding::encode(&token_pair.access_token),
        urlencoding::encode(&token_pair.refresh_token),
        urlencoding::encode(&session_token)
    );

    Ok(Redirect::temporary(&redirect_url))
}

/// Handler untuk OAuth check status
/// GET /api/auth/oauth/status
#[utoipa::path(
    get,
    path = "/api/auth/oauth/status",
    responses(
        (status = 200, description = "OAuth status", body = serde_json::Value),
    ),
    tag = "oauth"
)]
pub async fn oauth_status() -> Json<serde_json::Value> {
    let social_login_enabled = std::env::var("ENABLE_SOCIAL_LOGIN")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);

    let google_enabled = std::env::var("ENABLE_SOCIAL_GOOGLE")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);

    Json(serde_json::json!({
        "social_login_enabled": social_login_enabled,
        "google_oauth_enabled": google_enabled,
        "providers": if social_login_enabled {
            vec!["google".to_string()]
        } else {
            vec![]
        }
    }))
}

/// Helper untuk log security events (moved from auth.rs)
async fn log_security_event(
    pool: &sqlx::PgPool,
    user_id: Option<Uuid>,
    event_type: &str,
    event_data: serde_json::Value,
    success: bool,
) {
    let result = sqlx::query!(
        r#"
        INSERT INTO security_events (user_id, event_type, event_data, success)
        VALUES ($1, $2, $3, $4)
        "#,
        user_id,
        event_type,
        event_data,
        success
    )
    .execute(pool)
    .await;

    if let Err(e) = result {
        tracing::warn!("Failed to log security event: {}", e);
    }
}