// /pdf-bookstore/services/payment-service/src/middleware/rate_limit.rs
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{Json, Response},
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{Utc, Duration};
use crate::{AppState, models::ErrorResponse};

/// Rate limiter dengan token bucket algorithm
#[derive(Clone)]
pub struct RateLimiter {
    buckets: Arc<RwLock<HashMap<String, TokenBucket>>>,
    max_requests: u32,
    window_seconds: i64,
}

#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: u32,
    last_refill: chrono::DateTime<Utc>,
}

impl RateLimiter {
    /// Create rate limiter baru
    pub fn new(max_requests: u32, window_seconds: i64) -> Self {
        let limiter = Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            max_requests,
            window_seconds,
        };
        
        // Cleanup task
        let buckets_clone = limiter.buckets.clone();
        let window = window_seconds;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_secs(window as u64 * 2)
            );
            
            loop {
                interval.tick().await;
                let mut buckets = buckets_clone.write().await;
                let cutoff = Utc::now() - Duration::seconds(window * 2);
                buckets.retain(|_, bucket| bucket.last_refill > cutoff);
            }
        });
        
        limiter
    }
    
    /// Check rate limit
    pub async fn check_rate_limit(&self, identifier: &str) -> bool {
        let mut buckets = self.buckets.write().await;
        let now = Utc::now();
        
        let bucket = buckets.entry(identifier.to_string()).or_insert_with(|| {
            TokenBucket {
                tokens: self.max_requests,
                last_refill: now,
            }
        });
        
        // Refill tokens
        let elapsed = (now - bucket.last_refill).num_seconds();
        if elapsed >= self.window_seconds {
            bucket.tokens = self.max_requests;
            bucket.last_refill = now;
        } else if elapsed > 0 {
            // Gradual refill untuk smooth rate limiting
            let refill_rate = self.max_requests as f64 / self.window_seconds as f64;
            let tokens_to_add = (elapsed as f64 * refill_rate) as u32;
            bucket.tokens = (bucket.tokens + tokens_to_add).min(self.max_requests);
        }
        
        if bucket.tokens > 0 {
            bucket.tokens -= 1;
            true
        } else {
            false
        }
    }
}

/// Rate limiting middleware 
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let path = req.uri().path();
    
    // Skip untuk health dan webhook
    if path == "/health" || path.contains("/webhook") {
        return Ok(next.run(req).await);
    }
    
    // Extract IP as identifier
    let identifier = req.headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .or_else(|| {
            req.headers()
                .get("x-real-ip")
                .and_then(|h| h.to_str().ok())
        })
        .unwrap_or("unknown")
        .to_string();
    
    // Check rate limit menggunakan STATE
    if !state.rate_limiter.check_rate_limit(&identifier).await {
        tracing::warn!("Rate limit terlampaui untuk: {}", identifier);
        
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResponse {
                success: false,
                message: "Terlalu banyak request. Silakan coba lagi nanti.".to_string(),
                error_code: Some("RATE_LIMIT_EXCEEDED".to_string()),
                details: Some(serde_json::json!({
                    "retry_after_seconds": 60,
                    "limit": "100 requests per menit"
                })),
            })
        ));
    }
    
    Ok(next.run(req).await)
}
