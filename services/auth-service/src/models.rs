// /pdf-bookstore/services/auth-service/src/models.rs

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use validator::{Validate, ValidationError};
use regex::Regex;
use lazy_static::lazy_static;
use utoipa::ToSchema;

lazy_static! {
    static ref PHONE_REGEX: Regex = Regex::new(
        r"^(\+62|62|0)8[1-9][0-9]{6,11}$"
    ).unwrap();
}

// ===== USER ENTITIES =====

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub full_name: String,
    pub role: String,
    pub is_active: bool,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub role: String,
    pub exp: usize,
    pub iat: usize,
    pub iss: String,
    pub aud: String,
    pub session_id: Option<String>,
    pub permissions: Option<Vec<String>>,
    pub ip_address: Option<String>,
}

// ===== REQUEST MODELS =====

#[derive(Debug, Deserialize, Validate, Clone, ToSchema)]
pub struct RegisterRequest {
    #[validate(email(message = "Format email tidak valid"))]
    #[validate(length(max = 255, message = "Email maksimal 255 karakter"))]
    pub email: String,

    #[validate(length(min = 8, max = 128, message = "Password harus 8-128 karakter"))]
    #[validate(custom(function = "validate_password_strength", message = "Password tidak memenuhi kriteria keamanan"))]
    pub password: String,

