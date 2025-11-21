// /pdf-bookstore/services/api-gateway/src/circuit_breaker.rs

use crate::error::{AppError, AppResult};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use std::collections::HashMap;


/// State dari circuit breaker
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,     
    Open,       
    HalfOpen,   
}

/// Statistik untuk circuit breaker
#[derive(Debug, Clone)]
pub struct CircuitStats {
    pub success_count: u32,
    pub failure_count: u32,
    pub consecutive_failures: u32,
    pub last_failure_time: Option<Instant>,
    pub state_changed_at: Instant,
}

/// Circuit breaker untuk satu service
pub struct CircuitBreaker {
    name: String,
    state: Arc<RwLock<CircuitState>>,
    stats: Arc<RwLock<CircuitStats>>,
    config: CircuitBreakerConfig,
}

/// Konfigurasi circuit breaker
#[derive(Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,   
    pub success_threshold: u32,      
    pub timeout_duration: Duration, 
    pub half_open_max_calls: u32,  
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout_duration: Duration::from_secs(30),
            half_open_max_calls: 3,
        }
    }
}

impl CircuitBreaker {
    /// Buat circuit breaker baru untuk service tertentu
    pub fn new(name: String, config: CircuitBreakerConfig) -> Self {
        Self {
            name,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            stats: Arc::new(RwLock::new(CircuitStats {
                success_count: 0,
                failure_count: 0,
                consecutive_failures: 0,
                last_failure_time: None,
                state_changed_at: Instant::now(),
            })),
            config,
        }
    }

    /// Execute function dengan circuit breaker protection
    pub async fn call<F, T>(&self, f: F) -> AppResult<T>
    where
        F: std::future::Future<Output = AppResult<T>>,
    {
        // Cek state circuit
        let current_state = self.get_state().await;
        
        match current_state {
            CircuitState::Open => {
                // Cek apakah sudah waktunya untuk coba lagi
                let stats = self.stats.read().await;
                if stats.state_changed_at.elapsed() >= self.config.timeout_duration {
                    drop(stats); // Release lock
                    self.transition_to_half_open().await;
                } else {
                    return Err(AppError::ExternalService(
                        format!("Circuit breaker untuk {} sedang OPEN", self.name)
                    ));
                }
            }
            CircuitState::HalfOpen => {
                // Limit concurrent calls saat half-open
                tracing::info!("Circuit breaker {} dalam state HALF-OPEN, testing...", self.name);
            }
            CircuitState::Closed => {
                // Normal operation
            }
        }

        // Execute function
        match f.await {
            Ok(result) => {
                self.record_success().await;
                Ok(result)
            }
            Err(error) => {
                self.record_failure().await;
                Err(error)
            }
        }
    }

    /// Get current state
    async fn get_state(&self) -> CircuitState {
        self.state.read().await.clone()
    }

    /// Record successful call
    async fn record_success(&self) {
        let mut stats = self.stats.write().await;
        let mut state = self.state.write().await;
        
        stats.success_count += 1;
        stats.consecutive_failures = 0;
        
        // Kalau half-open dan sudah cukup success, transition ke closed
        if *state == CircuitState::HalfOpen {
            if stats.success_count >= self.config.success_threshold {
                *state = CircuitState::Closed;
                stats.state_changed_at = Instant::now();
                tracing::info!("Circuit breaker {} transisi ke CLOSED", self.name);
            }
        }
    }

    /// Record failed call
    async fn record_failure(&self) {
        let mut stats = self.stats.write().await;
        let mut state = self.state.write().await;
        
        stats.failure_count += 1;
        stats.consecutive_failures += 1;
        stats.last_failure_time = Some(Instant::now());
        
        // Kalau failures melebihi threshold, open circuit
        if stats.consecutive_failures >= self.config.failure_threshold {
            if *state != CircuitState::Open {
                *state = CircuitState::Open;
                stats.state_changed_at = Instant::now();
                tracing::warn!(
                    "Circuit breaker {} transisi ke OPEN setelah {} failures berturut-turut", 
                    self.name, stats.consecutive_failures
                );
            }
        }
    }

    /// Transition ke half-open untuk testing
    async fn transition_to_half_open(&self) {
        let mut state = self.state.write().await;
        let mut stats = self.stats.write().await;
        
        *state = CircuitState::HalfOpen;
        stats.success_count = 0;
        stats.failure_count = 0;
        stats.state_changed_at = Instant::now();
        
        tracing::info!("Circuit breaker {} transisi ke HALF-OPEN untuk testing", self.name);
    }

    /// Get circuit breaker metrics untuk monitoring
    pub async fn get_metrics(&self) -> serde_json::Value {
        let state = self.state.read().await;
        let stats = self.stats.read().await;
        
        serde_json::json!({
            "name": self.name,
            "state": format!("{:?}", *state),
            "success_count": stats.success_count,
            "failure_count": stats.failure_count,
            "consecutive_failures": stats.consecutive_failures,
            "state_changed_at": stats.state_changed_at.elapsed().as_secs(),
            "last_failure": stats.last_failure_time.map(|t| t.elapsed().as_secs()),
        })
    }
}

/// Circuit breaker manager untuk manage multiple circuit breakers
pub struct CircuitBreakerManager {
    breakers: Arc<RwLock<HashMap<String, Arc<CircuitBreaker>>>>,
}

impl CircuitBreakerManager {
    pub fn new() -> Self {
        Self {
            breakers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get atau create circuit breaker untuk service
    pub async fn get_or_create(&self, service_name: &str) -> Arc<CircuitBreaker> {
        let mut breakers = self.breakers.write().await;
        
        if let Some(breaker) = breakers.get(service_name) {
            return breaker.clone();
        }
        
        let breaker = Arc::new(CircuitBreaker::new(
            service_name.to_string(),
            CircuitBreakerConfig::default(),
        ));
        
        breakers.insert(service_name.to_string(), breaker.clone());
        breaker
    }

    /// Get semua circuit breaker metrics
    pub async fn get_all_metrics(&self) -> Vec<serde_json::Value> {
        let breakers = self.breakers.read().await;
        let mut metrics = Vec::new();
        
        for breaker in breakers.values() {
            metrics.push(breaker.get_metrics().await);
        }
        
        metrics
    }
}