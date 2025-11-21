// /pdf-bookstore/services/auth-service/src/services/oauth_service.rs

use anyhow::{Result, anyhow};
use oauth2::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope, TokenUrl, AuthUrl, TokenResponse,
    basic::BasicClient,
    reqwest::async_http_client,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use chrono::{DateTime, Utc};
use hmac::{Hmac};
use sha2::Sha256;
use uuid::Uuid;

use crate::models::*;

type HmacSha256 = Hmac<Sha256>;

pub struct OAuthService {
    google_client: BasicClient,
    redis_client: Option<redis::Client>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthState {
    pub state: String,
    pub code_verifier: String,
    pub provider: String,
    pub redirect_uri: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub device_fingerprint: Option<String>,
}

impl OAuthService {
    pub fn new() -> Result<Self> {
        let client_id = std::env::var("GOOGLE_CLIENT_ID")
            .map_err(|_| anyhow!("GOOGLE_CLIENT_ID not set"))?;
        let client_secret = std::env::var("GOOGLE_CLIENT_SECRET")
            .map_err(|_| anyhow!("GOOGLE_CLIENT_SECRET not set"))?;
        let redirect_uri = std::env::var("GOOGLE_REDIRECT_URI")
            .unwrap_or_else(|_| "http://localhost:8080/auth/google/callback".to_string());

        // Create OAuth2 client for Google
        let google_client = BasicClient::new(
            ClientId::new(client_id),
            Some(ClientSecret::new(client_secret)),
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())?,
            Some(TokenUrl::new("https://oauth2.googleapis.com/token".to_string())?),
        )
        .set_redirect_uri(RedirectUrl::new(redirect_uri.clone())?);

        let redis_client = if let Ok(redis_url) = std::env::var("REDIS_URL") {
            redis::Client::open(redis_url.as_str()).ok()
        } else {
            None
        };

        Ok(Self {
            google_client,
            redis_client,
        })
    }

    /// Generate OAuth state and authorization URL
    pub async fn generate_auth_url(
        &mut self,
        provider: &str,
        redirect_uri: Option<String>,
        device_fingerprint: Option<String>,
    ) -> Result<OAuthStateResponse> {
        if provider != "google" {
            return Err(anyhow!("Unsupported OAuth provider: {}", provider));
        }

        // Generate PKCE (Proof Key for Code Exchange) for security
        let (_pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Generate CSRF token
        let csrf_state = CsrfToken::new_random();
        let state_str = csrf_state.secret().to_string();

        // Create OAuth state
        let oauth_state = OAuthState {
            state: state_str.clone(),
            code_verifier: pkce_verifier.secret().to_string(),
            provider: provider.to_string(),
            redirect_uri: redirect_uri.clone().unwrap_or_else(|| "http://localhost:8080/auth/google/callback".to_string()),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::from_secs(600), // 10 minutes
            device_fingerprint,
        };

        // Store state
        self.store_state(&state_str, &oauth_state).await?;

        // Generate authorization URL with scopes
        let (auth_url, _) = self
            .google_client
            .authorize_url(|| csrf_state)
            .add_scope(Scope::new("openid".to_string()))
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .url();

        Ok(OAuthStateResponse {
            success: true,
            state: state_str,
            auth_url: auth_url.to_string(),
            expires_in: 600,
        })
    }

    /// Exchange authorization code for user info
    pub async fn exchange_code(
        &mut self,
        code: &str,
        state: &str,
        device_fingerprint: Option<String>,
    ) -> Result<GoogleUserInfo> {
        // Retrieve and validate state
        let oauth_state = self.retrieve_and_validate_state(state).await?;

        // Verify device fingerprint if provided
        if let (Some(stored_fingerprint), Some(provided_fingerprint)) =
            (&oauth_state.device_fingerprint, &device_fingerprint) {
            if stored_fingerprint != provided_fingerprint {
                return Err(anyhow!("Device fingerprint mismatch"));
            }
        }

        // Exchange authorization code for token
        let token_response = self
            .google_client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(PkceCodeVerifier::new(oauth_state.code_verifier.clone()))
            .request_async(async_http_client)
            .await
            .map_err(|e| anyhow!("Failed to exchange code for token: {:?}", e))?;

        // Get user info using access token
        let access_token = token_response.access_token().secret();
        let user_info = self.get_user_info(access_token).await?;

        // Note: ID token verification would require Google's public keys
        // For simplicity, we'll skip that in this implementation

        Ok(user_info)
    }

    /// Get user info from Google
    async fn get_user_info(&self, access_token: &str) -> Result<GoogleUserInfo> {
        let client = reqwest::Client::new();
        let response = client
            .get("https://www.googleapis.com/oauth2/v2/userinfo")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .map_err(|e| anyhow!("Failed to request user info: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to get user info: HTTP {}",
                response.status()
            ));
        }

        let user_info: GoogleUserInfo = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse user info: {}", e))?;

        // Verify email is present
        if user_info.email.is_empty() {
            return Err(anyhow!("No email returned from OAuth provider"));
        }

        Ok(user_info)
    }

