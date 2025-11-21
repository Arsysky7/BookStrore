// /pdf-bookstore/services/auth-service/src/main.rs

mod api;
mod middleware;
mod core;
mod db;
mod services;
mod utils;
mod models;
mod docs;

use axum::{
    routing::{get, post, put},
    Router,
    http::{Method, HeaderValue, header::{AUTHORIZATION, CONTENT_TYPE, ACCEPT}},
    response::Json,
    middleware as axum_middleware,
};
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
    timeout::TimeoutLayer,
};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{env, time::Duration, sync::Arc};
use std::net::SocketAddr;
use tracing::info;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use docs::ApiDoc;

use crate::{
    core::JwtService, 
    services::{ServiceClient, ServiceRegistry, CircuitBreakerManager},
    middleware::auth_middleware,
    api::handlers,
    utils::start_token_cleanup_job, 
};

/// State aplikasi dengan semua shared services
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub jwt_service: Arc<JwtService>,
    pub service_client: Arc<ServiceClient>,
    pub service_registry: Arc<ServiceRegistry>,
    pub circuit_manager: Arc<CircuitBreakerManager>,
    pub pepper: String,
}

#[tokio::main]
async fn main() {
    // Setup logging dengan environment filter
    tracing_subscriber::fmt()
        .with_env_filter("auth_service=debug,tower_http=debug")
        .init();

    // Load environment variables
    dotenvy::from_filename("../../.env").ok();
    dotenvy::dotenv().ok();

    // Setup database connection dengan retry logic
    let database_url = get_database_url();
    let pool = create_database_pool(&database_url).await;
    
    // Test database connection
    test_database_connection(&pool).await;
    info!("✅ Database connection established successfully");

    // Initialize service registry dengan health check otomatis
    let service_registry = Arc::new(ServiceRegistry::new());
    service_registry.init_default_services().await;

    // Initialize circuit breaker manager
    let circuit_manager = Arc::new(CircuitBreakerManager::new());
    
    // Initialize service client untuk inter-service communication
    let service_client = Arc::new(ServiceClient::new(circuit_manager.clone()));

    // Initialize JWT service sekali aja
    let jwt_service = Arc::new(
        JwtService::new()
            .expect("Failed to initialize JWT service")
    );

    // Start token cleanup scheduler
    match start_token_cleanup_job(pool.clone()).await {
        Ok(_) => info!("✅ Token cleanup scheduler started"),
        Err(e) => tracing::error!("❌ Failed to start scheduler: {}", e),
    }

    // Get password pepper
    let pepper = utils::common::get_pepper();

    // Create application state
    let app_state = AppState {
        db: pool.clone(),
        jwt_service,
        service_client,
        service_registry,
        circuit_manager,
        pepper,
    };

    // Setup CORS policy
    let cors = setup_cors();

    // Generate OpenAPI documentation
    let openapi = ApiDoc::openapi();

    // ======= endpoint definitions =======
    
    // Build public routes (tidak perlu auth)
    let public_routes = Router::new()
        // Health check
        .route("/health", get(health_check))

        // Swagger UI - HARUS PUBLIC!
        .merge(SwaggerUi::new("/swagger-ui")
            .url("/api-docs/openapi.json", openapi.clone()))

        // Public auth endpoints
        .route("/api/auth/register", post(handlers::register_user))
        .route("/api/auth/login", post(handlers::login_user))
        .route("/api/auth/verify-otp", post(handlers::verify_otp))
        .route("/api/auth/password-reset/request", post(handlers::request_password_reset))
        .route("/api/auth/password-reset/confirm", post(handlers::reset_password))
        .route("/api/auth/email/verify", post(handlers::verify_email))

        // OAuth endpoints (public)
        .route("/api/auth/oauth/google", post(handlers::start_google_oauth))
        .route("/api/auth/oauth/google/callback", get(handlers::google_oauth_callback))
        .route("/api/auth/oauth/status", get(handlers::oauth_status));

    // Build protected routes (perlu auth)
    let protected_routes = Router::new()
        // Token & Session management
        .route("/api/auth/verify", get(handlers::verify_token))
        .route("/api/auth/refresh", post(handlers::refresh_access_token))
        .route("/api/auth/logout", post(handlers::logout))
        .route("/api/auth/revoke-all", post(handlers::revoke_all_tokens))
        .route("/api/auth/session/validate", get(handlers::validate_session))
        .route("/api/auth/session/validate", post(handlers::validate_session))
        
        
        // User profile & settings
        .route("/api/auth/profile", get(handlers::get_profile))
        .route("/api/auth/profile", put(handlers::update_profile))
        .route("/api/auth/password/change", post(handlers::change_password))
        .route("/api/auth/login-history", get(handlers::get_login_history))
        .route("/api/auth/my-activity", get(handlers::get_my_activity))
        .route("/api/auth/email/send-verification", post(handlers::send_verification_email))
        
        // User data dengan service integration
        .route("/api/users/complete-profile", get(handlers::get_user_complete_profile))
        .route("/api/users/books/{book_id}/access", get(handlers::check_user_book_access))
        
        // Admin endpoints
        .route("/api/admin/users/stats", get(handlers::get_admin_user_stats))
        .route("/api/admin/users", get(handlers::get_admin_users))
        .route("/api/admin/users/activity", get(handlers::get_admin_activity_feed))
        .route("/api/admin/security/activity", get(handlers::get_security_activity_feed))
        .route("/api/admin/users/{id}/status", put(handlers::admin_update_user_status))
        
        // Apply auth middleware HANYA untuk protected routes
        .layer(axum_middleware::from_fn_with_state(app_state.clone(), auth_middleware));

    // Internal service endpoints (tanpa auth untuk service-to-service)
    let internal_routes = Router::new()
        .route("/api/internal/users/{id}", get(handlers::verify_user_internal))
        .route("/api/internal/users/{id}/payment", get(handlers::get_user_for_payment))
        .route("/api/internal/validate-token", post(handlers::validate_token_internal));

    // Combine all routes
    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .merge(internal_routes)
        // Apply global middleware (CORS, tracing, timeout)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::new(Duration::from_secs(30)))
                .layer(cors)
        )
        .with_state(app_state);

    // Get server configuration
    let (host, port) = get_server_config();
    let bind_address = format!("{}:{}", host, port);

    // Print startup information
    print_startup_info(&bind_address);

    // Start server
    let listener = tokio::net::TcpListener::bind(&bind_address)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {}: {}", bind_address, e));

    info!("🚀 Auth Service started successfully at {}", bind_address);

    axum::serve(
        listener, 
        app.into_make_service_with_connect_info::<SocketAddr>()
    )
    .await
    .unwrap_or_else(|e| panic!("Server failed to start: {}", e));

}

