// /pdf-bookstore/services/payment-service/src/repository/order.rs

use sqlx::{PgPool, Transaction, Postgres, Row}; 
use uuid::Uuid;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc, Datelike}; 

use crate::{
    models::*,
    utils::error::{AppError, AppResult},
};

/// Repository untuk order operations
pub struct OrderRepository {
    pool: PgPool,
}

impl OrderRepository {
    /// Create new order repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create order dengan atomic transaction (menggunakan stored procedure)
    pub async fn create_order_atomic(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        user_id: Uuid,
        book_id: Uuid,
        amount: BigDecimal,
        payment_method: String,
        idempotency_key: Option<String>,
    ) -> AppResult<Order> {
        // Check idempotency sebelum create
        if let Some(ref key) = idempotency_key {
            let existing = sqlx::query(
                "SELECT id FROM orders WHERE idempotency_key = $1"
            )
            .bind(key)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
            
            if existing.is_some() {
                return Err(AppError::Conflict("Duplicate order detected".to_string()));
            }
        }

        // Call atomic function dari database
        let result = sqlx::query(
            r#"
            SELECT create_order_with_idempotency($1, $2, $3, $4, $5) as result
            "#
        )
        .bind(user_id)
        .bind(book_id)
        .bind(&amount)
        .bind(&payment_method)
        .bind(idempotency_key)  
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        let json_result: serde_json::Value = result.try_get("result")?;

        if !json_result["success"].as_bool().unwrap_or(false) {
            let error = json_result["error"].as_str().unwrap_or("Unknown error");
            return match error {
                "BOOK_NOT_FOUND" => Err(AppError::NotFound("Book tidak ditemukan".to_string())),
                "BOOK_ALREADY_PURCHASED" => Err(AppError::Conflict("Book sudah dibeli".to_string())),
                _ => Err(AppError::Database(error.to_string())),
            };
        }  

        let order_id = json_result["order_id"].as_str() 
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| AppError::Database("Invalid order ID".to_string()))?;

