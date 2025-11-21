// /pdf-bookstore/services/auth-service/src/utils/error.rs

use axum::http::StatusCode;
use std::fmt;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    Unauthorized(String),
    BadRequest(String),
    InternalServer(String),
    Database(String),
    ExternalService(String),
    Validation(String),
    Configuration(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::NotFound(msg) => write!(f, "Not Found: {}", msg),
            AppError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            AppError::BadRequest(msg) => write!(f, "Bad Request: {}", msg),
            AppError::InternalServer(msg) => write!(f, "Internal Server Error: {}", msg),
            AppError::Database(msg) => write!(f, "Database Error: {}", msg),
            AppError::ExternalService(msg) => write!(f, "External Service Error: {}", msg),
            AppError::Validation(msg) => write!(f, "Validation Error: {}", msg),
            AppError::Configuration(msg) => write!(f, "Configuration Error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl From<AppError> for StatusCode {
    fn from(err: AppError) -> Self {
        match err {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::InternalServer(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::ExternalService(_) => StatusCode::SERVICE_UNAVAILABLE,
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::Configuration(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::NotFound("Data tidak ditemukan".to_string()),
            _ => AppError::Database(err.to_string()),
        }
    }
}