// /pdf-bookstore/services/auth-service/src/api/handlers/internal.rs

use axum::{
    extract::{State, Path},
    http::{StatusCode, HeaderMap},
    response::Json,
    Extension,
};
use uuid::Uuid;

use crate::{
    AppState,
    models::*,
    db::UserRepository,
    utils::get_pepper, 
};

/// Handler untuk verifikasi user dari service lain (internal use)
/// GET /api/internal/users/:id
pub async fn verify_user_internal(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let user_repository = UserRepository::new(get_pepper().as_bytes());
    
    match user_repository.find_by_id(&state.db, user_id).await {
        Ok(user) => {
            Ok(Json(serde_json::json!({
                "success": true,
                "user": {
                    "id": user.id,
                    "email": user.email,
                    "full_name": user.full_name,
                    "role": user.role,
                    "is_active": user.is_active,
                    "email_verified": user.email_verified
                }
            })))
        }
        Err(_) => {
            Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new("User tidak ditemukan", Some("USER_NOT_FOUND")))
            ))
        }
    }
}

/// Handler untuk mendapatkan user data untuk payment service
/// GET /api/internal/users/:id/payment
pub async fn get_user_for_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Verify internal service call dengan API key
    let service_key = headers
        .get("X-Service-Key")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    
    let expected_key = std::env::var("INTERNAL_SERVICE_KEY")
        .unwrap_or_else(|_| "internal-service-key-secret".to_string());
    
    if service_key != expected_key {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse::new("Unauthorized service call", Some("INVALID_SERVICE_KEY")))
        ));
    }
    
    let user_repository = UserRepository::new(get_pepper().as_bytes());
    
    match user_repository.find_by_id(&state.db, user_id).await {
        Ok(user) => {
            Ok(Json(serde_json::json!({
                "success": true,
                "user": {
                    "id": user.id,
                    "email": user.email,
                    "name": user.full_name,
                    "phone": None::<String>,
                }
            })))
        }
        Err(_) => {
            Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new("User tidak ditemukan", Some("USER_NOT_FOUND")))
            ))
        }
    }
}

/// Handler untuk validasi token internal antar service
/// POST /api/internal/validate-token
pub async fn validate_token_internal(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse::new("Missing token", Some("NO_TOKEN")))
        ))?;
    
    let claims = state.jwt_service.verify_token(token)
        .map_err(|_| (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse::new("Invalid token", Some("INVALID_TOKEN")))
        ))?;
    
    Ok(Json(serde_json::json!({
        "valid": true,
        "user_id": claims.sub,
        "role": claims.role,
        "email": claims.email,
    })))
}

/// Handler untuk cek akses user terhadap buku tertentu
/// GET /api/users/books/:book_id/access
pub async fn check_user_book_access(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Path(book_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Check ownership via payment service
    let has_access = state.service_client
        .check_book_ownership(user_id, book_id)
        .await
        .unwrap_or(false);
    
    // Get book details jika user punya akses
    let book_details = if has_access {
        state.service_client
            .get_book_details(book_id)
            .await
            .ok()
    } else {
        None
    };
    
    Ok(Json(serde_json::json!({
        "success": true,
        "has_access": has_access,
        "user_id": user_id,
        "book_id": book_id,
        "book": book_details
    })))
}

/// Handler untuk mendapatkan complete profile user dengan data dari services lain
/// GET /api/users/complete-profile
pub async fn get_user_complete_profile(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let user_repository = UserRepository::new(get_pepper().as_bytes());
    
    // Get user data
    let user = user_repository.find_by_id(&state.db, user_id)
        .await
        .map_err(|_| (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new("User tidak ditemukan", Some("USER_NOT_FOUND")))
        ))?;
    
    // Get purchases dari payment service
    let purchases = state.service_client
        .get_user_purchases(user_id, Some(10))
        .await
        .unwrap_or_default();
    
    // Get downloaded books dari book service
    let downloads = state.service_client
        .get_user_downloaded_books(user_id)
        .await
        .unwrap_or_default();
    
    // Get order stats
    let order_stats = state.service_client
        .get_user_order_stats(user_id)
        .await
        .unwrap_or_default();
    
    Ok(Json(serde_json::json!({
        "success": true,
        "user": {
            "id": user.id,
            "email": user.email,
            "full_name": user.full_name,
            "role": user.role,
            "email_verified": user.email_verified,
            "created_at": user.created_at,
        },
        "stats": {
            "total_orders": order_stats.total_orders,
            "total_spent": order_stats.total_spent,
            "last_purchase": order_stats.last_purchase,
            "total_downloads": downloads.len(),
        },
        "recent_purchases": purchases,
        "downloaded_books": downloads
    })))
}