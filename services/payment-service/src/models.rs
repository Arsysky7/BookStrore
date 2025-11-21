// /pdf-bookstore/services/payment-service/src/models.rs

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use validator::Validate;
use bigdecimal::BigDecimal;
use std::time::{Instant};

// ========================= DOMAIN MODELS =========================

/// Model Order dari database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub book_id: Option<Uuid>,
    pub order_number: String,
    pub amount: BigDecimal,
    pub status: String,
    pub payment_method: Option<String>,
    pub midtrans_order_id: Option<String>,
    pub payment_url: Option<String>,
    pub paid_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Model PaymentLog untuk audit trail
/// TODO: Implement payment log repository and usage in handlers
#[derive(Debug, Clone, FromRow, Serialize)]
#[allow(dead_code)]
pub struct PaymentLog {
    pub id: Uuid,
    pub order_id: Option<Uuid>,
    pub transaction_id: Option<String>,
    pub payment_type: Option<String>,
    pub gross_amount: Option<BigDecimal>,
    pub transaction_status: Option<String>,
    pub fraud_status: Option<String>,
    pub settlement_time: Option<DateTime<Utc>>,
    pub webhook_data: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Model UserPurchase untuk tracking pembelian
/// TODO: Implement purchase tracking repository
#[derive(Debug, Clone, FromRow, Serialize)]
#[allow(dead_code)]
pub struct UserPurchase {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub book_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub purchased_at: DateTime<Utc>,
    pub download_count: i32,
    pub last_downloaded_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct CachedToken {
    pub user_id: Uuid,
    pub role: String,
    pub email: String,
    pub expires: Instant
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BookDetails {
    pub title: String,
    pub author: String,
    pub price: BigDecimal,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserDetails {
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
}


// ========================= REQUEST DTOs =========================

/// Request untuk membuat order baru
#[derive(Debug, Deserialize, Validate)]
pub struct CreateOrderRequest {
    #[validate(length(min = 1, message = "Book ID diperlukan"))]
    pub book_id: String,
    
    #[validate(length(min = 1, message = "Metode pembayaran diperlukan"))]
    pub payment_method: String,
    
    /// Idempotency key untuk prevent duplicate orders
    pub idempotency_key: Option<String>,
}

/// Request untuk refund order
#[derive(Debug, Deserialize, Validate)]
pub struct RefundRequest {
    #[validate(length(min = 10, max = 500))]
    pub reason: Option<String>,
    
    /// Jumlah refund (optional, default full refund)
    pub amount: Option<BigDecimal>,
    
    /// Bank account details untuk refund
    pub bank_account: Option<String>,
}

/// Request untuk update order status
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateOrderRequest {
    #[validate(length(min = 1, max = 50))]
    pub status: Option<String>,
    
    #[validate(length(max = 500))]
    pub notes: Option<String>,
}

/// Query parameters untuk list orders
#[derive(Debug, Deserialize)]
pub struct OrderQueryParams {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub status: Option<String>,
    pub payment_method: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

// ========================= RESPONSE DTOs =========================

/// Order dengan detail tambahan untuk response
#[derive(Debug, Serialize, Deserialize)]
pub struct OrderWithDetails {
    #[serde(flatten)]
    pub order: Order,
    pub book_title: Option<String>,
    pub book_author: Option<String>,
    pub book_cover_path: Option<String>,
    pub user_email: Option<String>,
    pub user_name: Option<String>,
}

/// Response wrapper untuk single order
#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<OrderWithDetails>,
}

/// Response wrapper untuk list orders
#[derive(Debug, Serialize)]
pub struct OrdersListResponse {
    pub success: bool,
    pub message: String,
    pub data: Vec<OrderWithDetails>,
    pub pagination: Option<PaginationMeta>,
}

/// Metadata untuk pagination
#[derive(Debug, Serialize, Clone)]
pub struct PaginationMeta {
    pub current_page: u32,
    pub per_page: u32,
    pub total_items: i64,
    pub total_pages: u32,
    pub has_next: bool,
    pub has_prev: bool,
}

/// Standard error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub message: String,
    pub error_code: Option<String>,
    pub details: Option<serde_json::Value>,
}


// ========================= PAYMENT GATEWAY DTOs =========================

/// Midtrans payment request
#[derive(Debug, Serialize)]
pub struct MidtransPaymentRequest {
    pub transaction_details: TransactionDetails,
    pub credit_card: CreditCardSettings,
    pub customer_details: CustomerDetails,
    pub item_details: Vec<ItemDetail>,
    pub enabled_payments: Vec<String>,
    pub callbacks: Option<CallbackUrls>,
}

/// Transaction details untuk Midtrans
#[derive(Debug, Serialize)]
pub struct TransactionDetails {
    pub order_id: String,
    pub gross_amount: i64,
}

/// Credit card settings
#[derive(Debug, Serialize)]
pub struct CreditCardSettings {
    pub secure: bool,
    pub bank: Option<String>,
    pub installment: Option<InstallmentSettings>,
}

/// Installment settings
#[derive(Debug, Serialize)]
pub struct InstallmentSettings {
    pub required: bool,
    pub terms: Option<Vec<i32>>,
}

/// Customer details untuk payment
#[derive(Debug, Serialize)]
pub struct CustomerDetails {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: Option<String>,
    pub billing_address: Option<Address>,
}

/// Address model
#[derive(Debug, Serialize)]
pub struct Address {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: String,
    pub address: String,
    pub city: String,
    pub postal_code: String,
    pub country_code: String,
}

/// Item detail untuk payment
#[derive(Debug, Serialize)]
pub struct ItemDetail {
    pub id: String,
    pub price: i64,
    pub quantity: i32,
    pub name: String,
    pub brand: Option<String>,
    pub category: Option<String>,
}

/// Callback URLs untuk payment gateway
#[derive(Debug, Serialize)]
pub struct CallbackUrls {
    pub finish: String,
    pub unfinish: String,
    pub error: String,
}

/// Midtrans payment response
#[derive(Debug, Deserialize)]
pub struct MidtransPaymentResponse {
    pub status_code: String,
    pub status_message: String,
    pub transaction_id: String,
    pub order_id: String,
    pub merchant_id: String,
    pub gross_amount: String,
    pub currency: String,
    pub payment_type: String,
    pub transaction_time: String,
    pub transaction_status: String,
    pub fraud_status: Option<String>,
    pub redirect_url: Option<String>,
    pub token: Option<String>,
}

/// Midtrans refund response
#[derive(Debug, Deserialize)]
pub struct RefundResponse {
    pub status_code: String,
    pub status_message: String,
    pub refund_id: String,
    pub order_id: String,
    pub gross_amount: String,
    pub refund_amount: String,
    pub refund_time: String,
    pub refund_status: String,
}

/// Midtrans webhook payload
#[derive(Debug, Deserialize, Serialize)]
pub struct MidtransWebhookPayload {
    pub transaction_time: Option<String>,
    pub transaction_status: String,
    pub transaction_id: String,
    pub status_message: Option<String>,
    pub status_code: String,
    pub signature_key: String,
    pub settlement_time: Option<String>,
    pub payment_type: String,
    pub order_id: String,
    pub merchant_id: String,
    pub gross_amount: String,
    pub fraud_status: Option<String>,
    pub currency: String,
}

// ========================= ENUMS =========================

/// Payment status enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentStatus {
    Pending,
    Paid,
    Failed,
    Cancelled,
    Expired,
    Refunded,
}

impl PaymentStatus {
    /// Convert dari string database
    pub fn from_str(status: &str) -> Self {
        match status.to_lowercase().as_str() {
            "pending" => PaymentStatus::Pending,
            "paid" | "settlement" | "capture" => PaymentStatus::Paid,
            "failed" | "deny" => PaymentStatus::Failed,
            "cancelled" | "cancel" => PaymentStatus::Cancelled,
            "expired" | "expire" => PaymentStatus::Expired,
            "refunded" | "refund" => PaymentStatus::Refunded,
            _ => PaymentStatus::Failed,
        }
    }
    