/// Health check endpoint
async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "service": "auth-service",
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Mendapatkan database URL berdasarkan environment
fn get_database_url() -> String {
    let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());
    
    if environment == "development" {
        env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://bookstore_user:Bookstore_987@localhost:5432/bookstore".to_string())
    } else {
        env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://bookstore_user:Bookstore_987@postgres:5432/bookstore".to_string())
    }
}

/// Membuat database connection pool dengan konfigurasi optimal
async fn create_database_pool(database_url: &str) -> PgPool {
    let max_connections = env::var("DATABASE_MAX_CONNECTIONS")
        .unwrap_or_else(|_| "10".to_string())
        .parse()
        .unwrap_or(10);

    let acquire_timeout = env::var("DATABASE_ACQUIRE_TIMEOUT_SECONDS")
        .unwrap_or_else(|_| "3".to_string())
        .parse()
        .unwrap_or(3);

    PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_secs(acquire_timeout))
        .connect(database_url)
        .await
        .unwrap_or_else(|e| panic!("Failed to connect to PostgreSQL: {}", e))
}

/// Test database connection dengan retry mechanism
async fn test_database_connection(pool: &PgPool) {
    for attempt in 1..=3 {
        match sqlx::query("SELECT 1").fetch_one(pool).await {
            Ok(_) => return,
            Err(e) => {
                if attempt == 3 {
                    panic!("Database connection failed after 3 attempts: {}", e);
                }
                tracing::warn!("Database connection attempt {} failed: {}", attempt, e);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

/// Setup CORS policy berdasarkan environment
fn setup_cors() -> CorsLayer {
    let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());
    let allowed_origins = env::var("ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:8080,http://localhost:8081,http://localhost:3000".to_string());

    if environment == "development" {
        // Development: Allow multiple origins
        let origins: Vec<HeaderValue> = allowed_origins
            .split(',')
            .filter_map(|origin| origin.trim().parse().ok())
            .collect();
        
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
            .allow_headers([AUTHORIZATION, CONTENT_TYPE, ACCEPT])
            .allow_credentials(true)
    } else {
        // Production: More restrictive CORS
        let production_origins = env::var("PRODUCTION_ORIGINS")
            .unwrap_or_else(|_| "https://yourdomain.com".to_string());
        
        let origins: Vec<HeaderValue> = production_origins
            .split(',')
            .filter_map(|origin| origin.trim().parse().ok())
            .collect();

        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([Method::GET, Method::POST, Method::PUT])
            .allow_headers([AUTHORIZATION, CONTENT_TYPE, ACCEPT])
            .allow_credentials(true)
    }
}

/// Mendapatkan server configuration dari environment
fn get_server_config() -> (String, String) {
    let host = env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("AUTH_SERVICE_PORT")
        .or_else(|_| env::var("SERVER_PORT"))
        .unwrap_or_else(|_| "3001".to_string());
    
    (host, port)
}

/// Print startup information untuk monitoring
fn print_startup_info(bind_address: &str) {
    info!("🚀 Auth Service starting at {}", bind_address);
    info!("📋 Available endpoints:");
    info!("  Public endpoints:");
    info!("    GET  /health                          - Health check");
    info!("    POST /api/auth/register               - User registration");
    info!("    POST /api/auth/login                  - User login");
    info!("    POST /api/auth/password-reset/request - Request password reset");
    info!("    POST /api/auth/password-reset/confirm - Confirm password reset");
    info!("  Protected endpoints (JWT required):");
    info!("    GET  /api/auth/verify                 - Token verification");
    info!("    GET  /api/auth/profile                - User profile");
    info!("    PUT  /api/auth/profile                - Update profile");
    info!("    POST /api/auth/refresh                - Refresh token");
    info!("    POST /api/auth/logout                 - Logout");
    info!("  Admin endpoints (admin JWT required):");
    info!("    GET  /api/admin/users/stats           - User statistics");
    info!("    GET  /api/admin/users                 - User list (paginated)");
    info!("    GET  /api/admin/users/activity        - User activity feed");
    info!("    GET  /api/admin/security/activity     - Security activity feed");
    info!("    PUT  /api/admin/users/:id/status      - Update user status");
    info!("📚 Swagger UI available at: http://localhost:3001/swagger-ui");
    info!("📄 OpenAPI spec at: http://localhost:3001/api-docs/openapi.json");
    
    let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());
    info!("🌍 Environment: {}", environment);
    
    if environment == "development" {
        info!("🔓 Development mode: CORS allows multiple origins");
        info!("🔧 Test admin credentials:");
        info!("    Email: admin@bookstore.com");
        info!("    Password: Admin123!");
    }
}