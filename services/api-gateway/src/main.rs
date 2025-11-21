// /services/api-gateway/src/main.rs
mod circuit_breaker;
mod service_discovery;  
mod error;

use axum::{
    Router,
    extract::{Request, State},
    http::{StatusCode, HeaderValue},  
    response::{Response, Json},
    body::Body,
    routing::get,
    middleware::{self, Next},
};
use std::{sync::Arc, time::Duration, env};
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
    timeout::TimeoutLayer,
};
use service_discovery::ServiceRegistry;  
use circuit_breaker::CircuitBreakerManager;

#[derive(Clone)]
pub struct AppState {
    pub client: reqwest::Client,
    pub service_registry: Arc<ServiceRegistry>, 
    pub circuit_manager: Arc<CircuitBreakerManager>,  
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("api_gateway=debug,tower_http=debug")
        .init();

    dotenvy::dotenv().ok();
    
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(10)
        .build()
        .expect("Failed to create HTTP client");

    let service_registry = Arc::new(ServiceRegistry::new());
    service_registry.init_default_services().await;
    
    let circuit_manager = Arc::new(CircuitBreakerManager::new());
    
    let state = AppState { 
        client,
        service_registry,
        circuit_manager,
    };
    
    start_health_checker(state.clone());

    let cors = CorsLayer::new()
        .allow_origin([
            "http://localhost:8080".parse::<HeaderValue>().unwrap(),
            "http://localhost:8081".parse::<HeaderValue>().unwrap(),
            "http://localhost:3000".parse::<HeaderValue>().unwrap(),
            "http://127.0.0.1:8080".parse::<HeaderValue>().unwrap(),
        ])
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::header::ACCEPT,
            axum::http::header::ORIGIN,
        ])
        .allow_credentials(true);
    
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/gateway/status", get(gateway_status))
        .fallback(proxy_handler)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::new(Duration::from_secs(30)))
                .layer(cors)
                .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        )
        .with_state(state);
    
    let addr = "0.0.0.0:8000";

    println!("\n╔═══════════════════════════════════════════════════════╗");
    println!("║          🚀 API GATEWAY SUCCESSFULLY STARTED          ║");
    println!("╠═══════════════════════════════════════════════════════╣");
    println!("║  Gateway Address: http://localhost:{}                ║", "8000");
    println!("╠═══════════════════════════════════════════════════════╣");
    println!("║  📡 Routing Configuration:                            ║");
    println!("║    /api/auth/*    → Auth Service (3001)              ║");
    println!("║    /api/books/*   → Book Service (3002)              ║");
    println!("║    /api/orders/*  → Payment Service (3003)           ║");
    println!("╠═══════════════════════════════════════════════════════╣");
    println!("║  🔍 Monitoring:                                       ║");
    println!("║    GET /health              - Gateway health         ║");
    println!("║    GET /api/gateway/status  - All services status    ║");
    println!("╠═══════════════════════════════════════════════════════╣");
    println!("║  ✨ Features:                                         ║");
    println!("║    ✓ Service Discovery                               ║");
    println!("║    ✓ Health Checking (every 30s)                     ║");
    println!("║    ✓ Circuit Breaker                                 ║");
    println!("║    ✓ JWT Verification                                ║");
    println!("║    ✓ Request Proxying                                ║");
    println!("╚═══════════════════════════════════════════════════════╝\n");
    
    let listener = tokio::net::TcpListener::bind(addr).await
        .expect("Failed to bind gateway address");
    
    axum::serve(listener, app).await
        .expect("Failed to start gateway server");
}

