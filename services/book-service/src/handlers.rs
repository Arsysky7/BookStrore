// /pdf-bookstore/services/book-service/src/handlers.rs

use axum::{
    extract::{State, Path, Query, Multipart},
    http::StatusCode,
    response::{Json, Response},
    Extension,
};

use crate::models::*;

use crate::database::{BookRepository, DatabaseError};
use crate::upload::FileUploader;
use crate::AppState;
use uuid::Uuid;
use validator::Validate;
use tokio_util::io::ReaderStream;
use tokio::fs::File;
use std::env;
use bigdecimal::BigDecimal;
use tokio::time::timeout;
use std::time::Duration;

// Handler untuk mendapatkan daftar buku dengan pagination dan filter
pub async fn get_books(
    State(state): State<AppState>,                 
    Query(params): Query<BookQueryParams>,
) -> Result<Json<PaginatedBooksResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Input validation
    let validated_params = BookQueryParams {
        search: params.search.filter(|s| !s.trim().is_empty() && s.len() <= 255),
        category: params.category.filter(|c| !c.trim().is_empty() && c.len() <= 100),
        author: params.author.filter(|a| !a.trim().is_empty() && a.len() <= 300),
        language: params.language.filter(|l| l.len() <= 10),
        min_price: params.min_price.filter(|p| *p >= BigDecimal::from(0)),
        max_price: params.max_price.filter(|p| *p <= BigDecimal::from(10000000)),
        page: Some(params.page.unwrap_or(1).max(1)),
        limit: Some(params.limit.unwrap_or(12).min(100).max(1)),
        sort_by: params.sort_by,
        sort_order: params.sort_order,
    };
    
    match BookRepository::search_books(&state.db, validated_params).await {
        Ok((books, pagination)) => { 
            let base_url = env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3002".to_string());
            
            let books_with_fixed_urls = books.into_iter().map(|mut bwc| {
                if let Some(ref cover_path) = bwc.book.cover_path {
                    bwc.book.cover_path = Some(format!("{}{}", base_url, cover_path));
                }
                bwc
            }).collect();
            
            Ok(Json(PaginatedBooksResponse::success(books_with_fixed_urls, pagination)))
        }
        Err(DatabaseError::InvalidQuery) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                message: "Parameter query tidak valid".to_string(),
                error_code: Some("INVALID_QUERY".to_string()),
            })
        )),
        Err(DatabaseError::Connection(ref e)) if e.to_string().contains("timeout") => Err((
            StatusCode::GATEWAY_TIMEOUT,
            Json(ErrorResponse {
                success: false,
                message: "Database timeout, silakan coba lagi".to_string(),
                error_code: Some("DB_TIMEOUT".to_string()),
            })
        )),
        Err(e) => {
            tracing::error!("Database error in get_books: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: "Terjadi kesalahan saat mengambil data buku".to_string(),
                    error_code: Some("INTERNAL_ERROR".to_string()),
                })
            ))
        }
    }
}

// Handler untuk mendapatkan detail buku berdasarkan ID
pub async fn get_book_by_id(
    State(state): State<AppState>,                 
    Path(book_id): Path<Uuid>,                    
) -> Result<Json<BookResponse>, (StatusCode, Json<ErrorResponse>)> {
    match BookRepository::get_book_by_id(&state.db, book_id).await {
        Ok(mut book_with_categories) => {
            // Tambahkan base URL ke cover path
            if let Some(ref cover_path) = book_with_categories.book.cover_path {
                let base_url = env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3002".to_string());
                book_with_categories.book.cover_path = Some(format!("{}{}", base_url, cover_path));
            }
            Ok(Json(BookResponse::success(book_with_categories)))
        }
        Err(DatabaseError::BookNotFound) => {      
            Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    success: false,
                    message: "Book tidak ditemukan".to_string(),
                    error_code: Some("BOOK_NOT_FOUND".to_string()),
                })
            ))
        }
        Err(e) => {                                
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal mengambil book: {}", e),
                    error_code: Some("DATABASE_ERROR".to_string()),
                })
            ))
        }
    }
}

// helper create
async fn cleanup_uploaded_files(pdf_path: Option<&str>, cover_path: Option<&str>) {
    if let Some(pdf) = pdf_path {
        if let Err(e) = FileUploader::delete_file(pdf).await {
            tracing::warn!("Failed to cleanup PDF file {}: {}", pdf, e);
        }
    }
    
    if let Some(cover) = cover_path {
        if let Err(e) = FileUploader::delete_file(cover).await {
            tracing::warn!("Failed to cleanup cover file {}: {}", cover, e);
        }
    }
}

// Handler untuk membuat buku baru (Admin only)
pub async fn create_book(
    State(state): State<AppState>,                 
    Extension(user_role): Extension<String>,
    Extension(user_id): Extension<Uuid>,    
    multipart: Multipart,                          
) -> Result<Json<BookResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validasi akses admin
    if user_role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                success: false,
                message: "Akses admin diperlukan".to_string(),
                error_code: Some("INSUFFICIENT_PRIVILEGES".to_string()),
            }),
        ));
    }

    // Parse multipart form data dengan timeout
    let (book_request, pdf_path, cover_path, file_size_mb) = 
        timeout(
            Duration::from_secs(30),
            parse_create_book_multipart(multipart, &user_id.to_string())
        )
        .await
        .map_err(|_| (
            StatusCode::REQUEST_TIMEOUT,
            Json(ErrorResponse {
                success: false,
                message: "Request timeout saat upload file".to_string(),
                error_code: Some("UPLOAD_TIMEOUT".to_string()),
            }),
        ))??;

    // Validasi data buku
    if let Err(errors) = book_request.validate() {
        let error_msg = format!("Error validasi: {:?}", errors);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                message: error_msg,
                error_code: Some("VALIDATION_ERROR".to_string()),
            }),
        ));
    }

    // Simpan buku ke database
    match BookRepository::create_book(&state.db, book_request, pdf_path.clone(), cover_path.clone(), file_size_mb).await {
        Ok(book) => {
            // Ambil data lengkap buku dengan kategori
            match BookRepository::get_book_by_id(&state.db, book.id).await {
                Ok(mut book_with_categories) => {
                    if let Some(ref cover_path) = book_with_categories.book.cover_path {
                        let base_url = env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3002".to_string());
                        book_with_categories.book.cover_path = Some(format!("{}{}", base_url, cover_path));
                    }
                    Ok(Json(BookResponse::success(book_with_categories)))
                }
                Err(_) => {
                    Ok(Json(BookResponse::error("Book berhasil dibuat tapi gagal load detail")))
                }
            }
        }
        Err(e) => {
            // Cleanup file kalau gagal membuat buku
            cleanup_uploaded_files(
                pdf_path.as_deref(),
                cover_path.as_deref()
            ).await;

            match e {
                DatabaseError::IsbnExists => Err((
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        success: false,
                        message: "ISBN sudah ada".to_string(),
                        error_code: Some("ISBN_EXISTS".to_string()),
                    }),
                )),
                DatabaseError::CategoryNotFound => Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        success: false,
                        message: "Satu atau lebih kategori tidak ditemukan".to_string(),
                        error_code: Some("CATEGORY_NOT_FOUND".to_string()),
                    }),
                )),
                _ => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        success: false,
                        message: format!("Gagal membuat book: {}", e),
                        error_code: Some("DATABASE_ERROR".to_string()),
                    }),
                )),
            }
        }
    }
}


