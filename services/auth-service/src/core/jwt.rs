// /pdf-bookstore/services/auth-service/src/core/jwt.rs


use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey, Algorithm};
use uuid::Uuid;
use chrono::{Utc, Duration};
use std::env;

use crate::models::{User, Claims, EnhancedClaims, TokenPairResponse};


/// service untuk generate dan verify jwt token 
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
    issuer: String,
    audience: String,
}

impl JwtService {
    /// Setup JWT service dengan secret dari environment
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let secret = env::var("JWT_SECRET")
            .map_err(|_| "JWT_SECRET environment variable not set")?;
            
        if secret.len() < 32 {
            return Err("JWT_SECRET must be at least 32 characters long".into());
        }

        let issuer = env::var("JWT_ISSUER")
            .unwrap_or_else(|_| "bookstore-auth-service".to_string());
        let audience = env::var("JWT_AUDIENCE")
            .unwrap_or_else(|_| "bookstore-app".to_string());

        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[&issuer]);
        validation.set_audience(&[&audience]);
        validation.validate_exp = true;
        validation.leeway = 60;

        Ok(Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            validation,
            issuer,
            audience,
        })
    }

    /// Generate JWT token untuk user login
    pub fn generate_token(&self, user: &User) -> Result<String, Box<dyn std::error::Error>> {
        let now = Utc::now();
        let exp_hours = env::var("JWT_EXPIRES_IN")
            .unwrap_or_else(|_| "24h".to_string())
            .trim_end_matches('h')
            .parse::<i64>()
            .unwrap_or(24);

        let claims = Claims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            role: user.role.clone(),
            exp: (now + Duration::hours(exp_hours)).timestamp() as usize,
            iat: now.timestamp() as usize,
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            session_id: Some(Uuid::new_v4().to_string()),
            permissions: Some(get_permissions_for_role(&user.role)),
            ip_address: None,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| e.into())
    }
    
    /// Verify JWT token dan cek expiration
    pub fn verify_token(&self, token: &str) -> Result<Claims, Box<dyn std::error::Error>> {
        let token_data = decode::<Claims>(token, &self.decoding_key, &self.validation)?;
        
        let now = Utc::now().timestamp() as usize;
        if token_data.claims.exp <= now {
            return Err("Token has expired".into());
        }

        if token_data.claims.iat > now + 300 {
            return Err("Token issued in the future".into());
        }

        Ok(token_data.claims)
    }

    /// Generate token pair (access + refresh) untuk dual token authentication
    pub fn generate_token_pair(&self, user: &User) -> Result<TokenPairResponse, Box<dyn std::error::Error>> {
        let now = Utc::now();
        
        // ACCESS TOKEN - 15 menit
        let access_jti = Uuid::new_v4().to_string();
        let access_exp = (now + Duration::minutes(15)).timestamp() as usize;
        
        let access_claims = EnhancedClaims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            role: user.role.clone(),
            exp: access_exp,
            iat: now.timestamp() as usize,
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            jti: access_jti,
            token_type: "access".to_string(),
        };
        
        let access_token = encode(&Header::default(), &access_claims, &self.encoding_key)?;
        
        // REFRESH TOKEN - 7 hari
        let refresh_jti = Uuid::new_v4().to_string();
        let refresh_exp = (now + Duration::days(7)).timestamp() as usize;
        
        let refresh_claims = EnhancedClaims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            role: user.role.clone(),
            exp: refresh_exp,
            iat: now.timestamp() as usize,
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            jti: refresh_jti,
            token_type: "refresh".to_string(),
        };
        
        let refresh_token = encode(&Header::default(), &refresh_claims, &self.encoding_key)?;
        
        Ok(TokenPairResponse {
            access_token,
            refresh_token,
            expires_in: 900, 
            refresh_expires_in: 604800, 
        })
    }

    /// Generate token dengan durasi custom (untuk remember me)
    pub fn generate_token_with_duration(
        &self,
        user: &User,
        duration: Duration
    ) -> Result<String, Box<dyn std::error::Error>> {
        let now = Utc::now();
        let exp = (now + duration).timestamp() as usize;

        let claims = Claims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            role: user.role.clone(),
            exp,
            iat: now.timestamp() as usize,
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            session_id: Some(Uuid::new_v4().to_string()),
            permissions: Some(get_permissions_for_role(&user.role)),
            ip_address: None,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| e.into())
    }
    
    /// Verify token dengan blacklist check
    pub async fn verify_token_with_blacklist(
        &self,
        token: &str,
        db: &sqlx::PgPool
    ) -> Result<EnhancedClaims, Box<dyn std::error::Error>> {
        // Decode token
        let token_data = decode::<EnhancedClaims>(
            token,
            &self.decoding_key,
            &self.validation
        )?;
        
        // Check expiry
        let now = Utc::now().timestamp() as usize;
        if token_data.claims.exp <= now {
            return Err("Token has expired".into());
        }
        
        // Check blacklist
        let blacklisted = sqlx::query!(
            "SELECT COUNT(*) as count FROM token_blacklist WHERE token_jti = $1",
            token_data.claims.jti
        )
        .fetch_one(db)
        .await?;
        
        if blacklisted.count.unwrap_or(0) > 0 {
            return Err("Token has been revoked".into());
        }
        
        Ok(token_data.claims)
    }
}

/// Mendapatkan permissions berdasarkan role user
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