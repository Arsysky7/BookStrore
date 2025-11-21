// /pdf-bookstore/services/book-service/src/upload.rs
// ===== BOOK SERVICE UPLOAD SECURITY - GABUNGAN LENGKAP =====

use axum::http::StatusCode;
use axum::extract::{Multipart, multipart::Field};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;
use std::env;
use std::path::{Path, PathBuf};
use bigdecimal::BigDecimal;
use thiserror::Error;
use crate::models::ErrorResponse;
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use rand::Rng;

// ===== ERROR HANDLING =====
#[derive(Error, Debug)]
pub enum UploadError {
    #[error("Directory tidak ditemukan")]
    DirectoryNotFound,
    #[error("Gagal menyimpan file: {0}")]
    SaveError(String),
    #[error("Validasi keamanan gagal: {0}")]
    SecurityValidationFailed(String),
    #[error("Batas upload concurrent terlampaui")]
    ConcurrentUploadLimitExceeded,
}

// ===== FILE TYPE VALIDATION =====

// Struct validator untuk validasi tipe file dengan magic bytes dan size limit
#[derive(Debug, Clone)]
struct FileTypeValidator {
    mime_type: &'static str,
    extensions: &'static [&'static str],
    magic_bytes: &'static [&'static [u8]],
    max_size_mb: f64,
}

// Daftar tipe file yang diizinkan dengan validasi magic bytes keamanan tinggi
const ALLOWED_FILE_TYPES: &[FileTypeValidator] = &[
    FileTypeValidator {
        mime_type: "application/pdf",
        extensions: &["pdf"],
        magic_bytes: &[b"%PDF-"],
        max_size_mb: 50.0,
    },
    FileTypeValidator {
        mime_type: "image/jpeg",
        extensions: &["jpg", "jpeg"],
        magic_bytes: &[&[0xFF, 0xD8, 0xFF]],
        max_size_mb: 10.0,
    },
    FileTypeValidator {
        mime_type: "image/png",
        extensions: &["png"],
        magic_bytes: &[&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]],
        max_size_mb: 10.0,
    },
    FileTypeValidator {
        mime_type: "image/webp",
        extensions: &["webp"],
        magic_bytes: &[b"RIFF", b"WEBP"],
        max_size_mb: 10.0,
    },
];

// ===== CONCURRENT UPLOAD TRACKING =====

// Tracker untuk membatasi upload concurrent per user dengan auto cleanup
#[derive(Clone)]
pub struct UploadTracker {
    active_uploads: Arc<RwLock<HashMap<String, chrono::DateTime<chrono::Utc>>>>,
    max_concurrent: usize,
}

impl UploadTracker {
    pub fn new(max_concurrent: usize) -> Self {
        let tracker = Self {
            active_uploads: Arc::new(RwLock::new(HashMap::new())),
            max_concurrent,
        };
        
        // Spawn periodic cleanup task
        let uploads_clone = tracker.active_uploads.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
            
            loop {
                interval.tick().await;
                
                let mut uploads = uploads_clone.write().await;
                let cutoff = chrono::Utc::now() - chrono::Duration::minutes(5);
                let before = uploads.len();
                
                uploads.retain(|_, timestamp| *timestamp > cutoff);
                
                let after = uploads.len();
                if before != after {
                    tracing::info!("Upload tracker cleanup: removed {} stale entries", before - after);
                }
            }
        });
        
        tracker
    }

    // Ambil slot upload dengan cleanup otomatis uploads expired (>10 menit)
    pub async fn acquire_slot(&self, user_id: &str) -> Result<(), UploadError> {
        let mut uploads = self.active_uploads.write().await;
        
        let cutoff = chrono::Utc::now() - chrono::Duration::minutes(10);
        uploads.retain(|_, timestamp| *timestamp > cutoff);

        if uploads.len() >= self.max_concurrent {
            return Err(UploadError::ConcurrentUploadLimitExceeded);
        }

        uploads.insert(user_id.to_string(), chrono::Utc::now());
        Ok(())
    }

    // Release slot upload setelah selesai
    pub async fn release_slot(&self, user_id: &str) {
        let mut uploads = self.active_uploads.write().await;
        uploads.remove(user_id);
    }
}

