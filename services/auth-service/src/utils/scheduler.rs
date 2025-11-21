// /pdf-bookstore/services/auth-service/src/utils/scheduler.rs

use tokio_cron_scheduler::{JobScheduler, Job};
use sqlx::PgPool;

pub async fn start_token_cleanup_job(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let scheduler = JobScheduler::new().await?;

    // Job 1: Cleanup expired tokens setiap jam
    let pool_clone1 = pool.clone();
    let cleanup_job = Job::new_async("0 0 * * * *", move |_uuid, _l| {
        let pool = pool_clone1.clone();
        Box::pin(async move {
            match cleanup_expired_tokens(&pool).await {
                Ok(result) => {
                    tracing::info!("Token cleanup result: {}", result);
                }
                Err(e) => {
                    tracing::error!("Token cleanup failed: {}", e);
                }
            }
        })
    })?;

    scheduler.add(cleanup_job).await?;

    // Job 2: Cleanup expired sessions setiap 30 menit
    let pool_clone2 = pool.clone();
    let session_cleanup_job = Job::new_async("0 */30 * * * *", move |_uuid, _l| {
        let pool = pool_clone2.clone();
        Box::pin(async move {
            match cleanup_expired_sessions(&pool).await {
                Ok(count) => {
                    if count > 0 {
                        tracing::info!("Cleaned up {} expired sessions", count);
                    }
                }
                Err(e) => {
                    tracing::error!("Session cleanup failed: {}", e);
                }
            }
        })
    })?;

    scheduler.add(session_cleanup_job).await?;

    // Job 3: Cleanup old inactive sessions (>30 days)
    let pool_clone3 = pool.clone();
    let old_session_cleanup_job = Job::new_async("0 0 2 * * *", move |_uuid, _l| {
        let pool = pool_clone3.clone();
        Box::pin(async move {
            match cleanup_old_sessions(&pool).await {
                Ok(count) => {
                    if count > 0 {
                        tracing::info!("Cleaned up {} old sessions", count);
                    }
                }
                Err(e) => {
                    tracing::error!("Old session cleanup failed: {}", e);
                }
            }
        })
    })?;

    scheduler.add(old_session_cleanup_job).await?;

    scheduler.start().await?;

    tracing::info!("✅ Token & session cleanup scheduler started");
    Ok(())
}

async fn cleanup_expired_tokens(pool: &PgPool) -> Result<String, sqlx::Error> {
    let result = sqlx::query!(
        "SELECT cleanup_expired_tokens() as result"
    )
    .fetch_one(pool)
    .await?;

    // Return JSON as string
    Ok(result.result.map(|v| v.to_string()).unwrap_or_else(|| "{}".to_string()))
}

/// Cleanup expired sessions (past expires_at timestamp)
async fn cleanup_expired_sessions(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        DELETE FROM sessions
        WHERE expires_at < NOW()
        OR (is_active = false AND last_used_at < NOW() - INTERVAL '7 days')
        "#
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() as i64)
}

/// Cleanup old inactive sessions (>30 days)
async fn cleanup_old_sessions(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        DELETE FROM sessions
        WHERE created_at < NOW() - INTERVAL '30 days'
        "#
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() as i64)
}