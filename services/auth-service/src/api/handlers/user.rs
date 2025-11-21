// /pdf-bookstore/services/auth-service/src/api/handlers/user.rs

use axum::{
    extract::{State, Query},
    http::StatusCode,
    response::Json,
    Extension,
};
use uuid::Uuid;
use std::collections::HashMap;
use validator::Validate;
use chrono::Utc;
use utoipa;

use crate::{
    AppState,
    models::*,
    db::{UserRepository, DatabaseError},
    utils::common::{get_pepper, hash_token},
};

/// Handler untuk mendapatkan profile user
/// GET /api/auth/profile
#[utoipa::path(
    get,
    path = "/api/auth/profile",
    responses(
        (status = 200, description = "User profile", body = UserProfile),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
    ),
    tag = "auth",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_profile(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    let user_repository = UserRepository::new(get_pepper().as_bytes());
    
    match user_repository.find_by_id(&state.db, user_id).await {
        Ok(user) => {
            let user_profile = UserProfile::from(user);
            Ok(Json(AuthResponse::with_user(
                user_profile,
                "Profil berhasil diambil"
            )))
        }
        Err(DatabaseError::UserNotFound) => {
            Ok(Json(AuthResponse::error("Profil user tidak ditemukan")))
        }
        Err(e) => {
            tracing::error!("Get profile error: {}", e);
            Ok(Json(AuthResponse::error("Gagal mendapatkan profil")))
        }
    }
}

/// Handler untuk update profile user
/// PUT /api/auth/profile
pub async fn update_profile(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Json(request): Json<UpdateProfileRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validasi request
    if let Err(errors) = request.validate() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::validation_error(errors))
        ));
    }
    
    // Update profile
    let result = sqlx::query!(
        r#"
        UPDATE users
        SET full_name = COALESCE($1, full_name),
            phone = COALESCE($2, phone),
            bio = COALESCE($3, bio),
            avatar_url = COALESCE($4, avatar_url),
            updated_at = NOW()
        WHERE id = $5
        RETURNING id, email, full_name, role, email_verified, phone, bio, avatar_url, created_at, updated_at
        "#,
        request.full_name,
        request.phone,
        request.bio,
        request.avatar_url,
        user_id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update profile: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Failed to update profile", Some("UPDATE_ERROR")))
        )
    })?;
    
    let user = result.ok_or((
        StatusCode::NOT_FOUND,
        Json(ErrorResponse::new("User tidak ditemukan", Some("USER_NOT_FOUND")))
    ))?;

    let user_role = user.role.clone().unwrap_or("customer".to_string());
    
    let user_profile = UserProfile {
        id: user.id,
        email: user.email,
        full_name: user.full_name,
        role: user_role.clone(),
        email_verified: user.email_verified.unwrap_or(false),
        created_at: user.created_at.unwrap_or(Utc::now()),
        last_login: Some(user.updated_at.unwrap_or(Utc::now())),
        permissions: get_permissions_for_role(&user_role),
        subscription_status: None,
        preferences: Some(serde_json::json!({
            "phone": user.phone,
            "bio": user.bio,
            "avatar_url": user.avatar_url
        })),
    };
    
    Ok(Json(AuthResponse {
        success: true,
        message: "Profile berhasil diupdate".to_string(),
        user: Some(user_profile),
        token: None,
        refresh_token: None,
        expires_in: None,
        session_id: None,
        requires_verification: None,
        two_factor_required: None,
    }))
}

