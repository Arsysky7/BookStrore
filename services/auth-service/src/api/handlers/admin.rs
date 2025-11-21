// auth-service/src/api/handlers/admin.rs

use axum::{
    extract::{State, Query, Path},
    http::StatusCode,
    response::Json,
    Extension,
};
use uuid::Uuid;
use std::collections::HashMap;

use crate::{
    AppState,
    models::*,
    db::{UserRepository, DatabaseError},
    utils::get_pepper, 
};

/// Handler untuk mendapatkan statistik user (admin only)
/// GET /api/admin/users/stats
pub async fn get_admin_user_stats(
    State(state): State<AppState>,
    Extension(user_role): Extension<String>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Cek akses admin
    if user_role != "admin" {
        tracing::warn!("Non-admin user {} attempted to access admin stats", user_id);
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse::new("Akses admin diperlukan", Some("INSUFFICIENT_PRIVILEGES")))
        ));
    }

    let user_repository = UserRepository::new(get_pepper().as_bytes());
    
    match user_repository.get_admin_user_stats(&state.db).await {
        Ok(stats) => {
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "Statistik user berhasil diambil",
                "data": stats
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get admin user stats: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Gagal mengambil statistik", Some("DATABASE_ERROR")))
            ))
        }
    }
}

/// Handler untuk mendapatkan daftar user dengan paginasi (admin only)
/// GET /api/admin/users
pub async fn get_admin_users(
    State(state): State<AppState>,
    Extension(user_role): Extension<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Cek akses admin
    if user_role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse::new("Akses admin diperlukan", Some("INSUFFICIENT_PRIVILEGES")))
        ));
    }

    let page = params.get("page").and_then(|p| p.parse::<u32>().ok()).unwrap_or(1);
    let limit = params.get("limit").and_then(|l| l.parse::<u32>().ok()).unwrap_or(10);
    let search = params.get("search").map(|s| s.as_str());
    let role_filter = params.get("role").map(|r| r.as_str());

    let user_repository = UserRepository::new(get_pepper().as_bytes());

    match user_repository.get_admin_users_list(&state.db, page, limit, search, role_filter).await {
        Ok((mut users, pagination)) => {
            // Enrich dengan data dari payment service
            for user in &mut users {
                if let Ok(stats) = state.service_client.get_user_order_stats(user.id).await {
                    user.order_count = stats.total_orders;
                    user.total_spent = Some(format!("{:.2}", stats.total_spent));
                    user.last_purchase = stats.last_purchase;
                }
            }
            
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "Users berhasil diambil dengan order data",
                "data": users,
                "pagination": pagination
            })))
        }
        Err(e) => {
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(&format!("Gagal mendapatkan users: {}", e), Some("DATABASE_ERROR")))
            ))
        }
    }
}

/// Handler untuk mendapatkan feed aktivitas user (admin only)
/// GET /api/admin/users/activity
pub async fn get_admin_activity_feed(
    State(state): State<AppState>,
    Extension(user_role): Extension<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Cek akses admin
    if user_role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse::new("Akses admin diperlukan", Some("INSUFFICIENT_PRIVILEGES")))
        ));
    }

    let limit = params.get("limit").and_then(|l| l.parse::<u32>().ok()).unwrap_or(20);
    let activity_type = params.get("type").map(|t| t.as_str());

    let user_repository = UserRepository::new(get_pepper().as_bytes());

    match user_repository.get_user_activity_feed(&state.db, limit, activity_type).await {
        Ok(activities) => {
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "Feed aktivitas berhasil diambil",
                "data": activities
            })))
        }
        Err(e) => {
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(&format!("Gagal mendapatkan feed aktivitas: {}", e), Some("DATABASE_ERROR")))
            ))
        }
    }
}

