// /pdf-bookstore/services/payment-service/src/repository/audit.rs

use sqlx::{PgPool, Transaction, Postgres};
use uuid::Uuid;
use crate::utils::error::AppResult;

/// Repository untuk audit logging
pub struct AuditRepository {
    pool: PgPool,
}

impl AuditRepository {
    /// Create new audit repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool: pool.clone() } 
    }
    
    /// Log order created
    pub async fn log_order_created(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        user_id: Uuid,
        order_id: Uuid,
        order_number: &str,
    ) -> AppResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO audit_logs (user_id, action, resource_type, resource_id, details)
            VALUES ($1, 'ORDER_CREATED', 'order', $2, $3)
            "#,
            user_id,
            order_id,
            serde_json::json!({ "order_number": order_number })
        )
        .execute(&mut **tx)
        .await?;
        
        Ok(())
    }
    
    /// Log order cancelled
    pub async fn log_order_cancelled(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        user_id: Uuid,
        order_id: Uuid,
    ) -> AppResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO audit_logs (user_id, action, resource_type, resource_id, details)
            VALUES ($1, 'ORDER_CANCELLED', 'order', $2, $3)
            "#,
            user_id,
            order_id,
            serde_json::json!({ "cancelled_at": chrono::Utc::now() })
        )
        .execute(&mut **tx)
        .await?;
        
        Ok(())
    }
    
    /// Log webhook processed
    pub async fn log_webhook_processed(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: Uuid,
        transaction_id: &str,
    ) -> AppResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO audit_logs (action, resource_type, resource_id, details)
            VALUES ('WEBHOOK_PROCESSED', 'order', $1, $2)
            "#,
            order_id,
            serde_json::json!({ "transaction_id": transaction_id })
        )
        .execute(&mut **tx)
        .await?;
        
        Ok(())
    }

    // Tambah method untuk direct query kalau perlu
    pub async fn custom_audit_query(&self, query: &str) -> AppResult<()> {
        sqlx::query(query)
            .execute(&self.pool) 
            .await?;
        Ok(())
    }
}