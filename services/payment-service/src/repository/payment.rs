// /pdf-bookstore/services/payment-service/src/repository/payment.rs

use sqlx::{PgPool, Transaction, Postgres, Row};
use uuid::Uuid;
use bigdecimal::BigDecimal;
use crate::{
    models::{MidtransWebhookPayload},
    utils::error::{AppError, AppResult},
};
use std::time::Duration;

/// Repository untuk payment operations
pub struct PaymentRepository {
    pool: PgPool,
}

impl PaymentRepository {
    /// Create new payment repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Check apakah user sudah beli book
    pub async fn has_user_purchased_book(&self, user_id: Uuid, book_id: Uuid) -> AppResult<bool> {
        let result = sqlx::query!(
            "SELECT COUNT(*) as count FROM user_purchases WHERE user_id = $1 AND book_id = $2",
            user_id,
            book_id
        )
        .fetch_one(&self.pool)
        .await?;
        
        Ok(result.count.unwrap_or(0) > 0)
    }
    
    /// Complete payment dengan atomic function
    pub async fn complete_payment_atomic(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_number: &str,
        transaction_id: &str,
        webhook_data: Option<serde_json::Value>,
    ) -> AppResult<()> {
        let mut retries = 3;
        loop {
            let query = r#"
                SELECT * FROM complete_payment_atomic($1, $2, $3) as result
            "#;
            
            match sqlx::query(query)
                .bind(order_number)
                .bind(transaction_id)
                .bind(&webhook_data)
                .fetch_one(&mut **tx)
                .await 
            {
                Ok(row) => {
                    let json_result: serde_json::Value = row.try_get("result")?;
                    
                    if !json_result["success"].as_bool().unwrap_or(false) {
                        let error = json_result["error"].as_str()
                            .unwrap_or("Unknown error");
                        
                        // Check untuk idempotent response
                        if error == "ALREADY_PROCESSED" {
                            tracing::info!("Payment already processed: {}", order_number);
                            return Ok(()); // Idempotent, ga error
                        }
                        
                        return Err(AppError::Database(error.to_string()));
                    }
                    
                    tracing::info!("Payment completed: {}", order_number);
                    return Ok(());
                }
                Err(e) if retries > 0 && e.to_string().contains("deadlock") => {
                    retries -= 1;
                    tracing::warn!("Deadlock detected, retrying... ({})", retries);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => return Err(AppError::Database(e.to_string())),
            }
        }
    }
    
    /// Log payment webhook dengan enhanced validation
    pub async fn log_payment_webhook(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        payload: &MidtransWebhookPayload,
        order_id: Option<Uuid>,
    ) -> AppResult<()> {
        // Parse amount dengan validation
        let gross_amount = payload.gross_amount
            .parse::<f64>()
            .map_err(|_| AppError::BadRequest("Invalid gross amount format".to_string()))?
            .max(0.0); 
        
        let gross_amount_decimal = BigDecimal::from(gross_amount as i64);
     
        // Parse settlement time dengan fallback
        let settlement_time = payload.settlement_time.as_ref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));
        
        sqlx::query!(
            r#"
            INSERT INTO payment_logs (
                order_id, transaction_id, payment_type, gross_amount,
                transaction_status, fraud_status, settlement_time, webhook_data
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            order_id,
            payload.transaction_id,
            payload.payment_type,
            gross_amount_decimal,
            payload.transaction_status,
            payload.fraud_status,
            settlement_time, 
            serde_json::to_value(payload).ok()
        )
        .execute(&mut **tx)
        .await?;
        
        tracing::debug!("Payment webhook logged for transaction: {}", payload.transaction_id);
        Ok(())
    }

    /// Check apakah webhook sudah pernah diproses
    pub async fn check_webhook_exists(
        &self,
        transaction_id: &str,
        transaction_status: &str,
    ) -> AppResult<bool> {
        let result = sqlx::query!(
            r#"
            SELECT COUNT(*) as count 
            FROM webhook_events 
            WHERE transaction_id = $1 AND event_type = $2
            "#,
            transaction_id,
            transaction_status
        )
        .fetch_one(&self.pool)
        .await?;
        
        Ok(result.count.unwrap_or(0) > 0)
    }

    /// Simpan webhook event untuk deduplication
    pub async fn save_webhook_event(
        &self,
        transaction_id: &str,
        order_id: &str,
        event_type: &str,
        payload: Option<serde_json::Value>,
    ) -> AppResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO webhook_events (transaction_id, order_id, event_type, payload)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (transaction_id, event_type) DO NOTHING
            "#,
            transaction_id,
            order_id,
            event_type,
            payload
        )
        .execute(&self.pool)
        .await?;
        
        tracing::debug!("Webhook event saved: {} - {}", transaction_id, event_type);
        Ok(())
    }

    /// Get refund by order ID
    pub async fn get_refund_by_order_id(
        &self,
        order_id: Uuid,
    ) -> AppResult<Option<serde_json::Value>> {
        let result = sqlx::query!(
            r#"
            SELECT 
                id, refund_id, amount, reason, status, created_at
            FROM refunds 
            WHERE order_id = $1
            "#,
            order_id
        )
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(result.map(|r| serde_json::json!({
            "id": r.id,
            "refund_id": r.refund_id,
            "amount": r.amount,
            "reason": r.reason,
            "status": r.status,
            "created_at": r.created_at
        })))
    }

    /// Create refund record
    pub async fn create_refund(
        &self,
        order_id: Uuid,
        amount: BigDecimal,
        reason: Option<String>,
        refunded_by: Option<Uuid>,
        refund_id: String,
    ) -> AppResult<Uuid> {
        let id = Uuid::new_v4();
        
        sqlx::query!(
            r#"
            INSERT INTO refunds (id, order_id, refund_id, amount, reason, status, refunded_by)
            VALUES ($1, $2, $3, $4, $5, 'processing', $6)
            "#,
            id,
            order_id,
            refund_id,
            amount,
            reason,
            refunded_by
        )
        .execute(&self.pool)
        .await?;
        
        tracing::info!("Refund created: {} for order {}", id, order_id);
        Ok(id)
    }
}