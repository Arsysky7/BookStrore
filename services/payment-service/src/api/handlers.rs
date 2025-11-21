// /pdf-bookstore/services/payment-service/src/api/handlers.rs

use axum::{
    extract::{State, Path, Query},
    http::StatusCode,
    response::Json,
    Extension,
};

use bigdecimal::BigDecimal;
use std::str::FromStr;
use uuid::Uuid;
use validator::Validate;
use std::sync::Arc;

use crate::{
    models::*,
    AppState,
    utils::{
        error::{AppError, AppResult},
        validator as utils_validator,
        validator::validate_positive_amount,
        scheduler::trigger_maintenance_job,
        cache::CacheManager,
        {DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE},
        scheduler::SchedulerMetrics,
    },
};

// ========================= ORDER HANDLERS =========================

/// Handler untuk membuat order baru
/// POST /api/orders
pub async fn create_order(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<CreateOrderRequest>,
) -> AppResult<Json<OrderResponse>> {
    // Validasi input dengan enhanced validation
    payload.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    // Parse dan validasi book_id
    let book_id = utils_validator::validate_uuid(&payload.book_id, "book_id")?;
    let book_details = get_book_details(&state, book_id).await?;

    // Validasi payment method
    utils_validator::validate_payment_method(&payload.payment_method)?;
    validate_positive_amount(&book_details.price, "book price")?;
    
    // Validasi idempotency key jika ada
    if let Some(ref idempotency_key) = payload.idempotency_key {
        utils_validator::validate_idempotency_key(idempotency_key)?;
        
        // Check idempotency untuk prevent duplicate order
        if let Some(existing_order) = state.repository
            .order()
            .find_by_idempotency_key(idempotency_key)
            .await? 
        {
            tracing::info!("Idempotent order request detected for key: {}", idempotency_key);
            return Ok(Json(OrderResponse {
                success: true,
                message: "Order sudah ada (idempotent)".to_string(),
                data: Some(existing_order),
            }));
        }
    }
    
    // Process order melalui service layer
    let order = state.payment_service
        .create_order(user_id, book_id, payload.payment_method, payload.idempotency_key)
        .await?;
    
    //  Invalidate admin stats cache karena ada order baru
    if let Err(e) = state.cache_manager.delete("admin:order_stats").await {
        tracing::warn!("Failed to invalidate admin stats cache: {}", e);
    }
    
    tracing::info!("New order created: {} for user: {}", order.order.order_number, user_id);
    
    Ok(Json(OrderResponse {
        success: true,
        message: "Order berhasil dibuat".to_string(),
        data: Some(order),
    }))
}

// helper function untuk mendapatkan detail book
async fn get_book_details(
    state: &AppState,
    book_id: Uuid,
) -> AppResult<BookDetails> {
    let response = state.http_client
        .get(format!("{}/api/books/{}", 
            std::env::var("BOOK_SERVICE_URL").unwrap_or_else(|_| "http://book-service:3002".to_string()),
            book_id
        ))
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err(AppError::NotFound("Book tidak ditemukan".to_string()));
    }
    
    let data: serde_json::Value = response.json().await?;
    
    let book_obj = data["data"]["book"].as_object()
        .ok_or_else(|| AppError::ExternalService("Invalid book response format".to_string()))?;
    
    Ok(BookDetails {
        title: book_obj["title"].as_str().unwrap_or("Unknown").to_string(),
        author: book_obj["author"].as_str().unwrap_or("Unknown").to_string(),
        price: BigDecimal::from_str(
            book_obj["price"].as_str().unwrap_or("0")
        ).unwrap_or_else(|_| BigDecimal::from(0)),
    })
}

