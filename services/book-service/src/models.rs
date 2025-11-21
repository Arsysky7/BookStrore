// /pdf-bookstore/services/book-service/src/models.rs

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use validator::Validate;
use bigdecimal::{BigDecimal, Zero};



// ===== ENTITY MODELS =====

/// Entity buku dari tabel books di database
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Book {
    pub id: Uuid,
    pub title: String,
    pub author: String,
    pub description: Option<String>,
    pub isbn: Option<String>,
    pub price: BigDecimal,
    pub pdf_path: Option<String>,
    pub cover_path: Option<String>,
    pub file_size_mb: Option<BigDecimal>,
    pub total_pages: Option<i32>,
    pub language: String,
    pub is_active: bool,
    pub download_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Entity kategori dari tabel categories
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Category {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// Buku dengan kategori untuk response lengkap
#[derive(Debug, Serialize)]
pub struct BookWithCategories {
    #[serde(flatten)]
    pub book: Book,
    pub categories: Vec<Category>,
}

// ===== REQUEST MODELS =====

/// Request untuk membuat buku baru
#[derive(Debug, Deserialize, Validate)]
pub struct CreateBookRequest {
    #[validate(length(min = 1, max = 500, message = "Title harus 1-500 karakter"))]
    pub title: String,
    #[validate(length(min = 1, max = 300, message = "Author harus 1-300 karakter"))]
    pub author: String,
    pub description: Option<Option<String>>,
    #[validate(length(max = 20, message = "ISBN maksimal 20 karakter"))]
    pub isbn: Option<Option<String>>,
    pub price: BigDecimal,
    #[validate(length(min = 2, max = 10, message = "Kode bahasa 2-10 karakter"))]
    pub language: Option<String>,
    pub category_ids: Vec<Uuid>,
    pub total_pages: Option<i32>,
}

/// Request untuk update buku yang sudah ada
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateBookRequest {
    #[validate(length(min = 1, max = 500, message = "Title harus 1-500 karakter"))]
    pub title: Option<String>,
    #[validate(length(min = 1, max = 300, message = "Author harus 1-300 karakter"))]
    pub author: Option<String>,
    pub description: Option<Option<String>>,
    #[validate(length(max = 20, message = "ISBN maksimal 20 karakter"))]
    pub isbn: Option<Option<String>>,
    pub price: Option<BigDecimal>,
    #[validate(length(min = 2, max = 10, message = "Kode bahasa 2-10 karakter"))]
    pub language: Option<String>,
    pub category_ids: Option<Vec<Uuid>>,
    pub is_active: Option<bool>,
    pub total_pages: Option<i32>,
}

/// Parameter query untuk pencarian dan filter buku
#[derive(Debug, Deserialize)]
pub struct BookQueryParams {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub search: Option<String>,
    pub category: Option<String>,
    pub author: Option<String>,         
    pub language: Option<String>,       
    pub min_price: Option<BigDecimal>,  
    pub max_price: Option<BigDecimal>,  
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

/// Metadata pagination untuk response list
#[derive(Debug, Serialize)]
pub struct PaginationMeta {
    pub current_page: u32,
    pub per_page: u32,
    pub total_items: i64,
    pub total_pages: u32,
    pub has_next: bool,
    pub has_prev: bool,
}

// ===== RESPONSE MODELS =====

/// Response untuk list buku dengan pagination
#[derive(Debug, Serialize)]
pub struct PaginatedBooksResponse {
    pub success: bool,
    pub message: String,
    pub data: Vec<BookWithCategories>,
    pub pagination: PaginationMeta,
}

/// Response untuk single buku
#[derive(Debug, Serialize)]
pub struct BookResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<BookWithCategories>,
}

/// Response untuk file upload
#[derive(Debug, Serialize)]
pub struct FileUploadResponse {
    pub success: bool,
    pub message: String,
    pub file_path: Option<String>,
    pub file_size_mb: Option<BigDecimal>,
}

/// Response untuk error
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub message: String,
    pub error_code: Option<String>,
}

// ===== REVIEW MODELS =====

/// Entity review buku dari database
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct BookReview {
    pub id: Uuid,
    pub book_id: Uuid,
    pub user_id: Uuid,
    pub rating: i32,
    pub comment: String,
    pub helpful_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Review dengan informasi user untuk response
#[derive(Debug, Serialize)]
pub struct BookReviewWithUser {
    pub id: Uuid,
    pub book_id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub user_email: String,
    pub rating: i32,
    pub comment: String,
    pub helpful_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub can_edit: bool,
    pub has_voted_helpful: bool,
}

/// Request untuk membuat review baru
#[derive(Debug, Deserialize, Validate)]
pub struct CreateReviewRequest {
    #[validate(range(min = 1, max = 5, message = "Rating harus antara 1-5"))]
    pub rating: i32,
    
    #[validate(length(min = 10, max = 1000, message = "Comment harus 10-1000 karakter"))]
    pub comment: String,
}

/// Statistik review untuk buku
#[derive(Debug, Serialize)]
pub struct ReviewStats {
    pub total_reviews: i64,
    pub average_rating: f64,
    pub rating_distribution: RatingDistribution,
}

/// Distribusi rating 1-5
#[derive(Debug, Serialize)]
pub struct RatingDistribution {
    pub five_star: i64,
    pub four_star: i64,
    pub three_star: i64,
    pub two_star: i64,
    pub one_star: i64,
}

/// Response wrapper untuk list reviews
#[derive(Debug, Serialize)]
pub struct BookReviewsResponse {
    pub success: bool,
    pub message: String,
    pub data: Vec<BookReviewWithUser>,
    pub stats: ReviewStats,
}

/// Response wrapper untuk single review
#[derive(Debug, Serialize)]
pub struct ReviewResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<BookReviewWithUser>,
}

// ===== LIBRARY & PURCHASED BOOKS MODELS =====

/// Buku yang sudah dibeli user dengan purchase info
#[derive(Debug, Serialize)]
pub struct PurchasedBook {
    #[serde(flatten)]
    pub book: Book,
    pub purchased_at: DateTime<Utc>,
    pub download_count: i32,
    pub last_downloaded_at: Option<DateTime<Utc>>,
    pub categories: Vec<Category>,
}

/// Response untuk library books
#[derive(Debug, Serialize)]
pub struct LibraryBooksResponse {
    pub success: bool,
    pub message: String,
    pub data: Vec<PurchasedBook>,
    pub total_books: i64,
}

// ===== RELATED BOOKS MODELS =====

/// Response untuk related books
#[derive(Debug, Serialize)]
pub struct RelatedBooksResponse {
    pub success: bool,
    pub message: String,
    pub data: Vec<BookWithCategories>,
    pub relation_type: String,
}

// ===== PREVIEW MODELS =====

/// Data preview buku
#[derive(Debug, Serialize)]
pub struct BookPreviewData {
    pub book_id: Uuid,
    pub title: String,
    pub preview_url: Option<String>,
    pub preview_pages: i32,
    pub has_preview: bool,
    pub total_pages: Option<i32>,
}

/// Response untuk book preview
#[derive(Debug, Serialize)]
pub struct BookPreviewResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<BookPreviewData>,
}


