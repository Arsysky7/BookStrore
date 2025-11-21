// auth-service/src/db/security_service.rs

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::{rand_core::OsRng, SaltString};
use super::DatabaseError;

/// Layanan keamanan untuk handling password
pub struct SecurityService {
    argon2: Argon2<'static>,
    pub(super) pepper: Vec<u8>,
}

impl SecurityService {
    /// Membuat instance baru SecurityService
    pub fn new(pepper: &[u8]) -> Self {
        Self {
            argon2: Argon2::default(),
            pepper: pepper.to_vec(),
        }
    }

    /// Hash password dengan pepper
    pub fn hash_password(&self, password: &str) -> Result<String, DatabaseError> {
        let peppered_password = format!("{}{}", password, String::from_utf8_lossy(&self.pepper));
        let salt = SaltString::generate(&mut OsRng);
        
        let password_hash = self.argon2
            .hash_password(peppered_password.as_bytes(), &salt)
            .map_err(|_| DatabaseError::Connection(
                sqlx::Error::Protocol("Password hashing failed".into())
            ))?;
        
        Ok(password_hash.to_string())
    }

    /// Verifikasi password dengan proteksi timing attack
    pub async fn verify_password(&self, password: &str, hash: &str) -> Result<bool, DatabaseError> {
        let peppered_password = format!("{}{}", password, String::from_utf8_lossy(&self.pepper));
        
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|_| DatabaseError::InvalidCredentials)?;

        let result = self.argon2.verify_password(peppered_password.as_bytes(), &parsed_hash);
        
        // Delay konstan untuk mencegah timing attack
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        Ok(result.is_ok())
    }

    /// Validasi kekuatan password
    pub fn validate_password_strength(&self, password: &str) -> bool {
        password.len() >= 8 &&
        password.chars().any(|c| c.is_lowercase()) &&
        password.chars().any(|c| c.is_uppercase()) &&
        password.chars().any(|c| c.is_numeric()) &&
        password.chars().any(|c| "!@#$%^&*()_+-=[]{}|;:,.<>?".contains(c))
    }
}