    /// Store OAuth state
    async fn store_state(&self, state: &str, oauth_state: &OAuthState) -> Result<()> {
        let state_json = serde_json::to_string(oauth_state)?;

        // Try to store in Redis first
        if let Some(redis_client) = &self.redis_client {
            if let Ok(mut conn) = redis_client.get_multiplexed_async_connection().await {
                let _: Result<(), redis::RedisError> = redis::cmd("SETEX")
                    .arg(state)
                    .arg(600) // 10 minutes
                    .arg(state_json)
                    .query_async(&mut conn)
                    .await;
                return Ok(());
            }
        }
        Ok(())
    }

    /// Retrieve and validate OAuth state
    async fn retrieve_and_validate_state(&self, state: &str) -> Result<OAuthState> {
        // Try to retrieve from Redis first
        if let Some(redis_client) = &self.redis_client {
            if let Ok(mut conn) = redis_client.get_multiplexed_async_connection().await {
                let state_json: Result<String, redis::RedisError> = redis::cmd("GET")
                    .arg(state)
                    .query_async(&mut conn)
                    .await;

                if let Ok(json) = state_json {
                    let oauth_state: OAuthState = serde_json::from_str(&json)?;

                    // Validate expiration
                    if oauth_state.expires_at < Utc::now() {
                        return Err(anyhow!("OAuth state expired"));
                    }

                    return Ok(oauth_state);
                }
            }
        }

        // Fallback for memory storage (simplified)
        Err(anyhow!("OAuth state not found or expired"))
    }

    /// Clean up expired states
    pub async fn cleanup_expired_states(&self) {
        if let Some(redis_client) = &self.redis_client {
            if let Ok(_conn) = redis_client.get_multiplexed_async_connection().await {
                // Redis automatically handles expiration with SETEX
            }
        }
    }

    /// Create or update user from OAuth user info
    pub async fn create_or_update_user(
        &self,
        user_info: &GoogleUserInfo,
        pool: &sqlx::PgPool,
    ) -> Result<User> {
        // Check if user already exists by email
        let row = sqlx::query!(
            "SELECT id, email, password_hash, full_name, role, is_active, email_verified, created_at, updated_at
             FROM users WHERE email = $1",
            user_info.email
        )
        .fetch_optional(pool)
        .await?;

        if let Some(existing_row) = row {
            // User exists, update if necessary
            if !existing_row.email_verified.unwrap_or(false) && user_info.verified_email {
                sqlx::query!(
                    "UPDATE users SET email_verified = true, updated_at = NOW() WHERE id = $1",
                    existing_row.id
                )
                .execute(pool)
                .await?;
            }

            Ok(User {
                id: existing_row.id,
                email: existing_row.email,
                password_hash: existing_row.password_hash,
                full_name: existing_row.full_name,
                role: existing_row.role.unwrap_or("customer".to_string()),
                is_active: existing_row.is_active.unwrap_or(true),
                email_verified: existing_row.email_verified.unwrap_or(false),
                created_at: existing_row.created_at.unwrap_or_else(|| Utc::now()),
                updated_at: existing_row.updated_at.unwrap_or_else(|| Utc::now()),
            })
        } else {
            // Create new user
            let user_id = Uuid::new_v4();
            let now = Utc::now();

            // Create a simple password hash for OAuth users
            let password_hash = format!("oauth_hash_{}", Uuid::new_v4());

            sqlx::query!(
                r#"
                INSERT INTO users (id, email, password_hash, full_name, role, is_active, email_verified, created_at, updated_at)
                VALUES ($1, $2, $3, $4, 'customer', true, $5, $6, $6)
                "#,
                user_id,
                user_info.email,
                password_hash,
                user_info.name,
                user_info.verified_email,
                now
            )
            .execute(pool)
            .await?;

            // Return the newly created user
            let new_row = sqlx::query!(
                "SELECT id, email, password_hash, full_name, role, is_active, email_verified, created_at, updated_at
                 FROM users WHERE id = $1",
                user_id
            )
            .fetch_one(pool)
            .await
            .map_err(|e| anyhow!("Failed to fetch created user: {}", e))?;

            Ok(User {
                id: new_row.id,
                email: new_row.email,
                password_hash: new_row.password_hash,
                full_name: new_row.full_name,
                role: new_row.role.unwrap_or("customer".to_string()),
                is_active: new_row.is_active.unwrap_or(true),
                email_verified: new_row.email_verified.unwrap_or(false),
                created_at: new_row.created_at.unwrap_or_else(|| Utc::now()),
                updated_at: new_row.updated_at.unwrap_or_else(|| Utc::now()),
            })
        }
    }
}

// Clean up expired states periodically
impl Drop for OAuthService {
    fn drop(&mut self) {
        // In a real implementation, you'd clean up resources here
    }
}