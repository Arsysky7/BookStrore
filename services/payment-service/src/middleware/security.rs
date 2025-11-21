// /pdf-bookstore/services/payment-service/src/middleware/security.rs

use axum::{
    http::HeaderValue,
    middleware::Next,
    response::Response,
    extract::Request,
};

/// Enhanced security headers middleware
pub async fn security_headers_middleware(
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    
    // Helper function untuk safe parse
    fn parse_header_value(value: &str) -> HeaderValue {
        HeaderValue::from_str(value).unwrap_or_else(|_| {
            HeaderValue::from_static("default")
        })
    }
    
    // Security headers dengan safe parsing
    headers.insert("X-Content-Type-Options", parse_header_value("nosniff"));
    headers.insert("X-Frame-Options", parse_header_value("DENY"));
    headers.insert("X-XSS-Protection", parse_header_value("1; mode=block"));
    headers.insert("Referrer-Policy", parse_header_value("strict-origin-when-cross-origin"));
    headers.insert("Permissions-Policy", parse_header_value("geolocation=(), microphone=(), camera=()"));
    
    // Content Security Policy
    let csp_value = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; font-src 'self'; connect-src 'self'; frame-ancestors 'none';";
    headers.insert("Content-Security-Policy", parse_header_value(csp_value));
    
    // HSTS untuk production
    if std::env::var("ENVIRONMENT").unwrap_or_default() == "production" {
        headers.insert(
            "Strict-Transport-Security",
            parse_header_value("max-age=31536000; includeSubDomains")
        );
    }
    
    // Cache control untuk sensitive data
    if path.contains("/api/orders") || path.contains("/api/admin") {
        headers.insert(
            "Cache-Control",
            parse_header_value("no-store, no-cache, must-revalidate, private")
        );
    }
    
    response
}