    /// Convert ke string untuk database
    pub fn to_db_string(&self) -> &'static str {
        match self {
            PaymentStatus::Pending => "pending",
            PaymentStatus::Paid => "paid",
            PaymentStatus::Failed => "failed",
            PaymentStatus::Cancelled => "cancelled",
            PaymentStatus::Expired => "expired",
            PaymentStatus::Refunded => "refunded",
        }
    }
}

/// Payment method enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentMethod {
    CreditCard,
    BankTransfer,
    EWallet,
    Qris,
    ConvenienceStore,
}

impl PaymentMethod {
    /// Get display name untuk UI
    pub fn display_name(&self) -> &'static str {
        match self {
            PaymentMethod::CreditCard => "Kartu Kredit/Debit",
            PaymentMethod::BankTransfer => "Transfer Bank",
            PaymentMethod::EWallet => "E-Wallet",
            PaymentMethod::Qris => "QRIS",
            PaymentMethod::ConvenienceStore => "Convenience Store",
        }
    }
    
    /// Get enabled payment types untuk Midtrans
    pub fn get_enabled_payments(&self) -> Vec<String> {
        match self {
            PaymentMethod::CreditCard => vec!["credit_card".to_string()],
            PaymentMethod::BankTransfer => vec![
                "bca_va".to_string(),
                "bni_va".to_string(),
                "bri_va".to_string(),
                "permata_va".to_string(),
            ],
            PaymentMethod::EWallet => vec![
                "gopay".to_string(),
                "shopeepay".to_string(),
            ],
            PaymentMethod::Qris => vec!["qris".to_string()],
            PaymentMethod::ConvenienceStore => vec![
                "indomaret".to_string(),
                "alfamart".to_string(),
            ],
        }
    }
}