// Fungsi helper untuk parse multipart data saat create book
async fn parse_create_book_multipart(
    mut multipart: Multipart,
    user_id: &str,
) -> Result<(CreateBookRequest, Option<String>, Option<String>, Option<BigDecimal>), (StatusCode, Json<ErrorResponse>)> {
    const MAX_TEXT_FIELD_SIZE: usize = 10_000; 
    tracing::info!("User {} starting book creation upload", user_id);
    
    let mut title = None;
    let mut author = None;
    let mut description = None;
    let mut isbn = None;
    let mut price = None;
    let mut language = None;
    let mut category_ids = None;
    let mut total_pages = None;
    let mut pdf_path = None;
    let mut cover_path = None;
    let mut file_size_mb = None;

    // Proses setiap field dari multipart form
    while let Some(mut field) = multipart.next_field().await.map_err(|e| (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            success: false,
            message: format!("Gagal parse multipart data: {}", e),
            error_code: Some("MULTIPART_ERROR".to_string()),
        })
    ))? {
        let name = field.name().unwrap_or("").to_string();

        if matches!(name.as_str(), "title" | "author" | "description" | "isbn") {
            let mut buffer = Vec::new();

            while let Some(chunk) = field.chunk().await.map_err(|_| (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    success: false,
                    message: "Gagal membaca field chunk".to_string(),
                    error_code: Some("CHUNK_READ_ERROR".to_string()),
                })
            ))? {
                if buffer.len() + chunk.len() > MAX_TEXT_FIELD_SIZE {
                    return Err((
                        StatusCode::PAYLOAD_TOO_LARGE,
                        Json(ErrorResponse {
                            success: false,
                            message: format!("Field {} terlalu besar (max 10KB)", name),
                            error_code: Some("FIELD_TOO_LARGE".to_string()),
                        })
                    ));
                }
                buffer.extend_from_slice(&chunk);
            }

            let text = String::from_utf8(buffer).map_err(|_| (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    success: false,
                    message: "Encoding UTF-8 tidak valid".to_string(),
                    error_code: Some("INVALID_ENCODING".to_string()),
                })
            ))?;

            match name.as_str() {
                "title" => title = Some(text),
                "author" => author = Some(text),
                "description" => {
                    description = if text.trim().is_empty() {
                        Some(None)
                    } else {
                        Some(Some(text))
                    };
                }
                "isbn" => {
                    isbn = if text.trim().is_empty() {
                        Some(None)
                    } else {
                        Some(Some(text))
                    };
                }
                _ => {}
            }
        } else {
            match name.as_str() {
                "price" => {
                    let text = field.text().await.map_err(|_| (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            success: false,
                            message: "Gagal baca field price".to_string(),
                            error_code: Some("FIELD_READ_ERROR".to_string()),
                        })
                    ))?;
                    price = Some(text.parse::<BigDecimal>().map_err(|_| (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            success: false,
                            message: "Format price tidak valid".to_string(),
                            error_code: Some("INVALID_PRICE".to_string()),
                        })
                    ))?);
                }
                "language" => {
                    language = Some(field.text().await.map_err(|_| (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            success: false,
                            message: "Gagal baca field language".to_string(),
                            error_code: Some("FIELD_READ_ERROR".to_string()),
                        })
                    ))?);
                }
                "category_ids" => {
                    let text = field.text().await.map_err(|_| (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            success: false,
                            message: "Gagal baca field category_ids".to_string(),
                            error_code: Some("FIELD_READ_ERROR".to_string()),
                        })
                    ))?;
                    let ids: Result<Vec<Uuid>, _> = text.split(',')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.parse::<Uuid>())
                        .collect();

                    category_ids = Some(ids.map_err(|_| (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            success: false,
                            message: "Format category_ids tidak valid".to_string(),
                            error_code: Some("INVALID_CATEGORY_ID".to_string()),
                        })
                    ))?);
                }
                "total_pages" => {
                    let text = field.text().await.map_err(|_| (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            success: false,
                            message: "Gagal baca field total_pages".to_string(),
                            error_code: Some("FIELD_READ_ERROR".to_string()),
                        })
                    ))?;
                    if !text.trim().is_empty() {
                        total_pages = Some(text.parse::<i32>().map_err(|_| (
                            StatusCode::BAD_REQUEST,
                            Json(ErrorResponse {
                                success: false,
                                message: "Format total_pages tidak valid".to_string(),
                                error_code: Some("INVALID_PAGES".to_string()),
                            })
                        ))?);
                    }
                }
                "pdf_file" => {
                    match FileUploader::upload_pdf_from_field(field).await {
                        Ok((path, size)) => {
                            pdf_path = Some(path);
                            file_size_mb = Some(size);
                        }
                        Err((status, json)) => return Err((status, json)),
                    }
                }
                "cover_image" => {
                    match FileUploader::upload_cover_from_field(field).await {
                        Ok(path) => cover_path = Some(path),
                        Err((status, json)) => return Err((status, json)),
                    }
                }
                _ => {
                    // Ignore unknown fields
                    let _ = field.bytes().await;
                }
            }
        }
    }

    // Validasi final untuk field wajib
    let title = title.ok_or((
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            success: false,
            message: "Title diperlukan".to_string(),
            error_code: Some("MISSING_TITLE".to_string()),
        })
    ))?;

    let author = author.ok_or((
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            success: false,
            message: "Author diperlukan".to_string(),
            error_code: Some("MISSING_AUTHOR".to_string()),
        })
    ))?;

    let price = price.ok_or((
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            success: false,
            message: "Price diperlukan".to_string(),
            error_code: Some("MISSING_PRICE".to_string()),
        })
    ))?;

    let book_request = CreateBookRequest {
        title,
        author,
        description,
        isbn,
        price,
        language,
        category_ids: category_ids.unwrap_or_default(),
        total_pages,
    };

    // Validate business rules
    if let Err(e) = book_request.validate_business_rules() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                message: e,
                error_code: Some("VALIDATION_ERROR".to_string()),
            })
        ));
    }

    Ok((book_request, pdf_path, cover_path, file_size_mb))
}

