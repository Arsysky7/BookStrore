// /pdf-bookstore/services/payment-service/src/utils/banner.rs

/// Print startup banner
pub fn print_startup_banner(bind_address: &str) {
    println!(r#"
╔══════════════════════════════════════════════════════════╗
║                  PAYMENT SERVICE v1.0.0                   ║
║                    Enterprise Edition                     ║
╚══════════════════════════════════════════════════════════╝
    "#);
    
    tracing::info!("🚀 Payment Service starting at {}", bind_address);
    tracing::info!("📋 Available endpoints:");
    tracing::info!("  Public:");
    tracing::info!("    POST /api/webhook/midtrans     - Payment webhook");
    tracing::info!("  Protected:");
    tracing::info!("    POST /api/orders                - Create order");
    tracing::info!("    GET  /api/orders                - List orders");
    tracing::info!("    GET  /api/orders/:id            - Get order");
    tracing::info!("    PUT  /api/orders/:id/cancel     - Cancel order");
    tracing::info!("  Admin:");
    tracing::info!("    GET  /api/admin/orders/stats    - Order statistics");
    tracing::info!("    GET  /api/admin/analytics/*     - Analytics endpoints");
}