// /pdf-bookstore/services/payment-service/src/core/payment.rs

use std::sync::Arc;
use uuid::Uuid;
use bigdecimal::BigDecimal;
use std::str::FromStr;
use chrono::Utc; 
use std::time::Duration;
use crate::{
    models::*,
    repository::Repository,
    utils::error::{AppError, AppResult},
    utils::validator::validate_email_basic,
    utils::circuit_breaker::CircuitBreakerManager,
    utils::service_discovery::ServiceRegistry,
    utils::cache::CacheManager,
};

use super::midtrans::MidtransClient;

// Service untuk handle payment business logic dengan enterprise pattern
pub struct PaymentService {
    repository: Arc<Repository>,
    midtrans_client: Arc<MidtransClient>,
    service_registry: Arc<ServiceRegistry>,
    circuit_manager: Arc<CircuitBreakerManager>,
    cache_manager: Arc<CacheManager>,
    auth_service_url: String,
    http_client: reqwest::Client,
}

impl PaymentService {
    /// Initialize payment service dengan proper configuration
    pub async fn new(
        repository: Arc<Repository>,
        cache_manager: Arc<CacheManager>, 
    ) -> AppResult<Self> {
        let midtrans_client = Arc::new(
            MidtransClient::new()
                .map_err(|e| AppError::Configuration(format!("Midtrans config error: {}", e)))?
        );

        // Initialize service discovery
        let service_registry = Arc::new(ServiceRegistry::new());
        service_registry.init_default_services().await;
        
        // Initialize circuit breaker manager
        let circuit_manager = Arc::new(CircuitBreakerManager::new());
        
        let auth_service_url = std::env::var("AUTH_SERVICE_URL")
            .unwrap_or_else(|_| "http://auth-service:3001".to_string());
        
        // Create HTTP client dengan proper configuration
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .pool_max_idle_per_host(10)
            .build()
            .map_err(|e| AppError::Configuration(format!("HTTP client error: {}", e)))?;
        
        Ok(Self {
            repository,
            midtrans_client,
            service_registry,
            circuit_manager,
            cache_manager,
            auth_service_url,
            http_client,
        })
    }
    
    /// Create new order dengan comprehensive validation dan atomic transaction
    pub async fn create_order(
        &self,
        user_id: Uuid,
        book_id: Uuid,
        payment_method: String,
        idempotency_key: Option<String>,
    ) -> AppResult<OrderWithDetails> {
        // Get book details dari book service dengan enhanced error handling
        let book_details = self.get_book_details(book_id).await?;
        
        // Get user details dari auth service (simplified untuk sekarang)
        let user_details = self.get_user_details(user_id).await?;
        
        // Check apakah user sudah punya book ini
        if self.repository.payment()
            .has_user_purchased_book(user_id, book_id)
            .await? 
        {
            tracing::warn!("User {} attempted to purchase already owned book {}", user_id, book_id);
            return Err(AppError::Conflict("Anda sudah membeli book ini".to_string()));
        }
        
        // Start database transaction
        let mut tx = self.repository.begin_transaction().await?;
        
        // Create order di database dengan atomic function
        let order = self.repository.order()
            .create_order_atomic(
                &mut tx,
                user_id,
                book_id,
                book_details.price.clone(),
                payment_method.clone(),
                idempotency_key,
            )
            .await?;
        
        // Create payment request ke Midtrans
        let payment_request = self.build_payment_request(
            &order,
            &user_details,
            &book_details,
            payment_method,
        );
        
        // Send request ke Midtrans dengan retry logic
        let midtrans_response = self.create_payment_with_retry(&payment_request, 3).await?;
        
        // Update order dengan Midtrans data
        let payment_url = self.midtrans_client
            .create_payment_url(&midtrans_response.transaction_id);
        
        self.repository.order()
            .update_midtrans_data(
                &mut tx,
                order.id,
                &midtrans_response.transaction_id,
                Some(&payment_url),
            )
            .await?;
        
        // Log audit trail
        self.repository.audit()
            .log_order_created(&mut tx, user_id, order.id, &order.order_number)
            .await?;
        
        // Commit transaction
        tx.commit().await
            .map_err(|e| AppError::Database(e.to_string()))?;
        
        // Get complete order details
        let order_with_details = self.repository.order()
            .find_by_id(order.id)
            .await?
            .ok_or_else(|| AppError::NotFound("Order tidak ditemukan setelah dibuat".to_string()))?;
        
        tracing::info!("Order {} created successfully for user {}", order.order_number, user_id);
        
        Ok(order_with_details)
    }
    