// ===== MAIN FILE UPLOADER =====

// Main uploader dengan comprehensive security validation dan virus scanning
pub struct FileUploader {
    upload_dir: PathBuf,
    temp_dir: PathBuf,
    upload_tracker: UploadTracker,
}

impl FileUploader {
    // Initialize uploader dengan secure directories dan concurrent limits
    pub fn new() -> Result<Self, UploadError> {
        let upload_dir = PathBuf::from(env::var("UPLOAD_DIR").unwrap_or_else(|_| "./storage".to_string()));
        let temp_dir = upload_dir.join("temp");

        Self::create_secure_directory(&upload_dir)?;
        Self::create_secure_directory(&temp_dir)?;
        Self::create_secure_directory(&upload_dir.join("books"))?;
        Self::create_secure_directory(&upload_dir.join("covers"))?;

        let max_concurrent = env::var("MAX_CONCURRENT_UPLOADS")
            .unwrap_or_else(|_| "5".to_string())
            .parse()
            .unwrap_or(5);

        Ok(Self {
            upload_dir,
            temp_dir,
            upload_tracker: UploadTracker::new(max_concurrent),
        })
    }

    // ===== PDF UPLOAD METHODS =====

    // Upload PDF dengan concurrent control dan security validation lengkap
    pub async fn upload_pdf(
        &self,
        multipart: Multipart,
        user_id: &str,
    ) -> Result<(String, BigDecimal), (StatusCode, axum::Json<ErrorResponse>)> {
        self.upload_tracker.acquire_slot(user_id).await
            .map_err(|e| (
                StatusCode::TOO_MANY_REQUESTS,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Batas upload terlampaui: {}", e),
                    error_code: Some("CONCURRENT_UPLOAD_LIMIT".to_string()),
                })
            ))?;

        let result = self.process_pdf_upload(multipart).await;
        self.upload_tracker.release_slot(user_id).await;
        result
    }

    // Process PDF upload dengan validasi field dan security checks
    async fn process_pdf_upload(
        &self,
        mut multipart: Multipart,
    ) -> Result<(String, BigDecimal), (StatusCode, axum::Json<ErrorResponse>)> {
        while let Some(field) = multipart.next_field().await
            .map_err(|e| (
                StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Error parsing multipart: {}", e),
                    error_code: Some("MULTIPART_ERROR".to_string()),
                })
            ))? {
            
            let field_name = field.name().unwrap_or("");
            
            if field_name == "pdf_file" {
                return self.process_file_field(field, "pdf").await;
            }
        }

        Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ErrorResponse {
                success: false,
                message: "File PDF tidak ditemukan dalam upload".to_string(),
                error_code: Some("NO_FILE_FOUND".to_string()),
            })
        ))
    }

    // Upload PDF dari single field untuk compatibility dengan different endpoints
    pub async fn upload_pdf_from_field(
        field: Field<'_>
    ) -> Result<(String, BigDecimal), (StatusCode, axum::Json<ErrorResponse>)> {
        let upload_dir = env::var("UPLOAD_DIR").unwrap_or_else(|_| "./storage".to_string());
        let books_dir = format!("{}/books", upload_dir);

        if let Err(_) = fs::create_dir_all(&books_dir).await {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    success: false,
                    message: "Gagal membuat direktori upload".to_string(),
                    error_code: Some("DIRECTORY_ERROR".to_string()),
                })
            ));
        }

        let filename = field.file_name().unwrap_or("unknown.pdf").to_string();
        
        if !filename.to_lowercase().ends_with(".pdf") {
            return Err((
                StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse {
                    success: false,
                    message: "Hanya file PDF yang diizinkan".to_string(),
                    error_code: Some("INVALID_FILE_TYPE".to_string()),
                })
            ));
        }

        let file_extension = Path::new(&filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("pdf");

        let unique_filename = format!("{}_{}.{}", 
            Uuid::new_v4(),
            chrono::Utc::now().timestamp_millis(),
            file_extension
        );

        let file_path = format!("{}/{}", books_dir, unique_filename);

        let data = field.bytes().await
            .map_err(|e| (
                StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal membaca data file: {}", e),
                    error_code: Some("FILE_READ_ERROR".to_string()),
                })
            ))?;

        let file_size_bytes = data.len() as u64;
        let max_size_mb: f64 = env::var("MAX_FILE_SIZE_MB")
            .unwrap_or_else(|_| "50".to_string())
            .parse()
            .unwrap_or(50.0);

        let max_size_bytes = (max_size_mb * 1024.0 * 1024.0) as u64;

        if file_size_bytes > max_size_bytes {
            let file_size_mb = file_size_bytes as f64 / (1024.0 * 1024.0);
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("File terlalu besar: {:.2}MB (maks: {}MB)", 
                        file_size_mb, max_size_mb),
                    error_code: Some("FILE_TOO_LARGE".to_string()),
                })
            ));
        }

        if !Self::is_valid_pdf(&data) {
            return Err((
                StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse {
                    success: false,
                    message: "Format file PDF tidak valid".to_string(),
                    error_code: Some("INVALID_PDF".to_string()),
                })
            ));
        }

        let mut file = fs::File::create(&file_path).await
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal membuat file: {}", e),
                    error_code: Some("FILE_CREATE_ERROR".to_string()),
                })
            ))?;

        file.write_all(&data).await
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal menulis file: {}", e),
                    error_code: Some("FILE_WRITE_ERROR".to_string()),
                })
            ))?;

        file.flush().await
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal flush file: {}", e),
                    error_code: Some("FILE_FLUSH_ERROR".to_string()),
                })
            ))?;

        let file_size_mb = BigDecimal::from(file_size_bytes as i64) / BigDecimal::from(1024 * 1024);
        let relative_path = format!("/storage/books/{}", unique_filename);

        Ok((relative_path, file_size_mb))
    }

    // ===== IMAGE UPLOAD METHODS =====

    // Upload cover image dengan size limits dan format validation
    pub async fn upload_cover_image(
        &self,
        multipart: Multipart,
        user_id: &str,
    ) -> Result<String, (StatusCode, axum::Json<ErrorResponse>)> {
        self.upload_tracker.acquire_slot(user_id).await
            .map_err(|e| (
                StatusCode::TOO_MANY_REQUESTS,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Batas upload terlampaui: {}", e),
                    error_code: Some("CONCURRENT_UPLOAD_LIMIT".to_string()),
                })
            ))?;

        let result = self.process_cover_upload(multipart).await;
        self.upload_tracker.release_slot(user_id).await;
        result
    }

    // Process cover upload dengan validation
    async fn process_cover_upload(
        &self,
        mut multipart: Multipart,
    ) -> Result<String, (StatusCode, axum::Json<ErrorResponse>)> {
        while let Some(field) = multipart.next_field().await
            .map_err(|e| (
                StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Error parsing multipart: {}", e),
                    error_code: Some("MULTIPART_ERROR".to_string()),
                })
            ))? {
            
            let field_name = field.name().unwrap_or("");
            
            if field_name == "cover_image" {
                let (file_path, _) = self.process_file_field(field, "image").await?;
                return Ok(file_path);
            }
        }

        Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ErrorResponse {
                success: false,
                message: "Cover image tidak ditemukan dalam upload".to_string(),
                error_code: Some("NO_IMAGE_FOUND".to_string()),
            })
        ))
    }

    // Upload cover dari single field untuk compatibility
    pub async fn upload_cover_from_field(
        field: Field<'_>
    ) -> Result<String, (StatusCode, axum::Json<ErrorResponse>)> {
        let upload_dir = env::var("UPLOAD_DIR").unwrap_or_else(|_| "./storage".to_string());
        let covers_dir = format!("{}/covers", upload_dir);

        if let Err(_) = fs::create_dir_all(&covers_dir).await {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    success: false,
                    message: "Gagal membuat direktori upload".to_string(),
                    error_code: Some("DIRECTORY_ERROR".to_string()),
                })
            ));
        }

        let filename = field.file_name().unwrap_or("unknown.jpg").to_string();
        let allowed_extensions = vec!["jpg", "jpeg", "png", "webp"];
        
        let extension = Path::new(&filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase())
            .unwrap_or_default();

        if !allowed_extensions.contains(&extension.as_str()) {
            return Err((
                StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Tipe image tidak valid. Diizinkan: {}", 
                        allowed_extensions.join(", ")),
                    error_code: Some("INVALID_IMAGE_TYPE".to_string()),
                })
            ));
        }

        let unique_filename = format!("{}_{}.{}", 
            Uuid::new_v4(),
            chrono::Utc::now().timestamp_millis(),
            extension
        );

        let file_path = format!("{}/{}", covers_dir, unique_filename);

        let data = field.bytes().await
            .map_err(|e| (
                StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal membaca data image: {}", e),
                    error_code: Some("FILE_READ_ERROR".to_string()),
                })
            ))?;

        let file_size_bytes = data.len() as u64;
        let max_size_mb: f64 = 10.0; 
        let max_size_bytes = (max_size_mb * 1024.0 * 1024.0) as u64;

        if file_size_bytes > max_size_bytes {
            let file_size_mb = file_size_bytes as f64 / (1024.0 * 1024.0);
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Image terlalu besar: {:.2}MB (maks: {}MB)", 
                        file_size_mb, max_size_mb),
                    error_code: Some("IMAGE_TOO_LARGE".to_string()),
                })
            ));
        }

        if !Self::is_valid_image(&data, &extension) {
            return Err((
                StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse {
                    success: false,
                    message: "Format file image tidak valid".to_string(),
                    error_code: Some("INVALID_IMAGE_FORMAT".to_string()),
                })
            ));
        }

        let mut file = fs::File::create(&file_path).await
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal membuat file image: {}", e),
                    error_code: Some("FILE_CREATE_ERROR".to_string()),
                })
            ))?;

        file.write_all(&data).await
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal menulis file image: {}", e),
                    error_code: Some("FILE_WRITE_ERROR".to_string()),
                })
            ))?;

        file.flush().await
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal flush file image: {}", e),
                    error_code: Some("FILE_FLUSH_ERROR".to_string()),
                })
            ))?;

        let relative_path = format!("/storage/covers/{}", unique_filename);
        Ok(relative_path)
    }

    // ===== CORE FILE PROCESSING =====

    // Process file field dengan comprehensive security validation dan atomic writing
    async fn process_file_field(
        &self,
        field: Field<'_>,
        file_category: &str,
    ) -> Result<(String, BigDecimal), (StatusCode, axum::Json<ErrorResponse>)> {
        let filename = field.file_name()
            .unwrap_or("unknown")
            .to_string();

        let _content_type = field.content_type()
            .unwrap_or("application/octet-stream")
            .to_string();

        if !self.is_safe_filename(&filename) {
            return Err((
                StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse {
                    success: false,
                    message: "Nama file tidak aman terdeteksi".to_string(),
                    error_code: Some("INVALID_FILENAME".to_string()),
                })
            ));
        }

        let data = self.read_field_with_limit(field).await?;
        let file_type = self.validate_file_comprehensive(&data, &filename, &_content_type, file_category)?;
        let secure_filename = self.generate_secure_filename(&filename, file_type.extensions[0]);
        
        let temp_file_path = self.temp_dir.join(&secure_filename);
        let final_file_path = if file_category == "pdf" {
            self.upload_dir.join("books").join(&secure_filename)
        } else {
            self.upload_dir.join("covers").join(&secure_filename)
        };

        self.write_file_atomic(&temp_file_path, &data).await?;

        if env::var("ENABLE_VIRUS_SCANNING").unwrap_or_else(|_| "false".to_string()) == "true" {
            self.scan_file_for_viruses(&temp_file_path).await?;
        }

        fs::rename(&temp_file_path, &final_file_path).await
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal memindahkan file: {}", e),
                    error_code: Some("FILE_MOVE_ERROR".to_string()),
                })
            ))?;

        self.set_secure_permissions(&final_file_path).await?;
        let file_size_mb = BigDecimal::from(data.len() as i64) / BigDecimal::from(1024 * 1024);
        self.store_file_integrity(&final_file_path, &data).await?;

        let relative_path = if file_category == "pdf" {
            format!("/storage/books/{}", secure_filename)
        } else {
            format!("/storage/covers/{}", secure_filename)
        };

        Ok((relative_path, file_size_mb))
    }

    // ===== VALIDATION METHODS =====

    // Validasi nama file aman untuk mencegah path traversal dan reserved names
    fn is_safe_filename(&self, filename: &str) -> bool {
        if filename.is_empty() || filename.len() > 255 {
            return false;
        }

        let dangerous_patterns = [
            "..", "/", "\\", "<", ">", ":", "\"", "|", "?", "*",
            "\0", "\x01", "\x02", "\x03", "\x04", "\x05", "\x06", "\x07",
            "\x08", "\x09", "\x0A", "\x0B", "\x0C", "\x0D", "\x0E", "\x0F"
        ];

        for pattern in &dangerous_patterns {
            if filename.contains(pattern) {
                return false;
            }
        }

        let reserved_names = [
            "CON", "PRN", "AUX", "NUL",
            "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
            "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"
        ];

        let name_without_ext = filename.split('.').next().unwrap_or("").to_uppercase();
        if reserved_names.contains(&name_without_ext.as_str()) {
            return false;
        }

        true
    }

    // Baca field dengan strict size limits untuk prevent DoS attacks
    async fn read_field_with_limit(
        &self,
        field: Field<'_>,
    ) -> Result<Vec<u8>, (StatusCode, axum::Json<ErrorResponse>)> {
        const MAX_CHUNK_SIZE: usize = 1024 * 1024;
        const ABSOLUTE_MAX_SIZE: usize = 100 * 1024 * 1024;

        let mut data = Vec::new();
        let mut stream = field;

        while let Some(chunk) = stream.chunk().await
            .map_err(|e| (
                StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal membaca chunk file: {}", e),
                    error_code: Some("FILE_READ_ERROR".to_string()),
                })
            ))? {
            
            if chunk.len() > MAX_CHUNK_SIZE {
                return Err((
                    StatusCode::BAD_REQUEST,
                    axum::Json(ErrorResponse {
                        success: false,
                        message: "Ukuran chunk terlalu besar".to_string(),
                        error_code: Some("CHUNK_TOO_LARGE".to_string()),
                    })
                ));
            }

            if data.len() + chunk.len() > ABSOLUTE_MAX_SIZE {
                return Err((
                    StatusCode::PAYLOAD_TOO_LARGE,
                    axum::Json(ErrorResponse {
                        success: false,
                        message: "File terlalu besar".to_string(),
                        error_code: Some("FILE_TOO_LARGE".to_string()),
                    })
                ));
            }

            data.extend_from_slice(&chunk);
        }

        if data.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse {
                    success: false,
                    message: "File kosong tidak diizinkan".to_string(),
                    error_code: Some("EMPTY_FILE".to_string()),
                })
            ));
        }

        Ok(data)
    }

    // Validasi file comprehensive dengan extension, size, magic bytes, dan security scans
    fn validate_file_comprehensive(
        &self,
        data: &[u8],
        filename: &str,
        _content_type: &str,
        file_category: &str,
    ) -> Result<&FileTypeValidator, (StatusCode, axum::Json<ErrorResponse>)> {
        let extension = Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase())
            .unwrap_or_default();

        let mut matching_validators = Vec::new();
        
        for validator in ALLOWED_FILE_TYPES {
            let category_match = match file_category {
                "pdf" => validator.mime_type == "application/pdf",
                "image" => validator.mime_type.starts_with("image/"),
                _ => false,
            };

            if !category_match {
                continue;
            }

            if validator.extensions.contains(&extension.as_str()) {
                matching_validators.push(validator);
            }
        }

        if matching_validators.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Tipe file tidak diizinkan: {}", extension),
                    error_code: Some("INVALID_FILE_TYPE".to_string()),
                })
            ));
        }

        let file_size_mb = data.len() as f64 / (1024.0 * 1024.0);
        let applicable_validator = matching_validators[0];
        
        if file_size_mb > applicable_validator.max_size_mb {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("File terlalu besar: {:.2}MB (maks: {}MB)", 
                        file_size_mb, applicable_validator.max_size_mb),
                    error_code: Some("FILE_TOO_LARGE".to_string()),
                })
            ));
        }

        if !self.validate_magic_bytes(data, applicable_validator) {
            return Err((
                StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse {
                    success: false,
                    message: "Konten file tidak sesuai dengan tipe file".to_string(),
                    error_code: Some("INVALID_FILE_CONTENT".to_string()),
                })
            ));
        }

        self.perform_security_scans(data, applicable_validator)?;

        Ok(applicable_validator)
    }

    // Validasi magic bytes untuk memastikan file type authenticity
    fn validate_magic_bytes(&self, data: &[u8], validator: &FileTypeValidator) -> bool {
        if data.len() < 8 {
            return false;
        }

        for magic_pattern in validator.magic_bytes {
            if data.starts_with(magic_pattern) {
                return true;
            }
        }

        if validator.mime_type == "image/webp" {
            return data.len() >= 12 
                && data.starts_with(b"RIFF") 
                && &data[8..12] == b"WEBP";
        }

        false
    }

    // Security scans untuk detect malicious patterns dan excessive metadata
    fn perform_security_scans(
        &self,
        data: &[u8],
        _validator: &FileTypeValidator,
    ) -> Result<(), (StatusCode, axum::Json<ErrorResponse>)> {
        if let Ok(text_content) = std::str::from_utf8(data) {
            let dangerous_patterns = [
                "<script", "javascript:", "data:text/html", 
                "/JS", "/JavaScript", "eval(", "exec(",
                "\\u0000", "\\x00", "%00", "../", "..\\",
            ];

            for pattern in &dangerous_patterns {
                if text_content.to_lowercase().contains(&pattern.to_lowercase()) {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        axum::Json(ErrorResponse {
                            success: false,
                            message: "Konten berpotensi berbahaya terdeteksi".to_string(),
                            error_code: Some("MALICIOUS_CONTENT".to_string()),
                        })
                    ));
                }
            }
        }

        if data.len() > 0 && self.has_excessive_metadata(data) {
            return Err((
                StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse {
                    success: false,
                    message: "File mengandung metadata berlebihan".to_string(),
                    error_code: Some("EXCESSIVE_METADATA".to_string()),
                })
            ));
        }

        Ok(())
    }

    // Check excessive metadata dengan heuristic null bytes detection
    fn has_excessive_metadata(&self, data: &[u8]) -> bool {
        let check_size = std::cmp::min(1024, data.len());
        let null_count = data[..check_size].iter().filter(|&&b| b == 0).count();
        
        null_count > check_size / 2
    }

    // ===== FILE OPERATIONS =====

    // Generate cryptographically secure filename dengan timestamp dan UUID
    fn generate_secure_filename(&self, original: &str, extension: &str) -> String {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let uuid = Uuid::new_v4();
        
        let mut rng = rand::rng();
        let random_suffix: String = (0..8)
            .map(|_| {
                let chars = b"abcdefghijklmnopqrstuvwxyz0123456789";
                let idx = rng.random_range(0..chars.len());
                chars[idx] as char
            })
            .collect();

        format!("{}_{}_{}_{}.{}", 
            timestamp, 
            uuid.simple(), 
            random_suffix,
            self.sanitize_original_name(original),
            extension
        )
    }


    // Sanitize original filename untuk keamanan dan compatibility
    fn sanitize_original_name(&self, name: &str) -> String {
        let name_without_ext = Path::new(name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");

        name_without_ext
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .take(20)
            .collect::<String>()
            .trim_matches('-')
            .trim_matches('_')
            .to_string()
    }

    // Atomic file writing dengan temporary file untuk prevent corruption
    async fn write_file_atomic(
        &self,
        file_path: &Path,
        data: &[u8],
    ) -> Result<(), (StatusCode, axum::Json<ErrorResponse>)> {
        let temp_path = file_path.with_extension("tmp");

        let mut file = fs::File::create(&temp_path).await
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal membuat file sementara: {}", e),
                    error_code: Some("FILE_CREATE_ERROR".to_string()),
                })
            ))?;

        file.write_all(data).await
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal menulis file: {}", e),
                    error_code: Some("FILE_WRITE_ERROR".to_string()),
                })
            ))?;

        file.flush().await
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal flush file: {}", e),
                    error_code: Some("FILE_FLUSH_ERROR".to_string()),
                })
            ))?;

        fs::rename(&temp_path, file_path).await
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal finalisasi file: {}", e),
                    error_code: Some("FILE_FINALIZE_ERROR".to_string()),
                })
            ))?;

        Ok(())
    }

    // Set secure file permissions (read-write owner, read-only others)
    async fn set_secure_permissions(
        &self,
        file_path: &Path,
    ) -> Result<(), (StatusCode, axum::Json<ErrorResponse>)> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(file_path).await
                .map_err(|e| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(ErrorResponse {
                        success: false,
                        message: format!("Gagal mendapatkan permission file: {}", e),
                        error_code: Some("PERMISSION_ERROR".to_string()),
                    })
                ))?
                .permissions();

            perms.set_mode(0o644);
            fs::set_permissions(file_path, perms).await
                .map_err(|e| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(ErrorResponse {
                        success: false,
                        message: format!("Gagal set permission file: {}", e),
                        error_code: Some("PERMISSION_ERROR".to_string()),
                    })
                ))?;
        }

        Ok(())
    }

    // Store file integrity information dengan SHA-256 hash untuk verification
    async fn store_file_integrity(
        &self,
        file_path: &Path,
        data: &[u8],
    ) -> Result<(), (StatusCode, axum::Json<ErrorResponse>)> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = format!("{:x}", hasher.finalize());

        tracing::info!("Integritas file: path={}, hash={}, size={}", 
            file_path.display(), hash, data.len());

        Ok(())
    }

    // Virus scanning integration dengan basic malware signature detection
    async fn scan_file_for_viruses(
        &self,
        file_path: &Path,
    ) -> Result<(), (StatusCode, axum::Json<ErrorResponse>)> {
        let data = fs::read(file_path).await
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal membaca file untuk scan: {}", e),
                    error_code: Some("SCAN_READ_ERROR".to_string()),
                })
            ))?;

        let malware_signatures = [
            b"EICAR-STANDARD-ANTIVIRUS-TEST-FILE",
            b"X5O!P%@AP[4\\PZX54(P^)7CC)7}$EICAR-",
        ];

        for signature in &malware_signatures {
            if data.windows(signature.len()).any(|window| window == *signature) {
                let _ = fs::remove_file(file_path).await;
                
                return Err((
                    StatusCode::BAD_REQUEST,
                    axum::Json(ErrorResponse {
                        success: false,
                        message: "Malware terdeteksi dalam file".to_string(),
                        error_code: Some("MALWARE_DETECTED".to_string()),
                    })
                ));
            }
        }

        Ok(())
    }

    // ===== UTILITY METHODS =====

    // Create secure directories dengan proper permissions
    fn create_secure_directory(path: &Path) -> Result<(), UploadError> {
        if !path.exists() {
            std::fs::create_dir_all(path)
                .map_err(|_e| UploadError::DirectoryNotFound)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(path)
                    .map_err(|_e| UploadError::DirectoryNotFound)?
                    .permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(path, perms)
                    .map_err(|_e| UploadError::DirectoryNotFound)?;
            }
        }

        Ok(())
    }

    // ===== FILE DELETION METHODS =====

    // Secure file deletion dengan path traversal protection
    pub async fn delete_file(file_path: &str) -> Result<(), UploadError> {
        let upload_dir = env::var("UPLOAD_DIR").unwrap_or_else(|_| "./storage".to_string());
        
        if file_path.contains("..") || file_path.contains("//") || !file_path.starts_with("/storage/") {
            return Err(UploadError::SecurityValidationFailed("Path file tidak valid".to_string()));
        }

        let absolute_path = PathBuf::from(&upload_dir).join(&file_path[9..]);
        let canonical_upload = std::fs::canonicalize(&upload_dir)
            .map_err(|_e| UploadError::DirectoryNotFound)?;
        let canonical_file = std::fs::canonicalize(&absolute_path)
            .map_err(|e| UploadError::SaveError(format!("File tidak ditemukan: {}", e)))?;

        if !canonical_file.starts_with(&canonical_upload) {
            return Err(UploadError::SecurityValidationFailed("Path traversal terdeteksi".to_string()));
        }

        if env::var("SECURE_DELETE").unwrap_or_else(|_| "false".to_string()) == "true" {
            Self::secure_delete(&canonical_file).await?;
        } else {
            fs::remove_file(&canonical_file).await
                .map_err(|e| UploadError::SaveError(format!("Delete gagal: {}", e)))?;
        }

        Ok(())
    }

    // Secure deletion dengan multiple overwrite passes untuk sensitive data
    async fn secure_delete(file_path: &Path) -> Result<(), UploadError> {
        if let Ok(metadata) = fs::metadata(file_path).await {
            let file_size = metadata.len() as usize;
            
            for _ in 0..3 {
                let random_data: Vec<u8> = (0..file_size)
                    .map(|_| rand::random::<u8>())
                    .collect();
                
                fs::write(file_path, &random_data).await
                    .map_err(|e| UploadError::SaveError(format!("Secure overwrite gagal: {}", e)))?;
            }
        }

        fs::remove_file(file_path).await
            .map_err(|e| UploadError::SaveError(format!("Final delete gagal: {}", e)))?;

        Ok(())
    }

    // ===== STATIC VALIDATION METHODS =====

    // PDF validation dengan magic bytes check
    fn is_valid_pdf(data: &[u8]) -> bool {
        data.len() >= 5 && &data[0..5] == b"%PDF-"
    }

    // Image validation dengan magic bytes per format
    fn is_valid_image(data: &[u8], extension: &str) -> bool {
        if data.len() < 8 {
            return false;
        }

        match extension {
            "jpg" | "jpeg" => {
                data.len() >= 3 && data[0] == 0xFF && data[1] == 0xD8 && data[2] == 0xFF
            }
            "png" => {
                data.len() >= 8 && 
                data[0] == 0x89 && data[1] == 0x50 && data[2] == 0x4E && data[3] == 0x47 &&
                data[4] == 0x0D && data[5] == 0x0A && data[6] == 0x1A && data[7] == 0x0A
            }
            "webp" => {
                data.len() >= 12 &&
                &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP"
            }
            _ => false,
        }
    }
}