// ===== ADMIN ANALYTICS MODELS =====

/// Statistik buku untuk admin dashboard
#[derive(Debug, Serialize)]
pub struct AdminBookStats {
    pub total_books: i64,
    pub active_books: i64,
    pub inactive_books: i64,
    pub books_with_pdf: i64,
    pub books_with_cover: i64,
    pub total_downloads: i64,
    pub new_books_this_month: i64,
    pub avg_price: Option<BigDecimal>,
    pub total_file_size_mb: Option<BigDecimal>,
    pub books_by_language: Vec<LanguageStats>,
    pub monthly_growth_percentage: f64,
}

/// Statistik distribusi bahasa
#[derive(Debug, Serialize)]
pub struct LanguageStats {
    pub language: String,
    pub book_count: i64,
    pub percentage: f64,
}

/// Top buku berdasarkan metric
#[derive(Debug, Serialize)]
pub struct TopBook {
    pub id: Uuid,
    pub title: String,
    pub author: String,
    pub cover_path: Option<String>,
    pub download_count: i32,
    pub price: BigDecimal,
    pub created_at: DateTime<Utc>,
    pub metric_value: i64,
    pub metric_type: String,
}

/// Data analytics penjualan
#[derive(Debug, Serialize)]
pub struct SalesAnalytics {
    pub date: String,
    pub sales_count: i64,
    pub revenue: BigDecimal,
    pub books_sold: i64,
}

