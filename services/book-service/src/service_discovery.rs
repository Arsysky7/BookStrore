// /pdf-bookstore/services/auth-service/src/service_discovery.rs

use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use crate::error::{AppError, AppResult};

/// Service instance information
#[derive(Debug, Clone)]
pub struct ServiceInstance {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub health_check_url: String,
    pub is_healthy: bool,
    pub last_health_check: Option<Instant>,
    pub metadata: HashMap<String, String>,
}

impl ServiceInstance {
    /// Get full URL untuk service
    pub fn get_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

/// Service registry untuk menyimpan service instances
pub struct ServiceRegistry {
    instances: Arc<RwLock<HashMap<String, Vec<ServiceInstance>>>>,
    http_client: reqwest::Client,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            instances: Arc::new(RwLock::new(HashMap::new())),
            http_client,
        }
    }

    /// Register service instance
    pub async fn register(&self, instance: ServiceInstance) {
        let mut instances = self.instances.write().await;
        let service_instances = instances.entry(instance.name.clone()).or_insert_with(Vec::new);
        
        // Cek apakah instance sudah ada
        if !service_instances.iter().any(|i| i.id == instance.id) {
            service_instances.push(instance.clone());
            tracing::info!("Service {} instance {} registered", instance.name, instance.id);
        }
    }

    /// Deregister service instance
    pub async fn deregister(&self, service_name: &str, instance_id: &str) {
        let mut instances = self.instances.write().await;
        
        if let Some(service_instances) = instances.get_mut(service_name) {
            service_instances.retain(|i| i.id != instance_id);
            tracing::info!("Service {} instance {} deregistered", service_name, instance_id);
        }
    }

    /// Get healthy instance dengan load balancing (round-robin)
    pub async fn get_healthy_instance(&self, service_name: &str) -> AppResult<ServiceInstance> {
        let instances = self.instances.read().await;
        
        let service_instances = instances.get(service_name)
            .ok_or_else(|| AppError::NotFound(format!("Service {} tidak ditemukan", service_name)))?;
        
        // Filter healthy instances
        let healthy_instances: Vec<&ServiceInstance> = service_instances
            .iter()
            .filter(|i| i.is_healthy)
            .collect();
        
        if healthy_instances.is_empty() {
            return Err(AppError::ExternalService(
                format!("Tidak ada instance healthy untuk service {}", service_name)
            ));
        }
        
        // Simple round-robin (bisa diganti dengan strategy lain)
        let index = (Instant::now().elapsed().as_secs() as usize) % healthy_instances.len();
        Ok(healthy_instances[index].clone())
    }

    /// Perform health check untuk semua instances
    pub async fn health_check_all(&self) {
        let mut instances = self.instances.write().await;
        
        for service_instances in instances.values_mut() {
            for instance in service_instances.iter_mut() {
                let health = self.check_instance_health(instance).await;
                instance.is_healthy = health;
                instance.last_health_check = Some(Instant::now());
                
                if !health {
                    tracing::warn!("Instance {} dari service {} tidak healthy", 
                        instance.id, instance.name);
                }
            }
        }
    }

    /// Check health untuk satu instance
    async fn check_instance_health(&self, instance: &ServiceInstance) -> bool {
        match self.http_client
            .get(&instance.health_check_url)
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    /// Initialize dengan default services untuk development
    pub async fn init_default_services(&self) {
        // Auth Service
        self.register(ServiceInstance {
            id: "auth-service-1".to_string(),
            name: "auth-service".to_string(),
            host: std::env::var("AUTH_SERVICE_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: 3001,
            health_check_url: "http://localhost:3001/health".to_string(),
            is_healthy: true,
            last_health_check: None,
            metadata: HashMap::new(),
        }).await;

        // Book Service
        self.register(ServiceInstance {
            id: "book-service-1".to_string(),
            name: "book-service".to_string(),
            host: std::env::var("BOOK_SERVICE_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: 3002,
            health_check_url: "http://localhost:3002/health".to_string(),
            is_healthy: true,
            last_health_check: None,
            metadata: HashMap::new(),
        }).await;
        
        tracing::info!("Default services registered");
    }

    /// Get registry status untuk monitoring
    pub async fn get_status(&self) -> serde_json::Value {
        let instances = self.instances.read().await;
        let mut status = serde_json::json!({});
        
        for (service_name, service_instances) in instances.iter() {
            let healthy_count = service_instances.iter().filter(|i| i.is_healthy).count();
            
            status[service_name] = serde_json::json!({
                "total_instances": service_instances.len(),
                "healthy_instances": healthy_count,
                "instances": service_instances.iter().map(|i| serde_json::json!({
                    "id": i.id,
                    "url": i.get_url(),
                    "healthy": i.is_healthy,
                    "last_check": i.last_health_check.map(|t| t.elapsed().as_secs()),
                })).collect::<Vec<_>>(),
            });
        }
        
        status
    }
}