// Handler untuk update buku (Admin only)
pub async fn update_book(
    State(state): State<AppState>,                 
    Path(book_id): Path<Uuid>,                     
    Extension(user_role): Extension<String>,
    Extension(user_id): Extension<Uuid>,      
    multipart: Multipart,                          
) -> Result<Json<BookResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validasi akses admin
    if user_role != "admin" {                      
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                success: false,
                message: "Akses admin diperlukan".to_string(),
                error_code: Some("INSUFFICIENT_PRIVILEGES".to_string()),
            })
        ));
    }

    // Parse update data dengan timeout
    let (update_request, pdf_path, cover_path, file_size_mb) = 
        timeout(
            Duration::from_secs(30),
            parse_update_book_multipart(multipart, &user_id.to_string())
        )
        .await
        .map_err(|_| (
            StatusCode::REQUEST_TIMEOUT,
            Json(ErrorResponse {
                success: false,
                message: "Request timeout saat upload file".to_string(),
                error_code: Some("UPLOAD_TIMEOUT".to_string()),
            })
        ))??;

    // Validasi update data
    if let Err(errors) = update_request.validate() { 
        let error_msg = format!("Error validasi: {:?}", errors);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                message: error_msg,
                error_code: Some("VALIDATION_ERROR".to_string())
            })
        ));
    }

    // Update buku di database
    match BookRepository::update_book(&state.db, book_id, update_request, pdf_path, cover_path, file_size_mb).await {
        Ok(_) => {
            match BookRepository::get_book_by_id(&state.db, book_id).await {
                Ok(mut book_with_categories) => {
                    if let Some(ref cover_path) = book_with_categories.book.cover_path {
                        let base_url = env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3002".to_string());
                        book_with_categories.book.cover_path = Some(format!("{}{}", base_url, cover_path));
                    }
                    Ok(Json(BookResponse::success(book_with_categories)))
                }
                Err(_) => {
                    Ok(Json(BookResponse::error("Book berhasil diupdate tapi gagal load detail")))
                }
            }
        }
        Err(DatabaseError::BookNotFound) => {
            Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    success: false,
                    message: "Book tidak ditemukan".to_string(),
                    error_code: Some("BOOK_NOT_FOUND".to_string()),
                })
            ))
        }
        Err(DatabaseError::IsbnExists) => {
            Err((
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    success: false,
                    message: "ISBN sudah ada".to_string(),
                    error_code: Some("ISBN_EXISTS".to_string()),
                })
            ))
        }
        Err(DatabaseError::CategoryNotFound) => {
            Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    success: false,
                    message: "Satu atau lebih kategori tidak ditemukan".to_string(),
                    error_code: Some("CATEGORY_NOT_FOUND".to_string()),
                })
            ))
        }
        Err(e) => {
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal update book: {}", e),
                    error_code: Some("DATABASE_ERROR".to_string()),
                })
            ))
        }
    }
}