    #[validate(length(min = 2, max = 255, message = "Nama lengkap harus 2-255 karakter"))]
    #[validate(custom(function = "validate_no_html", message = "Nama tidak boleh mengandung HTML"))]
    pub full_name: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct LoginRequest {
    #[validate(email(message = "Format email tidak valid"))]
    pub email: String,

    #[validate(length(min = 1, message = "Password wajib diisi"))]
    pub password: String,

     #[allow(dead_code)]
    pub remember_me: Option<bool>,

    #[allow(dead_code)]
    pub enable_otp: Option<bool>,

    #[schema(example = "device-fingerprint-123")]
    pub device_fingerprint: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct VerifyOtpRequest {
    #[validate(email(message = "Format email tidak valid"))]
    pub email: String,

    #[validate(length(min = 6, max = 6, message = "OTP must be 6 digits"))]
    pub otp: String,

    pub remember_me: Option<bool>,
    pub device_fingerprint: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct PasswordResetRequest {
    #[validate(email(message = "Format email tidak valid"))]
    pub email: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ResetPasswordRequest {
    pub token: String,
    #[validate(length(min = 8, max = 128, message = "Password harus 8-128 karakter"))]
    #[validate(custom(function = "validate_password_strength", message = "Password tidak memenuhi kriteria keamanan"))]
    pub new_password: String,
    pub confirm_password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateProfileRequest {
    #[validate(length(min = 2, max = 255, message = "Nama harus 2-255 karakter"))]
    pub full_name: Option<String>,

    #[validate(custom(function = "validate_phone"))]
    pub phone: Option<String>,

    #[validate(length(max = 500, message = "Bio maksimal 500 karakter"))]
    pub bio: Option<String>,

    #[validate(url(message = "Format URL avatar tidak valid"))]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ChangePasswordRequest {
    pub old_password: String,

    #[validate(length(min = 8, max = 128, message = "Password baru harus 8-128 karakter"))]
    #[validate(custom(function = "validate_password_strength", message = "Password baru tidak memenuhi kriteria keamanan"))]
    pub new_password: String,

    pub confirm_password: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyEmailRequest {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct LoginHistoryItem {
    pub id: Uuid,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub device_fingerprint: Option<String>,
    pub login_status: String,
    pub failure_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ===== OAUTH REQUEST MODELS =====

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct GoogleAuthRequest {
    pub code: String,
    pub state: String,
    pub device_fingerprint: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct OAuthStateRequest {
    pub provider: String,
    pub redirect_uri: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct OAuthCallbackRequest {
    pub code: String,
    pub state: String,
    pub provider: String,
    pub device_fingerprint: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct OAuthLoginRequest {
    pub provider: String,
    pub redirect_uri: Option<String>,
    pub device_fingerprint: Option<String>,
}

// ===== RESPONSE MODELS =====

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    pub success: bool,
    pub message: String,
    pub user: Option<UserProfile>,
    pub token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
    pub session_id: Option<String>,
    pub requires_verification: Option<bool>,
    pub two_factor_required: Option<bool>,
}

#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct UserProfile {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub role: String,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub permissions: Vec<String>,
    pub subscription_status: Option<String>,
    pub preferences: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EnhancedClaims {
    pub sub: String,
    pub email: String,
    pub role: String,
    pub exp: usize,
    pub iat: usize,
    pub iss: String,
    pub aud: String,
    pub jti: String,
    pub token_type: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TokenPairResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub refresh_expires_in: i64,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
    pub device_fingerprint: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LogoutRequest {
    pub refresh_token: Option<String>,
    pub access_token_jti: Option<String>,
}

// ===== OAUTH RESPONSE MODELS =====

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OAuthState {
    pub state: String,
    pub code_verifier: String,
    pub provider: String,
    pub redirect_uri: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub device_fingerprint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OAuthStateResponse {
    pub success: bool,
    pub state: String,
    pub auth_url: String,
    pub expires_in: u64,
}

// ===== ADMIN MODELS =====

#[derive(Debug, Serialize)]
pub struct AdminUserStats {
    pub total_users: i64,
    pub active_users: i64,
    pub new_users_this_month: i64,
    pub verified_users: i64,
    pub admin_users: i64,
    pub customer_users: i64,
    pub user_growth_percentage: f64,
    pub verification_rate: f64,
    pub churn_rate: f64,
    pub daily_active_users: i64,
    pub weekly_active_users: i64,
    pub monthly_active_users: i64,
    pub failed_login_attempts_today: i64,
    pub locked_accounts: i64,
    pub suspicious_activities: i64,
    pub paying_customers: i64,
    pub average_lifetime_value: f64,
    pub conversion_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct AdminUserProfile {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub role: String,
    pub is_active: bool,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub order_count: i64,
    pub total_spent: Option<String>,
    pub lifetime_value: Option<f64>,
    pub last_purchase: Option<DateTime<Utc>>,
    pub failed_login_attempts: i32,
    pub last_failed_login: Option<DateTime<Utc>>,
    pub account_locked_until: Option<DateTime<Utc>>,
    pub risk_score: Option<i32>,
    pub login_count: i64,
    pub device_count: i32,
}

#[derive(Debug, Serialize)]
pub struct UserActivity {
    pub id: Option<Uuid>,
    pub user_id: Uuid,
    pub user_name: String,
    pub user_email: String,
    pub activity_type: String,
    pub description: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub session_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub severity: ActivitySeverity,
    pub location: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub enum ActivitySeverity {
    Info,
    Warning,
    Critical,
    Security,
}

#[derive(Debug, Serialize)]
pub struct AdminPaginationMeta {
    pub current_page: u32,
    pub per_page: u32,
    pub total_items: i64,
    pub total_pages: u32,
    pub has_next: bool,
    pub has_prev: bool,
}

// ===== SERVICE COMMUNICATION MODELS =====

#[derive(Debug, Clone, Serialize, Default)]
pub struct UserOrderStats {
    pub total_orders: i64,
    pub total_spent: f64,
    pub last_purchase: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Purchase {
    pub order_id: Uuid,
    pub book_id: Uuid,
    pub amount: f64,
    pub status: String,
    pub purchased_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BookDetails {
    pub id: Uuid,
    pub title: String,
    pub author: String,
    pub price: f64,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadedBook {
    pub book_id: Uuid,
    pub title: String,
    pub download_count: i32,
    pub last_downloaded: Option<DateTime<Utc>>,
}

// ===== ERROR HANDLING =====

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub success: bool,
    pub message: String,
    pub error_code: Option<String>,
    pub details: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
    pub suggestions: Vec<String>,
}

// ===== OAUTH GOOGLE MODELS =====

#[derive(Debug, Deserialize)]
pub struct GoogleTokenInfo {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: Option<String>,
    pub id_token: String,
}

#[derive(Debug, Deserialize)]
pub struct GoogleUserInfo {
    pub id: String,
    pub email: String,
    pub verified_email: bool,
    pub name: String,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub picture: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GoogleIdToken {
    pub iss: String,
    pub azp: String,
    pub aud: String,
    pub sub: String,
    pub email: String,
    pub email_verified: bool,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub locale: Option<String>,
    pub iat: usize,
    pub exp: usize,
}

#[derive(Debug, Clone)]
pub struct OAuthProvider {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub scope: String,
    pub auth_url: String,
    pub token_url: String,
    pub user_info_url: String,
}

// ===== IMPLEMENTATIONS =====

impl From<User> for UserProfile {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            full_name: user.full_name,
            role: user.role.clone(),
            email_verified: user.email_verified,
            created_at: user.created_at,
            last_login: Some(user.updated_at),
            permissions: get_permissions_for_role(&user.role),
            subscription_status: None,
            preferences: None,
        }
    }
}

impl AuthResponse {
    pub fn success(message: &str) -> Self {
        Self {
            success: true,
            message: message.to_string(),
            user: None,
            token: None,
            refresh_token: None,
            expires_in: None,
            session_id: None,
            requires_verification: None,
            two_factor_required: None,
        }
    }

    pub fn error(message: &str) -> Self {
        Self {
            success: false,
            message: message.to_string(),
            user: None,
            token: None,
            refresh_token: None,
            expires_in: None,
            session_id: None,
            requires_verification: None,
            two_factor_required: None,
        }
    }

    pub fn with_user(user: UserProfile, message: &str) -> Self {
        Self {
            success: true,
            message: message.to_string(),
            user: Some(user),
            token: None,
            refresh_token: None,
            expires_in: None,
            session_id: None,
            requires_verification: None,
            two_factor_required: None,
        }
    }
}

impl AdminPaginationMeta {
    pub fn new(page: u32, per_page: u32, total_items: i64) -> Self {
        let total_pages = ((total_items as f64) / (per_page as f64)).ceil().max(1.0) as u32;

        Self {
            current_page: page,
            per_page,
            total_items,
            total_pages,
            has_next: page < total_pages,
            has_prev: page > 1,
        }
    }
}

impl ErrorResponse {
    pub fn new(message: &str, code: Option<&str>) -> Self {
        Self {
            success: false,
            message: message.to_string(),
            error_code: code.map(|s| s.to_string()),
            details: None,
            timestamp: Utc::now(),
            request_id: None,
            trace_id: None,
            suggestions: vec![],
        }
    }

    pub fn validation_error(errors: validator::ValidationErrors) -> Self {
        let mut error_messages = Vec::new();
        for (field, field_errors) in errors.field_errors() {
            for error in field_errors {
                if let Some(msg) = &error.message {
                    error_messages.push(format!("{}: {}", field, msg));
                }
            }
        }

        Self {
            success: false,
            message: if error_messages.is_empty() {
                "Validation failed".to_string()
            } else {
                error_messages.join(", ")
            },
            error_code: Some("VALIDATION_ERROR".to_string()),
            details: None,
            timestamp: Utc::now(),
            request_id: None,
            trace_id: None,
            suggestions: vec![],
        }
    }
}

// ===== HELPER FUNCTIONS =====

fn get_permissions_for_role(role: &str) -> Vec<String> {
    match role {
        "admin" => vec![
            "users:read".to_string(),
            "users:write".to_string(),
            "users:delete".to_string(),
            "books:read".to_string(),
            "books:write".to_string(),
            "orders:read".to_string(),
            "orders:write".to_string(),
            "admin:access".to_string(),
        ],
        "customer" => vec![
            "books:read".to_string(),
            "orders:read".to_string(),
            "profile:write".to_string(),
        ],
        _ => vec!["books:read".to_string()],
    }
}

fn validate_password_strength(password: &str) -> Result<(), ValidationError> {
    if password.len() < 8 {
        return Err(ValidationError::new("too_short"));
    }

    if !password.chars().any(|c| c.is_lowercase()) {
        return Err(ValidationError::new("no_lowercase"));
    }

    if !password.chars().any(|c| c.is_uppercase()) {
        return Err(ValidationError::new("no_uppercase"));
    }

    if !password.chars().any(|c| c.is_numeric()) {
        return Err(ValidationError::new("no_digit"));
    }

    if !password.chars().any(|c| "!@#$%^&*()_+-=[]{}|;:,.<>?".contains(c)) {
        return Err(ValidationError::new("no_special"));
    }

    Ok(())
}

fn validate_no_html(input: &str) -> Result<(), ValidationError> {
    let html_patterns = ["<script", "<iframe", "<object", "<embed", "javascript:", "data:"];

    let input_lower = input.to_lowercase();
    if html_patterns.iter().any(|pattern| input_lower.contains(pattern)) {
        Err(ValidationError::new("contains_html"))
    } else {
        Ok(())
    }
}

fn validate_phone(phone: &str) -> Result<(), ValidationError> {
    if PHONE_REGEX.is_match(phone) {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_phone"))
    }
}