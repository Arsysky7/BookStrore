// /pdf-bookstore/services/payment-service/src/main.rs

mod models;
mod api;
mod core;
mod repository;
mod middleware;
mod utils;

use axum::{
    Router, 
    middleware as axum_middleware,
};
use tower::ServiceBuilder;
use tower_http::{
    trace::TraceLayer,
    timeout::TimeoutLayer,
};
use sqlx::postgres::PgPoolOptions;
use std::{env, sync::Arc, time::Duration};
use tracing::info; 
use crate::{
    api::routes,
    core::services::*,
    repository::Repository,
    middleware::{
        auth::auth_middleware,
        rate_limit::{RateLimiter, rate_limit_middleware},
    },
    utils::{
        scheduler::start_background_jobs,
        cache::CacheManager,
        circuit_breaker::CircuitBreakerManager,  
        service_discovery::ServiceRegistry,      
    },
};

#[derive(Clone)]
pub struct AppState {
    pub repository: Arc<Repository>,
    pub payment_service: Arc<PaymentService>,
    pub midtrans_service: Arc<MidtransClient>,
    pub cache_manager: Arc<CacheManager>,
    pub rate_limiter: Arc<RateLimiter>,
    pub circuit_manager: Arc<CircuitBreakerManager>,  
    pub service_registry: Arc<ServiceRegistry>,       
    pub http_client: reqwest::Client,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    utils::logger::init_logger();
    
    // Load environment variables
    dotenvy::dotenv().ok();
    
    // Setup database connection pool
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL harus di-set di environment");
    
    let pool = PgPoolOptions::new()
        .max_connections(
            env::var("DATABASE_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10)
        )
        .acquire_timeout(Duration::from_secs(
            env::var("DATABASE_ACQUIRE_TIMEOUT_SECONDS")
                .unwrap_or_else(|_| "3".to_string())
                .parse()
                .unwrap_or(3)
        ))
        .connect(&database_url)
        .await?;
    
    // Test database connection
    sqlx::query("SELECT 1")
        .fetch_one(&pool)
        .await
        .expect("Gagal ping database");
    
    info!("✅ Database berhasil terkoneksi");
    
    // Initialize repository layer
    let repository = Arc::new(Repository::new(pool.clone()));
    
    // Cache manager dengan fallback 
    let cache_manager = Arc::new(
        match CacheManager::new(
            &env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            "payment_service"
        ).await {
            Ok(cache) => {
                tracing::info!("✅ Redis cache berhasil terkoneksi");
                cache
            }
            Err(e) => {
                tracing::warn!("⚠️ Redis tidak tersedia, menggunakan dummy cache: {}", e);
                CacheManager::new_dummy("payment_service")
            }
        }
    );
    
    // Initialize payment service
    let payment_service = Arc::new(
        PaymentService::new(
            repository.clone(),
            cache_manager.clone(),
        ).await
        .expect("Failed to initialize payment service")
    );

    // Initialize Midtrans client
    let midtrans_service = Arc::new(
        MidtransClient::new()
            .expect("Gagal initialize Midtrans client")
    );

    
    
    // Initialize rate limiter
    let rate_limiter = Arc::new(
        RateLimiter::new(
            env::var("RATE_LIMIT_MAX_REQUESTS")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .unwrap_or(100),
            env::var("RATE_LIMIT_WINDOW_SECONDS")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .unwrap_or(60)
        )
    );

    // Initialize circuit breaker manager
    let circuit_manager = Arc::new(CircuitBreakerManager::new());

    // Initialize service discovery
    let service_registry = Arc::new(ServiceRegistry::new());
    service_registry.init_default_services().await;

    // Start health check background job
    start_health_check_job(service_registry.clone());

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .pool_max_idle_per_host(10)
        .build()?;
    
    // Start background jobs
    start_background_jobs(repository.clone()).await?;
    
    // Create application state
    let app_state = AppState {
        repository,
        payment_service,
        midtrans_service,
        cache_manager,
        rate_limiter,
        circuit_manager,
        service_registry,
        http_client,
    };
    
    // Setup CORS
    let cors = utils::cors::create_cors_layer();
    
    // Build application dengan middleware stack yang BENAR
    let app = Router::new()
        // Mount API routes
        .merge(routes::create_routes())
        // Health check endpoint
        .route("/health", axum::routing::get(health_check))
        // Apply state first
        .with_state(app_state.clone())
        // Then apply middlewares
        .layer(
            ServiceBuilder::new()
                // Request tracing (paling luar)
                .layer(TraceLayer::new_for_http())
                // Timeout protection
                .layer(TimeoutLayer::new(Duration::from_secs(30)))
                // CORS handling
                .layer(cors)
        )
        // Security headers middleware (langsung panggil function)
        .layer(axum_middleware::from_fn(
            middleware::security::security_headers_middleware
        ))
        // Rate limiting sebagai middleware terpisah
        .layer(axum_middleware::from_fn_with_state(
            app_state.clone(),
            rate_limit_middleware
        ))
        // Auth middleware
        .layer(axum_middleware::from_fn_with_state(
            app_state.clone(),
            auth_middleware
        ));
    
    // Server configuration
    let port = env::var("PAYMENT_SERVICE_PORT")
        .or_else(|_| env::var("SERVER_PORT"))
        .unwrap_or_else(|_| "3003".to_string());
    let bind_address = format!("0.0.0.0:{}", port);
    
    // Print startup information
    utils::banner::print_startup_banner(&bind_address);
    
    // Start server
    let listener = tokio::net::TcpListener::bind(&bind_address).await?;
    info!("🚀 Payment Service berjalan di {}", bind_address);
    
    axum::serve(listener, app)
        .await
        .map_err(|e| e.into())
}

// Health check endpoint
async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "service": "payment-service",
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
        "version": env!("CARGO_PKG_VERSION"),
        "features": {
            "midtrans": true,
            "order_management": true,
            "webhook_processing": true,
            "atomic_transactions": true,
            "admin_analytics": true,
            "background_jobs": true,
            "rate_limiting": true,
            "caching": true,
            "security_headers": true
        },
        "environment": env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
    }))
}

fn start_health_check_job(registry: Arc<ServiceRegistry>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            registry.health_check_all().await;
        }
    });
}