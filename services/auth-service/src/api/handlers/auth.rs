// /pdf-bookstore/services/auth-service/src/api/handlers/auth.rs

use axum::{
    extract::{State, ConnectInfo},
    http::{StatusCode, HeaderMap},
    response::Json,
    Extension,
};
use serde_json::json;
use std::net::SocketAddr;
use uuid::Uuid;
use validator::Validate;
use chrono::{Utc, Duration};
use utoipa;

use crate::{
    AppState,
    models::*,
    db::{UserRepository, DatabaseError, SessionInfo},
    utils::{hash_token, extract_device_info, contains_suspicious_patterns, get_pepper, EmailService}, 
};

/// Handler untuk registrasi user baru
/// POST /api/auth/register
#[utoipa::path(
    post,
    path = "/api/auth/register",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "Registration successful", body = AuthResponse),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 409, description = "Email already exists", body = ErrorResponse),
    ),
    tag = "auth"
)]
pub async fn register_user(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validasi input
    if let Err(errors) = request.validate() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::validation_error(errors))
        ));
    }

    // Cek panjang input
    if request.email.len() > 255 || request.full_name.len() > 255 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("Input terlalu panjang", Some("INPUT_TOO_LONG")))
        ));
    }

    // Cek pattern mencurigakan
    if contains_suspicious_patterns(&request.email) || contains_suspicious_patterns(&request.full_name) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("Input tidak valid terdeteksi", Some("SUSPICIOUS_INPUT")))
        ));
    }

    let user_repository = UserRepository::new(get_pepper().as_bytes());

    // Create user di database
    match user_repository.create_user(&state.db, request).await {
        Ok(user) => {
            // Log registration success
            log_security_event(
                &state.db,
                Some(user.id),
                "USER_REGISTERED",
                json!({
                    "email": user.email,
                    "ip": addr.ip().to_string(),
                    "user_agent": extract_device_info(&headers)
                }),
                true
            ).await;

            // Generate verification token
            let user_email = user.email.clone();
            let verify_token = format!("verify_{}", Uuid::new_v4());
            let token_hash = hash_token(&verify_token);

            // Store verification token
            sqlx::query!(
                r#"
                INSERT INTO email_verification_tokens (user_id, token_hash, expires_at)
                VALUES ($1, $2, NOW() + INTERVAL '24 hours')
                ON CONFLICT (user_id) DO UPDATE
                SET token_hash = $2, expires_at = NOW() + INTERVAL '24 hours'
                "#,
                user.id,
                token_hash
            )
            .execute(&state.db)
            .await
            .ok();

            // Send verification email
            match EmailService::new().await {
                Ok(service) => {
                    if let Err(e) = service.send_verification_email(&user_email, &verify_token).await {
                        tracing::error!("Failed to send verification email: {}", e);
                    } else {
                        tracing::info!("Verification email sent to {}", user_email);
                    }
                }
                Err(e) => tracing::error!("Email service failed: {}", e)
            }

            let user_profile = UserProfile::from(user);

            // Return WITHOUT tokens
            Ok(Json(AuthResponse {
                success: true,
                message: "Registration successful. Please check your email to verify your account.".to_string(),
                user: Some(user_profile),
                token: None,  
                refresh_token: None,  
                expires_in: None,
                session_id: None,
                requires_verification: Some(true),
                two_factor_required: None,
            }))
        }
        Err(DatabaseError::EmailExists) => {
            Err((
                StatusCode::CONFLICT,
                Json(ErrorResponse::new("Email sudah terdaftar", Some("EMAIL_EXISTS")))
            ))
        }
        Err(DatabaseError::RateLimitExceeded) => {
            Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorResponse::new("Terlalu banyak percobaan registrasi", Some("RATE_LIMIT_EXCEEDED")))
            ))
        }
        Err(e) => {
            tracing::error!("Registration failed: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Registrasi gagal", Some("REGISTRATION_ERROR")))
            ))
        }
    }
}