/// Handler untuk mendapatkan security activity feed (admin only)
/// GET /api/admin/security/activity
pub async fn get_security_activity_feed(
    State(state): State<AppState>,
    Extension(user_role): Extension<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Cek akses admin
    if user_role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse::new("Akses admin diperlukan", Some("INSUFFICIENT_PRIVILEGES")))
        ));
    }
    
    let limit = params.get("limit")
        .and_then(|l| l.parse::<u32>().ok())
        .unwrap_or(50)
        .min(100);
    
    let severity_filter = params.get("severity").map(|s| s.as_str());
    
    let user_filter = params.get("user_id")
        .and_then(|id| Uuid::parse_str(id).ok());
    
    let user_repository = UserRepository::new(get_pepper().as_bytes());
    
    match user_repository.get_security_activity_feed(
        &state.db,
        limit,
        severity_filter,
        user_filter
    ).await {
        Ok(activities) => {
            // Group by severity untuk stats
            let stats = serde_json::json!({
                "total": activities.len(),
                "info": activities.iter().filter(|a| matches!(a.severity, ActivitySeverity::Info)).count(),
                "warning": activities.iter().filter(|a| matches!(a.severity, ActivitySeverity::Warning)).count(),
                "critical": activities.iter().filter(|a| matches!(a.severity, ActivitySeverity::Critical)).count(),
                "security": activities.iter().filter(|a| matches!(a.severity, ActivitySeverity::Security)).count(),
            });
            
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "Security activities retrieved",
                "stats": stats,
                "data": activities
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get security activities: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Failed to get security activities", Some("DATABASE_ERROR")))
            ))
        }
    }
}

/// Handler untuk update status user (admin only)
/// PUT /api/admin/users/:id/status
pub async fn admin_update_user_status(
    State(state): State<AppState>,
    Extension(_user_role): Extension<String>,
    Extension(admin_user_id): Extension<Uuid>,
    Path(target_user_id): Path<Uuid>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let user_repository = UserRepository::new(get_pepper().as_bytes());
    
    // Verify admin access
    if let Err(e) = user_repository.check_user_access(&state.db, admin_user_id, Some("admin")).await {
        return match e {
            DatabaseError::AdminAccessDenied => Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponse::new("Admin access required", Some("ADMIN_ACCESS_DENIED")))
            )),
            DatabaseError::AccessDenied => Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponse::new("Account tidak aktif", Some("ACCOUNT_INACTIVE")))
            )),
            _ => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Access check failed", Some("ACCESS_CHECK_ERROR")))
            ))
        };
    }
    
    // Extract is_active dari payload
    let is_active = payload["is_active"].as_bool()
        .ok_or((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("Field is_active diperlukan", Some("MISSING_FIELD")))
        ))?;
    
    // Update user status di database
    let update_result = sqlx::query!(
        r#"
        UPDATE users
        SET is_active = $1, updated_at = NOW()
        WHERE id = $2
        RETURNING id, email, full_name, is_active
        "#,
        is_active,
        target_user_id
    )
    .fetch_optional(&state.db)
    .await;
    
    match update_result {
        Ok(Some(updated_user)) => {
            // Log admin action untuk audit
            let _ = log_security_event(
                &state.db,
                Some(admin_user_id),
                if is_active { "USER_ACTIVATED_BY_ADMIN" } else { "USER_DEACTIVATED_BY_ADMIN" },
                serde_json::json!({
                    "admin_id": admin_user_id,
                    "target_user_id": target_user_id,
                    "target_email": updated_user.email,
                    "new_status": is_active
                }),
                true
            ).await;
            
            Ok(Json(serde_json::json!({
                "success": true,
                "message": format!(
                    "User {} berhasil {}",
                    updated_user.email,
                    if is_active { "diaktifkan" } else { "dinonaktifkan" }
                ),
                "data": {
                    "id": updated_user.id,
                    "email": updated_user.email,
                    "full_name": updated_user.full_name,
                    "is_active": updated_user.is_active
                }
            })))
        }
        Ok(None) => {
            Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new("User tidak ditemukan", Some("USER_NOT_FOUND")))
            ))
        }
        Err(e) => {
            tracing::error!("Failed to update user status: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Gagal update status user", Some("DATABASE_ERROR")))
            ))
        }
    }
}

// Helper function
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