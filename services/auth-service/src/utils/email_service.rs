// /pdf-bookstore/services/auth-service/src/utils/email_service.rs

use lettre::{
    Message, 
    AsyncSmtpTransport,
    AsyncTransport,
    message::header::ContentType,
    transport::smtp::authentication::Credentials,
};
use std::env;

pub struct EmailService {
    mailer: AsyncSmtpTransport<lettre::Tokio1Executor>,
    from_email: String,
}

impl EmailService {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let smtp_username = env::var("SMTP_USERNAME")?;
        let smtp_password = env::var("SMTP_PASSWORD")?;
        let from_email = env::var("EMAIL_FROM")?;

        // Get SMTP host from env or default to Gmail
        let smtp_host = env::var("SMTP_HOST").unwrap_or_else(|_| "smtp.gmail.com".to_string());

        tracing::info!("Initializing EmailService with host: {}, user: {}", smtp_host, smtp_username);

        let creds = Credentials::new(smtp_username.clone(), smtp_password.clone());

        let mailer = AsyncSmtpTransport::<lettre::Tokio1Executor>::starttls_relay(&smtp_host)?
            .credentials(creds)
            .build();

        Ok(Self { mailer, from_email })
    }
    
    pub async fn send_verification_email(
        &self, 
        to: &str, 
        token: &str
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let verify_link = format!("http://localhost:8080/verify-email?token={}", token);
        
        let body = format!(
            r#"<!DOCTYPE html>
            <html>
            <body>
                <h2>Welcome to Bookstore!</h2>
                <p>Please verify your email by clicking the link below:</p>
                <a href="{}" style="display: inline-block; padding: 10px 20px; background: #4CAF50; color: white; text-decoration: none; border-radius: 5px;">
                    Verify Email
                </a>
                <p>Or copy this link: {}</p>
                <p>This link expires in 24 hours.</p>
            </body>
            </html>"#,
            verify_link, verify_link
        );
        
        let email = Message::builder()
            .from(self.from_email.parse()?)
            .to(to.parse()?)
            .subject("Verify your Bookstore account")
            .header(ContentType::TEXT_HTML)
            .body(body)?;
            
        self.mailer.send(email).await?;
        Ok(())
    }
    
    pub async fn send_login_otp(
        &self,
        to: &str,
        otp: &str
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let body = format!(
            r#"<!DOCTYPE html>
            <html>
            <body>
                <h2>Your Login Code</h2>
                <div style="font-size: 32px; font-weight: bold; padding: 20px; background: #f0f0f0; text-align: center; font-family: monospace; border-radius: 5px;">
                    {}
                </div>
                <p>This code expires in 5 minutes.</p>
                <p>If you didn't request this, please ignore this email.</p>
            </body>
            </html>"#,
            otp
        );
        
        let email = Message::builder()
            .from(self.from_email.parse()?)
            .to(to.parse()?)
            .subject("Your Bookstore login code")
            .header(ContentType::TEXT_HTML)
            .body(body)?;
            
        self.mailer.send(email).await?;
        Ok(())
    }

    pub async fn send_password_reset(
        &self,
        to: &str,
        reset_code: &str
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let body = format!(
            r#"<!DOCTYPE html>
            <html>
            <body>
                <h2>Password Reset Request</h2>
                <p>We received a request to reset your password. Use the code below:</p>
                <div style="font-size: 32px; font-weight: bold; padding: 20px; background: #f0f0f0; text-align: center; font-family: monospace; border-radius: 5px; letter-spacing: 4px;">
                    {}
                </div>
                <p>This code expires in 1 hour.</p>
                <p><strong>Important:</strong> If you didn't request this password reset, please ignore this email and ensure your account is secure.</p>
                <p style="color: #888; font-size: 12px;">For security reasons, never share this code with anyone.</p>
            </body>
            </html>"#,
            reset_code
        );

        let email = Message::builder()
            .from(self.from_email.parse()?)
            .to(to.parse()?)
            .subject("Reset Your Bookstore Password")
            .header(ContentType::TEXT_HTML)
            .body(body)?;

        self.mailer.send(email).await?;
        Ok(())
    }
}