/// Handler untuk mendapatkan detail order
/// GET /api/orders/{id}
pub async fn get_order(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
    Extension(user_id): Extension<Uuid>,
) -> AppResult<Json<OrderResponse>> {
    // CEK CACHE DULU
    let cache_key = format!("order:{}:{}", user_id, order_id);
    
    // Coba ambil dari cache
    if let Ok(Some(cached_order)) = state.cache_manager
        .get::<OrderWithDetails>(&cache_key).await 
    {
        tracing::debug!("Order {} retrieved from cache", order_id);
        return Ok(Json(OrderResponse {
            success: true,
            message: "Order berhasil diambil (cached)".to_string(),
            data: Some(cached_order),
        }));
    }
    
    // Jika tidak ada di cache, ambil dari database
    let order = state.repository
        .order()
        .find_by_id(order_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Order tidak ditemukan".to_string()))?;
    
    // Verifikasi ownership
    if order.order.user_id != Some(user_id) {
        tracing::warn!("User {} attempted to access order {} owned by {:?}", 
            user_id, order_id, order.order.user_id);
        return Err(AppError::Forbidden("Akses ditolak untuk order ini".to_string()));
    }
    
    // SIMPAN KE CACHE untuk request berikutnya
    if let Err(e) = state.cache_manager
        .set(&cache_key, &order, 300).await 
    {
        tracing::warn!("Failed to cache order {}: {}", order_id, e);
    }
    
    tracing::debug!("Order {} retrieved from database and cached", order_id);
    
    
    if let Some(ref payment_method) = order.order.payment_method {
        // Convert ke enum dan get display name
        let method_enum = match payment_method.as_str() {
            "credit_card" => PaymentMethod::CreditCard,
            "bank_transfer" => PaymentMethod::BankTransfer,
            "e_wallet" => PaymentMethod::EWallet,
            "qris" => PaymentMethod::Qris,
            "convenience_store" => PaymentMethod::ConvenienceStore,
            _ => PaymentMethod::Qris,
        };
        
        // Log display name untuk monitoring
        tracing::debug!("Payment method: {} ({})", 
            payment_method, 
            method_enum.display_name()
        );
    }
    
    Ok(Json(OrderResponse {
        success: true,
        message: "Order berhasil diambil".to_string(),
        data: Some(order),
    }))
}

/// Handler untuk list orders user
/// GET /api/orders
pub async fn list_orders(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Query(params): Query<OrderQueryParams>,
) -> AppResult<Json<OrdersListResponse>> {
    // Validate dan sanitize pagination
    let page = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE)
        .min(MAX_PAGE_SIZE);
    let (validated_page, validated_limit) = utils_validator::validate_pagination(page, limit)?;
    
    // Validate status filter jika ada
    if let Some(ref status) = params.status {
        utils_validator::validate_order_status(status)?;
    }
    
    // Validate payment method filter jika ada
    if let Some(ref payment_method) = params.payment_method {
        utils_validator::validate_payment_method(payment_method)?;
    }
    
    // Get orders dari repository
    let (orders, total) = state.repository
        .order()
        .find_by_user(user_id, validated_page, validated_limit, params)
        .await?;
    
    let pagination = PaginationMeta::new(validated_page, validated_limit, total);
    
    tracing::debug!("Listed {} orders for user {} (page {}/{})", 
        orders.len(), user_id, validated_page, pagination.total_pages);
    
    Ok(Json(OrdersListResponse {
        success: true,
        message: "Orders berhasil diambil".to_string(),
        data: orders,
        pagination: Some(pagination),
    }))
}