// Fungsi helper untuk parse multipart data saat update book
async fn parse_update_book_multipart(
    mut multipart: Multipart,
    _user_id: &str,
) -> Result<(UpdateBookRequest, Option<String>, Option<String>, Option<BigDecimal>), (StatusCode, Json<ErrorResponse>)> {
    let mut title = None;
    let mut author = None;
    let mut description = None;
    let mut isbn = None;
    let mut price = None;
    let mut language = None;
    let mut category_ids = None;
    let mut is_active = None;
    let mut total_pages = None;
    let mut pdf_path = None;
    let mut cover_path = None;
    let mut file_size_mb = None;

    while let Some(field) = multipart.next_field().await
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                message: format!("Gagal parse multipart data: {}", e),
                error_code: Some("MULTIPART_ERROR".to_string()),
            })
        ))? {
        
        let name = field.name().unwrap_or("").to_string();
        
        match name.as_str() {
            "title" => {
                let text = field.text().await.map_err(|_| (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        success: false,
                        message: "Gagal baca field title".to_string(),
                        error_code: Some("FIELD_READ_ERROR".to_string()),
                    })
                ))?;
                if !text.trim().is_empty() {
                    title = Some(text);
                }
            }
            "author" => {
                let text = field.text().await.map_err(|_| (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        success: false,
                        message: "Gagal baca field author".to_string(),
                        error_code: Some("FIELD_READ_ERROR".to_string()),
                    })
                ))?;
                if !text.trim().is_empty() {
                    author = Some(text);
                }
            }
            "description" => {
                let text = field.text().await.map_err(|_| (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        success: false,
                        message: "Gagal baca field description".to_string(),
                        error_code: Some("FIELD_READ_ERROR".to_string()),
                    })
                ))?;
                if !text.trim().is_empty() {
                    description = Some(Some(text));
                } else {
                    description = Some(None);
                }
            }
            "isbn" => {
                let text = field.text().await.map_err(|_| (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        success: false,
                        message: "Gagal baca field ISBN".to_string(),
                        error_code: Some("FIELD_READ_ERROR".to_string()),
                    })
                ))?;
                if !text.trim().is_empty() {
                    isbn = Some(Some(text));
                } else {
                    isbn = Some(None);
                }
            }
            "price" => {
                let text = field.text().await.map_err(|_| (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        success: false,
                        message: "Gagal baca field price".to_string(),
                        error_code: Some("FIELD_READ_ERROR".to_string()),
                    })
                ))?;
                if !text.trim().is_empty() {
                    price = Some(text.parse::<BigDecimal>().map_err(|_| (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            success: false,
                            message: "Format price tidak valid".to_string(),
                            error_code: Some("INVALID_PRICE".to_string()),
                        })
                    ))?);
                }
            }
            "language" => {
                let text = field.text().await.map_err(|_| (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        success: false,
                        message: "Gagal baca field language".to_string(),
                        error_code: Some("FIELD_READ_ERROR".to_string()),
                    })
                ))?;
                if !text.trim().is_empty() {
                    language = Some(text);
                }
            }
            "is_active" => {
                let text = field.text().await.map_err(|_| (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        success: false,
                        message: "Gagal baca field is_active".to_string(),
                        error_code: Some("FIELD_READ_ERROR".to_string()),
                    })
                ))?;
                if !text.trim().is_empty() {
                    is_active = Some(text.parse::<bool>().unwrap_or(true));
                }
            }
            "category_ids" => {
                let text = field.text().await.map_err(|_| (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        success: false,
                        message: "Gagal baca field category_ids".to_string(),
                        error_code: Some("FIELD_READ_ERROR".to_string()),
                    })
                ))?;
                
                if !text.trim().is_empty() {
                    let ids: Result<Vec<Uuid>, _> = text
                        .split(',')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.parse::<Uuid>())
                        .collect();
                    
                    category_ids = Some(ids.map_err(|_| (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            success: false,
                            message: "Format ID kategori tidak valid".to_string(),
                            error_code: Some("INVALID_CATEGORY_ID".to_string()),
                        })
                    ))?);
                }
            }
            "total_pages" => {
                let text = field.text().await.map_err(|_| (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        success: false,
                        message: "Gagal baca field total_pages".to_string(),
                        error_code: Some("FIELD_READ_ERROR".to_string()),
                    })
                ))?;
                if !text.trim().is_empty() {
                    total_pages = Some(text.parse::<i32>().map_err(|_| (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            success: false,
                            message: "Format total_pages tidak valid".to_string(),
                            error_code: Some("INVALID_PAGES".to_string()),
                        })
                    ))?);
                }
            }
            "pdf_file" => {
                match FileUploader::upload_pdf_from_field(field).await {
                    Ok((path, size)) => {
                        pdf_path = Some(path);
                        file_size_mb = Some(size);
                    }
                    Err((status, json_err)) => return Err((status, json_err)),
                }
            }
            "cover_image" => {
                match FileUploader::upload_cover_from_field(field).await {
                    Ok(path) => cover_path = Some(path),
                    Err((status, json_err)) => return Err((status, json_err)),
                }
            }
            _ => {
                let _ = field.bytes().await;
            }
        }
    }

    let update_request = UpdateBookRequest {
        title,
        author,
        description,
        isbn,
        price,
        language,
        category_ids,
        is_active,
        total_pages,
    };

    Ok((update_request, pdf_path, cover_path, file_size_mb))
}

// Handler untuk hapus buku (soft delete) - Admin only
pub async fn delete_book(
    State(state): State<AppState>,                 
    Path(book_id): Path<Uuid>,                    
    Extension(user_role): Extension<String>,      
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Validasi akses admin
    if user_role != "admin" {                     
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                success: false,
                message: "Akses admin diperlukan".to_string(),
                error_code: Some("INSUFFICIENT_PRIVILEGES".to_string()),
            })
        ));
    }

    // Hapus buku (soft delete di database)
    match BookRepository::delete_book(&state.db, book_id).await {
        Ok(()) => {                                
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "Book berhasil dihapus"
            })))
        }
        Err(DatabaseError::BookNotFound) => {
            Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    success: false,
                    message: "Book tidak ditemukan".to_string(),
                    error_code: Some("BOOK_NOT_FOUND".to_string()),
                })
            ))
        }
        Err(e) => {
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal hapus book: {}", e),
                    error_code: Some("DATABASE_ERROR".to_string()),
                })
            ))
        }
    }
}

// Handler untuk health check endpoint
pub async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "service": "book-service",
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
        "version": "1.0.0"
    }))
}

// Handler untuk download file PDF (memerlukan autentikasi)
pub async fn download_book_pdf(
    State(state): State<AppState>,                 
    Path(book_id): Path<Uuid>,                     
    Extension(_user_id): Extension<Uuid>,          
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {

    // Ambil data buku untuk mendapatkan path PDF
    let book = match BookRepository::get_book_by_id(&state.db, book_id).await {
        Ok(book_with_categories) => book_with_categories.book, 
        Err(DatabaseError::BookNotFound) => {      
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    success: false,
                    message: "Book tidak ditemukan".to_string(),
                    error_code: Some("BOOK_NOT_FOUND".to_string()),
                })
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal mengambil book: {}", e),
                    error_code: Some("DATABASE_ERROR".to_string()),
                })
            ));
        }
    };

    // Cek apakah file PDF tersedia
    let pdf_path = match book.pdf_path {           
        Some(path) => path,                        
        None => {                                  
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    success: false,
                    message: "File PDF tidak tersedia".to_string(),
                    error_code: Some("PDF_NOT_AVAILABLE".to_string()),
                })
            ));
        }
    };

    // Konversi path relatif ke path absolut
    let upload_dir = env::var("UPLOAD_DIR").unwrap_or_else(|_| "./storage".to_string());
    let absolute_path = if pdf_path.starts_with("/storage/") {
        format!("{}{}", upload_dir, &pdf_path[8..])
    } else {
        format!("{}/{}", upload_dir, pdf_path)     
    };

    // Buka file PDF untuk streaming
    let file = match File::open(&absolute_path).await {
        Ok(file) => file,                          
        Err(_) => {                                
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    success: false,
                    message: "File PDF tidak ditemukan di server".to_string(),
                    error_code: Some("FILE_NOT_FOUND".to_string()),
                })
            ));
        }
    };

    // Update counter download
    let _ = BookRepository::increment_download_count(&state.db, book_id).await;

    // Streaming file untuk download
    let stream = ReaderStream::new(file);          
    let body = axum::body::Body::from_stream(stream); 

    // Setup response headers
    let mut response = Response::new(body);
    let headers = response.headers_mut();

    // Set content type untuk PDF
    headers.insert("content-type", "application/pdf".parse().unwrap());

    // Set content disposition dengan nama file
    let filename = format!("{}_by_{}.pdf", 
        book.title.replace(" ", "_"),              
        book.author.replace(" ", "_")             
    );
    headers.insert("content-disposition", 
        format!("attachment; filename=\"{}\"", filename).parse().unwrap());

    // Set cache control
    headers.insert("cache-control", "private, max-age=3600".parse().unwrap());

    Ok(response)                                   
}

