// /pdf-bookstore/services/payment-service/src/repository/mod.rs

pub mod order;
pub mod payment;
pub mod audit;

use sqlx::{PgPool, Transaction, Postgres};
use std::sync::Arc;

/// Main repository struct yang menggabungkan semua repositories
pub struct Repository {
    pub pool: PgPool,
    order_repo: Arc<order::OrderRepository>,
    payment_repo: Arc<payment::PaymentRepository>,
    audit_repo: Arc<audit::AuditRepository>,
}

impl Repository {
    /// Create new repository instance
    pub fn new(pool: PgPool) -> Self {
        let order_repo = Arc::new(order::OrderRepository::new(pool.clone()));
        let payment_repo = Arc::new(payment::PaymentRepository::new(pool.clone()));
        let audit_repo = Arc::new(audit::AuditRepository::new(pool.clone()));
        
        Self {
            pool,
            order_repo,
            payment_repo,
            audit_repo,
        }
    }
    
    /// Get order repository
    pub fn order(&self) -> &order::OrderRepository {
        &self.order_repo
    }
    
    /// Get payment repository
    pub fn payment(&self) -> &payment::PaymentRepository {
        &self.payment_repo
    }
    
    /// Get audit repository
    pub fn audit(&self) -> &audit::AuditRepository {
        &self.audit_repo
    }
    
    /// Begin database transaction
    pub async fn begin_transaction(&self) -> Result<Transaction<'_, Postgres>, sqlx::Error> {
        self.pool.begin().await
    }

    /// Expose pool untuk audit logging (auth middleware needs this)
    pub fn get_pool(&self) -> &PgPool {
        &self.pool
    }
}