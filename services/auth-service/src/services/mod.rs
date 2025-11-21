// /pdf-bookstore/services/auth-service/src/services/mod.rs

pub mod client;
pub mod circuit_breaker;
pub mod service_discovery;
pub mod oauth_service;

pub use client::ServiceClient;
pub use circuit_breaker::CircuitBreakerManager;
pub use service_discovery::ServiceRegistry;
pub use oauth_service::OAuthService;