// Handler untuk mendapatkan semua kategori
pub async fn get_categories(
    State(state): State<AppState>,                 
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match BookRepository::get_all_categories(&state.db).await {
        Ok(categories) => {                       
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "Kategori berhasil diambil",
                "data": categories
            })))
        }
        Err(e) => {                                
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal mengambil kategori: {}", e),
                    error_code: Some("DATABASE_ERROR".to_string()),
                })
            ))
        }
    }
}

// Handler untuk upload file PDF saja (Admin only)
pub async fn upload_pdf_only(
    Extension(user_role): Extension<String>,
    Extension(user_id): Extension<Uuid>,       
    multipart: Multipart,                          
) -> Result<Json<FileUploadResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validasi akses admin
    if user_role != "admin" {                      
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                success: false,
                message: "Akses admin diperlukan".to_string(),
                error_code: Some("INSUFFICIENT_PRIVILEGES".to_string()),
            })
        ));
    }

    // Inisialisasi uploader dan upload PDF
    let file_uploader = FileUploader::new()
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                success: false,
                message: format!("Gagal inisialisasi uploader: {}", e),
                error_code: Some("UPLOADER_INIT_ERROR".to_string()),
            })
        ))?;

    match file_uploader.upload_pdf(multipart, &user_id.to_string()).await {
        Ok((file_path, file_size_mb)) => {         
            Ok(Json(FileUploadResponse::success(file_path, Some(file_size_mb))))
        }
        Err((status, error_response)) => {         
            Err((status, error_response))          
        }
    }
}

// Handler untuk upload cover image saja (Admin only)
pub async fn upload_cover_only(
    Extension(user_role): Extension<String>,
    Extension(_user_id): Extension<Uuid>,       
    multipart: Multipart,                          
) -> Result<Json<FileUploadResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validasi akses admin
    if user_role != "admin" {                      
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                success: false,
                message: "Akses admin diperlukan".to_string(),
                error_code: Some("INSUFFICIENT_PRIVILEGES".to_string()),
            })
        ));
    }

    // Inisialisasi uploader dan upload cover
    let file_uploader = FileUploader::new()
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                success: false,
                message: format!("Gagal inisialisasi uploader: {}", e),
                error_code: Some("UPLOADER_INIT_ERROR".to_string()),
            })
        ))?;

    match file_uploader.upload_cover_image(multipart, &_user_id.to_string()).await {
        Ok(file_path) => {                         
            Ok(Json(FileUploadResponse::success(file_path, None)))
         
        }
        Err((status, error_response)) => {         
            Err((status, error_response))          
        }
    }
}

/// Handler untuk validasi buku sebelum order Memastikan buku exists, active, dan punya PDF
pub async fn validate_book_for_order(
    State(state): State<AppState>,
    Path(book_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Ambil data buku
    match BookRepository::get_book_by_id(&state.db, book_id).await {
        Ok(book_with_categories) => {
            let book = &book_with_categories.book;
            
            // Validasi buku bisa diorder
            if !book.is_active {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        success: false,
                        message: "Buku tidak tersedia untuk pembelian".to_string(),
                        error_code: Some("BOOK_INACTIVE".to_string()),
                    })
                ));
            }
            
            if book.pdf_path.is_none() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        success: false,
                        message: "File PDF belum tersedia".to_string(),
                        error_code: Some("PDF_NOT_AVAILABLE".to_string()),
                    })
                ));
            }
            
            Ok(Json(serde_json::json!({
                "success": true,
                "valid": true,
                "data": {
                    "id": book.id,
                    "title": book.title,
                    "author": book.author,
                    "price": book.price,
                    "pdf_available": book.pdf_path.is_some(),
                    "language": book.language,
                }
            })))
        }
        Err(DatabaseError::BookNotFound) => {
            Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    success: false,
                    message: "Buku tidak ditemukan".to_string(),
                    error_code: Some("BOOK_NOT_FOUND".to_string()),
                })
            ))
        }
        Err(e) => {
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: format!("Error validasi buku: {}", e),
                    error_code: Some("VALIDATION_ERROR".to_string()),
                })
            ))
        }
    }
}

/// Handler untuk webhook setelah payment success Update download count dan catat transaksi
pub async fn handle_payment_success_webhook(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Validasi webhook signature untuk security
    let webhook_secret = env::var("WEBHOOK_SECRET").unwrap_or_else(|_| "default_secret".to_string());
    
    // Ambil signature dari header
    let signature = headers
        .get("X-Webhook-Signature")
        .and_then(|v| v.to_str().ok())
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                success: false,
                message: "Missing webhook signature".to_string(),
                error_code: Some("MISSING_SIGNATURE".to_string()),
            })
        ))?;
    
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    
    let mut mac = Hmac::<Sha256>::new_from_slice(webhook_secret.as_bytes())
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                success: false,
                message: "Invalid webhook secret".to_string(),
                error_code: Some("INVALID_SECRET".to_string()),
            })
        ))?;
    
    // Generate expected signature dari payload
    mac.update(serde_json::to_string(&payload).unwrap().as_bytes());
    let expected_signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));
    
    // Compare signatures
    if signature != expected_signature {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                success: false,
                message: "Invalid webhook signature".to_string(),
                error_code: Some("INVALID_SIGNATURE".to_string()),
            })
        ));
    }
    
    // Parse payload setelah validasi
    let book_id = payload.get("book_id")
        .and_then(|id| id.as_str())
        .and_then(|id| Uuid::parse_str(id).ok())
        .ok_or((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                message: "Invalid book_id dalam payload".to_string(),
                error_code: Some("INVALID_PAYLOAD".to_string()),
            })
        ))?;
    
    let order_id = payload.get("order_id")
        .and_then(|id| id.as_str())
        .unwrap_or("unknown");
    
    let user_id = payload.get("user_id")
        .and_then(|id| id.as_str())
        .and_then(|id| Uuid::parse_str(id).ok());
    
    // Increment download counter
    if let Err(e) = BookRepository::increment_download_count(&state.db, book_id).await {
        tracing::error!("Gagal update download count untuk book {}: {}", book_id, e);
    }
    
    // Log transaksi untuk audit trail
    if let Some(uid) = user_id {
        sqlx::query!(
            r#"
            INSERT INTO audit_logs (action, resource_type, resource_id, user_id, details)
            VALUES ('BOOK_PURCHASED', 'book', $1, $2, $3)
            "#,
            book_id,
            uid,
            serde_json::json!({
                "order_id": order_id,
                "timestamp": chrono::Utc::now()
            })
        )
        .execute(&state.db)
        .await
        .ok(); 
    }
    
    tracing::info!("Payment webhook processed: book={}, order={}", book_id, order_id);
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Webhook processed successfully",
        "book_id": book_id,
        "order_id": order_id
    })))
}

