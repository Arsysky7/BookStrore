// /pdf-bookstore/services/payment-service/src/utils/health.rs

use std::sync::Arc;
use std::collections::HashMap;
use crate::{
    repository::Repository,
    utils::{
        circuit_breaker::CircuitBreakerManager,
        service_discovery::ServiceRegistry,
        cache::CacheManager,
    },
};

#[derive(Debug, serde::Serialize)]
pub struct HealthCheckResult {
    pub status: HealthStatus,
    pub checks: HashMap<String, ComponentHealth>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, serde::Serialize, Clone)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, serde::Serialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub response_time_ms: Option<u64>,
}

/// Comprehensive health check untuk semua components
pub async fn comprehensive_health_check(
    repository: &Arc<Repository>,
    cache_manager: &Arc<CacheManager>,
    circuit_manager: &Arc<CircuitBreakerManager>,
    service_registry: &Arc<ServiceRegistry>,
) -> HealthCheckResult {
    let mut checks = HashMap::new();
    let start = std::time::Instant::now();

    
    
    // Database health check
    let db_health = check_database_health(repository).await;
    checks.insert("database".to_string(), db_health);
    
    // Redis cache health check
    let cache_health = check_cache_health(cache_manager).await;
    checks.insert("cache".to_string(), cache_health);
    
    // Service discovery health
    let discovery_health = check_service_discovery(service_registry).await;
    checks.insert("service_discovery".to_string(), discovery_health);
    
    // Circuit breakers status
    let circuit_health = check_circuit_breakers(circuit_manager).await;
    checks.insert("circuit_breakers".to_string(), circuit_health);
    
    // External services health
    let external_health = check_external_services(service_registry).await;
    checks.insert("external_services".to_string(), external_health);
    
    // Determine overall status
    let overall_status = determine_overall_status(&checks);
    
    let total_time_ms = start.elapsed().as_millis() as u64;
    
    // Add overall metrics
    checks.insert("_meta".to_string(), ComponentHealth {
        name: "Health Check Meta".to_string(),
        status: HealthStatus::Healthy,
        message: Some(format!("Total health check time: {}ms", total_time_ms)),
        response_time_ms: Some(total_time_ms),
    });
    
    HealthCheckResult {
        status: overall_status,
        checks,
        timestamp: chrono::Utc::now(),
    }
}

async fn check_database_health(repository: &Arc<Repository>) -> ComponentHealth {
    let start = std::time::Instant::now();
    
    match sqlx::query("SELECT 1").fetch_one(repository.get_pool()).await {
        Ok(_) => ComponentHealth {
            name: "PostgreSQL".to_string(),
            status: HealthStatus::Healthy,
            message: None,
            response_time_ms: Some(start.elapsed().as_millis() as u64), // GUNAKAN!
        },
        Err(e) => ComponentHealth {
            name: "PostgreSQL".to_string(),
            status: HealthStatus::Unhealthy,
            message: Some(format!("Database error: {}", e)),
            response_time_ms: None,
        },
    }
}

async fn check_cache_health(cache_manager: &Arc<CacheManager>) -> ComponentHealth {
    ComponentHealth {
        name: "Redis Cache".to_string(),
        status: if cache_manager.is_using_redis() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Degraded
        },
        message: None,
        response_time_ms: Some(1),
    }
}

async fn check_service_discovery(service_registry: &Arc<ServiceRegistry>) -> ComponentHealth {
    let start = std::time::Instant::now();
    let status = service_registry.get_status().await;
    
    // Check if any service registered
    let is_healthy = !status.as_object().unwrap().is_empty();
    
    ComponentHealth {
        name: "Service Discovery".to_string(),
        status: if is_healthy { 
            HealthStatus::Healthy 
        } else { 
            HealthStatus::Unhealthy 
        },
        message: Some(format!("{} services registered", status.as_object().unwrap().len())),
        response_time_ms: Some(start.elapsed().as_millis() as u64),
    }
}

async fn check_circuit_breakers(circuit_manager: &Arc<CircuitBreakerManager>) -> ComponentHealth {
    let start = std::time::Instant::now();
    let metrics = circuit_manager.get_all_metrics().await;
    
    // Check if any circuit is open
    let open_circuits = metrics.iter()
        .filter(|m| m["state"].as_str() == Some("Open"))
        .count();
    
    ComponentHealth {
        name: "Circuit Breakers".to_string(),
        status: if open_circuits == 0 {
            HealthStatus::Healthy
        } else if open_circuits < metrics.len() / 2 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        },
        message: Some(format!("{}/{} circuits healthy", 
            metrics.len() - open_circuits, metrics.len())),
        response_time_ms: Some(start.elapsed().as_millis() as u64),
    }
}

async fn check_external_services(service_registry: &Arc<ServiceRegistry>) -> ComponentHealth {
    let start = std::time::Instant::now();
    
    // Check book service health
    let book_healthy = service_registry
        .get_healthy_instance("book-service")
        .await
        .is_ok();
    
    // Check auth service health  
    let auth_healthy = service_registry
        .get_healthy_instance("auth-service")
        .await
        .is_ok();
    
    let all_healthy = book_healthy && auth_healthy;
    
    ComponentHealth {
        name: "External Services".to_string(),
        status: if all_healthy {
            HealthStatus::Healthy
        } else if book_healthy || auth_healthy {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        },
        message: Some(format!("Book: {}, Auth: {}", 
            if book_healthy { "UP" } else { "DOWN" },
            if auth_healthy { "UP" } else { "DOWN" }
        )),
        response_time_ms: Some(start.elapsed().as_millis() as u64),
    }
}

fn determine_overall_status(checks: &HashMap<String, ComponentHealth>) -> HealthStatus {
    for (_, health) in checks {
        if matches!(health.status, HealthStatus::Unhealthy) {
            return HealthStatus::Unhealthy;
        }
    }
    
    for (_, health) in checks {
        if matches!(health.status, HealthStatus::Degraded) {
            return HealthStatus::Degraded;
        }
    }
    
    HealthStatus::Healthy
}