        // Get created order
        self.find_by_id_tx(tx, order_id).await?
            .ok_or_else(|| AppError::Database("Order created but not found".to_string()))
    }

    pub async fn find_by_id(
        &self, order_id: Uuid
    ) -> AppResult<Option<OrderWithDetails>> {
        let row = sqlx::query(
            r#"
            SELECT 
                o.*,
                b.title as book_title,
                b.author as book_author,
                b.cover_path as book_cover_path,
                u.email as user_email,
                u.full_name as user_name
            FROM orders o
            LEFT JOIN books b ON o.book_id = b.id
            LEFT JOIN users u ON o.user_id = u.id
            WHERE o.id = $1
            "#
        )
        .bind(order_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        
        Ok(row.map(|r| self.map_row_to_order_with_details(r)))
    }

    /// Find order by ID dalam transaction
    async fn find_by_id_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: Uuid,
    ) -> AppResult<Option<Order>> {
        let order = sqlx::query_as::<_, Order>(
            r#"
            SELECT * FROM orders WHERE id = $1
            "#
        )
        .bind(order_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        
        Ok(order)
    }

    /// Find order by order number
    pub async fn find_by_order_number(
        &self, 
        order_number: &str,
    ) -> AppResult<Option<Order>> {
        let order = sqlx::query_as::<_, Order>(
            "SELECT * FROM orders WHERE order_number = $1"
        )
        .bind(order_number)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        
        Ok(order)
    }

    /// Find order by idempotency key
    pub async fn find_by_idempotency_key(
        &self,
        key: &str,
    ) -> AppResult<Option<OrderWithDetails>> {
        let row = sqlx::query(
            r#"
            SELECT 
                o.*,
                b.title as book_title,
                b.author as book_author,
                b.cover_path as book_cover_path,
                u.email as user_email,
                u.full_name as user_name
            FROM orders o
            LEFT JOIN books b ON o.book_id = b.id
            LEFT JOIN users u ON o.user_id = u.id
            WHERE o.idempotency_key = $1
            "#
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        
        Ok(row.map(|r| self.map_row_to_order_with_details(r)))
    }

    // helper function untuk validasi sort column dan order
    fn validate_sort_column(column: &str) -> &str {
        match column {
            "created_at" | "updated_at" | "amount" | "status" => column,
            _ => "created_at"
        }
    }

    fn validate_sort_order(order: &str) -> &str {
        match order.to_lowercase().as_str() {
            "asc" => "ASC",
            "desc" => "DESC",
            _ => "DESC"
        }
    }

    /// Find orders by user dengan pagination 
    pub async fn find_by_user(
        &self,
        user_id: Uuid,
        page: u32,
        limit: u32,
        params: OrderQueryParams,
    ) -> AppResult<(Vec<OrderWithDetails>, i64)> {
        let offset = (page - 1) * limit;

        // Build WHERE clause dengan parameter binding yang benar
        let mut conditions = vec!["o.user_id = $1".to_string()];
        let mut bind_params: Vec<String> = vec![];
        let mut bind_index = 2;

        // Status filter
        if let Some(ref status) = params.status {
            conditions.push(format!("o.status = ${}", bind_index));
            bind_params.push(status.clone());
            bind_index += 1;
        }

        // Payment method filter
        if let Some(ref payment_method) = params.payment_method {
            conditions.push(format!("o.payment_method = ${}", bind_index));
            bind_params.push(payment_method.clone());
        }
        
        let where_clause = conditions.join(" AND ");
        
        // Count query
        let count_query = format!(
            "SELECT COUNT(*) FROM orders o WHERE {}",
            where_clause
        );

        let mut count_q = sqlx::query(&count_query).bind(user_id);

        // Bind additional parameters dalam urutan yang benar
        for param in &bind_params {
            count_q = count_q.bind(param);
        }
        
        let count_row = count_q.fetch_one(&self.pool).await
            .map_err(|e| AppError::Database(e.to_string()))?;
        let total: i64 = count_row.get(0);
            
        // Data query dengan sorting
        let sort_by_value = params.sort_by.unwrap_or_else(|| "created_at".to_string());
        let sort_order_value = params.sort_order.unwrap_or_else(|| "desc".to_string());
        
        let sort_by = Self::validate_sort_column(&sort_by_value);
        let sort_order = Self::validate_sort_order(&sort_order_value);

        let data_query = format!(
            r#"
            SELECT 
                o.*,
                b.title as book_title,
                b.author as book_author,
                b.cover_path as book_cover_path,
                u.email as user_email,
                u.full_name as user_name
            FROM orders o
            LEFT JOIN books b ON o.book_id = b.id
            LEFT JOIN users u ON o.user_id = u.id
            WHERE {}
            ORDER BY {} {}
            LIMIT {} OFFSET {}
            "#,
            where_clause, sort_by, sort_order, limit, offset
        );
        
        let mut data_q = sqlx::query(&data_query).bind(user_id);
        
        
        // Bind parameter yang sama seperti count query
        for param in &bind_params {
            data_q = data_q.bind(param);
        }
        
        let rows = data_q.fetch_all(&self.pool).await
            .map_err(|e| AppError::Database(e.to_string()))?;
        
        let orders: Vec<OrderWithDetails> = rows.into_iter()
            .map(|r| self.map_row_to_order_with_details(r))
            .collect();
        
        Ok((orders, total))
    }

    /// Update order status
    pub async fn update_status(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: Uuid,
        status: PaymentStatus,
        paid_at: Option<DateTime<Utc>>,
    ) -> AppResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE orders 
            SET status = $1, paid_at = $2, updated_at = NOW()
            WHERE id = $3
            "#
        )
        .bind(status.to_db_string())
        .bind(paid_at)
        .bind(order_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Order tidak ditemukan untuk update".to_string()));
        }
        
        tracing::debug!("Order {} status updated to {:?}", order_id, status);
        Ok(())
    }

    // Tambahkan method simple untuk update status tanpa transaction
    pub async fn update_status_simple(
        &self,
        order_id: Uuid,
        status: &str,
    ) -> AppResult<()> {
        let result = sqlx::query!(
            r#"
            UPDATE orders 
            SET status = $1, updated_at = NOW()
            WHERE id = $2
            "#,
            status,
            order_id
        )
        .execute(&self.pool)
        .await?;
        
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Order tidak ditemukan".to_string()));
        }
        
        tracing::debug!("Order {} status updated to {}", order_id, status);
        Ok(())
    }

    /// Update Midtrans data
    pub async fn update_midtrans_data(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: Uuid,
        midtrans_order_id: &str,
        payment_url: Option<&str>,
    ) -> AppResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE orders 
            SET midtrans_order_id = $1, payment_url = $2, updated_at = NOW()
            WHERE id = $3
            "#
        )
        .bind(midtrans_order_id)
        .bind(payment_url)
        .bind(order_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Order tidak ditemukan untuk update Midtrans data".to_string()));
        }
        
        tracing::debug!("Order {} Midtrans data updated", order_id);
        Ok(())
    }
    
    /// Get admin statistics dengan enhanced error handling
    pub async fn get_admin_stats(&self) -> AppResult<AdminOrderStats> {
        let now = Utc::now();
        let current_month = now.month();
        let current_year = now.year();
        
        let (prev_month, prev_year) = if current_month == 1 {
            (12, current_year - 1)
        } else {
            (current_month - 1, current_year)
        };
        
        let stats_row = sqlx::query(
            r#"
            SELECT 
                COUNT(*) as total_orders,
                COUNT(*) FILTER (WHERE status = 'pending') as pending_orders,
                COUNT(*) FILTER (WHERE status = 'paid') as paid_orders,
                COUNT(*) FILTER (WHERE status = 'failed') as failed_orders,
                COUNT(*) FILTER (WHERE status = 'cancelled') as cancelled_orders,
                COALESCE(SUM(amount) FILTER (WHERE status = 'paid'), 0) as total_revenue,
                COUNT(*) FILTER (WHERE EXTRACT(MONTH FROM created_at) = $1 
                                   AND EXTRACT(YEAR FROM created_at) = $2) as orders_this_month,
                COALESCE(SUM(amount) FILTER (WHERE status = 'paid' 
                                               AND EXTRACT(MONTH FROM created_at) = $1 
                                               AND EXTRACT(YEAR FROM created_at) = $2), 0) as revenue_this_month,
                COUNT(*) FILTER (WHERE EXTRACT(MONTH FROM created_at) = $3 
                                   AND EXTRACT(YEAR FROM created_at) = $4) as orders_last_month,
                AVG(amount) FILTER (WHERE status = 'paid') as avg_order_value
            FROM orders
            "#
        )
        .bind(current_month as i32)
        .bind(current_year)
        .bind(prev_month as i32)
        .bind(prev_year)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        
        let total_orders: i64 = stats_row.get("total_orders");
        let orders_last_month: i64 = stats_row.get("orders_last_month");
        let orders_this_month: i64 = stats_row.get("orders_this_month");
        
        let monthly_growth_percentage = if orders_last_month > 0 {
            ((orders_this_month - orders_last_month) as f64 / orders_last_month as f64) * 100.0
        } else if orders_this_month > 0 {
            100.0
        } else {
            0.0
        };

        // Log warning jika tidak ada orders (untuk monitoring)
        if total_orders == 0 {
            tracing::warn!("No orders found in database - this might indicate a problem");
        }
        
        // Get payment method breakdown
        let payment_methods = self.get_payment_method_breakdown().await?;
        
        Ok(AdminOrderStats {
            total_orders: stats_row.get("total_orders"),
            pending_orders: stats_row.get("pending_orders"),
            paid_orders: stats_row.get("paid_orders"),
            failed_orders: stats_row.get("failed_orders"),
            cancelled_orders: stats_row.get("cancelled_orders"),
            total_revenue: stats_row.get("total_revenue"),
            orders_this_month,
            revenue_this_month: stats_row.get("revenue_this_month"),
            monthly_growth_percentage,
            avg_order_value: stats_row.get("avg_order_value"),
            payment_method_breakdown: payment_methods,
        })
    }
    
    /// Get payment method breakdown dengan fallback
    async fn get_payment_method_breakdown(&self) -> AppResult<Vec<PaymentMethodStat>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                COALESCE(payment_method, 'unknown') as payment_method,
                COUNT(*) as order_count,
                COALESCE(SUM(amount), 0) as total_amount,
                (COUNT(*) * 100.0 / GREATEST(SUM(COUNT(*)) OVER(), 1)) as percentage
            FROM orders 
            WHERE status = 'paid'
            GROUP BY payment_method
            ORDER BY order_count DESC
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        
        let stats = rows.into_iter()
            .map(|row| PaymentMethodStat {
                payment_method: row.get("payment_method"),
                order_count: row.get("order_count"),
                total_amount: row.get("total_amount"),
                percentage: row.get("percentage"),
            })
            .collect();
        
        Ok(stats)
    }
    
    /// Get revenue analytics dengan enhanced error handling
    pub async fn get_revenue_analytics(
        &self,
        period: &str,
        days: u32,
    ) -> AppResult<RevenueAnalytics> {
        let days = days.min(365);
        
        let (date_format, group_by) = match period {
            "daily" => ("%Y-%m-%d", "DATE(created_at)"),
            "weekly" => ("%Y-W%V", "DATE_TRUNC('week', created_at)"),
            "monthly" => ("%Y-%m", "DATE_TRUNC('month', created_at)"),
            "yearly" => ("%Y", "DATE_TRUNC('year', created_at)"),
            _ => ("%Y-%m-%d", "DATE(created_at)"),
        };
        
        let query = format!(
            r#"
            SELECT 
                TO_CHAR({}, '{}') as period_date,
                COALESCE(SUM(amount), 0) as revenue,
                COUNT(*) as orders_count,
                COALESCE(AVG(amount), 0) as avg_order_value
            FROM orders
            WHERE status = 'paid' 
              AND created_at >= NOW() - INTERVAL '1 day' * $1
            GROUP BY {}
            ORDER BY {} ASC
            "#,
            group_by, date_format, group_by, group_by
        );
        
        let rows = sqlx::query(&query)
            .bind(days as i32)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        
        let mut data_points = Vec::new();
        let mut total_revenue = BigDecimal::from(0);
        let mut total_orders = 0i64;
        
        for row in rows {
            let revenue: BigDecimal = row.get("revenue");
            let orders_count: i64 = row.get("orders_count");
            
            total_revenue = &total_revenue + &revenue;
            total_orders += orders_count;
            
            data_points.push(RevenueDataPoint {
                date: row.get("period_date"),
                revenue,
                orders_count,
                avg_order_value: row.get("avg_order_value"),
            });
        }
        
        let avg_order_value = if total_orders > 0 {
            &total_revenue / BigDecimal::from(total_orders)
        } else {
            BigDecimal::from(0)
        };
        
        // Calculate growth rate dari 2 data point terakhir
        let growth_rate = if data_points.len() >= 2 {
            let recent = &data_points[data_points.len() - 1];
            let previous = &data_points[data_points.len() - 2];
            
            if previous.revenue > BigDecimal::from(0) {
                let change = &recent.revenue - &previous.revenue;
                (change.to_string().parse::<f64>().unwrap_or(0.0) / 
                 previous.revenue.to_string().parse::<f64>().unwrap_or(1.0)) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        };
        
        Ok(RevenueAnalytics {
            period: period.to_string(),
            data_points,
            total_revenue,
            total_orders,
            avg_order_value,
            growth_rate,
        })
    }
    
    /// Get recent orders untuk admin dengan flexible filtering
    pub async fn get_recent_orders(
        &self,
        limit: u32,
        status_filter: Option<&str>,
    ) -> AppResult<Vec<OrderWithDetails>> {
        let limit = limit.min(50);
        
        let query = if let Some(status) = status_filter {
            sqlx::query(
                r#"
                SELECT 
                    o.*,
                    b.title as book_title,
                    b.author as book_author,
                    b.cover_path as book_cover_path,
                    u.email as user_email,
                    u.full_name as user_name
                FROM orders o
                LEFT JOIN books b ON o.book_id = b.id
                LEFT JOIN users u ON o.user_id = u.id
                WHERE o.status = $1
                ORDER BY o.created_at DESC
                LIMIT $2
                "#
            )
            .bind(status)
            .bind(limit as i64)
        } else {
            sqlx::query(
                r#"
                SELECT 
                    o.*,
                    b.title as book_title,
                    b.author as book_author,
                    b.cover_path as book_cover_path,
                    u.email as user_email,
                    u.full_name as user_name
                FROM orders o
                LEFT JOIN books b ON o.book_id = b.id
                LEFT JOIN users u ON o.user_id = u.id
                ORDER BY o.created_at DESC
                LIMIT $1
                "#
            )
            .bind(limit as i64)
        };
        
        let rows = query.fetch_all(&self.pool).await
            .map_err(|e| AppError::Database(e.to_string()))?;
        
        let orders = rows.into_iter()
            .map(|r| self.map_row_to_order_with_details(r))
            .collect();
        
        Ok(orders)
    }
    
    /// Cleanup expired orders dengan enhanced logging
    pub async fn cleanup_expired_orders(&self) -> AppResult<u64> {
        let result = sqlx::query(
            r#"
            UPDATE orders 
            SET status = 'expired', updated_at = NOW()
            WHERE status = 'pending' AND expires_at < NOW()
            "#
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        
        let expired_count = result.rows_affected();
        
        if expired_count > 0 {
            tracing::info!("Expired {} orders in cleanup job", expired_count);
        }
        
        Ok(expired_count)
    }

    // Helper method untuk mapping row ke OrderWithDetails 
    fn map_row_to_order_with_details(
        &self, 
        row: sqlx::postgres::PgRow,
    ) -> OrderWithDetails {
        let order = Order {
            id: row.get("id"),
            user_id: row.get("user_id"),
            book_id: row.get("book_id"),
            order_number: row.get("order_number"),
            amount: row.get("amount"),
            status: row.get("status"),
            payment_method: row.get("payment_method"),
            midtrans_order_id: row.get("midtrans_order_id"),
            payment_url: row.get("payment_url"),
            paid_at: row.get("paid_at"),
            expires_at: row.get("expires_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        };
        
        OrderWithDetails {
            order,
            book_title: row.try_get("book_title").ok(),
            book_author: row.try_get("book_author").ok(),
            book_cover_path: row.try_get("book_cover_path").ok(),
            user_email: row.try_get("user_email").ok(),
            user_name: row.try_get("user_name").ok(),
        }
    }
}