async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = req.uri().path().to_string();
    
    let public_paths = [
        "/health",
        "/api/gateway/status",
        "/api/auth/register",
        "/api/auth/login",
        "/api/auth/password-reset/request",
        "/api/auth/password-reset/confirm",
        "/api/auth/verify-otp",
        "/api/auth/email/verify",
        "/storage/",
    ];

    for public_path in public_paths.iter() {
        if path.starts_with(public_path) {
            return Ok(next.run(req).await);
        }
    }
    
    if req.method() == axum::http::Method::GET {
        if path.starts_with("/api/books") 
            && !path.contains("/download") 
            && !path.contains("/my-library") {
            return Ok(next.run(req).await);
        }
        
        if path.starts_with("/api/categories") {
            return Ok(next.run(req).await);
        }
        
        if path.contains("/preview") || path.contains("/related") {
            return Ok(next.run(req).await);
        }
        
        if path.contains("/reviews") {
            return Ok(next.run(req).await);
        }
    }
    
    let auth_header = req.headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());
    
    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            header.strip_prefix("Bearer ").unwrap()
        }
        _ => {
            tracing::warn!("Missing or invalid Authorization header for path: {}", path);
            return Err(StatusCode::UNAUTHORIZED);
        }
    };
    
    let auth_service_url = env::var("AUTH_SERVICE_URL")
        .unwrap_or_else(|_| "http://localhost:3001".to_string());
    
    let verify_response = state.client
        .get(&format!("{}/api/auth/verify", auth_service_url))
        .header("Authorization", format!("Bearer {}", token))
        .timeout(Duration::from_secs(5))
        .send()
        .await;
    
    match verify_response {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(user_data) = resp.json::<serde_json::Value>().await {
                if let (Some(user_id), Some(user_role)) = (
                    user_data["user"]["id"].as_str(),
                    user_data["user"]["role"].as_str(),
                ) {
                    req.headers_mut().insert(
                        "X-Gateway-Request",
                        HeaderValue::from_static("true"),
                    );
                    req.headers_mut().insert(
                        "X-User-Id",
                        HeaderValue::from_str(user_id).unwrap(),
                    );
                    req.headers_mut().insert(
                        "X-User-Role",
                        HeaderValue::from_str(user_role).unwrap(),
                    );
                    
                    tracing::debug!("Auth success: user_id={}, role={}, path={}", 
                        user_id, user_role, path);
                    
                    return Ok(next.run(req).await);
                }
            }
            
            tracing::warn!("Invalid user data in verify response");
            Err(StatusCode::UNAUTHORIZED)
        }
        Ok(resp) => {
            tracing::warn!("Auth verification failed with status: {}", resp.status());
            Err(StatusCode::UNAUTHORIZED)
        }
        Err(e) => {
            tracing::error!("Failed to contact auth service: {}", e);
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

async fn gateway_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let status = state.service_registry.get_status().await;
    
    Json(serde_json::json!({
        "service": "api-gateway",
        "status": "healthy",
        "version": "1.0.0",
        "services": status,
        "timestamp": chrono::Utc::now(),
        "features": {
            "service_discovery": true,
            "health_checking": true,
            "circuit_breaker": true,
            "jwt_verification": true,
            "request_proxying": true
        }
    }))
}

async fn proxy_handler(
    State(state): State<AppState>,
    req: Request,
) -> Result<Response<Body>, StatusCode> {
    let path = req.uri().path().to_string();
    let method = req.method().clone();
    
    tracing::debug!("Proxying request: {} {}", method, path);
    
    let service_name = if path.starts_with("/api/auth") {
        "auth-service"
    } else if path.starts_with("/api/books") || path.starts_with("/api/categories") {
        "book-service"
    } else if path.starts_with("/api/orders") || path.starts_with("/api/payments") {
        "payment-service"
    } else if path.starts_with("/storage") {
        "book-service"
    } else {
        tracing::warn!("No service found for path: {}", path);
        return Err(StatusCode::NOT_FOUND);
    };
    
    let service = state.service_registry
        .get_healthy_instance(service_name)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get healthy instance for {}: {:?}", service_name, e);
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    
    let service_url = service.get_url();
    
    let query = req.uri().query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();
    let url = format!("{}{}{}", service_url, path, query);
    
    tracing::debug!("Forwarding to: {}", url);
    
    let (parts, body) = req.into_parts();
    let body_bytes = axum::body::to_bytes(body, usize::MAX).await
        .map_err(|e| {
            tracing::error!("Failed to read request body: {}", e);
            StatusCode::BAD_REQUEST
        })?;
    
    let mut req_builder = state.client.request(parts.method.clone(), &url);
    
    for (key, value) in parts.headers.iter() {
        req_builder = req_builder.header(key, value);
    }
    
    let response = req_builder
        .body(body_bytes.to_vec())
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Proxy error to {} ({}): {}", service_name, url, e);
            StatusCode::BAD_GATEWAY
        })?;
    
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await
        .map_err(|e| {
            tracing::error!("Failed to read response body: {}", e);
            StatusCode::BAD_GATEWAY
        })?;
    
    tracing::debug!("Proxy response: {} from {}", status, service_name);
    
    let mut builder = Response::builder().status(status);
    
    for (key, value) in headers.iter() {
        builder = builder.header(key, value);
    }
    
    builder
        .body(Body::from(body))
        .map_err(|e| {
            tracing::error!("Failed to build response: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

fn start_health_checker(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            tracing::debug!("Running health check for all services...");
            state.service_registry.health_check_all().await;
        }
    });
    
    tracing::info!("✓ Health checker started (interval: 30s)");
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "service": "api-gateway",
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
        "version": "1.0.0"
    }))
}