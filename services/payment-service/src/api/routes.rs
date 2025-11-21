// /pdf-bookstore/services/payment-service/src/api/routes.rs


use axum::{
    routing::{get, post, put},
    Router,
};
use crate::AppState;
use super::handlers;
// use super::csrf_handlers;

/// Create semua routes untuk payment service
pub fn create_routes() -> Router<AppState> {
    Router::new()
        // Order management routes
        .route("/api/orders", post(handlers::create_order))
        .route("/api/orders", get(handlers::list_orders))
        
        // Order detail dan actions
        .route("/api/orders/{id}", get(handlers::get_order))
        .route("/api/orders/{id}/cancel", put(handlers::cancel_order))

        // Route untuk refund
        .route("/api/orders/{id}/refund", post(handlers::request_refund))  
        // Purchase verification
        .route("/api/purchases/{book_id}", get(handlers::check_purchase_status))
        
        // Webhook endpoint (public, no auth)
        .route("/api/webhook/midtrans", post(handlers::handle_midtrans_webhook))
        
        // Debug routes (dev only)
        .route("/api/debug/scheduler/status", get(handlers::get_scheduler_status))
        .route("/api/payment/config", get(handlers::get_payment_config))
        .route("/health/detailed", get(handlers::comprehensive_health_check_handler))
        
        // Admin routes
        .route("/api/admin/orders/stats", get(handlers::get_admin_order_stats))
        .route("/api/admin/analytics/revenue", get(handlers::get_revenue_analytics))
        .route("/api/admin/orders/recent", get(handlers::get_recent_orders_admin))
        .route("/api/admin/orders/{id}/status", put(handlers::admin_update_order_status))
        // Maintenance endpoint (admin only)
        .route("/api/admin/maintenance/trigger", post(handlers::trigger_maintenance))
        .route("/api/admin/system/health", get(handlers::get_system_health))
       

}