/// Handler untuk ganti password user yang sudah login
/// POST /api/auth/password/change
pub async fn change_password(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Json(request): Json<ChangePasswordRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validasi passwords match
    if request.new_password != request.confirm_password {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("Password baru tidak sama", Some("PASSWORD_MISMATCH")))
        ));
    }
    
    // Validasi not same as old
    if request.old_password == request.new_password {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("Password baru tidak boleh sama dengan yang lama", Some("SAME_PASSWORD")))
        ));
    }
    
    let user_repository = UserRepository::new(get_pepper().as_bytes());
    
    // Get current user
    let user = user_repository.find_by_id(&state.db, user_id)
        .await
        .map_err(|_| (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new("User tidak ditemukan", Some("USER_NOT_FOUND")))
        ))?;
    
    // Verify old password
    let valid = user_repository.security_service
        .verify_password(&request.old_password, &user.password_hash)
        .await
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Failed to verify password", Some("VERIFY_ERROR")))
        ))?;
    
    if !valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse::new("Password lama salah", Some("INVALID_OLD_PASSWORD")))
        ));
    }
    
    // Hash new password
    let new_hash = user_repository.security_service
        .hash_password(&request.new_password)
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Failed to hash password", Some("HASH_ERROR")))
        ))?;
    
    // Update password
    sqlx::query!(
        r#"
        UPDATE users 
        SET password_hash = $1,
            last_password_change = NOW(),
            updated_at = NOW()
        WHERE id = $2
        "#,
        new_hash,
        user_id
    )
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update password: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Failed to update password", Some("UPDATE_ERROR")))
        )
    })?;
    
    // Revoke all tokens (force re-login)
    sqlx::query!(
        r#"
        UPDATE refresh_tokens 
        SET is_revoked = true, revoked_reason = 'Password changed'
        WHERE user_id = $1
        "#,
        user_id
    )
    .execute(&state.db)
    .await
    .ok();

    log_security_event(
        &state.db,
        Some(user_id),
        "PASSWORD_CHANGED",
        serde_json::json!({
            "timestamp": Utc::now(),
            "forced_logout": true
        }),
        true
    ).await;
    
    Ok(Json(AuthResponse::success("Password berhasil diubah. Silakan login kembali")))
}

/// Handler untuk mendapatkan login history
/// GET /api/auth/login-history
pub async fn get_login_history(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let limit = params.get("limit")
        .and_then(|l| l.parse::<i64>().ok())
        .unwrap_or(20)
        .min(100);
    
    let history = sqlx::query!(
        r#"
        SELECT 
            id,
            ip_address::text as ip_address,
            user_agent,
            device_fingerprint,
            login_status,
            failure_reason,
            created_at
        FROM login_history
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT $2
        "#,
        user_id,
        limit
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get login history: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Failed to get login history", Some("DB_ERROR")))
        )
    })?;
    
    let items: Vec<LoginHistoryItem> = history.into_iter().map(|row| {
        LoginHistoryItem {
            id: row.id,
            ip_address: row.ip_address,
            user_agent: row.user_agent,
            device_fingerprint: row.device_fingerprint,
            login_status: row.login_status.unwrap_or("unknown".to_string()),
            failure_reason: row.failure_reason,
            created_at: row.created_at.unwrap_or(Utc::now()),
        }
    }).collect();
    
    Ok(Json(serde_json::json!({
        "success": true,
        "history": items,
        "total": items.len()
    })))
}

/// Handler untuk kirim email verifikasi
/// POST /api/auth/email/send-verification
pub async fn send_verification_email(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    let user = sqlx::query!(
        "SELECT email, email_verified FROM users WHERE id = $1",
        user_id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Database error", Some("DB_ERROR")))
        )
    })?
    .ok_or((
        StatusCode::NOT_FOUND,
        Json(ErrorResponse::new("User tidak ditemukan", Some("USER_NOT_FOUND")))
    ))?;
    
    // Check if already verified
    if user.email_verified.unwrap_or(false) {
        return Ok(Json(AuthResponse::success("Email sudah terverifikasi")));
    }
    
    let verify_token = format!("verify_{}", Uuid::new_v4());
    let token_hash = hash_token(&verify_token);
    
    sqlx::query!(
        r#"
        INSERT INTO email_verification_tokens (user_id, token_hash, expires_at)
        VALUES ($1, $2, NOW() + INTERVAL '24 hours')
        ON CONFLICT (user_id) DO UPDATE
        SET token_hash = $2, expires_at = NOW() + INTERVAL '24 hours'
        "#,
        user_id,
        token_hash
    )
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to store verification token: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Failed to create verification token", Some("TOKEN_ERROR")))
        )
    })?;
    
    // Send email (development mode)
    send_email(&user.email, "Email Verification", &format!(
        "Click here to verify: http://localhost:8080/verify-email?token={}",
        verify_token
    ));
    
    Ok(Json(AuthResponse::success("Email verifikasi telah dikirim")))
}