/// Handler untuk cancel order
/// PUT /api/orders/{id}/cancel
pub async fn cancel_order(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
    Extension(user_id): Extension<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    // Get order untuk verify ownership dan status
    let order = state.repository
        .order()
        .find_by_id(order_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Order tidak ditemukan".to_string()))?;
    
    // Verify ownership
    if order.order.user_id != Some(user_id) {
        tracing::warn!("User {} attempted to cancel order {} owned by {:?}", 
            user_id, order_id, order.order.user_id);
        return Err(AppError::Forbidden("Akses ditolak untuk order ini".to_string()));
    }
    
    // Verify status - hanya pending yang bisa dibatalkan
    if order.order.status != "pending" {
        return Err(AppError::BadRequest(
            format!("Order dengan status '{}' tidak bisa dibatalkan", order.order.status)
        ));
    }
    
    // Check apakah order sudah expired
    if let Some(expires_at) = order.order.expires_at {
        if expires_at < chrono::Utc::now() {
            return Err(AppError::BadRequest("Order sudah expired dan tidak bisa dibatalkan".to_string()));
        }
    }
    
    // Cancel order melalui service
    state.payment_service
        .cancel_order(order_id, user_id)
        .await?;
    
    invalidate_order_cache(&state.cache_manager, user_id, order_id).await;
    
    tracing::info!("Order {} cancelled by user {}", order.order.order_number, user_id);
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Order berhasil dibatalkan",
        "order_id": order_id,
        "cancelled_at": chrono::Utc::now()
    })))
}

// fungsi untuk invalidate cache saat order berubah
async fn invalidate_order_cache(
    cache_manager: &Arc<CacheManager>,
    user_id: Uuid,
    order_id: Uuid,
) {
    let cache_key = format!("order:{}:{}", user_id, order_id);
    
    if let Err(e) = cache_manager.delete(&cache_key).await {
        tracing::warn!("Failed to invalidate cache for order {}: {}", order_id, e);
    }
    
    // Invalidate juga admin stats karena data berubah
    if let Err(e) = cache_manager.delete("admin:order_stats").await {
        tracing::warn!("Failed to invalidate admin stats cache: {}", e);
    }
}

/// Handler untuk request refund
/// POST /api/orders/{id}/refund
pub async fn request_refund(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<RefundRequest>,
) -> AppResult<Json<serde_json::Value>> {
    // Validate refund request
    payload.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;
    
    // Get order untuk verify
    let order = state.repository
        .order()
        .find_by_id(order_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Order tidak ditemukan".to_string()))?;
    
    // Verify ownership
    if order.order.user_id != Some(user_id) {
        return Err(AppError::Forbidden("Tidak bisa refund order orang lain".to_string()));
    }

    if let Some(ref reason) = payload.reason {
        utils_validator::validate_string_length(reason, "refund reason", 10, 500)?;
    }
    
    // Verify status - hanya yang paid bisa di-refund
    if order.order.status != "paid" {
        return Err(AppError::BadRequest(
            format!("Order dengan status '{}' tidak bisa di-refund", order.order.status)
        ));
    }
    
    // Check apakah sudah pernah di-refund
    let existing_refund = state.repository
        .payment()
        .get_refund_by_order_id(order_id)
        .await?;
    
    if existing_refund.is_some() {
        return Err(AppError::Conflict("Order sudah pernah di-refund".to_string()));
    }

    let refund_amount = if let Some(requested_amount) = payload.amount {
        if requested_amount > order.order.amount {
            return Err(AppError::BadRequest("Jumlah refund melebihi nilai order".to_string()));
        }
        requested_amount
    } else {
        order.order.amount.clone() 
    };

    if let Some(ref bank_acc) = payload.bank_account {
        tracing::info!("Refund akan dikirim ke rekening: {}", bank_acc);
    }

    let refund_reason = payload.reason.as_ref()
        .map(|r| r.as_str())
        .unwrap_or("Customer request");
    
    // Process refund melalui Midtrans
    let refund_result = state.midtrans_service
        .process_refund(
            &order.order.midtrans_order_id.unwrap_or_default(),
            refund_amount.clone(), 
            refund_reason
        )
        .await?;
    
    // Simpan refund record
    let refund_id = state.repository
        .payment()
        .create_refund(
            order_id,
            refund_amount.clone(),
            payload.reason, 
            Some(user_id),
            refund_result.refund_id
        )
        .await?;
    
    // Update order status
    state.repository
        .order()
        .update_status_simple(order_id, "refunded")
        .await?;
    
    tracing::info!("Refund processed for order {} by user {}", order_id, user_id);
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Refund berhasil diproses",
        "refund_id": refund_id,
        "status": "processing",
        "estimated_days": "3-7", 
        "refund_amount": refund_amount.to_string() 
    })))
}

