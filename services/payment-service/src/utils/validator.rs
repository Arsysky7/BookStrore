// /pdf-bookstore/services/payment-service/src/utils/validator.rs

use uuid::Uuid;
use bigdecimal::BigDecimal;
use crate::utils::error::{AppError, AppResult};

/// Validasi UUID format
pub fn validate_uuid(uuid_str: &str, field_name: &str) -> AppResult<Uuid> {
    Uuid::parse_str(uuid_str)
        .map_err(|_| AppError::BadRequest(format!("Format {} tidak valid", field_name)))
}

/// Validasi amount harus positif
pub fn validate_positive_amount(amount: &BigDecimal, field_name: &str) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::BadRequest(format!("{} harus lebih besar dari 0", field_name)));
    }
    Ok(())
}

/// Validasi payment method yang didukung
pub fn validate_payment_method(method: &str) -> AppResult<()> {
    let valid_methods = [
        "credit_card", "bank_transfer", "e_wallet", 
        "qris", "convenience_store"
    ];
    
    if !valid_methods.contains(&method) {
        return Err(AppError::BadRequest(
            format!("Payment method '{}' tidak didukung. Valid: {:?}", method, valid_methods)
        ));
    }
    Ok(())
}

/// Validasi order status
pub fn validate_order_status(status: &str) -> AppResult<()> {
    let valid_statuses = [
        "pending", "paid", "failed", "cancelled", "expired", "refunded"
    ];
    
    if !valid_statuses.contains(&status) {
        return Err(AppError::BadRequest(
            format!("Order status '{}' tidak valid. Valid: {:?}", status, valid_statuses)
        ));
    }
    Ok(())
}

/// Validasi pagination parameters
pub fn validate_pagination(page: u32, limit: u32) -> AppResult<(u32, u32)> {
    if page == 0 {
        return Err(AppError::BadRequest("Page harus dimulai dari 1".to_string()));
    }
    
    if limit == 0 {
        return Err(AppError::BadRequest("Limit harus lebih besar dari 0".to_string()));
    }
    
    if limit > 100 {
        return Err(AppError::BadRequest("Limit maksimal 100 items per page".to_string()));
    }
    
    Ok((page, limit))
}

/// Validasi email format basic
pub fn validate_email_basic(email: &str) -> AppResult<()> {
    if !email.contains('@') || !email.contains('.') || email.len() < 5 {
        return Err(AppError::BadRequest("Format email tidak valid".to_string()));
    }
    
    if email.len() > 255 {
        return Err(AppError::BadRequest("Email terlalu panjang (max 255 karakter)".to_string()));
    }
    
    Ok(())
}

/// Validasi string tidak kosong dan dalam batas panjang
pub fn validate_string_length(
    value: &str, 
    field_name: &str, 
    min_len: usize, 
    max_len: usize
) -> AppResult<()> {
    let trimmed = value.trim();
    
    if trimmed.is_empty() {
        return Err(AppError::BadRequest(format!("{} tidak boleh kosong", field_name)));
    }
    
    if trimmed.len() < min_len {
        return Err(AppError::BadRequest(
            format!("{} minimal {} karakter", field_name, min_len)
        ));
    }
    
    if trimmed.len() > max_len {
        return Err(AppError::BadRequest(
            format!("{} maksimal {} karakter", field_name, max_len)
        ));
    }
    
    Ok(())
}

/// Validasi idempotency key format
pub fn validate_idempotency_key(key: &str) -> AppResult<()> {
    if key.is_empty() {
        return Err(AppError::BadRequest("Idempotency key tidak boleh kosong".to_string()));
    }
    
    if key.len() > 255 {
        return Err(AppError::BadRequest("Idempotency key terlalu panjang (max 255 karakter)".to_string()));
    }
    
    // Check basic alphanumeric + dash/underscore
    if !key.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(AppError::BadRequest(
            "Idempotency key hanya boleh mengandung huruf, angka, dash, dan underscore".to_string()
        ));
    }
    
    Ok(())
}

/// Validasi transaction ID dari payment gateway
pub fn validate_transaction_id(tx_id: &str) -> AppResult<()> {
    if tx_id.is_empty() {
        return Err(AppError::BadRequest("Transaction ID tidak boleh kosong".to_string()));
    }
    
    if tx_id.len() > 100 {
        return Err(AppError::BadRequest("Transaction ID terlalu panjang".to_string()));
    }
    
    Ok(())
}

/// Validasi analytics period
pub fn validate_analytics_period(period: &str) -> AppResult<()> {
    let valid_periods = ["daily", "weekly", "monthly", "yearly"];
    
    if !valid_periods.contains(&period) {
        return Err(AppError::BadRequest(
            format!("Analytics period '{}' tidak valid. Valid: {:?}", period, valid_periods)
        ));
    }
    
    Ok(())
}

/// Validasi days range untuk analytics
pub fn validate_days_range(days: u32) -> AppResult<u32> {
    if days == 0 {
        return Err(AppError::BadRequest("Days harus lebih besar dari 0".to_string()));
    }
    
    if days > 365 {
        return Err(AppError::BadRequest("Days maksimal 365 hari".to_string()));
    }
    
    Ok(days)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::BigDecimal;

    #[test]
    fn test_validate_positive_amount() {
        let positive = BigDecimal::from(100);
        let zero = BigDecimal::from(0);
        let negative = BigDecimal::from(-50);
        
        assert!(validate_positive_amount(&positive, "amount").is_ok());
        assert!(validate_positive_amount(&zero, "amount").is_err());
        assert!(validate_positive_amount(&negative, "amount").is_err());
    }
    
    #[test]
    fn test_validate_payment_method() {
        assert!(validate_payment_method("credit_card").is_ok());
        assert!(validate_payment_method("qris").is_ok());
        assert!(validate_payment_method("invalid_method").is_err());
    }

    #[test]
    fn test_validate_pagination() {
        assert!(validate_pagination(1, 10).is_ok());
        assert!(validate_pagination(0, 10).is_err());
        assert!(validate_pagination(1, 0).is_err());
        assert!(validate_pagination(1, 101).is_err());
    }
}