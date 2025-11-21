// /pdf-bookstore/services/auth-service/src/db/user_repository.rs

use sqlx::{PgPool, Row, Postgres, QueryBuilder};
use uuid::Uuid;
use chrono::{Utc, Datelike};
use sha2::{Sha256, Digest};
use std::net::IpAddr;
use thiserror::Error;

use crate::models::{User, RegisterRequest, AdminUserStats, AdminUserProfile, AdminPaginationMeta, UserActivity, ActivitySeverity};
use super::security_service::SecurityService;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Error koneksi database")]
    Connection(#[from] sqlx::Error),
    #[error("User tidak ditemukan")]
    UserNotFound,
    #[error("Email sudah terdaftar")]
    EmailExists,
    #[error("Kredensial tidak valid")]
    InvalidCredentials,
    #[error("Akun terkunci karena alasan keamanan")]
    AccountLocked,
    #[error("Akses ditolak")]
    AccessDenied,
    #[error("Batas rate limit terlampaui")]
    RateLimitExceeded,
    #[error("Session tidak valid")]
    InvalidSession,
    #[error("Pagination tidak valid")]
    InvalidPagination,
    #[error("Akses admin ditolak")]
    AdminAccessDenied,
    #[error("Invalid query parameters")]
    InvalidQuery,
}

/// Informasi sesi untuk tracking login user
pub struct SessionInfo {
    pub device_info: Option<String>,
    pub ip_address: Option<IpAddr>,
}

/// Repository untuk operasi database terkait user
pub struct UserRepository {
    pub security_service: SecurityService,
}

impl UserRepository {
    /// Membuat instance baru UserRepository
    pub fn new(pepper: &[u8]) -> Self {
        Self {
            security_service: SecurityService::new(pepper),
        }
    }

    /// Membuat user baru di database
    pub async fn create_user(
        &self,
        pool: &PgPool,
        request: RegisterRequest,
    ) -> Result<User, DatabaseError> {
        let normalized_email = request.email.trim().to_lowercase();

        // Cek apakah email sudah terdaftar
        let existing = sqlx::query!(
            "SELECT COUNT(*) as count FROM users WHERE LOWER(TRIM(email)) = $1",
            normalized_email
        )
        .fetch_one(pool)
        .await?;

        if existing.count.unwrap_or(0) > 0 {
            self.log_security_event(
                pool,
                None,
                "REGISTRATION_ATTEMPT_EXISTING_EMAIL",
                serde_json::json!({ "email": normalized_email }),
                false,
            ).await?;
            
            return Err(DatabaseError::EmailExists);
        }

        // Validasi kekuatan password
        if !self.security_service.validate_password_strength(&request.password) {
            return Err(DatabaseError::InvalidCredentials);
        }

        // Hash password
        let password_hash = self.security_service.hash_password(&request.password)?;
        
        // Mulai transaksi database
        let mut tx = pool.begin().await?;
        
        // Insert user baru
        let user = sqlx::query(
            r#"
            INSERT INTO users (email, password_hash, full_name, role)
            VALUES ($1, $2, $3, 'customer')
            RETURNING id, email, password_hash, full_name, role, is_active, 
                      email_verified, created_at, updated_at
            "#
        )
        .bind(&normalized_email)
        .bind(&password_hash)
        .bind(request.full_name.trim())
        .fetch_one(&mut *tx)
        .await
        .map(|row| User {
            id: row.get("id"),
            email: row.get("email"),
            password_hash: row.get("password_hash"),
            full_name: row.get("full_name"),
            role: row.get::<Option<String>, _>("role").unwrap_or_else(|| "customer".to_string()),
            is_active: row.get::<Option<bool>, _>("is_active").unwrap_or(true),
            email_verified: row.get::<Option<bool>, _>("email_verified").unwrap_or(false),
            created_at: row.get::<Option<chrono::DateTime<Utc>>, _>("created_at").unwrap_or_else(|| Utc::now()),
            updated_at: row.get::<Option<chrono::DateTime<Utc>>, _>("updated_at").unwrap_or_else(|| Utc::now()),
        })?;

        // Log pembuatan user berhasil
        self.log_security_event(
            &mut *tx,
            Some(user.id),
            "USER_REGISTERED",
            serde_json::json!({ "email": normalized_email }),
            true,
        ).await?;

        tx.commit().await?;
        Ok(user)
    }