/// Handler untuk login user
/// POST /api/auth/login
#[utoipa::path(
    post,
    path = "/api/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Invalid credentials", body = ErrorResponse),
    ),
    tag = "auth"
)]
pub async fn login_user(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validasi request
    if let Err(errors) = request.validate() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::validation_error(errors))
        ));
    }

    let user_repository = UserRepository::new(get_pepper().as_bytes());
    
    // Get user dari database
    let user = match user_repository.find_by_email(&state.db, &request.email, Some(addr.ip())).await {
        Ok(user) => user,
        Err(DatabaseError::UserNotFound) => {
            log_security_event(
                &state.db,
                None,
                "LOGIN_FAILED",
                json!({
                    "email": request.email,
                    "ip": addr.ip().to_string(),
                    "reason": "User not found"
                }),
                false
            ).await;
            
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse::new("Kredensial tidak valid", Some("INVALID_CREDENTIALS")))
            ));
        }
        Err(DatabaseError::RateLimitExceeded) => {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorResponse::new("Terlalu banyak percobaan login", Some("RATE_LIMIT_EXCEEDED")))
            ));
        }
        Err(DatabaseError::AccountLocked) => {
            return Err((
                StatusCode::LOCKED,
                Json(ErrorResponse::new("Akun sementara terkunci", Some("ACCOUNT_LOCKED")))
            ));
        }
        Err(e) => {
            tracing::error!("Login error: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Login gagal", Some("LOGIN_ERROR")))
            ));
        }
    };

    // CHECK EMAIL VERIFICATION FIRST
    if !user.email_verified {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse::new(
                "Email belum terverifikasi. Silakan cek email Anda untuk link verifikasi.", 
                Some("EMAIL_NOT_VERIFIED")
            ))
        ));
    }

    // Verify password
    match user_repository.verify_password_with_attempts(
        &state.db,
        user.id,
        &request.password,
        Some(addr.ip()),
    ).await {
        Ok(true) => {
            log_security_event(
                &state.db,
                Some(user.id),
                "PASSWORD_VERIFIED",
                json!({
                    "ip": addr.ip().to_string(),
                    "user_agent": extract_device_info(&headers)
                }),
                true
            ).await;
            
            track_login_attempt(
                &state.db,
                Some(user.id),
                Some(addr.ip()),
                extract_device_info(&headers),
                request.device_fingerprint.clone(),
                "password_success",
                None
            ).await;

            // ALWAYS SEND OTP (wajib)
            let otp = format!("{:06}", rand::random::<u32>() % 1000000);
            let otp_hash = hash_token(&otp);

            // Store OTP di database
            sqlx::query!(
                r#"
                INSERT INTO login_otps (user_id, otp_hash, expires_at)
                VALUES ($1, $2, NOW() + INTERVAL '5 minutes')
                ON CONFLICT (user_id) DO UPDATE
                SET otp_hash = $2, expires_at = NOW() + INTERVAL '5 minutes'
                "#,
                user.id,
                otp_hash
            )
            .execute(&state.db)
            .await
            .map_err(|e| {
                tracing::error!("Failed to store OTP: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse::new("Failed to generate OTP", Some("OTP_ERROR")))
                )
            })?;

            // Send OTP via email
            let user_email = user.email.clone();
            match EmailService::new().await {
                Ok(service) => {
                    if let Err(e) = service.send_login_otp(&user_email, &otp).await {
                        tracing::error!("Failed to send OTP: {}", e);
                    } else {
                        tracing::info!("OTP sent to {}", user_email);
                    }
                }
                Err(e) => tracing::error!("Email service failed: {}", e)
            }

            // Return OTP response - NO TOKEN
            Ok(Json(AuthResponse {
                success: true,
                message: "Kode OTP telah dikirim ke email Anda. Silakan cek inbox/spam.".to_string(),
                user: None,
                token: None,
                refresh_token: None,
                expires_in: None,
                session_id: None,
                requires_verification: None,
                two_factor_required: Some(true),
            }))
        }
        Ok(false) => {
            log_security_event(
                &state.db,
                Some(user.id),
                "LOGIN_FAILED",
                json!({
                    "ip": addr.ip().to_string(),
                    "reason": "Invalid password"
                }),
                false
            ).await;
            
            track_login_attempt(
                &state.db,
                Some(user.id),
                Some(addr.ip()),
                extract_device_info(&headers),
                request.device_fingerprint.clone(),
                "failed",
                Some("Invalid password")
            ).await;
            
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            
            Err((
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse::new("Kredensial tidak valid", Some("INVALID_CREDENTIALS")))
            ))
        }
        Err(e) => {
            tracing::error!("Password verification error: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Autentikasi gagal", Some("AUTH_ERROR")))
            ))
        }
    }
}