    /// Cancel order dengan enhanced validation
    pub async fn cancel_order(
        &self,
        order_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<()> {
        // Start transaction
        let mut tx = self.repository.begin_transaction().await?;
        
        // Get order untuk validation
        let order = self.repository.order()
            .find_by_id(order_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Order tidak ditemukan".to_string()))?;
        
        // Additional validation: check expiry
        if let Some(expires_at) = order.order.expires_at {
            if expires_at < Utc::now() {
                return Err(AppError::BadRequest("Order sudah expired".to_string()));
            }
        }
        
        // Cancel di Midtrans jika ada transaction ID
        if let Some(midtrans_order_id) = &order.order.midtrans_order_id {
            match self.midtrans_client.cancel_payment(midtrans_order_id).await {
                Ok(_) => {
                    tracing::info!("Successfully cancelled payment {} in Midtrans", midtrans_order_id);
                }
                Err(e) => {
                    tracing::warn!("Failed to cancel payment {} in Midtrans: {}", midtrans_order_id, e);
                    // Continue dengan cancel di database meskipun Midtrans fail
                }
            }
        }
        
        // Update status di database
        self.repository.order()
            .update_status(&mut tx, order_id, PaymentStatus::Cancelled, None)
            .await?;
        
        // Log audit
        self.repository.audit()
            .log_order_cancelled(&mut tx, user_id, order_id)
            .await?;
        
        // Commit
        tx.commit().await
            .map_err(|e| AppError::Database(e.to_string()))?;
        
        tracing::info!("Order {} cancelled by user {}", order.order.order_number, user_id);
        
        Ok(())
    }
    
    /// Process webhook dari Midtrans dengan enhanced security dan idempotency
    pub async fn process_webhook(
        &self,
        payload: &MidtransWebhookPayload,
        signature_key: &str,
    ) -> AppResult<()> {
        // Verify signature untuk security
        if !self.midtrans_client.verify_webhook_signature(payload, signature_key)? {
            tracing::warn!("Invalid webhook signature for transaction: {}", payload.transaction_id);
            return Err(AppError::Unauthorized("Invalid webhook signature".to_string()));
        }

        // IMPLEMENTASI WEBHOOK DEDUPLICATION
        // Check apakah webhook sudah pernah diproses
        let webhook_exists = self.repository
            .payment()
            .check_webhook_exists(&payload.transaction_id, &payload.transaction_status)
            .await?;
        
        if webhook_exists {
            tracing::info!("Webhook sudah diproses sebelumnya: {} - {}", 
                payload.transaction_id, payload.transaction_status);
            return Ok(()); // Skip processing, sudah pernah diproses
        }
        
        // Simpan webhook event untuk deduplication
        self.repository
            .payment()
            .save_webhook_event(
                &payload.transaction_id,
                &payload.order_id,
                &payload.transaction_status,
                serde_json::to_value(payload).ok()
            )
            .await?;
        
        // Get order dengan validation
        let order = self.repository.order()
            .find_by_order_number(&payload.order_id)
            .await?
            .ok_or_else(|| {
                tracing::error!("Webhook received for unknown order: {}", payload.order_id);
                AppError::NotFound("Order tidak ditemukan".to_string())
            })?;
        
        // Process payment status dari webhook
        let payment_status = self.midtrans_client
            .process_webhook_notification(payload)?;
        
        // Start transaction
        let mut tx = self.repository.begin_transaction().await?;
        
        // Update order status hanya jika status berubah
        if order.status != payment_status.to_db_string() {
            let paid_at = if matches!(payment_status, PaymentStatus::Paid) {
                Some(Utc::now())
            } else {
                None
            };
            
            self.repository.order()
                .update_status(&mut tx, order.id, payment_status.clone(), paid_at)
                .await?;
            
            tracing::info!("Order {} status updated to {:?}", payload.order_id, payment_status);
        } else {
            tracing::debug!("Order {} status unchanged: {:?}", payload.order_id, payment_status);
        }
        
        // If paid, create user purchase record menggunakan atomic function
        if matches!(payment_status, PaymentStatus::Paid) {
            self.repository.payment()
                .complete_payment_atomic(
                    &mut tx,
                    &payload.order_id,
                    &payload.transaction_id,
                    serde_json::to_value(payload).ok(),
                )
                .await?;
            
            tracing::info!("Payment completed for order: {}", payload.order_id);
        }
        
        // Log payment webhook untuk audit
        self.repository.payment()
            .log_payment_webhook(&mut tx, payload, Some(order.id))
            .await?;
        
        // Log audit
        self.repository.audit()
            .log_webhook_processed(&mut tx, order.id, &payload.transaction_id)
            .await?;
        
        // Commit
        tx.commit().await
            .map_err(|e| AppError::Database(e.to_string()))?;
        
        tracing::info!(
            "Webhook processed successfully: order={}, tx={}, status={:?}",
            payload.order_id,
            payload.transaction_id,
            payment_status
        );
        
        Ok(())
    }
    
    // ========================= HELPER METHODS =========================
    
    /// Get book details dari book service dengan retry dan fallback
    async fn get_book_details(&self, book_id: Uuid) -> AppResult<BookDetails> {
        // Check cache dulu
        let cache_key = format!("book_details_{}", book_id);
        if let Ok(Some(cached)) = self.cache_manager.get::<BookDetails>(&cache_key).await {
            tracing::debug!("Book {} found in cache", book_id);
            return Ok(cached);
        }

        // get healthy instance
        let book_service = self.service_registry
            .get_healthy_instance("book-service")
            .await
            .unwrap_or_else(|_| {
                crate::utils::service_discovery::ServiceInstance {
                    id: "book-service-1".to_string(),
                    name: "book-service".to_string(),
                    host: "localhost".to_string(),
                    port: 3002,
                    health_check_url: "http://localhost:3002/health".to_string(),
                    is_healthy: true,
                    last_health_check: None,
                    metadata: std::collections::HashMap::new(),
                }
            });
        
        // Get circuit breaker
        let circuit_breaker = self.circuit_manager
            .get_or_create("book-service")
            .await;
        
        // Clone variables yang dibutuhkan untuk move ke async block
        let url = format!("{}/api/books/{}", book_service.get_url(), book_id);
        let client = self.http_client.clone();
        let cache_manager = self.cache_manager.clone();  
        let cache_key_copy = cache_key.clone();

        // Execute SEKALI dengan circuit breaker protection
        circuit_breaker.call(async move {
            let response = client
                .get(&url)
                .timeout(Duration::from_secs(5))
                .send()
                .await
                .map_err(|e| AppError::ExternalService(format!("Book service error: {}", e)))?;
            
            if !response.status().is_success() {
                return Err(AppError::NotFound("Book tidak ditemukan".to_string()));
            }
            
            let data: serde_json::Value = response.json().await
                .map_err(|e| AppError::ExternalService(format!("Parse error: {}", e)))?;
            
            let book_obj = data["data"]["book"].as_object()
                .ok_or_else(|| AppError::ExternalService("Invalid book response format".to_string()))?;
            
            let book_details = BookDetails {
                title: book_obj["title"].as_str().unwrap_or("Unknown").to_string(),
                author: book_obj["author"].as_str().unwrap_or("Unknown").to_string(),
                price: BigDecimal::from_str(
                    book_obj["price"].as_str().unwrap_or("0")
                ).unwrap_or_else(|_| BigDecimal::from(0)),
            };

            let _ = cache_manager.set(&cache_key_copy, &book_details, 3600).await;
            
            Ok(book_details)
        }).await
    }
    
    /// Get user details - simplified implementation untuk sekarang
    async fn get_user_details(&self, user_id: Uuid) -> AppResult<UserDetails> {
        // Request ke auth service
        let response = self.http_client
            .get(format!("{}/api/auth/profile", self.auth_service_url))
            .header("X-User-Id", user_id.to_string())
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Failed to contact auth service: {}", e);
                AppError::ExternalService(format!("Auth service error: {}", e))
            })?;
        
        if !response.status().is_success() {
            // Fallback dengan validasi
            let fallback_email = format!("user-{}@bookstore.com", &user_id.to_string()[..8]);
            
            // Validate email format meskipun fallback
            validate_email_basic(&fallback_email)?;
            
            return Ok(UserDetails {
                name: format!("User {}", &user_id.to_string()[..8]),
                email: fallback_email,
                phone: None,
            });
        }
        
        // Parse response JSON
        let data: serde_json::Value = response.json().await
            .map_err(|e| AppError::ExternalService(format!("Parse auth response error: {}", e)))?;
        
        // Extract user details dengan validation
        let user_data = data.get("user")
            .ok_or_else(|| AppError::ExternalService("Invalid auth response format".to_string()))?;
        
        // Ambil data user dengan fallback values
        let full_name = user_data["full_name"]
            .as_str()
            .unwrap_or("Unknown User")
            .to_string();
        
        let email = user_data["email"]
            .as_str()
            .unwrap_or_else(|| {
                tracing::warn!("Email not found for user {}", user_id);
                "noemail@bookstore.com"
            })
            .to_string();
        
        let phone = user_data["phone"]
            .as_str()
            .map(|s| s.to_string());
        
        tracing::debug!("Retrieved user details for {}: {} ({})", user_id, full_name, email);
        
        Ok(UserDetails {
            name: full_name,
            email,
            phone,
        })
    }
    
