// /pdf-bookstore/services/payment-service/src/utils/error.rs
// Centralized error handling untuk payment service

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use thiserror::Error;
use crate::models::ErrorResponse;

/// Type alias untuk Result dengan AppError
pub type AppResult<T> = Result<T, AppError>;

/// Application error enum dengan semua possible errors
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Conflict: {0}")]
    Conflict(String),
    
    #[error("Bad request: {0}")]
    BadRequest(String),
    
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    
    #[error("Forbidden: {0}")]
    Forbidden(String),
    
    #[error("External service error: {0}")]
    ExternalService(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Internal server error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    /// Convert AppError ke HTTP response
    fn into_response(self) -> Response {
        let (status, error_code, message) = match &self {
            AppError::Database(msg) => {
                tracing::error!("Database error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "DATABASE_ERROR",
                    "Database operation failed".to_string(),
                )
            }
            AppError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                msg.clone(),
            ),
            AppError::Conflict(msg) => (
                StatusCode::CONFLICT,
                "CONFLICT",
                msg.clone(),
            ),
            AppError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                "BAD_REQUEST",
                msg.clone(),
            ),
            AppError::ValidationError(msg) => (
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                msg.clone(),
            ),
            AppError::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                msg.clone(),
            ),
            AppError::Forbidden(msg) => (
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                msg.clone(),
            ),
            AppError::ExternalService(msg) => {
                tracing::error!("External service error: {}", msg);
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "EXTERNAL_SERVICE_ERROR",
                    "External service unavailable".to_string(),
                )
            }
            AppError::Configuration(msg) => {
                tracing::error!("Configuration error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "CONFIGURATION_ERROR",
                    "Service configuration error".to_string(),
                )
            }
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "Internal server error".to_string(),
                )
            }
        };
        
        let body = Json(ErrorResponse {
            success: false,
            message,
            error_code: Some(error_code.to_string()),
            details: None,
        });
        
        (status, body).into_response()
    }
}

// Implement conversions dari common error types
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::NotFound("Resource not found".to_string()),
            _ => AppError::Database(err.to_string()),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::ExternalService(err.to_string())
    }
}
