// /pdf-bookstore/services/auth-service/src/docs.rs

use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

use crate::models::{
    RegisterRequest,
    LoginRequest,
    AuthResponse,
    UserProfile,
    ErrorResponse,
};

pub struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            )
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        // Cuma include yang udah ada #[utoipa::path] di handlers
        crate::api::handlers::auth::register_user,
        crate::api::handlers::auth::login_user,
        crate::api::handlers::auth::refresh_access_token,
        crate::api::handlers::auth::logout,
        crate::api::handlers::user::get_profile,
    ),
    components(
        schemas(
            RegisterRequest,
            LoginRequest,
            AuthResponse,
            UserProfile,
            ErrorResponse,
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "auth", description = "Authentication endpoints"),
        (name = "profile", description = "User profile management"),
    ),
    info(
        title = "Bookstore Auth Service API",
        version = "1.0.0",
        description = "Auth service with test credentials:\n\n\
                      - User: `test@example.com` / `Test123!@#`\n\
                      - Admin: `admin@bookstore.com` / `Test123!@#`",
    ),
    servers(
        (url = "http://localhost:3001", description = "Local"),
    )
)]
pub struct ApiDoc;