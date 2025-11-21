// /pdf-bookstore/services/payment-service/src/utils/cache.rs

use redis::{aio::ConnectionManager, AsyncCommands, Client, RedisError};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;


/// Redis cache manager dengan async support
#[derive(Clone)]
pub struct CacheManager {
    conn_manager: Option<Arc<ConnectionManager>>,
    namespace: String,
    dummy_cache: Arc<RwLock<HashMap<String, (String, chrono::DateTime<chrono::Utc>)>>>,
    is_dummy: bool,
}


impl CacheManager {
    /// Create cache manager baru dengan async connection
    pub async fn new(redis_url: &str, namespace: &str) -> Result<Self, RedisError> {
        let client = Client::open(redis_url)?;
        let conn_manager = ConnectionManager::new(client).await?;
        
        tracing::info!("Redis cache manager berhasil terhubung");
        
        Ok(Self {
            conn_manager: Some(Arc::new(conn_manager)),
            namespace: namespace.to_string(),
            dummy_cache: Arc::new(RwLock::new(HashMap::new())),
            is_dummy: false,
        })
    }

    // Create dummy cache untuk fallback ketika Redis tidak tersedia
    pub fn new_dummy(namespace: &str) -> Self {
        tracing::warn!("⚠️ Menggunakan dummy in-memory cache (Redis tidak tersedia)");
        
        Self {
            conn_manager: None,
            namespace: namespace.to_string(),
            dummy_cache: Arc::new(RwLock::new(HashMap::new())),
            is_dummy: true,
        }
    }
    
    /// Generate cache key dengan namespace
    fn make_key(&self, key: &str) -> String {
        format!("{}:{}", self.namespace, key)
    }
    