/// Handler untuk verifikasi OTP login
/// POST /api/auth/verify-otp
pub async fn verify_otp(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,  // <- ADD THIS
    headers: HeaderMap,  // <- ADD THIS
    Json(request): Json<VerifyOtpRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    let otp_hash = hash_token(&request.otp);
    
    // Get user and validate OTP
    let result = sqlx::query!(
        r#"
        SELECT o.user_id, o.expires_at, o.used_at, 
               u.id, u.email, u.full_name, u.role, u.email_verified
        FROM login_otps o
        JOIN users u ON u.id = o.user_id
        WHERE u.email = $1 AND o.otp_hash = $2
        "#,
        request.email.to_lowercase(),
        otp_hash
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Database error", Some("DB_ERROR")))
        )
    })?;
    
    let otp_data = result.ok_or((
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse::new("Invalid OTP", Some("INVALID_OTP")))
    ))?;
    
    // Check expiry
    if otp_data.expires_at < Utc::now() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse::new("OTP expired", Some("OTP_EXPIRED")))
        ));
    }
    
    // Check if used
    if otp_data.used_at.is_some() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse::new("OTP already used", Some("OTP_USED")))
        ));
    }
    
    // Mark as used
    sqlx::query!(
        "UPDATE login_otps SET used_at = NOW() WHERE user_id = $1 AND otp_hash = $2",
        otp_data.user_id,
        otp_hash
    )
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to mark OTP as used: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Database error", Some("DB_ERROR")))
        )
    })?;
    
    // Get full user
    let user_repository = UserRepository::new(get_pepper().as_bytes());
    let user = user_repository.find_by_id(&state.db, otp_data.user_id).await
        .map_err(|_| (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new("User not found", Some("USER_NOT_FOUND")))
        ))?;
    
    // Check remember_me dari request
    let remember_me = request.remember_me.unwrap_or(false);
    
    // Generate tokens dengan durasi berdasarkan remember_me
    let (token_pair, token_expiry) = if remember_me {
        let custom_token = state.jwt_service
            .generate_token_with_duration(&user, Duration::days(30))
            .map_err(|_| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Token generation failed", Some("TOKEN_ERROR")))
            ))?;

        (
            TokenPairResponse {
                access_token: custom_token.clone(),
                refresh_token: format!("refresh_{}", custom_token),
                expires_in: 30 * 24 * 3600,
                refresh_expires_in: 30 * 24 * 3600,
            },
            Duration::days(30)
        )
    } else {
        let token_pair = state.jwt_service.generate_token_pair(&user)
            .map_err(|_| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Token generation failed", Some("TOKEN_ERROR")))
            ))?;
        (token_pair, Duration::days(7))
    };
    
    // STORE REFRESH TOKEN IN DATABASE
    let token_hash = hash_token(&token_pair.refresh_token);
    let expires_at = Utc::now() + token_expiry;
    let device_fingerprint = request.device_fingerprint.clone()
        .or_else(|| extract_device_info(&headers))
        .unwrap_or_default();
    
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
    let session_info = SessionInfo {
        device_info: Some(device_fingerprint),
        ip_address: Some(addr.ip()),
    };
    
    let session_token = user_repository.update_last_login_with_session(
        &state.db,
        user.id,
        session_info,
    ).await.unwrap_or_else(|_| Uuid::new_v4().to_string());
    
    // Log successful login
    log_security_event(
        &state.db,
        Some(user.id),
        "OTP_LOGIN_SUCCESS",
        json!({
            "ip": addr.ip().to_string(),
            "user_agent": extract_device_info(&headers)
        }),
        true
    ).await;
    
    let user_profile = UserProfile::from(user);
    
    Ok(Json(AuthResponse {
        success: true,
        message: "Login successful".to_string(),
        user: Some(user_profile),
        token: Some(token_pair.access_token),
        refresh_token: Some(token_pair.refresh_token),
        expires_in: Some(token_pair.expires_in),
        session_id: Some(session_token),
        requires_verification: Some(!otp_data.email_verified.unwrap_or(false)),
        two_factor_required: None,
    }))
}