/// Handler untuk mendapatkan stock info 
pub async fn get_book_stock(
    State(state): State<AppState>,
    Path(book_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Check buku exists dan active
    match BookRepository::get_book_by_id(&state.db, book_id).await {
        Ok(book_with_categories) => {
            let book = &book_with_categories.book;
            
            // Digital product = unlimited stock
            let stock_info = if book.is_active && book.pdf_path.is_some() {
                serde_json::json!({
                    "available": true,
                    "stock": 999999,
                    "type": "digital"
                })
            } else {
                serde_json::json!({
                    "available": false,
                    "stock": 0,
                    "type": "digital"
                })
            };
            
            Ok(Json(serde_json::json!({
                "success": true,
                "data": stock_info
            })))
        }
        Err(_) => {
            Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    success: false,
                    message: "Buku tidak ditemukan".to_string(),
                    error_code: Some("BOOK_NOT_FOUND".to_string()),
                })
            ))
        }
    }
}

// ========================= LIBRARY HANDLERS =========================

/// Handler untuk mendapatkan library user (buku yang sudah dibeli)
/// GET /api/books/my-library
pub async fn get_my_library(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<LibraryBooksResponse>, (StatusCode, Json<ErrorResponse>)> {
    match BookRepository::get_user_library(&state.db, user_id).await {
        Ok(books) => {
            let total = books.len() as i64;
            let base_url = env::var("BASE_URL")
                .unwrap_or_else(|_| "http://localhost:3002".to_string());
            
            let books_with_fixed_urls = books.into_iter().map(|mut pb| {
                if let Some(ref cover_path) = pb.book.cover_path {
                    pb.book.cover_path = Some(format!("{}{}", base_url, cover_path));
                }
                pb
            }).collect();
            
            tracing::info!("User {} library fetched: {} books", user_id, total);
            Ok(Json(LibraryBooksResponse::success(books_with_fixed_urls, total)))
        }
        Err(e) => {
            tracing::error!("Failed to fetch library for user {}: {}", user_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal mengambil library: {}", e),
                    error_code: Some("LIBRARY_ERROR".to_string()),
                })
            ))
        }
    }
}

// ========================= PREVIEW HANDLERS =========================

/// Handler untuk mendapatkan preview data buku
/// GET /api/books/{id}/preview
pub async fn get_book_preview(
    State(state): State<AppState>,
    Path(book_id): Path<Uuid>,
) -> Result<Json<BookPreviewResponse>, (StatusCode, Json<ErrorResponse>)> {
    match BookRepository::get_book_preview(&state.db, book_id).await {
        Ok(Some(preview_data)) => {
            if preview_data.has_preview {
                let mut fixed_data = preview_data;
                if let Some(ref preview_url) = fixed_data.preview_url {
                    let base_url = env::var("BASE_URL")
                        .unwrap_or_else(|_| "http://localhost:3002".to_string());
                    fixed_data.preview_url = Some(format!("{}{}", base_url, preview_url));
                }
                Ok(Json(BookPreviewResponse::success(fixed_data)))
            } else {
                Ok(Json(BookPreviewResponse::not_available()))
            }
        }
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                success: false,
                message: "Buku tidak ditemukan".to_string(),
                error_code: Some("BOOK_NOT_FOUND".to_string()),
            })
        )),
        Err(e) => {
            tracing::error!("Failed to fetch preview for book {}: {}", book_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal mengambil preview: {}", e),
                    error_code: Some("PREVIEW_ERROR".to_string()),
                })
            ))
        }
    }
}

// ========================= RELATED BOOKS HANDLERS =========================

/// Handler untuk mendapatkan buku terkait
/// GET /api/books/{id}/related
pub async fn get_related_books(
    State(state): State<AppState>,
    Path(book_id): Path<Uuid>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<RelatedBooksResponse>, (StatusCode, Json<ErrorResponse>)> {
    let limit = params.get("limit")
        .and_then(|l| l.parse::<u32>().ok())
        .unwrap_or(6)
        .min(20);

    match BookRepository::get_related_books(&state.db, book_id, limit).await {
        Ok(books) => {
            let base_url = env::var("BASE_URL")
                .unwrap_or_else(|_| "http://localhost:3002".to_string());
            
            let books_with_fixed_urls: Vec<BookWithCategories> = books.into_iter().map(|mut bwc| {
                if let Some(ref cover_path) = bwc.book.cover_path {
                    bwc.book.cover_path = Some(format!("{}{}", base_url, cover_path));
                }
                bwc
            }).collect();
            
            tracing::info!("Related books for {} fetched: {} books", book_id, books_with_fixed_urls.len());
            Ok(Json(RelatedBooksResponse::success(
                books_with_fixed_urls,
                "same_category".to_string()
            )))
        }
        Err(e) => {
            tracing::error!("Failed to fetch related books for {}: {}", book_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal mengambil related books: {}", e),
                    error_code: Some("RELATED_BOOKS_ERROR".to_string()),
                })
            ))
        }
    }
}

// ========================= REVIEW HANDLERS =========================

