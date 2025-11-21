// /pdf-bookstore/services/payment-service/src/core/midtrans.rs

use reqwest::Client;
use uuid::Uuid;
use bigdecimal::BigDecimal;
use std::env;
use base64::Engine;
use sha2::{Sha512, Digest};
use hex;
use crate::{
    models::*,
    utils::error::{AppError, AppResult},
};

/// Client untuk integrasi dengan Midtrans payment gateway
pub struct MidtransClient {
    client: Client,
    server_key: String,
    client_key: String,
    is_production: bool,
    base_url: String,
}

impl MidtransClient {
    /// Initialize Midtrans client
    pub fn new() -> AppResult<Self> {
        let server_key = env::var("MIDTRANS_SERVER_KEY")
            .map_err(|_| AppError::Configuration("MIDTRANS_SERVER_KEY not set".to_string()))?;
        
        let client_key = env::var("MIDTRANS_CLIENT_KEY")
            .map_err(|_| AppError::Configuration("MIDTRANS_CLIENT_KEY not set".to_string()))?;
        
        let is_production = env::var("MIDTRANS_IS_PRODUCTION")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);
        
        let base_url = if is_production {
            "https://api.midtrans.com/v2".to_string()
        } else {
            "https://api.sandbox.midtrans.com/v2".to_string()
        };
        
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::Configuration(format!("Failed to build HTTP client: {}", e)))?;
        
        Ok(Self {
            client,
            server_key,
            client_key,
            is_production,
            base_url,
        })
    }
    
    /// Create payment transaction
    pub async fn create_payment(&self, request: &MidtransPaymentRequest) -> AppResult<MidtransPaymentResponse> {
        let auth_header = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("{}:", self.server_key))
        );
        
        let response = self.client
            .post(format!("{}/charge", self.base_url))
            .header("Authorization", auth_header)
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::ExternalService(format!("Midtrans error: {}", error_text)));
        }
        
        let payment_response: MidtransPaymentResponse = response.json().await
            .map_err(|e| AppError::ExternalService(format!("Failed to parse Midtrans response: {}", e)))?;
        
        Ok(payment_response)
    }
    
    /// Verify webhook signature
    pub fn verify_webhook_signature(&self, payload: &MidtransWebhookPayload, signature_key: &str) -> AppResult<bool> {
        let signature_string = format!(
            "{}{}{}{}",
            payload.order_id,
            payload.status_code,
            payload.gross_amount,
            self.server_key
        );
        
        let mut hasher = Sha512::new();
        hasher.update(signature_string.as_bytes());
        let calculated_signature = hex::encode(hasher.finalize());
        
        Ok(calculated_signature == signature_key)
    }
    
    /// Process webhook notification
    pub fn process_webhook_notification(&self, payload: &MidtransWebhookPayload) -> AppResult<PaymentStatus> {
        let status = match payload.transaction_status.as_str() {
            "capture" => match payload.fraud_status.as_deref() {
                Some("challenge") => PaymentStatus::Pending,
                Some("deny") => PaymentStatus::Failed,
                _ => PaymentStatus::Paid,
            },
            "settlement" => PaymentStatus::Paid,
            "pending" => PaymentStatus::Pending,
            "deny" => PaymentStatus::Failed,
            "cancel" => PaymentStatus::Cancelled,
            "expire" => PaymentStatus::Expired,
            "failure" => PaymentStatus::Failed,
            _ => PaymentStatus::Failed,
        };
        
        Ok(status)
    }
    
    /// Cancel payment
    pub async fn cancel_payment(&self, order_id: &str) -> AppResult<()> {
        let auth_header = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("{}:", self.server_key))
        );
        
        let response = self.client
            .post(format!("{}/{}/cancel", self.base_url, order_id))
            .header("Authorization", auth_header)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::ExternalService(format!("Failed to cancel payment: {}", error_text)));
        }
        
        Ok(())
    }
    
    /// Create payment URL
    pub fn create_payment_url(&self, transaction_id: &str) -> String {
        if self.is_production {
            format!("https://app.midtrans.com/snap/v1/transactions/{}/pay", transaction_id)
        } else {
            format!("https://app.sandbox.midtrans.com/snap/v1/transactions/{}/pay", transaction_id)
        }
    }

    /// Process refund melalui Midtrans
    pub async fn process_refund(
        &self,
        order_id: &str,
        amount: BigDecimal,
        reason: &str,
    ) -> AppResult<RefundResponse> {
        let auth_header = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("{}:", self.server_key))
        );
        
        let refund_request = serde_json::json!({
            "refund_key": Uuid::new_v4().to_string(),
            "amount": amount.to_string().parse::<i64>().unwrap_or(0),
            "reason": reason
        });
        
        let response = self.client
            .post(format!("{}/{}/refund", self.base_url, order_id))
            .header("Authorization", auth_header)
            .header("Content-Type", "application/json")
            .json(&refund_request)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::ExternalService(format!("Midtrans refund error: {}", error_text)));
        }
        
        let refund_response: RefundResponse = response.json().await
            .map_err(|e| AppError::ExternalService(format!("Failed to parse refund response: {}", e)))?;
        
        Ok(refund_response)
    }

    /// Get client key untuk frontend Snap integration
    pub fn get_client_key(&self) -> &str {
        &self.client_key
    }
    
    /// Get Snap token untuk payment
    pub async fn get_snap_token(&self, request: &MidtransPaymentRequest) -> AppResult<String> {
        let response = self.create_payment(request).await?;
        
        // Return token yang bisa dipakai di frontend dengan client_key
        Ok(response.token.unwrap_or_else(|| response.transaction_id.clone()))
    }
}