    /// Set value di cache dengan TTL
    pub async fn set<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl_seconds: u64,
    ) -> Result<(), RedisError> {
        if self.is_dummy {
            // Gunakan dummy cache
            let serialized = serde_json::to_string(value)
                .map_err(|e| RedisError::from((redis::ErrorKind::TypeError,
                    "Serialization gagal", e.to_string())))?;
            
            let expires_at = chrono::Utc::now() + chrono::Duration::seconds(ttl_seconds as i64);
            let mut cache = self.dummy_cache.write().await;
            cache.insert(self.make_key(key), (serialized, expires_at));
            
            tracing::debug!("Dummy cache set: key={}, ttl={}s", key, ttl_seconds);
            return Ok(());
        }
        
        // Handle Option dengan pattern matching
        match &self.conn_manager {
            Some(conn_manager) => {
                let serialized = serde_json::to_string(value)
                    .map_err(|e| RedisError::from((
                        redis::ErrorKind::TypeError,
                        "Serialization failed",
                        e.to_string()
                    )))?;
                
                let mut conn = conn_manager.as_ref().clone();

                conn.set_ex::<_, _, ()>(
                    self.make_key(key), 
                    serialized, 
                    ttl_seconds
                ).await?;
                
                tracing::debug!("Cache set: key={}, ttl={}s", key, ttl_seconds);
                Ok(())
            }
            None => Ok(()) 
        }
    }
    
    /// Get value dari cache
    pub async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        key: &str,
    ) -> Result<Option<T>, RedisError> {
        if self.is_dummy {
            // Gunakan dummy cache
            let mut cache = self.dummy_cache.write().await;
            let now = chrono::Utc::now();
            
            // Cleanup expired entries
            cache.retain(|_, (_, expires_at)| *expires_at > now);
            
            if let Some((data, expires_at)) = cache.get(&self.make_key(key)) {
                if *expires_at > now {
                    let deserialized = serde_json::from_str(data)
                        .map_err(|e| RedisError::from((redis::ErrorKind::TypeError,
                            "Deserialization gagal", e.to_string())))?;
                    tracing::debug!("Dummy cache hit: key={}", key);
                    return Ok(Some(deserialized));
                }
            }
            
            tracing::debug!("Dummy cache miss: key={}", key);
            return Ok(None);
        }
        
        // Handle Option dengan pattern matching
        match &self.conn_manager {
            Some(conn_manager) => {
                // Clone connection dari Arc
                let mut conn = conn_manager.as_ref().clone();
                let result: Option<String> = conn.get(self.make_key(key)).await?;
                
                match result {
                    Some(data) => {
                        let deserialized = serde_json::from_str(&data)
                            .map_err(|e| RedisError::from((redis::ErrorKind::TypeError,
                                "Deserialization gagal", e.to_string())))?;
                        tracing::debug!("Redis cache hit: key={}", key);
                        Ok(Some(deserialized))
                    }
                    None => {
                        tracing::debug!("Redis cache miss: key={}", key);
                        Ok(None)
                    }
                }
            }
            None => {
                // Fallback: cache miss jika conn_manager None
                Ok(None)
            }
        }
    }
    
    /// Delete key dari cache
    pub async fn delete(&self, key: &str) -> Result<(), RedisError> {
        if self.is_dummy {
            // Gunakan dummy cache
            let mut cache = self.dummy_cache.write().await;
            cache.remove(&self.make_key(key));
            tracing::debug!("Dummy cache delete: key={}", key);
            return Ok(());
        }
        
        // Handle Option dengan pattern matching
        match &self.conn_manager {
            Some(conn_manager) => {
                // Clone connection dari Arc
                let mut conn = conn_manager.as_ref().clone();
                conn.del::<_, ()>(self.make_key(key)).await?;
                
                tracing::debug!("Redis cache delete: key={}", key);
                Ok(())
            }
            None => {
                // Silently succeed jika conn_manager None
                Ok(())
            }
        }
    }
    
    /// Invalidate pattern - hapus semua key yang cocok dengan pola
    pub async fn invalidate_pattern(&self, pattern: &str) -> Result<u64, RedisError> {
        if self.is_dummy {
            // Gunakan dummy cache
            let mut cache = self.dummy_cache.write().await;
            let pattern_key = format!("{}:{}*", self.namespace, pattern);
            let mut deleted = 0u64;
            
            cache.retain(|key, _| {
                if key.starts_with(&pattern_key.trim_end_matches('*')) {
                    deleted += 1;
                    false
                } else {
                    true
                }
            });
            
            tracing::debug!("Dummy cache invalidate pattern: pattern={}, deleted={}", pattern, deleted);
            return Ok(deleted);
        }
        
        // Handle Option dengan pattern matching
        match &self.conn_manager {
            Some(conn_manager) => {
                // Clone connection dari Arc
                let mut conn = conn_manager.as_ref().clone();
                let pattern_key = format!("{}:{}*", self.namespace, pattern);
                
                // Gunakan SCAN instead of KEYS untuk production (lebih aman)
                let keys: Vec<String> = redis::cmd("KEYS")
                    .arg(&pattern_key)
                    .query_async(&mut conn)
                    .await?;
                
                if keys.is_empty() {
                    return Ok(0);
                }
                
                let deleted: u64 = conn.del(keys).await?;
                tracing::debug!("Redis cache invalidate pattern: pattern={}, deleted={}", pattern, deleted);
                Ok(deleted)
            }
            None => {
                Ok(0)
            }
        }
    }

    /// Check apakah cache menggunakan Redis atau dummy
    pub fn is_using_redis(&self) -> bool {
        !self.is_dummy
    }
    
    /// Get cache stats untuk monitoring
    pub async fn get_stats(&self) -> serde_json::Value {
        if self.is_dummy {
            let cache = self.dummy_cache.read().await;
            let now = chrono::Utc::now();
            let active_entries = cache.values()
                .filter(|(_, expires_at)| *expires_at > now)
                .count();
            
            serde_json::json!({
                "type": "dummy_in_memory",
                "total_entries": cache.len(),
                "active_entries": active_entries,
                "expired_entries": cache.len() - active_entries,
                "namespace": self.namespace,
            })
        } else {
            serde_json::json!({
                "type": "redis",
                "connected": self.conn_manager.is_some(),
                "namespace": self.namespace,
            })
        }
    }
}