/// Handler untuk verifikasi JWT token
/// GET /api/auth/verify
pub async fn verify_token(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    let user_repository = UserRepository::new(get_pepper().as_bytes());
    
    match user_repository.find_by_id(&state.db, user_id).await {
        Ok(user) => {
            let user_profile = UserProfile::from(user);
            Ok(Json(AuthResponse::with_user(
                user_profile,
                "Token valid"
            )))
        }
        Err(DatabaseError::UserNotFound) => {
            Ok(Json(AuthResponse::error("User tidak ditemukan atau tidak aktif")))
        }
        Err(DatabaseError::AccountLocked) => {
            Ok(Json(AuthResponse::error("Akun terkunci")))
        }
        Err(e) => {
            tracing::error!("Token verification error: {}", e);
            Ok(Json(AuthResponse::error("Verifikasi token gagal")))
        }
    }
}

// [Lanjutkan dengan fungsi lainnya: refresh_access_token, logout, validate_session, dll]

/// Helper untuk track login attempts
async fn track_login_attempt(
    pool: &sqlx::PgPool,
    user_id: Option<Uuid>,
    ip: Option<std::net::IpAddr>,
    user_agent: Option<String>,
    device_fingerprint: Option<String>,
    status: &str,
    failure_reason: Option<&str>,
) {
    if let Some(uid) = user_id {
        let ip_str = ip.map(|i| i.to_string());
        
        let result = sqlx::query(
            r#"
            INSERT INTO login_history (
                user_id,
                ip_address,
                user_agent,
                device_fingerprint,
                login_status,
                failure_reason
            )
            VALUES ($1, $2::inet, $3, $4, $5, $6)
            "#
        )
        .bind(uid)
        .bind(ip_str.as_deref())
        .bind(user_agent)
        .bind(device_fingerprint)
        .bind(status)
        .bind(failure_reason)
        .execute(pool)
        .await;
        
        if let Err(e) = result {
            tracing::warn!("Failed to track login attempt: {}", e);
        }
    }
}

/// Helper untuk log security event
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

