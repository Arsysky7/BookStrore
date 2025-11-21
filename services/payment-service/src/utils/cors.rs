// /pdf-bookstore/services/payment-service/src/utils/cors.rs

use tower_http::cors::CorsLayer;
use axum::http::{header, Method, HeaderValue};
use std::env;

/// Setup CORS layer untuk payment service
pub fn create_cors_layer() -> CorsLayer {
    let environment = get_environment();
    
    match environment.as_str() {
        "production" => build_production_cors(),
        _ => build_development_cors(),
    }
}

/// Build CORS configuration untuk development environment
fn build_development_cors() -> CorsLayer {
    let origins = parse_allowed_origins();
    
    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods(get_allowed_methods())
        .allow_headers(get_allowed_headers())
        .allow_credentials(true)
        .max_age(std::time::Duration::from_secs(3600))
}

/// Build CORS configuration untuk production dengan security ketat
fn build_production_cors() -> CorsLayer {
    let origins = parse_production_origins();
    
    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods(get_production_methods())
        .allow_headers(get_allowed_headers())
        .allow_credentials(true)
        .expose_headers([header::CONTENT_LENGTH, header::CONTENT_TYPE])
        .max_age(std::time::Duration::from_secs(86400))
}

/// Parse origins dari environment variable
fn parse_allowed_origins() -> Vec<HeaderValue> {
    let origins_str = env::var("ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    origins_str
        .split(',')
        .filter_map(|origin| {
            let trimmed = origin.trim();
            match trimmed.parse::<HeaderValue>() {
                Ok(header) => {
                    tracing::debug!("CORS origin registered: {}", trimmed);
                    Some(header)
                }
                Err(e) => {
                    tracing::warn!("Invalid origin format '{}': {}", trimmed, e);
                    None
                }
            }
        })
        .collect()
}

/// Parse production origins dengan validasi lebih ketat
fn parse_production_origins() -> Vec<HeaderValue> {
    let prod_origins = env::var("PRODUCTION_ORIGINS")
        .or_else(|_| env::var("ALLOWED_ORIGINS"))
        .unwrap_or_else(|_| "https://yourdomain.com".to_string());
    
    prod_origins
        .split(',')
        .filter(|origin| origin.trim().starts_with("https://"))
        .filter_map(|origin| origin.trim().parse().ok())
        .collect()
}

/// Daftar HTTP methods yang diperbolehkan untuk development
fn get_allowed_methods() -> Vec<Method> {
    vec![
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::PATCH,
        Method::OPTIONS,
    ]
}

/// Daftar HTTP methods untuk production (lebih restrictive)
fn get_production_methods() -> Vec<Method> {
    vec![
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::DELETE,
    ]
}

/// Daftar headers yang diizinkan (explicit list required untuk credentials)
fn get_allowed_headers() -> Vec<header::HeaderName> {
    vec![
        header::AUTHORIZATION,
        header::CONTENT_TYPE,
        header::ACCEPT,
        header::ORIGIN,
        header::USER_AGENT,
        header::CACHE_CONTROL,
        header::ACCEPT_LANGUAGE,
        header::ACCEPT_ENCODING,
        header::DNT,
        header::IF_MODIFIED_SINCE,
        header::REFERER,
    ]
}

/// Ambil environment mode dari env variable
fn get_environment() -> String {
    env::var("ENVIRONMENT")
        .unwrap_or_else(|_| "development".to_string())
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_origins() {
        env::set_var("ALLOWED_ORIGINS", "http://localhost:8080,http://localhost:3000");
        let origins = parse_allowed_origins();
        assert_eq!(origins.len(), 2);
    }

    #[test]
    fn test_environment_detection() {
        env::set_var("ENVIRONMENT", "production");
        assert_eq!(get_environment(), "production");
        
        env::remove_var("ENVIRONMENT");
        assert_eq!(get_environment(), "development");
    }
}