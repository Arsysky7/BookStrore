// /pdf-bookstore/services/auth-service/src/services/client.rs


use reqwest::Client;
use std::time::Duration;
use uuid::Uuid;
use serde_json::Value;
use chrono::{DateTime, Utc};
use std::sync::Arc;

use crate:: {
    models::{UserOrderStats, Purchase, BookDetails, DownloadedBook},
    services::CircuitBreakerManager,
    utils::AppError,
};

/// Client untuk komunikasi antar service
#[derive(Clone)]
pub struct ServiceClient {
    client: Client,
    book_service_url: String,
    payment_service_url: String,
    internal_key: String,
    circuit_manager: Arc<CircuitBreakerManager>, 
}

impl ServiceClient {
    /// Membuat service client baru
    pub fn new(
        circuit_manager: Arc<CircuitBreakerManager>,
    ) -> Self { 
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to create HTTP client");
        
        let book_service_url = std::env::var("BOOK_SERVICE_URL")
            .unwrap_or_else(|_| "http://localhost:3002".to_string());
        
        let payment_service_url = std::env::var("PAYMENT_SERVICE_URL")
            .unwrap_or_else(|_| "http://localhost:3003".to_string());
        
        let internal_key = std::env::var("INTERNAL_SERVICE_KEY")
            .unwrap_or_else(|_| "internal-service-key-secret".to_string());
        
        Self {
            client,
            book_service_url,
            payment_service_url,
            internal_key,
            circuit_manager, 
        }
    }
    
    // ========== PAYMENT SERVICE CALLS ==========
    
    /// Mendapatkan statistik order user dari payment service
    pub async fn get_user_order_stats(
        &self,
        user_id: Uuid
    ) -> Result<UserOrderStats, Box<dyn std::error::Error>> {
        let circuit_breaker = self.circuit_manager.get_or_create("payment-service").await;
        let url = format!("{}/api/internal/users/{}/stats", self.payment_service_url, user_id);
        let client = self.client.clone();
        let key = self.internal_key.clone();
        
        circuit_breaker.call(async move {
            let response = client
                .get(&url)
                .header("X-Service-Key", &key)
                .send()
                .await
                .map_err(|e| AppError::ExternalService(format!("Payment service error: {}", e)))?;
            
            if response.status().is_success() {
                let data: Value = response.json().await
                    .map_err(|e| AppError::ExternalService(format!("Parse error: {}", e)))?;
                Ok(UserOrderStats {
                    total_orders: data["total_orders"].as_i64().unwrap_or(0),
                    total_spent: data["total_spent"].as_f64().unwrap_or(0.0),
                    last_purchase: data["last_purchase"]
                        .as_str()
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                })
            } else {
                Ok(UserOrderStats::default())
            }
        }).await.map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other, 
                format!("{:?}", e) // Pake Debug format karena AppError ga implement Display
            )) as Box<dyn std::error::Error>
        })
    }

    
    /// Mengecek apakah user memiliki buku tertentu
    pub async fn check_book_ownership(
        &self,
        user_id: Uuid,
        book_id: Uuid
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let response = self.client
            .get(format!("{}/api/internal/ownership/check", self.payment_service_url))
            .query(&[
                ("user_id", user_id.to_string()),
                ("book_id", book_id.to_string()),
            ])
            .header("X-Service-Key", &self.internal_key)
            .send()
            .await?;
        
        if response.status().is_success() {
            let data: Value = response.json().await?;
            Ok(data["owns_book"].as_bool().unwrap_or(false))
        } else {
            Ok(false)
        }
    }
    
    /// Mendapatkan riwayat pembelian user
    pub async fn get_user_purchases(
        &self,
        user_id: Uuid,
        limit: Option<u32>
    ) -> Result<Vec<Purchase>, Box<dyn std::error::Error>> {
        let mut request = self.client
            .get(format!("{}/api/internal/users/{}/purchases", self.payment_service_url, user_id))
            .header("X-Service-Key", &self.internal_key);
        
        if let Some(limit) = limit {
            request = request.query(&[("limit", limit.to_string())]);
        }
        
        let response = request.send().await?;
        
        if response.status().is_success() {
            let data: Value = response.json().await?;
            let purchases = data["purchases"].as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|p| {
                    Some(Purchase {
                        order_id: p["order_id"].as_str()?.parse().ok()?,
                        book_id: p["book_id"].as_str()?.parse().ok()?,
                        amount: p["amount"].as_f64()?,
                        status: p["status"].as_str()?.to_string(),
                        purchased_at: p["purchased_at"].as_str()
                            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                            .map(|dt| dt.with_timezone(&Utc)),
                    })
                })
                .collect();
            Ok(purchases)
        } else {
            Ok(vec![])
        }
    }
    
    // ========== BOOK SERVICE CALLS ==========
    
    /// Mendapatkan detail buku dari book service
    pub async fn get_book_details(
        &self,
        book_id: Uuid
    ) -> Result<BookDetails, Box<dyn std::error::Error>> {
        let response = self.client
            .get(format!("{}/api/books/{}", self.book_service_url, book_id))
            .send()
            .await?;
        
        if response.status().is_success() {
            let data: Value = response.json().await?;
            Ok(BookDetails {
                id: book_id,
                title: data["data"]["book"]["title"].as_str().unwrap_or("").to_string(),
                author: data["data"]["book"]["author"].as_str().unwrap_or("").to_string(),
                price: data["data"]["book"]["price"].as_f64().unwrap_or(0.0),
                is_active: data["data"]["book"]["is_active"].as_bool().unwrap_or(true),
            })
        } else {
            Err("Book not found".into())
        }
    }
    
    /// Mendapatkan daftar buku yang sudah didownload user
    pub async fn get_user_downloaded_books(
        &self,
        user_id: Uuid
    ) -> Result<Vec<DownloadedBook>, Box<dyn std::error::Error>> {
        let response = self.client
            .get(format!("{}/api/internal/users/{}/downloads", self.book_service_url, user_id))
            .header("X-Service-Key", &self.internal_key)
            .send()
            .await?;
        
        if response.status().is_success() {
            let data: Value = response.json().await?;
            let books = data["downloads"].as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|b| {
                    Some(DownloadedBook {
                        book_id: b["book_id"].as_str()?.parse().ok()?,
                        title: b["title"].as_str()?.to_string(),
                        download_count: b["download_count"].as_i64()? as i32,
                        last_downloaded: b["last_downloaded"].as_str()
                            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                            .map(|dt| dt.with_timezone(&Utc)),
                    })
                })
                .collect();
            Ok(books)
        } else {
            Ok(vec![])
        }
    }
    
    /// Mengecek ketersediaan buku
    pub async fn check_book_availability(
        &self,
        book_id: Uuid
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let response = self.client
            .get(format!("{}/api/books/{}/availability", self.book_service_url, book_id))
            .send()
            .await?;
        
        if response.status().is_success() {
            let data: Value = response.json().await?;
            Ok(data["available"].as_bool().unwrap_or(false))
        } else {
            Ok(false)
        }
    }
}