/// Handler untuk mendapatkan reviews buku (public, optional auth)
/// GET /api/books/{id}/reviews
pub async fn get_book_reviews(
    State(state): State<AppState>,
    Path(book_id): Path<Uuid>,
) -> Result<Json<BookReviewsResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Try to get user_id from extensions (optional)
    // Auth middleware will inject this if user is authenticated
    let user_id = None; // SIMPLIFIED: Always None untuk GET reviews (public)
    
    match BookRepository::get_book_by_id(&state.db, book_id).await {
        Ok(_) => {
            match BookRepository::get_book_reviews(&state.db, book_id, user_id).await {
                Ok((reviews, stats)) => {
                    tracing::info!(
                        "Reviews for book {} fetched: {} reviews, avg: {:.1}",
                        book_id, reviews.len(), stats.average_rating
                    );
                    Ok(Json(BookReviewsResponse::success(reviews, stats)))
                }
                Err(e) => {
                    tracing::error!("Failed to fetch reviews for {}: {}", book_id, e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            success: false,
                            message: format!("Gagal mengambil reviews: {}", e),
                            error_code: Some("REVIEWS_ERROR".to_string()),
                        })
                    ))
                }
            }
        }
        Err(DatabaseError::BookNotFound) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                success: false,
                message: "Buku tidak ditemukan".to_string(),
                error_code: Some("BOOK_NOT_FOUND".to_string()),
            })
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                success: false,
                message: format!("Error: {}", e),
                error_code: Some("DATABASE_ERROR".to_string()),
            })
        ))
    }
}

/// Handler untuk membuat review buku
/// POST /api/books/{id}/reviews
pub async fn create_book_review(
    State(state): State<AppState>,
    Path(book_id): Path<Uuid>,
    Extension(user_id): Extension<Uuid>,
    Json(review_request): Json<CreateReviewRequest>,
) -> Result<Json<ReviewResponse>, (StatusCode, Json<ErrorResponse>)> {
    if let Err(errors) = review_request.validate() {
        let error_msg = format!("Validation error: {:?}", errors);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                message: error_msg,
                error_code: Some("VALIDATION_ERROR".to_string()),
            })
        ));
    }

    match BookRepository::check_user_purchased_book(&state.db, user_id, book_id).await {
        Ok(true) => {
            match BookRepository::create_book_review(
                &state.db, book_id, user_id, 
                review_request.rating, review_request.comment,
            ).await {
                Ok(review) => {
                    let user_info = sqlx::query!(
                        "SELECT full_name, email FROM users WHERE id = $1",
                        user_id
                    )
                    .fetch_one(&state.db)
                    .await
                    .map_err(|e| (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            success: false,
                            message: format!("Failed to fetch user: {}", e),
                            error_code: Some("DATABASE_ERROR".to_string()),
                        })
                    ))?;

                    let review_with_user = BookReviewWithUser {
                        id: review.id,
                        book_id: review.book_id,
                        user_id: review.user_id,
                        user_name: user_info.full_name,
                        user_email: user_info.email,
                        rating: review.rating,
                        comment: review.comment,
                        helpful_count: review.helpful_count,
                        created_at: review.created_at,
                        updated_at: review.updated_at,
                        can_edit: true,
                        has_voted_helpful: false,
                    };

                    tracing::info!("Review created: user={}, book={}, rating={}", 
                        user_id, book_id, review.rating);

                    Ok(Json(ReviewResponse::success(review_with_user)))
                }
                Err(e) => {
                    tracing::error!("Failed to create review: {}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            success: false,
                            message: format!("Gagal membuat review: {}", e),
                            error_code: Some("REVIEW_CREATE_ERROR".to_string()),
                        })
                    ))
                }
            }
        }
        Ok(false) => Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                success: false,
                message: "Anda harus membeli buku ini untuk membuat review".to_string(),
                error_code: Some("NOT_PURCHASED".to_string()),
            })
        )),
        Err(e) => {
            tracing::error!("Failed to check purchase: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: "Gagal memverifikasi pembelian".to_string(),
                    error_code: Some("PURCHASE_CHECK_ERROR".to_string()),
                })
            ))
        }
    }
}

// ========================= HANDLER ADMIN ANALYTICS =========================

// Handler untuk statistik buku admin dashboard
pub async fn get_admin_book_stats(
    State(state): State<AppState>,
    Extension(user_role): Extension<String>,
) -> Result<Json<AdminBookStatsResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validasi akses admin
    if user_role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                success: false,
                message: "Akses admin diperlukan".to_string(),
                error_code: Some("INSUFFICIENT_PRIVILEGES".to_string()),
            })
        ));
    }

    // Ambil statistik dari database
    match BookRepository::get_admin_book_stats(&state.db).await {
        Ok(stats) => {
            tracing::info!("Admin book stats berhasil diambil: {} total books, {}% monthly growth", 
                stats.total_books, stats.monthly_growth_percentage);
            
            Ok(Json(AdminBookStatsResponse::success(stats)))
        }
        Err(e) => {
            tracing::error!("Gagal mengambil admin book stats: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal mengambil statistik book: {}", e),
                    error_code: Some("STATS_ERROR".to_string()),
                })
            ))
        }
    }
}

// Handler untuk top books berdasarkan metrik tertentu
pub async fn get_top_books(
    State(state): State<AppState>,
    Extension(user_role): Extension<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<AdminTopBooksResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validasi akses admin
    if user_role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                success: false,
                message: "Akses admin diperlukan".to_string(),
                error_code: Some("INSUFFICIENT_PRIVILEGES".to_string()),
            })
        ));
    }

    // Parse parameter query
    let metric_type = params.get("metric")
        .map(|m| m.as_str())
        .unwrap_or("downloads");

    let limit = params.get("limit")
        .and_then(|l| l.parse::<u32>().ok())
        .unwrap_or(10)
        .min(50);

    // Validasi tipe metrik
    let valid_metrics = vec!["downloads", "sales", "revenue", "recent"];
    if !valid_metrics.contains(&metric_type) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                message: format!("Tipe metrik tidak valid. Opsi valid: {}", valid_metrics.join(", ")),
                error_code: Some("INVALID_METRIC_TYPE".to_string()),
            })
        ));
    }

    // Ambil data top books
    match BookRepository::get_top_books_by_metric(&state.db, metric_type, limit).await {
        Ok(top_books) => {
            tracing::info!("Admin top books berhasil diambil: {} books berdasarkan {}", 
                top_books.len(), metric_type);
            
            Ok(Json(AdminTopBooksResponse::success(top_books)))
        }
        Err(DatabaseError::InvalidMetricType) => {
            Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    success: false,
                    message: format!("Tipe metrik tidak valid: {}", metric_type),
                    error_code: Some("INVALID_METRIC_TYPE".to_string()),
                })
            ))
        }
        Err(e) => {
            tracing::error!("Gagal mengambil top books: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal mengambil top books: {}", e),
                    error_code: Some("TOP_BOOKS_ERROR".to_string()),
                })
            ))
        }
    }
}