pub async fn get_scheduler_status(
    Extension(user_role): Extension<String>,
) -> AppResult<Json<serde_json::Value>> {
    // Admin only
    if user_role != "admin" {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }
    
    let metrics = SchedulerMetrics::new();
    let status = metrics.get_status().await;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "data": status
    })))
}

/// Get Midtrans client key untuk frontend
/// GET /api/payment/config
pub async fn get_payment_config(
    State(state): State<AppState>,
    Extension(_user_id): Extension<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "success": true,
        "client_key": state.midtrans_service.get_client_key(),
        "is_production": std::env::var("MIDTRANS_IS_PRODUCTION")
            .unwrap_or_else(|_| "false".to_string()) == "true"
    })))
}

// ========================= WEBHOOK HANDLERS =========================

/// Handler untuk Midtrans webhook
/// POST /api/webhook/midtrans
pub async fn handle_midtrans_webhook(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<MidtransWebhookPayload>,
) -> AppResult<Json<serde_json::Value>> {
    // Validate transaction ID
    utils_validator::validate_transaction_id(&payload.transaction_id)?;
    
    // Extract signature dari header dengan validation
    let signature_key = headers
        .get("x-signature-key")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("Webhook received without signature key");
            AppError::BadRequest("Signature key tidak ada".to_string())
        })?;
    
    // Log webhook untuk monitoring
    tracing::info!("Processing webhook for transaction: {} with status: {}", 
        payload.transaction_id, payload.transaction_status);
    
    // Process webhook melalui service
    state.payment_service
        .process_webhook(&payload, signature_key)
        .await?;
    
    tracing::info!("Webhook processed successfully for transaction: {}", payload.transaction_id);
    
    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "Webhook berhasil diproses",
        "transaction_id": payload.transaction_id,
        "processed_at": chrono::Utc::now()
    })))
}

// ========================= PURCHASE STATUS HANDLERS =========================

/// Handler untuk check status pembelian book
/// GET /api/purchases/{book_id}
pub async fn check_purchase_status(
    State(state): State<AppState>,
    Path(book_id): Path<Uuid>,
    Extension(user_id): Extension<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    // Check purchase status
    let has_purchased = state.repository
        .payment()
        .has_user_purchased_book(user_id, book_id)
        .await?;
    
    tracing::debug!("Purchase status check: user {} book {} = {}", 
        user_id, book_id, has_purchased);
    
    Ok(Json(serde_json::json!({
        "success": true,
        "user_id": user_id,
        "book_id": book_id,
        "has_purchased": has_purchased,
        "message": if has_purchased {
            "Book sudah dibeli dan tersedia untuk download"
        } else {
            "Book belum dibeli"
        },
        "checked_at": chrono::Utc::now()
    })))
}

/// Trigger maintenance job manually
/// POST /api/admin/maintenance/trigger
pub async fn trigger_maintenance(
    State(state): State<AppState>,
    Extension(user_role): Extension<String>,
) -> AppResult<Json<serde_json::Value>> {
    if user_role != "admin" {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }
    
    // Trigger maintenance
    trigger_maintenance_job(state.repository.clone()).await?;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Maintenance job triggered successfully",
        "timestamp": chrono::Utc::now()
    })))
}
// ========================= ADMIN HANDLERS =========================