    /// Mencari user berdasarkan email
    pub async fn find_by_email(
        &self,
        pool: &PgPool,
        email: &str,
        client_ip: Option<IpAddr>,
    ) -> Result<User, DatabaseError> {
        let normalized_email = email.trim().to_lowercase();
        
        // Cek rate limiting
        if let Some(ip) = client_ip {
            if !self.check_login_rate_limit(pool, &ip.to_string()).await? {
                return Err(DatabaseError::RateLimitExceeded);
            }
        }

        // Query database
        let user = sqlx::query(
            r#"
            SELECT id, email, password_hash, full_name, role, is_active, 
                   email_verified, created_at, updated_at 
            FROM users 
            WHERE LOWER(TRIM(email)) = $1 AND is_active = true
            "#
        )
        .bind(&normalized_email)
        .fetch_optional(pool)
        .await?
        .map(|row| User {
            id: row.get("id"),
            email: row.get("email"),
            password_hash: row.get("password_hash"),
            full_name: row.get("full_name"),
            role: row.get::<Option<String>, _>("role").unwrap_or_else(|| "customer".to_string()),
            is_active: row.get::<Option<bool>, _>("is_active").unwrap_or(true),
            email_verified: row.get::<Option<bool>, _>("email_verified").unwrap_or(false),
            created_at: row.get::<Option<chrono::DateTime<Utc>>, _>("created_at").unwrap_or_else(|| Utc::now()),
            updated_at: row.get::<Option<chrono::DateTime<Utc>>, _>("updated_at").unwrap_or_else(|| Utc::now()),
        });

        match user {
            Some(user) => {
                if self.is_account_locked(pool, user.id).await? {
                    return Err(DatabaseError::AccountLocked);
                }
                Ok(user)
            }
            None => {
                if let Some(ip) = client_ip {
                    self.log_security_event(
                        pool,
                        None,
                        "LOGIN_ATTEMPT_UNKNOWN_EMAIL",
                        serde_json::json!({ 
                            "email": normalized_email,
                            "ip": ip.to_string()
                        }),
                        false,
                    ).await?;
                }
                Err(DatabaseError::UserNotFound)
            }
        }
    }