/// Handler untuk refresh access token
/// POST /api/auth/refresh  
#[utoipa::path(
    post,
    path = "/api/auth/refresh",
    request_body = RefreshTokenRequest,
    responses(
        (status = 200, description = "Token refreshed", body = TokenPairResponse),
        (status = 401, description = "Invalid refresh token", body = ErrorResponse),
    ),
    tag = "auth"
)]
pub async fn refresh_access_token(
    State(state): State<AppState>,
    Json(request): Json<RefreshTokenRequest>,
) -> Result<Json<TokenPairResponse>, (StatusCode, Json<ErrorResponse>)> {
    
    // Verify refresh token dengan enhanced claims
    let claims = state.jwt_service
        .verify_token_with_blacklist(&request.refresh_token, &state.db)
        .await
        .map_err(|e| {
            tracing::warn!("Invalid refresh token: {}", e);
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse::new("Invalid or expired refresh token", Some("INVALID_REFRESH_TOKEN")))
            )
        })?;
    
    // Parse user ID
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse::new("Invalid user ID in token", Some("INVALID_USER_ID")))
        ))?;
    
    // Get user dari database
    let user_repository = UserRepository::new(get_pepper().as_bytes());
    let user = user_repository.find_by_id(&state.db, user_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to find user: {}", e);
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new("User tidak ditemukan", Some("USER_NOT_FOUND")))
            )
        })?;
    
    // Get original token expiry from database to preserve remember_me duration
    let old_token_hash = hash_token(&request.refresh_token);
    let original_token_data = sqlx::query!(
        r#"
        SELECT expires_at, created_at
        FROM refresh_tokens
        WHERE token_hash = $1 AND is_revoked = false
        "#,
        old_token_hash
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch original token: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Database error", Some("DB_ERROR")))
        )
    })?
    .ok_or((
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse::new("Refresh token not found or already revoked", Some("TOKEN_NOT_FOUND")))
    ))?;

    // Calculate original token duration
    let created_at = original_token_data.created_at.unwrap_or_else(|| Utc::now());
    let original_duration = original_token_data.expires_at - created_at;
    let is_remember_me = original_duration.num_days() > 14; 

    // Generate new token pair with same duration as original
    let (token_pair, token_expiry) = if is_remember_me {
        let custom_token = state.jwt_service
            .generate_token_with_duration(&user, Duration::days(30))
            .map_err(|e| {
                tracing::error!("Failed to generate token: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse::new("Token generation failed", Some("TOKEN_ERROR")))
                )
            })?;

        (
            TokenPairResponse {
                access_token: custom_token.clone(),
                refresh_token: format!("refresh_{}", custom_token),
                expires_in: 30 * 24 * 3600,
                refresh_expires_in: 30 * 24 * 3600,
            },
            Duration::days(30)
        )
    } else {
        let token_pair = state.jwt_service.generate_token_pair(&user)
            .map_err(|e| {
                tracing::error!("Failed to generate token pair: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse::new("Failed to generate tokens", Some("TOKEN_ERROR")))
                )
            })?;
        (token_pair, Duration::days(7))
    };

    // Store new refresh token hash di database with preserved expiry
    let token_hash = hash_token(&token_pair.refresh_token);
    let expires_at = Utc::now() + token_expiry;
    
    let mut tx = state.db.begin().await
        .map_err(|e| {
            tracing::error!("Failed to begin transaction: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Database error", Some("DB_ERROR")))
            )
        })?;

    // Revoke old refresh token (old_token_hash already defined above)
    sqlx::query!(
        r#"
        UPDATE refresh_tokens 
        SET is_revoked = true, 
            revoked_at = NOW(), 
            revoked_reason = 'Token refreshed'
        WHERE token_hash = $1
        "#,
        old_token_hash
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to revoke old token: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Failed to revoke old token", Some("DB_ERROR")))
        )
    })?;
    
    // Store new refresh token
    sqlx::query!(
        r#"
        INSERT INTO refresh_tokens (user_id, token_hash, device_fingerprint, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
        user_id,
        token_hash,
        request.device_fingerprint,
        expires_at
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to store refresh token: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Failed to store token", Some("DB_ERROR")))
        )
    })?;
    
    tx.commit().await
        .map_err(|e| {
            tracing::error!("Failed to commit transaction: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Transaction failed", Some("DB_ERROR")))
            )
        })?;
    
    tracing::info!("Token refreshed for user {}", user_id);
    
    Ok(Json(token_pair))
}

