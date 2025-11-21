// /pdf-bookstore/services/auth-service/src/services/service_discovery.rs

use std::sync::Arc;
use rand::Rng;
use tokio::sync::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use crate::utils::error::{AppError, AppResult};

/// Informasi instance service
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
    // Production enhancements
    pub response_time_ms: u64,      
    pub failed_checks: u32,          
    pub weight: u32,                 
}

impl ServiceInstance {
    /// Mendapatkan URL lengkap service
    pub fn get_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
    
    /// Menghitung score untuk load balancing berdasarkan response time dan health
    pub fn calculate_score(&self) -> f64 {
        if !self.is_healthy {
            return 0.0;
        }
        
        // Score berdasarkan response time (makin cepat makin tinggi)
        let base_score = 1000.0 / (self.response_time_ms as f64 + 1.0);
        
        // Aplikasikan weight
        base_score * (self.weight as f64 / 100.0)
    }
}

/// Registry untuk menyimpan dan manage service instances
pub struct ServiceRegistry {
    instances: Arc<RwLock<HashMap<String, Vec<ServiceInstance>>>>,
    http_client: reqwest::Client,
    health_check_interval: Duration,
    max_failed_checks: u32,
}

impl ServiceRegistry {
    /// Membuat service registry baru
    pub fn new() -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            instances: Arc::new(RwLock::new(HashMap::new())),
            http_client,
            health_check_interval: Duration::from_secs(30),
            max_failed_checks: 3,
        }
    }

    /// Register service instance baru
    pub async fn register(&self, mut instance: ServiceInstance) {
        // Set default weight jika belum ada
        if instance.weight == 0 {
            instance.weight = 100;
        }
        
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

    /// Mendapatkan instance yang healthy dengan weighted round-robin
    pub async fn get_healthy_instance(&self, service_name: &str) -> AppResult<ServiceInstance> {
        let mut instances = self.instances.write().await;
        
        let service_instances = instances.get_mut(service_name)
            .ok_or_else(|| AppError::NotFound(format!("Service {} tidak ditemukan", service_name)))?;
        
        // Filter hanya instance yang healthy
        let healthy_instances: Vec<&ServiceInstance> = service_instances
            .iter()
            .filter(|i| i.is_healthy)
            .collect();
        
        if healthy_instances.is_empty() {
            return Err(AppError::ExternalService(
                format!("Tidak ada instance healthy untuk service {}", service_name)
            ));
        }
        
        // Weighted selection berdasarkan response time
        let selected = self.weighted_select(&healthy_instances);
        Ok(selected.clone())
    }

    /// Memilih instance berdasarkan weight dan response time
    fn weighted_select<'a>(&self, instances: &[&'a ServiceInstance]) -> &'a ServiceInstance {
        // Kalau cuma 1 instance, langsung return
        if instances.len() == 1 {
            return instances[0];
        }
        
        // Hitung total score
        let total_score: f64 = instances.iter()
            .map(|i| i.calculate_score())
            .sum();
        
        if total_score == 0.0 {
            let index = (std::time::Instant::now().elapsed().as_secs() as usize) % instances.len();
            return instances[index];
        }
        
        // Weighted random selection
        let mut rng = rand::rng();
        let mut random = rng.random::<f64>() * total_score;
        
        for instance in instances {
            random -= instance.calculate_score();
            if random <= 0.0 {
                return instance;
            }
        }
        
        // Fallback (seharusnya tidak pernah sampai sini)
        instances[0]
    }

    /// Melakukan health check untuk semua instances
    pub async fn health_check_all(&self) {
        let mut instances = self.instances.write().await;
        
        for service_instances in instances.values_mut() {
            for instance in service_instances.iter_mut() {
                let start = Instant::now();
                let health = self.check_instance_health(instance).await;
                let response_time = start.elapsed().as_millis() as u64;
                
                // Update response time dengan moving average
                instance.response_time_ms = (instance.response_time_ms * 9 + response_time) / 10;
                
                if health {
                    instance.is_healthy = true;
                    instance.failed_checks = 0;
                } else {
                    instance.failed_checks += 1;
                    
                    // Tandai unhealthy setelah max failed checks
                    if instance.failed_checks >= self.max_failed_checks {
                        instance.is_healthy = false;
                        tracing::warn!(
                            "Instance {} dari service {} marked unhealthy setelah {} failed checks", 
                            instance.id, instance.name, instance.failed_checks
                        );
                    }
                }
                
                instance.last_health_check = Some(Instant::now());
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
            Err(e) => {
                tracing::debug!("Health check failed untuk {}: {}", instance.id, e);
                false
            }
        }
    }

    /// Memulai background task untuk health check otomatis
    pub async fn start_health_check_background(&self) {
        let registry = self.clone();
        let interval = self.health_check_interval;
        
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            
            loop {
                interval_timer.tick().await;
                registry.health_check_all().await;
            }
        });
        
        tracing::info!("Background health check started dengan interval {:?}", interval);
    }

    /// Initialize dengan default services untuk development
    pub async fn init_default_services(&self) {
        // Auth Service (diri sendiri)
        self.register(ServiceInstance {
            id: "auth-service-1".to_string(),
            name: "auth-service".to_string(),
            host: std::env::var("AUTH_SERVICE_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: 3001,
            health_check_url: "http://localhost:3001/health".to_string(),
            is_healthy: true,
            last_health_check: None,
            metadata: HashMap::new(),
            response_time_ms: 50,
            failed_checks: 0,
            weight: 100,
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
            response_time_ms: 50,
            failed_checks: 0,
            weight: 100,
        }).await;

        // Payment Service
        self.register(ServiceInstance {
            id: "payment-service-1".to_string(),
            name: "payment-service".to_string(),
            host: std::env::var("PAYMENT_SERVICE_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: 3003,
            health_check_url: "http://localhost:3003/health".to_string(),
            is_healthy: true,
            last_health_check: None,
            metadata: HashMap::new(),
            response_time_ms: 50,
            failed_checks: 0,
            weight: 100,
        }).await;
        
        // Start background health checking
        self.start_health_check_background().await;
        
        tracing::info!("Default services registered dengan health check otomatis");
    }

    /// Mendapatkan status registry untuk monitoring
    pub async fn get_status(&self) -> serde_json::Value {
        let instances = self.instances.read().await;
        let mut status = serde_json::json!({});
        
        for (service_name, service_instances) in instances.iter() {
            let healthy_count = service_instances.iter().filter(|i| i.is_healthy).count();
            let total_avg_response_time = if !service_instances.is_empty() {
                service_instances.iter().map(|i| i.response_time_ms).sum::<u64>() / 
                service_instances.len() as u64
            } else {
                0
            };
            
            status[service_name] = serde_json::json!({
                "total_instances": service_instances.len(),
                "healthy_instances": healthy_count,
                "avg_response_time_ms": total_avg_response_time,
                "instances": service_instances.iter().map(|i| serde_json::json!({
                    "id": i.id,
                    "url": i.get_url(),
                    "healthy": i.is_healthy,
                    "response_time_ms": i.response_time_ms,
                    "weight": i.weight,
                    "failed_checks": i.failed_checks,
                    "last_check": i.last_health_check.map(|t| t.elapsed().as_secs()),
                    "score": i.calculate_score(),
                })).collect::<Vec<_>>(),
            });
        }
        
        status
    }
}

// Implementasi Clone untuk background task
impl Clone for ServiceRegistry {
    fn clone(&self) -> Self {
        Self {
            instances: self.instances.clone(),
            http_client: self.http_client.clone(),
            health_check_interval: self.health_check_interval,
            max_failed_checks: self.max_failed_checks,
        }
    }
}