// /pdf-bookstore/services/payment-service/src/utils/scheduler.rs

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use tokio_cron_scheduler::{JobScheduler, Job};
use crate::{
    repository::Repository,
    utils::error::AppResult,
};

// Scheduler metrics for monitoring
pub struct SchedulerMetrics {
    pub cleanup_runs: AtomicU64,
    pub cleanup_errors: AtomicU64,
    pub stats_runs: AtomicU64,
    pub last_cleanup: RwLock<Option<chrono::DateTime<chrono::Utc>>>,
}

impl SchedulerMetrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            cleanup_runs: AtomicU64::new(0),
            cleanup_errors: AtomicU64::new(0),
            stats_runs: AtomicU64::new(0),
            last_cleanup: RwLock::new(None),
        })
    }
    
    pub async fn get_status(&self) -> serde_json::Value {
        let last_cleanup = self.last_cleanup.read().await;
        serde_json::json!({
            "cleanup_runs": self.cleanup_runs.load(Ordering::Relaxed),
            "cleanup_errors": self.cleanup_errors.load(Ordering::Relaxed),
            "stats_runs": self.stats_runs.load(Ordering::Relaxed),
            "last_cleanup": *last_cleanup,
            "status": "running"
        })
    }
}

/// Start background jobs untuk maintenance tasks
pub async fn start_background_jobs(repository: Arc<Repository>) -> AppResult<()> {
    let scheduler = JobScheduler::new().await
        .map_err(|e| crate::utils::error::AppError::Configuration(
            format!("Failed to create scheduler: {}", e)
        ))?;
    
    // Job 1: Cleanup expired orders setiap 1 jam
    let repo_clone = repository.clone();
    let cleanup_job = Job::new_async("0 0 */1 * * *", move |_uuid, _l| {
        let repo = repo_clone.clone();
        Box::pin(async move {
            if let Err(e) = cleanup_expired_orders_job(repo, SchedulerMetrics::new()).await {
                tracing::error!("Failed to cleanup expired orders: {}", e);
            }
            tracing::info!("Cleanup expired orders job completed");
        })
    })
    .map_err(|e| crate::utils::error::AppError::Configuration(
        format!("Failed to create cleanup job: {}", e)
    ))?;
    
    scheduler.add(cleanup_job).await
        .map_err(|e| crate::utils::error::AppError::Configuration(
            format!("Failed to add cleanup job: {}", e)
        ))?;
    
    // Job 2: Log statistics setiap hari jam 00:00
    let repo_clone2 = repository.clone();
    let stats_job = Job::new_async("0 0 0 * * *", move |_uuid, _l| {
        let repo = repo_clone2.clone();
        Box::pin(async move {
            if let Err(e) = daily_stats_job(repo).await {
                tracing::error!("Failed to log daily stats: {}", e);
            }
        })
    })
    .map_err(|e| crate::utils::error::AppError::Configuration(
        format!("Failed to create stats job: {}", e)
    ))?;
    
    scheduler.add(stats_job).await
        .map_err(|e| crate::utils::error::AppError::Configuration(
            format!("Failed to add stats job: {}", e)
        ))?;

    // Job 3: Clean token cache setiap 30 menit
    let cache_cleanup_job = Job::new_async("0 */30 * * * *", move |_uuid, _l| {
        Box::pin(async move {
            crate::middleware::auth::clean_token_cache().await;
            tracing::debug!("Token cache cleanup completed");
        })
    })
    .map_err(|e| crate::utils::error::AppError::Configuration(
        format!("Failed to create cache cleanup job: {}", e)
    ))?;

    scheduler.add(cache_cleanup_job).await
        .map_err(|e| crate::utils::error::AppError::Configuration(
            format!("Failed to add cache cleanup job: {}", e)
        ))?;

    // Start scheduler
    scheduler.start().await
        .map_err(|e| crate::utils::error::AppError::Configuration(
            format!("Failed to start scheduler: {}", e)
        ))?;
    
    tracing::info!("✅ Background jobs scheduler started");
    
    // Keep scheduler running in background
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    });
    
    Ok(())
}
/// Health status structure
#[derive(Debug)]
#[allow(dead_code)]
struct HealthStatus {
    database_connected: bool,
    total_orders: i64,
    pending_orders: i64,
    timestamp: DateTime<Utc>,
}

