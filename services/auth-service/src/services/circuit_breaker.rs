// /pdf-bookstore/services/auth-service/src/services/circuit_breaker.rs

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use std::collections::HashMap;
use crate::utils::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
pub struct CircuitStats {
    pub success_count: u32,
    pub failure_count: u32,
    pub consecutive_failures: u32,
    pub last_failure_time: Option<Instant>,
    pub state_changed_at: Instant,
    pub total_requests: u64,
    pub total_response_time: Duration,
}

#[derive(Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout_duration: Duration,
    pub half_open_max_calls: u32,
    pub max_retries: u32,
    pub retry_delay: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout_duration: Duration::from_secs(30),
            half_open_max_calls: 3,
            max_retries: 3,
            retry_delay: Duration::from_millis(100),
        }
    }
}

pub struct CircuitBreaker {
    name: String,
    state: Arc<RwLock<CircuitState>>,
    stats: Arc<RwLock<CircuitStats>>,
    config: CircuitBreakerConfig,
    half_open_calls: Arc<RwLock<u32>>,
}

impl CircuitBreaker {
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
                total_requests: 0,
                total_response_time: Duration::ZERO,
            })),
            config,
            half_open_calls: Arc::new(RwLock::new(0)),
        }
    }

    /// Execute function dengan circuit breaker protection
    pub async fn call<F, T>(&self, f: F) -> AppResult<T>
    where
        F: std::future::Future<Output = AppResult<T>>,
    {
        let current_state = self.get_state().await;
        
        match current_state {
            CircuitState::Open => {
                let stats = self.stats.read().await;
                if stats.state_changed_at.elapsed() >= self.config.timeout_duration {
                    drop(stats);
                    self.transition_to_half_open().await;
                } else {
                    return Err(AppError::ExternalService(
                        format!("Circuit breaker untuk {} sedang OPEN", self.name)
                    ));
                }
            }
            CircuitState::HalfOpen => {
                let mut calls = self.half_open_calls.write().await;
                if *calls >= self.config.half_open_max_calls {
                    return Err(AppError::ExternalService(
                        format!("Circuit breaker {} dalam HALF-OPEN, max calls reached", self.name)
                    ));
                }
                *calls += 1;
            }
            CircuitState::Closed => {}
        }

        let start = Instant::now();
        match f.await {
            Ok(result) => {
                self.record_success(start.elapsed()).await;
                Ok(result)
            }
            Err(error) => {
                self.record_failure().await;
                Err(error)
            }
        }
    }

    /// Execute dengan retry mechanism (production enhancement)
    pub async fn call_with_retry<F, Fut, T>(&self, mut f: F) -> AppResult<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = AppResult<T>>,
    {
        let mut last_error = None;
        let mut retry_delay = self.config.retry_delay;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                tracing::warn!("Retry attempt {} untuk {}", attempt, self.name);
                tokio::time::sleep(retry_delay).await;
                retry_delay *= 2; // Exponential backoff
            }

            match self.call(f()).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                    if attempt == self.config.max_retries {
                        break;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| 
            AppError::ExternalService(format!("Max retries exceeded untuk {}", self.name))
        ))
    }

    async fn get_state(&self) -> CircuitState {
        self.state.read().await.clone()
    }

    async fn record_success(&self, response_time: Duration) {
        let mut stats = self.stats.write().await;
        let mut state = self.state.write().await;
        
        stats.success_count += 1;
        stats.consecutive_failures = 0;
        stats.total_requests += 1;
        stats.total_response_time += response_time;
        
        if *state == CircuitState::HalfOpen {
            if stats.success_count >= self.config.success_threshold {
                *state = CircuitState::Closed;
                stats.state_changed_at = Instant::now();
                *self.half_open_calls.write().await = 0;
                tracing::info!("Circuit breaker {} transisi ke CLOSED", self.name);
            }
        }
    }

    async fn record_failure(&self) {
        let mut stats = self.stats.write().await;
        let mut state = self.state.write().await;
        
        stats.failure_count += 1;
        stats.consecutive_failures += 1;
        stats.last_failure_time = Some(Instant::now());
        stats.total_requests += 1;
        
        if stats.consecutive_failures >= self.config.failure_threshold {
            if *state != CircuitState::Open {
                *state = CircuitState::Open;
                stats.state_changed_at = Instant::now();
                tracing::warn!(
                    "Circuit breaker {} transisi ke OPEN setelah {} failures", 
                    self.name, stats.consecutive_failures
                );
            }
        }
    }

    async fn transition_to_half_open(&self) {
        let mut state = self.state.write().await;
        let mut stats = self.stats.write().await;
        
        *state = CircuitState::HalfOpen;
        stats.success_count = 0;
        stats.failure_count = 0;
        stats.state_changed_at = Instant::now();
        *self.half_open_calls.write().await = 0;
        
        tracing::info!("Circuit breaker {} transisi ke HALF-OPEN", self.name);
    }

    pub async fn get_metrics(&self) -> serde_json::Value {
        let state = self.state.read().await;
        let stats = self.stats.read().await;
        
        let avg_response_time = if stats.total_requests > 0 {
            stats.total_response_time.as_millis() as f64 / stats.total_requests as f64
        } else {
            0.0
        };
        
        serde_json::json!({
            "name": self.name,
            "state": format!("{:?}", *state),
            "success_count": stats.success_count,
            "failure_count": stats.failure_count,
            "consecutive_failures": stats.consecutive_failures,
            "total_requests": stats.total_requests,
            "avg_response_time_ms": avg_response_time,
            "state_changed_at": stats.state_changed_at.elapsed().as_secs(),
            "last_failure": stats.last_failure_time.map(|t| t.elapsed().as_secs()),
        })
    }
}

pub struct CircuitBreakerManager {
    breakers: Arc<RwLock<HashMap<String, Arc<CircuitBreaker>>>>,
    default_config: CircuitBreakerConfig,
}

impl CircuitBreakerManager {
    pub fn new() -> Self {
        Self {
            breakers: Arc::new(RwLock::new(HashMap::new())),
            default_config: CircuitBreakerConfig::default(),
        }
    }

    pub async fn get_or_create(&self, service_name: &str) -> Arc<CircuitBreaker> {
        let mut breakers = self.breakers.write().await;
        
        if let Some(breaker) = breakers.get(service_name) {
            return breaker.clone();
        }
        
        // Custom config untuk service tertentu
        let config = match service_name {
            "payment-service" => CircuitBreakerConfig {
                failure_threshold: 3, // Payment lebih sensitif
                max_retries: 2,
                ..self.default_config.clone()
            },
            "book-service" => CircuitBreakerConfig {
                failure_threshold: 10, // Book service lebih toleran
                max_retries: 5,
                ..self.default_config.clone()
            },
            _ => self.default_config.clone(),
        };
        
        let breaker = Arc::new(CircuitBreaker::new(
            service_name.to_string(),
            config,
        ));
        
        breakers.insert(service_name.to_string(), breaker.clone());
        breaker
    }

    pub async fn get_all_metrics(&self) -> Vec<serde_json::Value> {
        let breakers = self.breakers.read().await;
        let mut metrics = Vec::new();
        
        for breaker in breakers.values() {
            metrics.push(breaker.get_metrics().await);
        }
        
        metrics
    }
}