/// Handler untuk admin order statistics
/// GET /api/admin/orders/stats
pub async fn get_admin_order_stats(
    State(state): State<AppState>,
    Extension(user_role): Extension<String>,
) -> AppResult<Json<serde_json::Value>> {
    // Verifikasi admin role
    if user_role != "admin" {
        return Err(AppError::Forbidden("Akses admin diperlukan".to_string()));
    }
    
    // CEK CACHE untuk admin stats (cache lebih lama karena tidak sering berubah)
    let cache_key = "admin:order_stats";
    
    if let Ok(Some(cached_stats)) = state.cache_manager
        .get::<serde_json::Value>(cache_key).await
    {
        tracing::debug!("Admin stats retrieved from cache");
        return Ok(Json(cached_stats));
    }
    
    // Get stats dari repository
    let stats = state.repository
        .order()
        .get_admin_stats()
        .await?;
    
    tracing::info!("Admin stats requested: {} total orders, {} revenue", 
        stats.total_orders, stats.total_revenue);
    
    let response = serde_json::json!({
        "success": true,
        "message": "Order statistics berhasil diambil",
        "data": stats,
        "generated_at": chrono::Utc::now()
    });
    
    // CACHE admin stats selama 15 menit
    if let Err(e) = state.cache_manager
        .set(cache_key, &response, 900).await
    {
        tracing::warn!("Failed to cache admin stats: {}", e);
    }
    
    Ok(Json(response))
}

// Handler untuk update order status (Admin only)
/// PUT /api/admin/orders/{id}/status
pub async fn admin_update_order_status(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
    Extension(user_role): Extension<String>,
    Extension(admin_id): Extension<Uuid>,
    Json(payload): Json<UpdateOrderRequest>,
) -> AppResult<Json<serde_json::Value>> {
    // Verify admin role
    if user_role != "admin" {
        return Err(AppError::Forbidden("Akses admin diperlukan".to_string()));
    }
    
    // Validate request
    payload.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;
    
    // Get order untuk verify exists
    let order = state.repository
        .order()
        .find_by_id(order_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Order tidak ditemukan".to_string()))?;
    
    // Update status jika ada
    if let Some(ref new_status) = payload.status {
        // Validate status transition
        if !is_valid_status_transition(&order.order.status, new_status) {
            return Err(AppError::BadRequest(
                format!("Transisi status tidak valid: {} -> {}", order.order.status, new_status)
            ));
        }
        
        // Update status di database
        state.repository
            .order()
            .update_status_simple(order_id, new_status)
            .await?;
        
        tracing::info!("Admin {} updated order {} status to {}", admin_id, order_id, new_status);
    }
    
    // Add notes jika ada
    if let Some(ref notes) = payload.notes {
        // Log audit dengan notes
        sqlx::query!(
            r#"
            INSERT INTO audit_logs (user_id, action, resource_type, resource_id, details)
            VALUES ($1, 'ORDER_STATUS_UPDATED', 'order', $2, $3)
            "#,
            admin_id,
            order_id,
            serde_json::json!({
                "notes": notes,
                "old_status": order.order.status,
                "new_status": payload.status
            })
        )
        .execute(state.repository.get_pool())
        .await?;
    }

    // Validate notes length jika ada
    if let Some(ref notes) = payload.notes {
        utils_validator::validate_string_length(notes, "notes", 1, 500)?;
    }
    
    // Invalidate cache
    let cache_key = format!("order:*:{}", order_id);
    if let Err(e) = state.cache_manager.invalidate_pattern(&cache_key).await {
        tracing::warn!("Failed to invalidate order cache: {}", e);
    }
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Order berhasil diupdate",
        "order_id": order_id,
        "updated_status": payload.status,
        "notes": payload.notes,
        "updated_by": admin_id,
        "updated_at": chrono::Utc::now()
    })))
}

// Helper function untuk validate status transition
fn is_valid_status_transition(current: &str, new: &str) -> bool {
    match (current, new) {
        // Pending bisa ke paid, failed, cancelled, expired
        ("pending", "paid") | ("pending", "failed") | 
        ("pending", "cancelled") | ("pending", "expired") => true,
        
        // Paid bisa ke refunded
        ("paid", "refunded") => true,
        
        // Failed dan cancelled bisa retry ke pending (untuk admin)
        ("failed", "pending") | ("cancelled", "pending") => true,
        
        // Expired bisa di-reactivate ke pending (untuk admin)
        ("expired", "pending") => true,
        
        // Sama status = no-op tapi valid
        (a, b) if a == b => true,
        
        // Lainnya invalid
        _ => false,
    }
}