    /// Mencari user berdasarkan ID
    pub async fn find_by_id(&self, pool: &PgPool, user_id: Uuid) -> Result<User, DatabaseError> {
        let user = sqlx::query(
            r#"
            SELECT id, email, password_hash, full_name, role, is_active, 
                   email_verified, created_at, updated_at 
            FROM users 
            WHERE id = $1 AND is_active = true
            "#
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .map(|row| User {
            id: row.get("id"),
            email: row.get("email"),
            password_hash: row.get("password_hash"),
            full_name: row.get("full_name"),
            role: row.get::<Option<String>, _>("role").unwrap_or_else(|| "customer".to_string()),
            is_active: row.get::<Option<bool>, _>("is_active").unwrap_or(true),
            email_verified: row.get::<Option<bool>, _>("email_verified").unwrap_or(false),
            created_at: row.get::<Option<chrono::DateTime<Utc>>, _>("created_at").unwrap_or_else(|| Utc::now()),
            updated_at: row.get::<Option<chrono::DateTime<Utc>>, _>("updated_at").unwrap_or_else(|| Utc::now()),
        });

        match user {
            Some(user) => {
                if self.is_account_locked(pool, user.id).await? {
                    return Err(DatabaseError::AccountLocked);
                }
                Ok(user)
            }
            None => Err(DatabaseError::UserNotFound),
        }
    }

    /// Verifikasi password user dengan tracking percobaan
    pub async fn verify_password_with_attempts(
        &self,
        pool: &PgPool,
        user_id: Uuid,
        password: &str,
        client_ip: Option<IpAddr>,
    ) -> Result<bool, DatabaseError> {
        let user = sqlx::query!(
            "SELECT password_hash FROM users WHERE id = $1 AND is_active = true",
            user_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DatabaseError::UserNotFound)?;

        let is_valid = self.security_service.verify_password(password, &user.password_hash).await?;

        let event_type = if is_valid {"LOGIN_SUCCESS"} else {"LOGIN_FAILED"};
        self.log_security_event(
            pool,
            Some(user_id),
            event_type,
            serde_json::json!({
                "ip": client_ip.map(|ip| ip.to_string()),
                "success": is_valid
            }),
            is_valid,
        ).await?;

        if !is_valid {
            self.increment_failed_login_attempts(pool, user_id).await?;
        } else {
            self.reset_failed_login_attempts(pool, user_id).await?;
        }

        Ok(is_valid)
    }

    /// Validasi session dan get user
    pub async fn validate_session_and_get_user(
        &self,
        pool: &PgPool,
        session_token: &str,
    ) -> Result<User, DatabaseError> {
        let token_hash = self.hash_session_token(session_token);
        
        let result = sqlx::query!(
            r#"
            SELECT u.id, u.email, u.password_hash, u.full_name, u.role, 
                u.is_active, u.email_verified, u.created_at, u.updated_at,
                s.device_info, s.ip_address
            FROM sessions s  
            JOIN users u ON u.id = s.user_id
            WHERE s.session_token = $1  
            AND s.expires_at > NOW()
            AND s.is_active = true
            AND u.is_active = true
            "#,
            token_hash
        )
        .fetch_optional(pool)
        .await?;
        
        match result {
            Some(row) => {
                sqlx::query!(
                    "UPDATE sessions SET last_used_at = NOW() WHERE session_token = $1",
                    token_hash
                )
                .execute(pool)
                .await?;
                
                Ok(User {
                    id: row.id,
                    email: row.email,
                    password_hash: row.password_hash,
                    full_name: row.full_name,
                    role: row.role.unwrap_or("customer".to_string()),
                    is_active: row.is_active.unwrap_or(true),
                    email_verified: row.email_verified.unwrap_or(false),
                    created_at: row.created_at.unwrap_or(Utc::now()),
                    updated_at: row.updated_at.unwrap_or(Utc::now()),
                })
            }
            None => Err(DatabaseError::InvalidSession),
        }
    }

    /// Update last login dan buat session baru
    pub async fn update_last_login_with_session(
        &self,
        pool: &PgPool,
        user_id: Uuid,
        session_info: SessionInfo,
    ) -> Result<String, DatabaseError> {
        let mut tx = pool.begin().await?;

        sqlx::query!(
            "UPDATE users SET updated_at = NOW() WHERE id = $1",
            user_id
        )
        .execute(&mut *tx)
        .await?;

        let session_token = uuid::Uuid::new_v4().to_string();
        let token_hash = self.hash_session_token(&session_token);
        
        let ip_for_db = session_info.ip_address.map(|ip| ip.to_string());
      
        sqlx::query(
            r#"
            INSERT INTO sessions (user_id, session_token, device_info, ip_address, expires_at)
            VALUES ($1, $2, $3, $4::inet, NOW() + INTERVAL '24 hours')
            "#
        )
        .bind(user_id)
        .bind(token_hash)  
        .bind(session_info.device_info.clone())
        .bind(ip_for_db.as_deref())
        .execute(&mut *tx)
        .await?;

        self.log_security_event(
            &mut *tx,
            Some(user_id),
            "SESSION_CREATED",
            serde_json::json!({
                "ip": ip_for_db,
                "device": session_info.device_info
            }),
            true,
        ).await?;

        tx.commit().await?;
        Ok(session_token)
    }

    /// Check user access dengan proper error
    pub async fn check_user_access(
        &self,
        pool: &PgPool,
        user_id: Uuid,
        required_role: Option<&str>,
    ) -> Result<(), DatabaseError> {
        let user = self.find_by_id(pool, user_id).await?;
        
        if let Some(role) = required_role {
            if role == "admin" && user.role != "admin" {
                return Err(DatabaseError::AdminAccessDenied);
            } else if user.role != role {
                return Err(DatabaseError::AccessDenied);
            }
        }
        
        if !user.is_active {
            return Err(DatabaseError::AccessDenied);
        }
        
        Ok(())
    }

    /// Mendapatkan statistik user untuk admin dashboard
    pub async fn get_admin_user_stats(&self, pool: &PgPool) -> Result<AdminUserStats, DatabaseError> {
        // Hitung periode bulan ini dan bulan lalu
        let now = Utc::now();
        let current_month = now.month();
        let current_year = now.year();
        
        let (prev_month, prev_year) = if current_month == 1 {
            (12, current_year - 1)
        } else {
            (current_month - 1, current_year)
        };

        // Query statistik user
        let stats_query = sqlx::query!(
            r#"
            SELECT 
                COUNT(*) as total_users,
                COUNT(*) FILTER (WHERE is_active = true) as active_users,
                COUNT(*) FILTER (WHERE email_verified = true) as verified_users,
                COUNT(*) FILTER (WHERE role = 'admin') as admin_users,
                COUNT(*) FILTER (WHERE role = 'customer') as customer_users,
                COUNT(*) FILTER (WHERE EXTRACT(MONTH FROM created_at) = $1 
                                   AND EXTRACT(YEAR FROM created_at) = $2) as new_users_this_month,
                COUNT(*) FILTER (WHERE EXTRACT(MONTH FROM created_at) = $3 
                                   AND EXTRACT(YEAR FROM created_at) = $4) as new_users_last_month
            FROM users
            "#,
            current_month as i32,
            current_year as i32,
            prev_month as i32,
            prev_year as i32
        )
        .fetch_one(pool)
        .await?;

        // Ekstrak hasil query
        let total_users = stats_query.total_users.unwrap_or(0);
        let active_users = stats_query.active_users.unwrap_or(0);
        let verified_users = stats_query.verified_users.unwrap_or(0);
        let admin_users = stats_query.admin_users.unwrap_or(0);
        let customer_users = stats_query.customer_users.unwrap_or(0);
        let new_users_this_month = stats_query.new_users_this_month.unwrap_or(0);
        let new_users_last_month = stats_query.new_users_last_month.unwrap_or(0);

        // Hitung persentase pertumbuhan user
        let user_growth_percentage = if new_users_last_month > 0 {
            ((new_users_this_month - new_users_last_month) as f64 / new_users_last_month as f64) * 100.0
        } else if new_users_this_month > 0 {
            100.0
        } else {
            0.0
        };

        // Hitung persentase verifikasi email
        let verification_rate = if total_users > 0 {
            (verified_users as f64 / total_users as f64) * 100.0
        } else {
            0.0
        };

        // Kembalikan struct statistik
        Ok(AdminUserStats {
            total_users,
            active_users,
            new_users_this_month,
            verified_users,
            admin_users,
            customer_users,
            user_growth_percentage,
            verification_rate,
            churn_rate: 0.0,
            daily_active_users: 0,
            weekly_active_users: 0,
            monthly_active_users: 0,
            failed_login_attempts_today: 0,
            locked_accounts: 0,
            suspicious_activities: 0,
            paying_customers: 0,
            average_lifetime_value: 0.0,
            conversion_rate: 0.0,
        })
    }

    /// Mendapatkan daftar user dengan paginasi (untuk admin)
    pub async fn get_admin_users_list(
        &self,
        pool: &PgPool,
        page: u32,
        per_page: u32,
        search: Option<&str>,
        role_filter: Option<&str>,
    ) -> Result<(Vec<AdminUserProfile>, AdminPaginationMeta), DatabaseError> {
        if page == 0 || per_page == 0 || per_page > 100 {
            return Err(DatabaseError::InvalidPagination);
        }

        let offset = (page - 1) * per_page;

        // Gunakan QueryBuilder untuk flexible filter
        let mut query_builder = QueryBuilder::<Postgres>::new(
            r#"
            SELECT 
                u.id, u.email, u.full_name, u.role, u.is_active, 
                u.email_verified, u.created_at, u.updated_at,
                u.updated_at as last_login,
                COALESCE(order_stats.order_count, 0) as order_count,
                COALESCE(order_stats.total_spent::text, '0') as total_spent,
                COUNT(*) OVER() as total_count
            FROM users u
            LEFT JOIN (
                SELECT 
                    user_id, 
                    COUNT(*) as order_count,
                    SUM(amount) as total_spent
                FROM orders 
                WHERE status = 'paid'
                GROUP BY user_id
            ) order_stats ON u.id = order_stats.user_id
            WHERE 1=1
            "#
        );

        if let Some(search_term) = search {
            let trimmed = search_term.trim();
            if !trimmed.is_empty() {
                query_builder.push(" AND (u.email ILIKE ");
                query_builder.push_bind(format!("%{}%", trimmed));
                query_builder.push(" OR u.full_name ILIKE ");
                query_builder.push_bind(format!("%{}%", trimmed));
                query_builder.push(")");
            }
        }

        if let Some(role) = role_filter {
            let trimmed = role.trim();
            if !trimmed.is_empty() {
                query_builder.push(" AND u.role = ");
                query_builder.push_bind(trimmed);
            }
        }

        query_builder.push(" ORDER BY u.created_at DESC");
        query_builder.push(" LIMIT ");
        query_builder.push_bind(per_page as i64);
        query_builder.push(" OFFSET ");
        query_builder.push_bind(offset as i64);

        let query = query_builder.build();
        let rows = query.fetch_all(pool).await?;

        let total_items = rows.get(0).map_or(0, |r| r.get::<i64, _>("total_count"));

        let mut users = Vec::with_capacity(rows.len());

        for row in rows {
            let user = AdminUserProfile {
                id: row.get("id"),
                email: row.get("email"),
                full_name: row.get("full_name"),
                role: row.get("role"),
                is_active: row.get("is_active"),
                email_verified: row.get("email_verified"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                last_login: row.get("last_login"),
                order_count: row.get("order_count"),
                total_spent: row.get("total_spent"),
                lifetime_value: None,
                last_purchase: None,
                failed_login_attempts: 0,
                last_failed_login: None,
                account_locked_until: None,
                risk_score: None,
                login_count: 0,
                device_count: 0,
            };
            users.push(user);
        }

        let pagination = AdminPaginationMeta::new(page, per_page, total_items);

        Ok((users, pagination))
    }

    /// Mendapatkan aktivitas user terbaru (untuk monitoring)
    pub async fn get_user_activity_feed(
        &self,
        pool: &PgPool,
        limit: u32,
        activity_type: Option<&str>, 
    ) -> Result<Vec<UserActivity>, DatabaseError> {
        let limit = std::cmp::min(limit, 100) as i64;

        let valid_types = ["registration", "login", "logout", "update", "delete"];
        if let Some(activity) = activity_type {
            if !valid_types.contains(&activity) {
                return Err(DatabaseError::InvalidQuery);
            }
        }

        // Query aktivitas registrasi user
        let query = if let Some(activity) = activity_type {
            sqlx::query(
                r#"
                SELECT 
                    u.id as user_id,
                    u.full_name as user_name, 
                    u.email as user_email,
                    $1 as activity_type,
                    CASE 
                        WHEN $1 = 'login' THEN 'User melakukan login'
                        WHEN $1 = 'logout' THEN 'User logout dari sistem'
                        WHEN $1 = 'update' THEN 'User mengubah profil'
                        WHEN $1 = 'delete' THEN 'User dihapus'
                        ELSE 'Aktivitas user'
                    END as description,
                    NULL::text as ip_address,
                    NULL::text as user_agent,
                    u.created_at as timestamp
                FROM users u
                WHERE u.created_at >= NOW() - INTERVAL '7 days'
                ORDER BY u.created_at DESC
                LIMIT $2
                "#
            )
            .bind(activity)
            .bind(limit)
        } else {
            // Default: aktivitas registrasi
            sqlx::query(
                r#"
                SELECT 
                    u.id as user_id,
                    u.full_name as user_name, 
                    u.email as user_email,
                    'registration' as activity_type,
                    'User mendaftar ke platform' as description,
                    NULL::text as ip_address,
                    NULL::text as user_agent,
                    u.created_at as timestamp
                FROM users u
                WHERE u.created_at >= NOW() - INTERVAL '7 days'
                ORDER BY u.created_at DESC
                LIMIT $1
                "#
            )
            .bind(limit)
        };
        let rows = query.fetch_all(pool).await?;

        // Mapping hasil query ke struct UserActivity
        let mut activities = Vec::new();
        for row in rows {

            // Determinasi severity berdasarkan activity_type
            let severity = match activity_type {
                Some("login") | Some("registration") => ActivitySeverity::Info,
                Some("logout") => ActivitySeverity::Warning,
                Some("update") | Some("delete") => ActivitySeverity::Critical,
                _ => ActivitySeverity::Info, 
            };

            let activity = UserActivity {
                id: Some(row.get("id")),
                user_id: row.get("user_id"),
                user_name: row.try_get("user_name").ok().unwrap_or_else(|| "Unknown".to_string()),
                user_email: row.try_get("user_email").ok().unwrap_or_else(|| "Unknown".to_string()),
                activity_type: row.get("activity_type"),
                description: row.get("description"),
                ip_address: row.get("ip_address"),
                user_agent: row.get("user_agent"),
                timestamp: row.get("timestamp"),
                resource_type: None,
                resource_id: None,
                session_id: None,
                metadata: None,
                severity,
                location: None,
            };
            activities.push(activity);
        }

        // Urutkan berdasarkan timestamp terbaru
        activities.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        activities.truncate(limit as usize);

        Ok(activities)
    }

    pub async fn get_security_activity_feed(
        &self,
        pool: &PgPool,
        limit: u32,
        severity_filter: Option<&str>,
        user_filter: Option<Uuid>,
    ) -> Result<Vec<UserActivity>, DatabaseError> {
        let limit = std::cmp::min(limit, 100) as i64;
        
        let mut query_str = r#"
            SELECT 
                se.id,
                se.user_id,
                u.full_name as user_name,
                u.email as user_email,
                se.event_type,
                se.event_data,
                se.success,
                se.created_at
            FROM security_events se
            LEFT JOIN users u ON u.id = se.user_id
            WHERE se.created_at >= NOW() - INTERVAL '30 days'
        "#.to_string();
        
        if user_filter.is_some() {
            query_str.push_str(" AND se.user_id = $2");
        }
        
        query_str.push_str(" ORDER BY se.created_at DESC LIMIT $1");
        
        let rows = if let Some(user_id) = user_filter {
            sqlx::query(&query_str)
                .bind(limit)
                .bind(user_id)
                .fetch_all(pool)
                .await?
        } else {
            sqlx::query(&query_str)
                .bind(limit)
                .fetch_all(pool)
                .await?
        };
        
        let mut activities = Vec::new();
        for row in rows {
            let event_type: String = row.get("event_type");
            let success: bool = row.get("success");
            let event_data: Option<serde_json::Value> = row.get("event_data");
            
            let severity = match event_type.as_str() {
                "USER_REGISTERED" | "LOGIN_SUCCESS" | "SESSION_CREATED" => ActivitySeverity::Info,
                "LOGIN_FAILED" | "PASSWORD_CHANGED" => ActivitySeverity::Warning,
                "ACCOUNT_LOCKED" | "USER_DELETED" => ActivitySeverity::Critical,
                "SUSPICIOUS_ACTIVITY" | "UNAUTHORIZED_ACCESS" => ActivitySeverity::Security,
                _ if !success => ActivitySeverity::Warning,
                _ => ActivitySeverity::Info,
            };
            
            let description = self.get_event_description(&event_type, success);
            
            let ip_address = event_data.as_ref()
                .and_then(|d| d["ip"].as_str())
                .map(|s| s.to_string());
            
            let user_agent = event_data.as_ref()
                .and_then(|d| d["user_agent"].as_str())
                .map(|s| s.to_string());
            
            let activity = UserActivity {
                id: Some(row.get("id")),
                user_id: row.get("user_id"),
                user_name: row.try_get("user_name").ok().unwrap_or_else(|| "Unknown".to_string()),
                user_email: row.try_get("user_email").ok().unwrap_or_else(|| "Unknown".to_string()),
                activity_type: event_type,
                description,
                ip_address,
                user_agent,
                timestamp: row.get("created_at"),
                resource_type: None,
                resource_id: None,
                session_id: None,
                metadata: event_data,
                severity,
                location: None,
            };
            activities.push(activity);
        }
        
        if let Some(severity_filter) = severity_filter {
            activities.retain(|a| match severity_filter {
                "info" => matches!(a.severity, ActivitySeverity::Info),
                "warning" => matches!(a.severity, ActivitySeverity::Warning),
                "critical" => matches!(a.severity, ActivitySeverity::Critical),
                "security" => matches!(a.severity, ActivitySeverity::Security),
                _ => true,
            });
        }
        
        Ok(activities)
    }

    // ========== HELPER METHODS ==========

    /// Cek rate limiting untuk login
    async fn check_login_rate_limit(
        &self,
        pool: &PgPool,
        identifier: &str,
    ) -> Result<bool, DatabaseError> {
        let now = Utc::now();
        let window_start = now - chrono::Duration::minutes(15); // Jendela 15 menit
        
        // Hitung percobaan gagal dalam jendela waktu
        let failed_attempts = sqlx::query!(
            r#"
            SELECT COUNT(*) as count 
            FROM security_events 
            WHERE event_type = 'LOGIN_FAILED' 
                AND (event_data->>'ip' = $1 OR event_data->>'email' = $1)
                AND created_at > $2
            "#,
            identifier,
            window_start
        )
        .fetch_one(pool)
        .await?;

        // Maksimal 5 percobaan gagal per 15 menit
        Ok(failed_attempts.count.unwrap_or(0) < 5)
    }

    /// Cek apakah akun terkunci
    async fn is_account_locked(
        &self,
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<bool, DatabaseError> {
        let lock_window = Utc::now() - chrono::Duration::hours(1); // Jendela 1 jam
        
        // Hitung percobaan gagal dalam jendela waktu
        let failed_attempts = sqlx::query!(
            r#"
            SELECT COUNT(*) as count 
            FROM security_events 
            WHERE user_id = $1 
                AND event_type = 'LOGIN_FAILED' 
                AND created_at > $2
            "#,
            user_id,
            lock_window
        )
        .fetch_one(pool)
        .await?;

        // Kunci akun jika >= 10 percobaan gagal
        Ok(failed_attempts.count.unwrap_or(0) >= 10)
    }

    /// Tambah counter percobaan login gagal
    async fn increment_failed_login_attempts(
        &self, 
        pool: &PgPool, 
        user_id: Uuid
    ) -> Result<(), DatabaseError> {
        self.log_security_event(
            pool,
            Some(user_id),
            "FAILED_LOGIN_ATTEMPT_INCREMENTED",
            serde_json::json!({"attempt": 1}),
            false,
        ).await?;
        
        Ok(())
    }

    /// Reset counter percobaan gagal setelah login berhasil
    async fn reset_failed_login_attempts(
        &self,
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<(), DatabaseError> {
        self.log_security_event(
            pool,
            Some(user_id),
            "LOGIN_ATTEMPTS_RESET",
            serde_json::json!({}),
            true,
        ).await?;
        
        Ok(())
    }

    /// Log event keamanan ke database
    async fn log_security_event(
        &self,
        executor: impl sqlx::Executor<'_, Database = Postgres>,
        user_id: Option<Uuid>,
        event_type: &str,
        event_data: serde_json::Value,
        success: bool,
    ) -> Result<(), DatabaseError> {
        sqlx::query!(
            r#"
            INSERT INTO security_events (user_id, event_type, event_data, success)
            VALUES ($1, $2, $3, $4)
            "#,
            user_id,
            event_type,
            event_data,
            success
        )
        .execute(executor)
        .await?;

        Ok(())
    }

    /// Hash token sesi dengan pepper
    fn hash_session_token(&self, token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes()); 
        hasher.update(&self.security_service.pepper); 
        format!("{:x}", hasher.finalize()) 
    }

    // Helper method untuk generate description
    fn get_event_description(&self, event_type: &str, success: bool) -> String {
        match event_type {
            "USER_REGISTERED" => "User baru terdaftar di sistem".to_string(),
            "LOGIN_SUCCESS" => "Login berhasil".to_string(),
            "LOGIN_FAILED" => "Percobaan login gagal".to_string(),
            "SESSION_CREATED" => "Session baru dibuat".to_string(),
            "LOGOUT" => "User logout dari sistem".to_string(),
            "PASSWORD_CHANGED" => "Password berhasil diubah".to_string(),
            "PROFILE_UPDATED" => "Profile user diperbarui".to_string(),
            "ACCOUNT_LOCKED" => "Akun dikunci karena percobaan login berlebihan".to_string(),
            "TOKEN_REFRESHED" => "Token berhasil di-refresh".to_string(),
            "TOKEN_EXPIRED" => "Token kedaluwarsa".to_string(),
            "SUSPICIOUS_ACTIVITY" => "Aktivitas mencurigakan terdeteksi".to_string(),
            "UNAUTHORIZED_ACCESS" => "Percobaan akses tidak sah".to_string(),
            "BRUTE_FORCE_DETECTED" => "Serangan brute force terdeteksi".to_string(),
            _ if !success => format!("{} - Gagal", event_type).to_string(),
            _ => event_type.to_string(),
        }
    }
}