async fn check_system_health(repository: &Arc<Repository>) -> AppResult<HealthStatus> {
    // Test database connection
    let database_connected = match sqlx::query("SELECT 1")
        .fetch_one(&repository.pool)
        .await {
        Ok(_) => true,
        Err(_) => false,
    };
    
    // Get basic stats jika DB connected
    let (total_orders, pending_orders) = if database_connected {
        let stats = repository.order().get_admin_stats().await?;
        (stats.total_orders, stats.pending_orders)
    } else {
        (0, 0)
    };
    
    Ok(HealthStatus {
        database_connected,
        total_orders,
        pending_orders,
        timestamp: Utc::now(),
    })
}

/// Background job: Cleanup expired orders
async fn cleanup_expired_orders_job(
    repository: Arc<Repository>,
    metrics: Arc<SchedulerMetrics>,
) -> AppResult<()> {
    tracing::debug!("Starting cleanup expired orders job");

    let health = check_system_health(&repository).await?;
    
    if !health.database_connected {
        tracing::error!("Database not connected, skipping cleanup");
        return Err(crate::utils::error::AppError::Database("Database disconnected".to_string()));
    }

    // Log current system status
    tracing::info!("System health: {} total orders, {} pending", 
        health.total_orders, health.pending_orders);
    
    match repository.order().cleanup_expired_orders().await {
        Ok(expired_count) => {
            metrics.cleanup_runs.fetch_add(1, Ordering::Relaxed);
            *metrics.last_cleanup.write().await = Some(chrono::Utc::now());
            
            if expired_count > 0 {
                tracing::info!("Cleaned up {} expired orders", expired_count);
            }
            Ok(())
        }
        Err(e) => {
            metrics.cleanup_errors.fetch_add(1, Ordering::Relaxed);
            tracing::error!("Cleanup job failed: {}", e);
            Err(e)
        }
    }
}

async fn daily_stats_job(repository: Arc<Repository>) -> AppResult<()> {
    tracing::debug!("Starting daily stats logging job");
    
    let stats = repository.order().get_admin_stats().await?;
    
    tracing::info!("Daily Stats: {} orders, {} revenue", stats.total_orders, stats.total_revenue);
    
    Ok(())
}

/// Create maintenance job untuk database optimization
pub async fn trigger_maintenance_job(repository: Arc<Repository>) -> AppResult<()> {
    tracing::info!("Starting maintenance job untuk cleanup old payment logs");

    let cleanup_result = sqlx::query!(
        r#"
        DELETE FROM payment_logs 
        WHERE created_at < NOW() - INTERVAL '90 days'
        RETURNING id
        "#
    )
    .fetch_all(&repository.pool)
    .await;
    
    match cleanup_result {
        Ok(deleted_rows) => {
            let count = deleted_rows.len();
            if count > 0 {
                tracing::info!("Maintenance job: {} payment logs lama berhasil dihapus", count);
            } else {
                tracing::debug!("Maintenance job: Tidak ada payment logs lama untuk dihapus");
            }
        }
        Err(e) => {
            tracing::error!("Maintenance job gagal: {}", e);
            return Err(crate::utils::error::AppError::Database(e.to_string()));
        }
    }

    if let Err(e) = sqlx::query("VACUUM ANALYZE payment_logs")
        .execute(&repository.pool)
        .await {
        tracing::warn!("Vacuum payment_logs gagal: {}", e);
    }
    
    tracing::info!("Maintenance job selesai");
    Ok(())
}