    /// Create payment dengan retry logic
    async fn create_payment_with_retry(
        &self,
        request: &MidtransPaymentRequest,
        max_retries: u32,
    ) -> AppResult<MidtransPaymentResponse> {
        let mut last_error = None;
        
        for attempt in 1..=max_retries {
            match self.midtrans_client.create_payment(request).await {
                Ok(response) => {
                    if attempt > 1 {
                        tracing::info!("Payment created successfully on attempt {}", attempt);
                    }
                    return Ok(response);
                }
                Err(e) => {
                    tracing::warn!("Payment creation attempt {} failed: {}", attempt, e);
                    last_error = Some(e);
                    
                    if attempt < max_retries {
                        tokio::time::sleep(std::time::Duration::from_millis(1000 * attempt as u64)).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| 
            AppError::ExternalService("All payment attempts failed".to_string())
        ))
    }
    
    /// Build payment request untuk Midtrans dengan proper validation
    fn build_payment_request(
        &self,
        order: &Order,
        user: &UserDetails,
        book: &BookDetails,
        payment_method: String,
    ) -> MidtransPaymentRequest {
        let gross_amount = order.amount.to_string()
            .parse::<f64>()
            .unwrap_or(0.0)
            .max(0.0) as i64; // Ensure non-negative
        
        let name_parts: Vec<&str> = user.name.split_whitespace().collect();
        let first_name = name_parts.first().unwrap_or(&"").to_string();
        let last_name = if name_parts.len() > 1 {
            name_parts[1..].join(" ")
        } else {
            "".to_string()
        };
        
        let payment_method_enum = match payment_method.as_str() {
            "credit_card" => PaymentMethod::CreditCard,
            "bank_transfer" => PaymentMethod::BankTransfer,
            "e_wallet" => PaymentMethod::EWallet,
            "qris" => PaymentMethod::Qris,
            "convenience_store" => PaymentMethod::ConvenienceStore,
            _ => PaymentMethod::Qris, // Default fallback
        };
        
        MidtransPaymentRequest {
            transaction_details: TransactionDetails {
                order_id: order.order_number.clone(),
                gross_amount,
            },
            credit_card: CreditCardSettings {
                secure: true,
                bank: None,
                installment: None,
            },
            customer_details: CustomerDetails {
                first_name,
                last_name,
                email: user.email.clone(),
                phone: user.phone.clone(),
                billing_address: None,
            },
            item_details: vec![ItemDetail {
                id: order.book_id.map(|id| id.to_string()).unwrap_or_else(|| "unknown".to_string()),
                price: gross_amount,
                quantity: 1,
                name: format!("{} - {}", book.title, book.author),
                brand: Some("PDF Bookstore".to_string()),
                category: Some("Digital Book".to_string()),
            }],
            enabled_payments: payment_method_enum.get_enabled_payments(),
            callbacks: Some(CallbackUrls {
                finish: format!("{}/payment/finish", std::env::var("FRONTEND_BASE_URL").unwrap_or_default()),
                unfinish: format!("{}/payment/unfinish", std::env::var("FRONTEND_BASE_URL").unwrap_or_default()),
                error: format!("{}/payment/error", std::env::var("FRONTEND_BASE_URL").unwrap_or_default()),
            }),
        }
    }
}