/// Data chart buku populer
#[derive(Debug, Serialize)]
pub struct PopularBooksChart {
    pub labels: Vec<String>,
    pub data: Vec<i64>,
    pub colors: Vec<String>,
}

/// Analytics per kategori
#[derive(Debug, Serialize)]
pub struct CategoryAnalytics {
    pub category_name: String,
    pub category_slug: String,
    pub book_count: i64,
    pub total_downloads: i64,
    pub total_revenue: BigDecimal,
    pub avg_price: Option<BigDecimal>,
}

// ===== ADMIN RESPONSE WRAPPERS =====

#[derive(Debug, Serialize)]
pub struct AdminBookStatsResponse {
    pub success: bool,
    pub message: String,
    pub data: AdminBookStats,
}

#[derive(Debug, Serialize)]
pub struct AdminTopBooksResponse {
    pub success: bool,
    pub message: String,
    pub data: Vec<TopBook>,
}

#[derive(Debug, Serialize)]
pub struct AdminSalesAnalyticsResponse {
    pub success: bool,
    pub message: String,
    pub data: Vec<SalesAnalytics>,
}

#[derive(Debug, Serialize)]
pub struct AdminPopularBooksChartResponse {
    pub success: bool,
    pub message: String,
    pub data: PopularBooksChart,
}

#[derive(Debug, Serialize)]
pub struct AdminCategoryAnalyticsResponse {
    pub success: bool,
    pub message: String,
    pub data: Vec<CategoryAnalytics>,
}

// ===== IMPLEMENTATIONS =====

impl Default for BookQueryParams {
    fn default() -> Self {
        Self {
            page: Some(1),
            limit: Some(12),
            search: None,
            category: None,
            author: None,
            language: None,
            min_price: None,
            max_price: None,
            sort_by: Some("created_at".to_string()),
            sort_order: Some("desc".to_string()),
        }
    }
}

impl PaginationMeta {
    /// Membuat metadata pagination dari hasil query
    pub fn new(current_page: u32, per_page: u32, total_items: i64) -> Self {
        let total_pages = ((total_items as f64) / (per_page as f64)).ceil() as u32;
        
        Self {
            current_page,
            per_page,
            total_items,
            total_pages: if total_pages == 0 { 1 } else { total_pages },
            has_next: current_page < total_pages,
            has_prev: current_page > 1,
        }
    }
}

impl CreateBookRequest {
    /// Validasi custom untuk business rules
    pub fn validate_business_rules(&self) -> Result<(), String> {
        // Validasi price range
        if self.price.is_zero() || self.price < Zero::zero() {
            return Err("Harga harus lebih dari 0".to_string());
        }
        
        if self.price > BigDecimal::from(10_000_000) {
            return Err("Harga maksimal 10 juta".to_string());
        }
        
        // Validasi ISBN format jika ada
        if let Some(Some(ref isbn)) = &self.isbn {
            let cleaned = isbn.replace("-", "").replace(" ", "");
            if cleaned.len() != 10 && cleaned.len() != 13 {
                return Err("ISBN harus 10 atau 13 digit".to_string());
            }
        }
        
        // Validasi category_ids tidak kosong
        if self.category_ids.is_empty() {
            return Err("Minimal pilih satu kategori".to_string());
        }
        
        Ok(())
    }
}