/// Handler untuk revenue analytics
/// GET /api/admin/analytics/revenue
pub async fn get_revenue_analytics(
    State(state): State<AppState>,
    Extension(user_role): Extension<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> AppResult<Json<serde_json::Value>> {
    // Verify admin role
    if user_role != "admin" {
        return Err(AppError::Forbidden("Akses admin diperlukan".to_string()));
    }
    
    // Parse dan validate parameters
    let period = params.get("period")
        .map(|p| p.as_str())
        .unwrap_or("monthly");
    
    utils_validator::validate_analytics_period(period)?;
    
    let days = params.get("days")
        .and_then(|d| d.parse::<u32>().ok())
        .unwrap_or(30);
    
    let validated_days = utils_validator::validate_days_range(days)?;
    
    // Get analytics dari repository
    let analytics = state.repository
        .order()
        .get_revenue_analytics(period, validated_days)
        .await?;
    
    tracing::info!("Revenue analytics requested: {} period, {} days", period, validated_days);
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Revenue analytics berhasil diambil",
        "data": analytics,
        "parameters": {
            "period": period,
            "days": validated_days
        },
        "generated_at": chrono::Utc::now()
    })))
}

/// Handler untuk recent orders (admin) dengan flexible filtering
/// GET /api/admin/orders/recent
pub async fn get_recent_orders_admin(
    State(state): State<AppState>,
    Extension(user_role): Extension<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> AppResult<Json<OrdersListResponse>> {
    // Verify admin role
    if user_role != "admin" {
        return Err(AppError::Forbidden("Akses admin diperlukan".to_string()));
    }
    
    // Parse dan validate parameters
    let limit = params.get("limit")
        .and_then(|l| l.parse::<u32>().ok())
        .unwrap_or(10)
        .min(50); // Cap at 50 for performance
    
    let status_filter = params.get("status")
        .map(|s| s.as_str());
    
    // Validate status filter jika ada
    if let Some(status) = status_filter {
        utils_validator::validate_order_status(status)?;
    }
    
    // Get orders dari repository
    let orders = state.repository
        .order()
        .get_recent_orders(limit, status_filter)
        .await?;
    
    tracing::info!("Recent orders requested by admin: {} orders, filter: {:?}", 
        orders.len(), status_filter);
    
    Ok(Json(OrdersListResponse {
        success: true,
        message: "Recent orders berhasil diambil".to_string(),
        data: orders,
        pagination: None, // No pagination for recent orders
    }))
}

/// Handler untuk system health check (Admin only)
/// GET /api/admin/system/health
pub async fn get_system_health(
    State(state): State<AppState>,
    Extension(user_role): Extension<String>,
) -> AppResult<Json<serde_json::Value>> {
    // Verify admin role
    if user_role != "admin" {
        return Err(AppError::Forbidden("Akses admin diperlukan".to_string()));
    }
    
    // Check database connectivity
    let database_connected = match sqlx::query("SELECT 1")
        .fetch_one(state.repository.get_pool())
        .await {
        Ok(_) => true,
        Err(_) => false,
    };
    
    // Get system stats
    let stats = if database_connected {
        Some(state.repository.order().get_admin_stats().await?)
    } else {
        None
    };
    
    // Get cache status
    let cache_stats = state.cache_manager.get_stats().await;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "data": {
            "database_connected": database_connected,
            "cache_status": cache_stats,
            "order_stats": stats,
            "timestamp": chrono::Utc::now(),
            "service_version": env!("CARGO_PKG_VERSION"),
        }
    })))
}

// Handler untuk comprehensive health check
pub async fn comprehensive_health_check_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let health_result = crate::utils::health::comprehensive_health_check(
        &state.repository,
        &state.cache_manager,
        &state.circuit_manager,
        &state.service_registry,
    ).await;
    
    Ok(Json(serde_json::to_value(health_result).unwrap()))
}
