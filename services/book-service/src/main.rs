// /pdf-bookstore/services/book-service/src/main.rs
mod models;
mod handlers;
mod database;
mod upload;
mod error;
mod circuit_breaker;
mod service_discovery;

use axum::{
    routing::{get, post, put, delete},
    Router,
    http::{StatusCode, Method, HeaderValue, header},
    response::Json,
    extract::{Request, State},
    middleware::{self, Next},
};
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
    timeout::TimeoutLayer,
    services::ServeDir,
};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{env, time::Duration, sync::Arc};
use tracing::info;
use tracing_subscriber;
use uuid::Uuid;
use service_discovery::ServiceRegistry;
use circuit_breaker::CircuitBreakerManager;

use handlers::*;
use models::ErrorResponse;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub http_client: Arc<reqwest::Client>,
    pub service_registry: Arc<ServiceRegistry>,
    pub circuit_manager: Arc<CircuitBreakerManager>,
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("book_service=debug,tower_http=debug")
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    let http_client = Arc::new(
        reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(10)
        .build()
        .expect("Failed to create HTTP client")
    );

    // Database configuration
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://bookstore_user:Bookstore_987@localhost:5432/bookstore".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(3))
        .connect(&database_url)
        .await
        .expect("Failed to connect to PostgreSQL");
    
    let service_registry = Arc::new(ServiceRegistry::new());
    service_registry.init_default_services().await;
    
    let circuit_manager = Arc::new(CircuitBreakerManager::new());

    // Test database connection
    sqlx::query("SELECT 1")
        .fetch_one(&pool)
        .await
        .expect("Failed to ping database");

    info!("✅ Database connected successfully");

    // Create application state
    let app_state = AppState {
        db: pool,
        http_client,
        service_registry,
        circuit_manager,
    };

    // CORS configuration - FIXED VERSION
    let cors = CorsLayer::new()
        .allow_origin([
            "http://localhost:8080".parse::<HeaderValue>().unwrap(),
            "http://localhost:8081".parse::<HeaderValue>().unwrap(),
            "http://localhost:3000".parse::<HeaderValue>().unwrap(),
        ])
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::ACCEPT,
            header::ORIGIN,
        ])
        .allow_credentials(true);

    // Complete router setup
    let app = Router::new()
        // Health endpoint
        .route("/health", get(health_check))
        
        // Public Book API
        .route("/api/books", get(get_books))
        .route("/api/books/{id}", get(get_book_by_id))
        .route("/api/books/{id}/validate", get(validate_book_for_order))

        // Preview & Related (public)
        .route("/api/books/{id}/preview", get(get_book_preview))
        .route("/api/books/{id}/related", get(get_related_books))
        
        // Review endpoints
        .route("/api/books/{id}/reviews", get(get_book_reviews).post(create_book_review))
    
        // Authenticated Book API
        .route("/api/books", post(create_book))
        .route("/api/books/{id}", put(update_book))
        .route("/api/books/{id}", delete(delete_book))
        .route("/api/books/{id}/download", get(download_book_pdf))
        .route("/api/books/{id}/stock", get(get_book_stock))
        
        // Library (Protected)
        .route("/api/books/my-library", get(get_my_library))
        
        // Categories
        .route("/api/categories", get(get_categories))
        
        // File Upload
        .route("/api/upload/pdf", post(upload_pdf_only))
        .route("/api/upload/cover", post(upload_cover_only))
        
        // Admin endpoints
        .route("/api/admin/books/stats", get(get_admin_book_stats))
        .route("/api/admin/books/top", get(get_top_books))
        .route("/api/admin/books/activity", get(get_recent_activity))
        .route("/api/admin/analytics/sales", get(get_sales_analytics))
        .route("/api/admin/analytics/popular-books", get(get_popular_books_chart_data))
        .route("/api/admin/analytics/categories", get(get_category_analytics))
        .route("/api/admin/dashboard/metrics", get(get_dashboard_metrics))
        
        // Webhooks
        .route("/api/webhooks/payment-success", post(handle_payment_success_webhook))
        
        // Serve static files
        .nest_service("/storage", ServeDir::new(
            env::var("STORAGE_BASE_PATH").unwrap_or_else(|_| "./storage".to_string())
        ))
        
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::new(Duration::from_secs(30)))
                .layer(cors)
                .layer(middleware::from_fn_with_state(app_state.clone(), auth_middleware))
        )
        .with_state(app_state);

    // Server configuration
    let host = env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("BOOK_SERVICE_PORT").unwrap_or_else(|_| "3002".to_string());
    let bind_address = format!("{}:{}", host, port);

    info!("🚀 Book Service starting on {}", bind_address);

    // Start server
    let listener = tokio::net::TcpListener::bind(&bind_address)
        .await
        .expect("Failed to bind server address");

    axum::serve(listener, app)
        .await
        .expect("Failed to start server");
}