/// Handler untuk verifikasi email
/// POST /api/auth/email/verify
pub async fn verify_email(
    State(state): State<AppState>,
    Json(request): Json<VerifyEmailRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    let token_hash = hash_token(&request.token);
    
    let token_data = sqlx::query!(
        r#"
        SELECT user_id, expires_at, verified_at
        FROM email_verification_tokens
        WHERE token_hash = $1
        "#,
        token_hash
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Database error", Some("DB_ERROR")))
        )
    })?
    .ok_or((
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse::new("Token tidak valid", Some("INVALID_TOKEN")))
    ))?;
    
    // Check if already verified
    if token_data.verified_at.is_some() {
        return Ok(Json(AuthResponse::success("Email sudah terverifikasi sebelumnya")));
    }
    
    // Check if expired
    if token_data.expires_at < Utc::now() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("Token sudah expired", Some("TOKEN_EXPIRED")))
        ));
    }
    
    let mut tx = state.db.begin().await.map_err(|e| {
        tracing::error!("Transaction error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Database error", Some("DB_ERROR")))
        )
    })?;
    
    sqlx::query!(
        "UPDATE users SET email_verified = true, updated_at = NOW() WHERE id = $1",
        token_data.user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to verify email: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Failed to verify email", Some("UPDATE_ERROR")))
        )
    })?;
    
    sqlx::query!(
        "UPDATE email_verification_tokens SET verified_at = NOW() WHERE token_hash = $1",
        token_hash
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to mark token used: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Database error", Some("DB_ERROR")))
        )
    })?;
    
    tx.commit().await.map_err(|e| {
        tracing::error!("Transaction commit error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Database error", Some("DB_ERROR")))
        )
    })?;
    
    Ok(Json(AuthResponse::success("Email berhasil diverifikasi")))
}

/// Handler untuk mendapatkan aktivitas user sendiri
/// GET /api/auth/my-activity
pub async fn get_my_activity(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let limit = params.get("limit")
        .and_then(|l| l.parse::<u32>().ok())
        .unwrap_or(20)
        .min(50);
    
    let user_repository = UserRepository::new(get_pepper().as_bytes());
    
    match user_repository.get_security_activity_feed(
        &state.db,
        limit,
        None,
        Some(user_id)
    ).await {
        Ok(activities) => {
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "Your activities retrieved",
                "total": activities.len(),
                "data": activities
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get user activities: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Failed to get activities", Some("DATABASE_ERROR")))
            ))
        }
    }
}

// Helper functions
fn send_email(to: &str, subject: &str, body: &str) {
    tracing::info!(
        "📧 Email terkirim [DEVELOPMENT MODE]\nTo: {}\nSubject: {}\nBody: {}",
        to, subject, body
    );
}

fn get_permissions_for_role(role: &str) -> Vec<String> {
    match role {
        "admin" => vec![
            "users:read".to_string(),
            "users:write".to_string(),
            "users:delete".to_string(),
            "books:read".to_string(),
            "books:write".to_string(),
            "orders:read".to_string(),
            "orders:write".to_string(),
            "admin:access".to_string(),
        ],
        "customer" => vec![
            "books:read".to_string(),
            "orders:read".to_string(),
            "profile:write".to_string(),
        ],
        _ => vec!["books:read".to_string()],
    }
}

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