// ========================= ADMIN MODELS =========================

/// Statistik order untuk admin dashboard
#[derive(Debug, Serialize)]
pub struct AdminOrderStats {
    pub total_orders: i64,
    pub pending_orders: i64,
    pub paid_orders: i64,
    pub failed_orders: i64,
    pub cancelled_orders: i64,
    pub total_revenue: BigDecimal,
    pub orders_this_month: i64,
    pub revenue_this_month: BigDecimal,
    pub monthly_growth_percentage: f64,
    pub avg_order_value: Option<BigDecimal>,
    pub payment_method_breakdown: Vec<PaymentMethodStat>,
}

/// Statistik per payment method
#[derive(Debug, Serialize)]
pub struct PaymentMethodStat {
    pub payment_method: String,
    pub order_count: i64,
    pub total_amount: BigDecimal,
    pub percentage: f64,
}

/// Revenue analytics untuk charts
#[derive(Debug, Serialize)]
pub struct RevenueAnalytics {
    pub period: String,
    pub data_points: Vec<RevenueDataPoint>,
    pub total_revenue: BigDecimal,
    pub total_orders: i64,
    pub avg_order_value: BigDecimal,
    pub growth_rate: f64,
}

/// Data point untuk revenue chart
#[derive(Debug, Serialize)]
pub struct RevenueDataPoint {
    pub date: String,
    pub revenue: BigDecimal,
    pub orders_count: i64,
    pub avg_order_value: BigDecimal,
}

// ========================= HELPER IMPLEMENTATIONS =========================

impl PaginationMeta {
    /// Create pagination metadata dari hasil query
    pub fn new(current_page: u32, per_page: u32, total_items: i64) -> Self {
        let total_pages = ((total_items as f64) / (per_page as f64)).ceil() as u32;
        
        Self {
            current_page,
            per_page,
            total_items,
            total_pages: if total_pages == 0 { 1 } else { total_pages },
            has_next: current_page < total_pages,
            has_prev: current_page > 1,
        }
    }
}

impl Default for OrderQueryParams {
    fn default() -> Self {
        Self {
            page: Some(1),
            limit: Some(10),
            status: None,
            payment_method: None,
            sort_by: Some("created_at".to_string()),
            sort_order: Some("desc".to_string()),
        }
    }
}