// Auth middleware
async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<axum::response::Response, (StatusCode, Json<ErrorResponse>)> {
    let path = req.uri().path();
    let method = req.method();
    
    // Skip auth untuk public endpoints
    if path.contains("/health")
        || path.contains("/storage")
        || path.contains("/api/categories")
        || path.contains("/preview")
        || path.contains("/related")
        || (path.contains("/api/books") && method == &Method::GET && 
            !path.contains("/download") && 
            !path.contains("/my-library"))
        || (path.contains("/reviews") && method == &Method::GET) {
        return Ok(next.run(req).await);
    }

    // Gateway header support
    if let Some(_gateway_header) = req.headers().get("X-Gateway-Request") {
        if let Some(user_id_header) = req.headers().get("X-User-Id") {
            if let Ok(user_id_str) = user_id_header.to_str() {
                if let Ok(user_id) = Uuid::parse_str(user_id_str) {
                    // Extract role from gateway
                    let user_role = req.headers()
                        .get("X-User-Role")
                        .and_then(|h| h.to_str().ok())
                        .unwrap_or("customer")
                        .to_string();
                    
                    tracing::debug!("Gateway auth: user={}, role={}", user_id, user_role);
                    req.extensions_mut().insert(user_id);
                    req.extensions_mut().insert(user_role);
                    return Ok(next.run(req).await);
                }
            }
        }
    }

    // Extract token dari header
    let token = {
        let auth_header = req
            .headers()
            .get("authorization")
            .and_then(|header| header.to_str().ok())
            .and_then(|header| {
                if header.starts_with("Bearer ") {
                    Some(header.strip_prefix("Bearer ").unwrap().to_string())
                } else {
                    None
                }
            });

        match auth_header {
            Some(token) => token,
            None => {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        success: false,
                        message: "Authorization header missing or invalid".to_string(),
                        error_code: Some("MISSING_TOKEN".to_string()),
                    })
                ));
            }
        }
    };

    // Verify JWT token
    let auth_service_url = env::var("AUTH_SERVICE_URL").unwrap_or_else(|_| "http://localhost:3001".to_string());

    let verify_response = state.http_client
        .get(&format!("{}/api/auth/verify", auth_service_url))
        .header("Authorization", format!("Bearer {}", token))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                success: false,
                message: "Failed to contact auth service".to_string(),
                error_code: Some("AUTH_SERVICE_UNAVAILABLE".to_string()),
            })
        ))?;

    // Parse response
    let verify_data: serde_json::Value = verify_response
        .json()
        .await
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                success: false,
                message: "Failed to parse auth service response".to_string(),
                error_code: Some("AUTH_PARSE_ERROR".to_string()),
            })
        ))?;

    // Extract user info
    let user_id = verify_data["user"]["id"].as_str()
        .and_then(|id| Uuid::parse_str(id).ok())
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                success: false,
                message: "Invalid user ID in token".to_string(),
                error_code: Some("INVALID_USER_ID".to_string()),
            })
        ))?;

    let user_role = verify_data["user"]["role"].as_str()
        .unwrap_or("customer")
        .to_string();

    // Add to request extensions
    req.extensions_mut().insert(user_id);
    req.extensions_mut().insert(user_role);
    req.extensions_mut().insert(token);

    Ok(next.run(req).await)
}