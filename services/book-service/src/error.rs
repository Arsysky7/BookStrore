// /pdf-bookstore/services/auth-service/src/error.rs
use axum::http::StatusCode;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    ExternalService(String),
    Database(String),
    Unauthorized(String),
}

impl From<AppError> for StatusCode {
    fn from(err: AppError) -> Self {
        match err {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::ExternalService(_) => StatusCode::SERVICE_UNAVAILABLE,
            AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
        }
    }
}