/// Handler untuk logout
/// POST /api/auth/logout
#[utoipa::path(
    post,
    path = "/api/auth/logout",
    request_body = LogoutRequest,
    responses(
        (status = 200, description = "Logout successful", body = AuthResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
    ),
    tag = "auth",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn logout(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<LogoutRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Revoke refresh token jika ada
    if let Some(refresh_token) = &payload.refresh_token {
        let token_hash = hash_token(refresh_token);
        
        match sqlx::query!(
            r#"
            UPDATE refresh_tokens 
            SET is_revoked = true, 
                revoked_at = NOW(), 
                revoked_reason = 'User logout'
            WHERE token_hash = $1 AND user_id = $2
            "#,
            token_hash,
            user_id
        )
        .execute(&state.db)
        .await {
            Ok(result) => {
                tracing::info!("Revoked {} refresh tokens for user {}", result.rows_affected(), user_id);
            }
            Err(e) => {
                tracing::error!("Failed to revoke refresh token: {}", e);
            }
        }
    }
    
    // Blacklist access token jika ada JTI
    if let Some(access_jti) = &payload.access_token_jti {
        match sqlx::query!(
            r#"
            INSERT INTO token_blacklist (token_jti, user_id, expires_at, reason)
            VALUES ($1, $2, NOW() + INTERVAL '24 hours', 'User logout')
            ON CONFLICT (token_jti) DO NOTHING
            "#,
            access_jti,
            user_id
        )
        .execute(&state.db)
        .await {
            Ok(_) => {
                tracing::info!("Blacklisted access token for user {}", user_id);
            }
            Err(e) => {
                tracing::error!("Failed to blacklist access token: {}", e);
            }
        }
    }
    
    tracing::info!("User {} logged out successfully", user_id);

    log_security_event(
        &state.db,
        Some(user_id),
        "LOGOUT",
        serde_json::json!({
            "timestamp": Utc::now(),
            "session_revoked": payload.refresh_token.is_some()
        }),
        true
    ).await;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Logout berhasil",
        "user_id": user_id.to_string()
    })))
}

/// Handler untuk request password reset
/// POST /api/auth/password-reset/request
pub async fn request_password_reset(
    State(state): State<AppState>,
    Json(request): Json<PasswordResetRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    if let Err(errors) = request.validate() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::validation_error(errors))
        ));
    }
    
    let user = sqlx::query!(
        "SELECT id, email, full_name FROM users WHERE email = $1 AND is_active = true",
        request.email.trim().to_lowercase()
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Database error", Some("DB_ERROR")))
        )
    })?;
    
    if let Some(user) = user {
        let reset_token = format!("reset_{}", Uuid::new_v4());
        let token_hash = hash_token(&reset_token);
        
        sqlx::query!(
            r#"
            INSERT INTO password_reset_tokens (user_id, token_hash, expires_at)
            VALUES ($1, $2, NOW() + INTERVAL '1 hour')
            ON CONFLICT (user_id) DO UPDATE
            SET token_hash = $2, expires_at = NOW() + INTERVAL '1 hour'
            "#,
            user.id,
            token_hash
        )
        .execute(&state.db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to store reset token: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Failed to create reset token", Some("TOKEN_ERROR")))
            )
        })?;

        // SECURITY FIX: Kirim hanya 6 digit code, bukan full token di URL
        // Generate short 6-digit code untuk user input
        let reset_code = &reset_token[reset_token.len()-6..]; // Last 6 chars

        // Send email via proper email service
        match crate::utils::EmailService::new().await {
            Ok(service) => {
                if let Err(e) = service.send_password_reset(&user.email, reset_code).await {
                    tracing::error!("Failed to send password reset email: {}", e);
                } else {
                    tracing::info!("Password reset code sent to {}", user.email);
                }
            }
            Err(e) => {
                tracing::error!("Email service failed: {}", e);
                // For development, log the code
                tracing::warn!("🔐 [DEV ONLY] Reset code for {}: {}", user.email, reset_code);
            }
        }
    }

    Ok(Json(AuthResponse::success(
        "Jika email terdaftar, kode reset password telah dikirim ke email Anda"
    )))
}

