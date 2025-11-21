// /pdf-bookstore/services/auth-service/src/api/handlers/mod.rs

pub mod auth;
pub mod user;
pub mod admin;
pub mod internal;
pub mod oauth;

// Re-export semua handler functions
pub use auth::*;
pub use user::*;
pub use admin::*;
pub use internal::*;
pub use oauth::*;