// Handler untuk analytics penjualan
pub async fn get_sales_analytics(
    State(state): State<AppState>,
    Extension(user_role): Extension<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<AdminSalesAnalyticsResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validasi akses admin
    if user_role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                success: false,
                message: "Akses admin diperlukan".to_string(),
                error_code: Some("INSUFFICIENT_PRIVILEGES".to_string()),
            })
        ));
    }

    // Parse jumlah hari untuk analytics
    let days = params.get("days")
        .and_then(|d| d.parse::<u32>().ok())
        .unwrap_or(30)
        .min(365);

    // Ambil sales analytics
    match BookRepository::get_sales_analytics(&state.db, days).await {
        Ok(analytics) => {
            tracing::info!("Admin sales analytics berhasil diambil: {} hari data", days);
            
            Ok(Json(AdminSalesAnalyticsResponse::success(analytics)))
        }
        Err(e) => {
            tracing::error!("Gagal mengambil sales analytics: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal mengambil sales analytics: {}", e),
                    error_code: Some("ANALYTICS_ERROR".to_string()),
                })
            ))
        }
    }
}

// Handler untuk data chart popular books
pub async fn get_popular_books_chart_data(
    State(state): State<AppState>,
    Extension(user_role): Extension<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<AdminPopularBooksChartResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validasi akses admin
    if user_role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                success: false,
                message: "Akses admin diperlukan".to_string(),
                error_code: Some("INSUFFICIENT_PRIVILEGES".to_string()),
            })
        ));
    }

    // Parse limit untuk chart
    let limit = params.get("limit")
        .and_then(|l| l.parse::<u32>().ok())
        .unwrap_or(10)
        .min(15);

    // Ambil data chart
    match BookRepository::get_popular_books_chart_data(&state.db, limit).await {
        Ok(chart_data) => {
            tracing::info!("Popular books chart data berhasil diambil: {} books", 
                chart_data.labels.len());
            
            Ok(Json(AdminPopularBooksChartResponse::success(chart_data)))
        }
        Err(e) => {
            tracing::error!("Gagal mengambil popular books chart data: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal mengambil chart data: {}", e),
                    error_code: Some("CHART_DATA_ERROR".to_string()),
                })
            ))
        }
    }
}

// Handler untuk analytics per kategori
pub async fn get_category_analytics(
    State(state): State<AppState>,
    Extension(user_role): Extension<String>,
) -> Result<Json<AdminCategoryAnalyticsResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validasi akses admin
    if user_role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                success: false,
                message: "Akses admin diperlukan".to_string(),
                error_code: Some("INSUFFICIENT_PRIVILEGES".to_string()),
            })
        ));
    }

    // Ambil analytics kategori
    match BookRepository::get_category_analytics(&state.db).await {
        Ok(analytics) => {
            tracing::info!("Category analytics berhasil diambil: {} kategori", analytics.len());
            
            Ok(Json(AdminCategoryAnalyticsResponse::success(analytics)))
        }
        Err(e) => {
            tracing::error!("Gagal mengambil category analytics: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal mengambil category analytics: {}", e),
                    error_code: Some("CATEGORY_ANALYTICS_ERROR".to_string()),
                })
            ))
        }
    }
}

// Handler untuk dashboard metrics gabungan
pub async fn get_dashboard_metrics(
    State(state): State<AppState>,
    Extension(user_role): Extension<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Validasi akses admin
    if user_role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                success: false,
                message: "Akses admin diperlukan".to_string(),
                error_code: Some("INSUFFICIENT_PRIVILEGES".to_string()),
            })
        ));
    }

    // Ambil multiple metrics secara parallel untuk performa
    let (book_stats_result, top_books_result) = tokio::join!(
        BookRepository::get_admin_book_stats(&state.db),
        BookRepository::get_top_books_by_metric(&state.db, "downloads", 5)
    );

    // Handle hasil dengan fallback
    let book_stats = book_stats_result.map_err(|e| {
        tracing::warn!("Gagal mengambil book stats untuk dashboard: {}", e);
        e
    }).ok();

    let top_books = top_books_result.map_err(|e| {
        tracing::warn!("Gagal mengambil top books untuk dashboard: {}", e);
        e
    }).ok();

    // Build response dashboard
    let dashboard_data = serde_json::json!({
        "success": true,
        "message": "Dashboard metrics berhasil diambil",
        "data": {
            "book_stats": book_stats,
            "top_books": top_books,
            "last_updated": chrono::Utc::now(),
            "cache_duration": 300
        }
    });

    Ok(Json(dashboard_data))
}

/// Handler untuk mengambil aktivitas buku terbaru (Admin only)
pub async fn get_recent_activity(
    State(state): State<AppState>,
    Extension(user_role): Extension<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Validasi akses admin
    if user_role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                success: false,
                message: "Akses admin diperlukan".to_string(),
                error_code: Some("INSUFFICIENT_PRIVILEGES".to_string()),
            })
        ));
    }

    // Parse limit parameter
    let limit = params.get("limit")
        .and_then(|l| l.parse::<u32>().ok())
        .unwrap_or(20)
        .min(50);

    // Ambil recent activity dari database
    match BookRepository::get_recent_book_activity(&state.db, limit).await {
        Ok(activities) => {
            tracing::info!("Recent activity berhasil diambil: {} items", activities.len());
            
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "Aktivitas buku terbaru berhasil diambil",
                "data": activities,
                "count": activities.len()
            })))
        }
        Err(e) => {
            tracing::error!("Gagal mengambil recent activity: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    message: format!("Gagal mengambil aktivitas: {}", e),
                    error_code: Some("ACTIVITY_ERROR".to_string()),
                })
            ))
        }
    }
}