/// Handler untuk reset password dengan token
/// POST /api/auth/password-reset/confirm
pub async fn reset_password(
    State(state): State<AppState>,
    Json(request): Json<ResetPasswordRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    if request.new_password != request.confirm_password {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("Password tidak sama", Some("PASSWORD_MISMATCH")))
        ));
    }
    
    if let Err(errors) = request.validate() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::validation_error(errors))
        ));
    }
    
    let token_hash = hash_token(&request.token);
    
    let token_data = sqlx::query!(
        r#"
        SELECT user_id, expires_at, used_at
        FROM password_reset_tokens
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
    })?;
    
    let token_data = token_data.ok_or((
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse::new("Token tidak valid atau expired", Some("INVALID_TOKEN")))
    ))?;
    
    if token_data.used_at.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("Token sudah digunakan", Some("TOKEN_USED")))
        ));
    }
    
    if token_data.expires_at < Utc::now() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("Token sudah expired", Some("TOKEN_EXPIRED")))
        ));
    }
    
    let user_repository = UserRepository::new(get_pepper().as_bytes());
    let new_hash = user_repository.security_service.hash_password(&request.new_password)
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Failed to hash password", Some("HASH_ERROR")))
        ))?;
    
    let mut tx = state.db.begin().await.map_err(|e| {
        tracing::error!("Transaction error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Database error", Some("DB_ERROR")))
        )
    })?;
    
    sqlx::query!(
        r#"
        UPDATE users 
        SET password_hash = $1, 
            last_password_change = NOW(),
            updated_at = NOW()
        WHERE id = $2
        "#,
        new_hash,
        token_data.user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update password: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Failed to update password", Some("UPDATE_ERROR")))
        )
    })?;
    
    sqlx::query!(
        "UPDATE password_reset_tokens SET used_at = NOW() WHERE token_hash = $1",
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
    
    Ok(Json(AuthResponse::success("Password berhasil direset")))
}

/// Handler untuk revoke semua tokens user
/// POST /api/auth/revoke-all
pub async fn revoke_all_tokens(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match sqlx::query!(
        r#"
        UPDATE refresh_tokens 
        SET is_revoked = true, 
            revoked_at = NOW(), 
            revoked_reason = 'Revoke all tokens'
        WHERE user_id = $1 AND is_revoked = false
        "#,
        user_id
    )
    .execute(&state.db)
    .await {
        Ok(result) => {
            let revoked_count = result.rows_affected();
            tracing::info!("Revoked {} refresh tokens for user {}", revoked_count, user_id);
            
            Ok(Json(serde_json::json!({
                "success": true,
                "message": format!("Berhasil revoke {} token", revoked_count),
                "revoked_count": revoked_count
            })))
        }
        Err(e) => {
            tracing::error!("Failed to revoke tokens for user {}: {}", user_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Gagal revoke tokens", Some("DATABASE_ERROR")))
            ))
        }
    }
}

/// Handler untuk validasi session
/// GET/POST /api/auth/session/validate
pub async fn validate_session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    let session_token = headers
        .get("X-Session-Token")
        .and_then(|h| h.to_str().ok())
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse::new("Missing session token", Some("NO_SESSION")))
        ))?;
    
    let user_repository = UserRepository::new(get_pepper().as_bytes());
    
    match user_repository.validate_session_and_get_user(&state.db, session_token).await {
        Ok(user) => {
            let user_profile = UserProfile::from(user);
            Ok(Json(AuthResponse::with_user(user_profile, "Session valid")))
        }
        Err(DatabaseError::InvalidSession) => {
            Ok(Json(AuthResponse::error("Session invalid atau expired")))
        }
        Err(e) => {
            tracing::error!("Session validation error: {}", e);
            Ok(Json(AuthResponse::error("Gagal validasi session")))
        }
    }
}

// Removed: Old send_email function replaced with EmailService