impl PaginatedBooksResponse {
    /// Helper untuk membuat response sukses
    pub fn success(data: Vec<BookWithCategories>, pagination: PaginationMeta) -> Self {
        Self {
            success: true,
            message: "Buku berhasil diambil".to_string(),
            data,
            pagination,
        }
    }
}

impl BookResponse {
    /// Helper untuk membuat response sukses dengan data buku
    pub fn success(book: BookWithCategories) -> Self {
        Self {
            success: true,
            message: "Buku berhasil diambil".to_string(),
            data: Some(book),
        }
    }

    /// Helper untuk membuat response error
    pub fn error(message: &str) -> Self {
        Self {
            success: false,
            message: message.to_string(),
            data: None,
        }
    }
}

impl FileUploadResponse {
    /// Helper untuk membuat response upload sukses
    pub fn success(file_path: String, file_size_mb: Option<BigDecimal>) -> Self {
        Self {
            success: true,
            message: "File berhasil diupload".to_string(),
            file_path: Some(file_path),
            file_size_mb,
        }
    }
}

impl BookReviewsResponse {
    /// Helper untuk membuat response reviews sukses
    pub fn success(reviews: Vec<BookReviewWithUser>, stats: ReviewStats) -> Self {
        Self {
            success: true,
            message: "Reviews berhasil diambil".to_string(),
            data: reviews,
            stats,
        }
    }
}

impl ReviewResponse {
    /// Helper untuk membuat response review sukses
    pub fn success(review: BookReviewWithUser) -> Self {
        Self {
            success: true,
            message: "Review berhasil ditambahkan".to_string(),
            data: Some(review),
        }
    }
}

impl LibraryBooksResponse {
    /// Helper untuk membuat response library sukses
    pub fn success(books: Vec<PurchasedBook>, total: i64) -> Self {
        Self {
            success: true,
            message: "Library berhasil diambil".to_string(),
            data: books,
            total_books: total,
        }
    }
}

impl RelatedBooksResponse {
    /// Helper untuk membuat response related books sukses
    pub fn success(books: Vec<BookWithCategories>, relation_type: String) -> Self {
        Self {
            success: true,
            message: "Related books berhasil diambil".to_string(),
            data: books,
            relation_type,
        }
    }
}

impl BookPreviewResponse {
    /// Helper untuk membuat response preview sukses
    pub fn success(data: BookPreviewData) -> Self {
        Self {
            success: true,
            message: "Preview data berhasil diambil".to_string(),
            data: Some(data),
        }
    }
    
    /// Helper untuk membuat response preview tidak tersedia
    pub fn not_available() -> Self {
        Self {
            success: false,
            message: "Preview tidak tersedia untuk buku ini".to_string(),
            data: None,
        }
    }
}

impl AdminBookStatsResponse {
    /// Helper untuk membuat response statistik admin
    pub fn success(stats: AdminBookStats) -> Self {
        Self {
            success: true,
            message: "Statistik buku berhasil diambil".to_string(),
            data: stats,
        }
    }
}

impl AdminTopBooksResponse {
    /// Helper untuk membuat response top books
    pub fn success(books: Vec<TopBook>) -> Self {
        Self {
            success: true,
            message: "Top buku berhasil diambil".to_string(),
            data: books,
        }
    }
}

impl AdminSalesAnalyticsResponse {
    /// Helper untuk membuat response sales analytics
    pub fn success(analytics: Vec<SalesAnalytics>) -> Self {
        Self {
            success: true,
            message: "Analytics penjualan berhasil diambil".to_string(),
            data: analytics,
        }
    }
}

impl AdminPopularBooksChartResponse {
    /// Helper untuk membuat response popular books chart
    pub fn success(chart_data: PopularBooksChart) -> Self {
        Self {
            success: true,
            message: "Data chart buku populer berhasil diambil".to_string(),
            data: chart_data,
        }
    }
}

impl AdminCategoryAnalyticsResponse {
    /// Helper untuk membuat response category analytics
    pub fn success(analytics: Vec<CategoryAnalytics>) -> Self {
        Self {
            success: true,
            message: "Analytics kategori berhasil diambil".to_string(),
            data: analytics,
        }
    }
}