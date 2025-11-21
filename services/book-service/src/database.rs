// /pdf-bookstore/services/book-service/src/database.rs

use crate::models::*;

use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use uuid::Uuid;
use bigdecimal::BigDecimal;
use thiserror::Error;
use chrono::{Utc, Datelike};
use std::collections::{HashMap, HashSet};

// ===== ERROR HANDLING =====
#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Database connection error")]
    Connection(#[from] sqlx::Error),
    #[error("Book not found")]
    BookNotFound,
    #[error("Category not found")]
    CategoryNotFound,
    #[error("ISBN already exists")]
    IsbnExists,
    #[error("Invalid query parameters")]
    InvalidQuery,
    #[error("Invalid metric type")]
    InvalidMetricType,
    #[error("Concurrent modification detected")]
    ConcurrentModificationError,
}

// ===== FUNGSI HELPER =====

/// Membersihkan input pencarian dari karakter berbahaya
/// Mencegah SQL injection dengan escape karakter khusus
fn sanitize_search_input(input: &str) -> String {
    input
        .trim()
        .replace("'", "''")
        .replace("\\", "\\\\")
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || ".,!?-".contains(*c))
        .collect::<String>()
        .truncate_to(255)
}

/// Membersihkan nama file dari karakter yang tidak aman
/// Hanya mengizinkan alphanumeric dan beberapa karakter aman
fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .collect::<String>()
        .truncate_to(100)
}

/// Trait untuk memotong string dengan panjang maksimal
trait StringTruncate {
    fn truncate_to(self, max_len: usize) -> String;
}

impl StringTruncate for String {
    fn truncate_to(mut self, max_len: usize) -> String {
        if self.len() > max_len {
            self.truncate(max_len);
        }
        self
    }
}

// ===== REPOSITORY PATTERN =====
pub struct BookRepository;

impl BookRepository {
    /// Mengambil buku dengan kategori berdasarkan ID
    /// Menggabungkan data dari tabel books, book_categories, dan categories
    async fn fetch_books_with_categories(
        pool: &PgPool,
        book_ids: Vec<Uuid>,
    ) -> Result<Vec<BookWithCategories>, DatabaseError> {
        let rows = sqlx::query!(
            r#"
            SELECT 
                b.id, b.title, b.author, b.description, b.isbn, b.price, 
                b.pdf_path, b.cover_path, b.file_size_mb, b.total_pages, 
                b.language as "language!", 
                b.is_active as "is_active!", 
                b.download_count as "download_count!", 
                b.created_at as "created_at!", 
                b.updated_at as "updated_at!",
                c.id as "category_id?", 
                c.name as "category_name?", 
                c.slug as "category_slug?", 
                c.description as "category_description?", 
                c.is_active as "category_active?", 
                c.created_at as "category_created_at?"
            FROM books b
            LEFT JOIN book_categories bc ON b.id = bc.book_id
            LEFT JOIN categories c ON bc.category_id = c.id AND c.is_active = true
            WHERE b.id = ANY($1)
            ORDER BY ARRAY_POSITION($1, b.id)
            "#,
            &book_ids
        )
        .fetch_all(pool)
        .await?;

        // Grouping hasil query berdasarkan book ID
        let mut books_map: HashMap<Uuid, BookWithCategories> = HashMap::new();

        for row in rows {
            let book_entry = books_map.entry(row.id).or_insert_with(|| {
                BookWithCategories {
                    book: Book {
                        id: row.id,
                        title: row.title.clone(),
                        author: row.author.clone(),
                        description: row.description.clone(),
                        isbn: row.isbn.clone(),
                        price: row.price.clone(),
                        pdf_path: row.pdf_path.clone(),
                        cover_path: row.cover_path.clone(),
                        file_size_mb: row.file_size_mb.clone(),
                        total_pages: row.total_pages,
                        language: row.language.clone(),
                        is_active: row.is_active,
                        download_count: row.download_count,
                        created_at: row.created_at,
                        updated_at: row.updated_at,
                    },
                    categories: Vec::new(),
                }
            });
        
            // Tambahkan kategori jika ada
            if let Some(category_id) = row.category_id {
                book_entry.categories.push(Category {
                    id: category_id,
                    name: row.category_name.unwrap_or_default(),
                    slug: row.category_slug.unwrap_or_default(),
                    description: row.category_description,
                    is_active: row.category_active.unwrap_or(true),
                    created_at: row.category_created_at.unwrap_or(Utc::now()),
                });
            }
        }

        // Konversi ke Vec dengan urutan sesuai input
        let mut result = Vec::new();
        for book_id in book_ids {
            if let Some(book) = books_map.remove(&book_id) {
                result.push(book);
            }
        }

        Ok(result)
    }

    /// Membuat buku baru dengan validasi lengkap
    /// Menggunakan transaction untuk atomicity
    pub async fn create_book(
        pool: &PgPool,
        request: CreateBookRequest,
        pdf_path: Option<String>,
        cover_path: Option<String>,
        file_size_mb: Option<BigDecimal>,
    ) -> Result<Book, DatabaseError> {
        let mut tx = pool.begin().await?;

        // Validasi ISBN unik jika ada
        if let Some(ref isbn_option) = request.isbn {
            if let Some(ref isbn_value) = isbn_option {
                let existing = sqlx::query!(
                    "SELECT COUNT(*) as count FROM books WHERE isbn = $1", 
                    isbn_value.trim()
                )
                .fetch_one(&mut *tx)
                .await?;

                if existing.count.unwrap_or(0) > 0 {
                    tx.rollback().await?;
                    return Err(DatabaseError::IsbnExists);
                }
            }
        }

        // Validasi kategori exists dan aktif
        if !request.category_ids.is_empty() {
            let category_count = sqlx::query!(
                "SELECT COUNT(*) as count FROM categories WHERE id = ANY($1) AND is_active = true",
                &request.category_ids
            )
            .fetch_one(&mut *tx)
            .await?;

            if category_count.count.unwrap_or(0) != request.category_ids.len() as i64 {
                tx.rollback().await?;
                return Err(DatabaseError::CategoryNotFound);
            }
        }

        // Validasi dan sanitasi path file
        let validated_pdf_path = pdf_path.as_ref().map(|path| {
            if path.starts_with("/storage/books/") && path.ends_with(".pdf") {
                path.clone()
            } else {
                format!("/storage/books/{}", sanitize_filename(path))
            }
        });

        let validated_cover_path = cover_path.as_ref().map(|path| {
            if path.starts_with("/storage/covers/") {
                path.clone()
            } else {
                format!("/storage/covers/{}", sanitize_filename(path))
            }
        });

        // Insert buku baru ke database
        let book_row = sqlx::query!(
            r#"
            INSERT INTO books (title, author, description, isbn, price, pdf_path, 
                            cover_path, file_size_mb, total_pages, language)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING 
                id, title, author, description, isbn, price, pdf_path, cover_path,
                file_size_mb, total_pages, language as "language!", 
                is_active as "is_active!", download_count as "download_count!",
                created_at as "created_at!", updated_at as "updated_at!"
            "#,
            request.title.trim(),
            request.author.trim(),
            request.description.as_ref().and_then(|opt| opt.as_ref()).map(|s| s.trim()),
            request.isbn.as_ref().and_then(|opt| opt.as_ref()).map(|s| s.trim()),
            request.price,
            validated_pdf_path,
            validated_cover_path,
            file_size_mb,
            request.total_pages,
            request.language.unwrap_or_else(|| "id".to_string())
        )
        .fetch_one(&mut *tx)
        .await?;

        let book = Book {
            id: book_row.id,
            title: book_row.title,
            author: book_row.author,
            description: book_row.description,
            isbn: book_row.isbn,
            price: book_row.price,
            pdf_path: book_row.pdf_path,
            cover_path: book_row.cover_path,
            file_size_mb: book_row.file_size_mb,
            total_pages: book_row.total_pages,
            language: book_row.language,
            is_active: book_row.is_active,
            download_count: book_row.download_count,
            created_at: book_row.created_at,
            updated_at: book_row.updated_at,
        };

        // Insert relasi buku-kategori
        if !request.category_ids.is_empty() {
            let mut query_builder = QueryBuilder::new(
                "INSERT INTO book_categories (book_id, category_id) "
            );
            
            query_builder.push_values(request.category_ids.iter(), |mut b, category_id| {
                b.push_bind(book.id).push_bind(category_id);
            });

            query_builder.build().execute(&mut *tx).await?;
        }

        // Insert inventory tracking (untuk digital product unlimited)
        sqlx::query!(
            "INSERT INTO book_inventory (book_id, stock_quantity) VALUES ($1, $2)",
            book.id,
            999999
        )
        .execute(&mut *tx)
        .await?;

        // Log audit trail untuk tracking
        sqlx::query!(
            r#"
            INSERT INTO audit_logs (action, resource_type, resource_id, details)
            VALUES ('BOOK_CREATED', 'book', $1, $2)
            "#,
            book.id,
            serde_json::json!({
                "title": book.title,
                "author": book.author,
                "price": book.price.to_string()
            })
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(book)
    }

    /// Mengambil detail buku berdasarkan ID
    /// Include semua kategori yang terkait
    pub async fn get_book_by_id(
        pool: &PgPool,
        book_id: Uuid,
    ) -> Result<BookWithCategories, DatabaseError> {
        let rows = sqlx::query!(
            r#"
            SELECT 
                b.id, b.title, b.author, b.description, b.isbn, b.price, 
                b.pdf_path, b.cover_path, b.file_size_mb, b.total_pages, 
                b.language as "language!", 
                b.is_active as "is_active!", 
                b.download_count as "download_count!", 
                b.created_at as "created_at!", 
                b.updated_at as "updated_at!",
                c.id as "category_id?", 
                c.name as "category_name?", 
                c.slug as "category_slug?", 
                c.description as category_description, 
                c.is_active as "category_active?", 
                c.created_at as category_created_at
            FROM books b
            LEFT JOIN book_categories bc ON b.id = bc.book_id
            LEFT JOIN categories c ON bc.category_id = c.id AND c.is_active = true
            WHERE b.id = $1 AND b.is_active = true
            "#,
            book_id
        )
        .fetch_all(pool)
        .await?;

        if rows.is_empty() {
            return Err(DatabaseError::BookNotFound);
        }

        // Build book object dari row pertama
        let first_row = &rows[0];
        let book = Book {
            id: first_row.id,
            title: first_row.title.clone(),
            author: first_row.author.clone(),
            description: first_row.description.clone(),
            isbn: first_row.isbn.clone(),
            price: first_row.price.clone(),
            pdf_path: first_row.pdf_path.clone(),
            cover_path: first_row.cover_path.clone(),
            file_size_mb: first_row.file_size_mb.clone(),
            total_pages: first_row.total_pages,
            language: first_row.language.clone(),
            is_active: first_row.is_active,
            download_count: first_row.download_count,
            created_at: first_row.created_at,
            updated_at: first_row.updated_at,
        };

        // Collect categories dari semua rows
        let mut categories = Vec::new();
        for row in rows {
            if let Some(category_id) = row.category_id {
                categories.push(Category {
                    id: category_id,
                    name: row.category_name.unwrap_or_default(),
                    slug: row.category_slug.unwrap_or_default(),
                    description: row.category_description,
                    is_active: row.category_active.unwrap_or(true),
                    created_at: row.category_created_at.unwrap_or(Utc::now()),
                });
            }
        }

        // Remove duplicate categories
        categories.sort_by(|a, b| a.id.cmp(&b.id));
        categories.dedup_by(|a, b| a.id == b.id);

        Ok(BookWithCategories { book, categories })
    }

    /// Pencarian buku dengan filter lengkap dan pagination
    /// Mendukung full-text search, filter kategori, author, language, dan price range
    pub async fn search_books(
        pool: &PgPool,
        params: BookQueryParams,
    ) -> Result<(Vec<BookWithCategories>, PaginationMeta), DatabaseError> {
        let page = params.page.unwrap_or(1);
        let limit = params.limit.unwrap_or(12);
        
        // Validasi parameter pagination
        if page == 0 || limit == 0 || limit > 100 {
            return Err(DatabaseError::InvalidQuery);
        }
        
        let offset = (page - 1) * limit;
            
        // Build COUNT query untuk total items
        let mut count_builder = QueryBuilder::<Postgres>::new(
            "SELECT COUNT(DISTINCT b.id) as total
            FROM books b 
            LEFT JOIN book_categories bc ON b.id = bc.book_id 
            LEFT JOIN categories c ON bc.category_id = c.id AND c.is_active = true 
            WHERE b.is_active = true"
        );
        
        // Build MAIN query untuk fetch book IDs
        let mut query_builder = QueryBuilder::<Postgres>::new(
            "SELECT DISTINCT b.id 
            FROM books b 
            LEFT JOIN book_categories bc ON b.id = bc.book_id 
            LEFT JOIN categories c ON bc.category_id = c.id AND c.is_active = true 
            WHERE b.is_active = true"
        );
        
        // Search dengan sanitization
        if let Some(search) = &params.search {
            let sanitized = sanitize_search_input(search);
            if !sanitized.is_empty() {
                let condition = " AND to_tsvector('indonesian', b.title || ' ' || b.author || ' ' || COALESCE(b.description, '')) @@ plainto_tsquery('indonesian', ";
                
                count_builder.push(condition);
                count_builder.push_bind(sanitized.clone());
                count_builder.push(")");
                
                query_builder.push(condition);
                query_builder.push_bind(sanitized);
                query_builder.push(")");
            }
        }
        
        // Filter category
        if let Some(category) = &params.category {
            let sanitized = category.trim();
            if !sanitized.is_empty() && sanitized.len() <= 100 {
                count_builder.push(" AND c.slug = ");
                count_builder.push_bind(sanitized);
                
                query_builder.push(" AND c.slug = ");
                query_builder.push_bind(sanitized);
            }
        }
        
        // Filter author
        if let Some(author) = &params.author {
            let sanitized = sanitize_search_input(author);
            if !sanitized.is_empty() {
                count_builder.push(" AND LOWER(b.author) LIKE LOWER(");
                count_builder.push_bind(format!("%{}%", sanitized));
                count_builder.push(")");
                
                query_builder.push(" AND LOWER(b.author) LIKE LOWER(");
                query_builder.push_bind(format!("%{}%", sanitized));
                query_builder.push(")");
            }
        }
        
        // Filter language
        if let Some(language) = &params.language {
            let lang = language.trim();
            if !lang.is_empty() && lang.len() <= 10 {
                count_builder.push(" AND b.language = ");
                count_builder.push_bind(lang);
                
                query_builder.push(" AND b.language = ");
                query_builder.push_bind(lang);
            }
        }
        
        // Price range
        if let Some(min_price) = &params.min_price {
            if *min_price >= BigDecimal::from(0) {
                count_builder.push(" AND b.price >= ");
                count_builder.push_bind(min_price);
                
                query_builder.push(" AND b.price >= ");
                query_builder.push_bind(min_price);
            }
        }
        
        if let Some(max_price) = &params.max_price {
            if *max_price <= BigDecimal::from(10000000) {
                count_builder.push(" AND b.price <= ");
                count_builder.push_bind(max_price);
                
                query_builder.push(" AND b.price <= ");
                query_builder.push_bind(max_price);
            }
        }
        
        // Execute count query - FIX: use build() instead of bind_values()
        let count_row = count_builder.build()
            .fetch_one(pool)
            .await?;
        let total_items: i64 = count_row.get("total");
            
        // Sorting
        let sort_column = match params.sort_by.as_deref() {
            Some("title") => "b.title",
            Some("author") => "b.author",
            Some("price") => "b.price",
            Some("created_at") | _ => "b.created_at",
        };
        
        let sort_direction = match params.sort_order.as_deref() {
            Some("asc") => "ASC",
            _ => "DESC",
        };
        
        query_builder.push(" ORDER BY ");
        query_builder.push(sort_column);
        query_builder.push(" ");
        query_builder.push(sort_direction);
        
        query_builder.push(" LIMIT ");
        query_builder.push_bind(limit as i64);
        query_builder.push(" OFFSET ");
        query_builder.push_bind(offset as i64);
        
        // Execute main query - FIX: use build()
        let rows = query_builder.build()
            .fetch_all(pool)
            .await?;
            
        let book_ids: Vec<Uuid> = rows.into_iter().map(|row| row.get("id")).collect();
    
        // Fetch complete data
        let books_with_categories = if !book_ids.is_empty() {
            Self::fetch_books_with_categories(pool, book_ids).await?
        } else {
            Vec::new()
        };
        
        let pagination = PaginationMeta::new(page, limit, total_items);
        Ok((books_with_categories, pagination))
    }


    /// Update buku dengan optimistic locking
    /// Menggunakan transaction untuk memastikan konsistensi
    pub async fn update_book(
        pool: &PgPool,
        book_id: Uuid,
        request: UpdateBookRequest,
        pdf_path: Option<String>,
        cover_path: Option<String>,
        file_size_mb: Option<BigDecimal>,
    ) -> Result<(), DatabaseError> {
        let mut tx = pool.begin().await?;

        // Lock row untuk update
        let current_book = sqlx::query!(
            "SELECT updated_at FROM books WHERE id = $1 AND is_active = true FOR UPDATE",
            book_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        if current_book.is_none() {
            tx.rollback().await?;
            return Err(DatabaseError::BookNotFound);
        }

        // Validasi ISBN unik jika diupdate
        if let Some(isbn_option) = &request.isbn {
            if let Some(isbn_value) = isbn_option {
                let existing = sqlx::query!(
                    "SELECT COUNT(*) as count FROM books WHERE isbn = $1 AND id != $2",
                    isbn_value.trim(),
                    book_id
                )
                .fetch_one(&mut *tx)
                .await?;
                
                if existing.count.unwrap_or(0) > 0 {
                    tx.rollback().await?;
                    return Err(DatabaseError::IsbnExists);
                }
            }
        }

        // Build update query dinamis
        let mut query_builder = QueryBuilder::new("UPDATE books SET ");
        let mut separated = query_builder.separated(", ");
        let mut has_updates = false;
        
        if let Some(title) = &request.title {
            separated.push("title = ");
            separated.push_bind_unseparated(title.trim());
            has_updates = true;
        }
        
        if let Some(author) = &request.author {
            separated.push("author = ");
            separated.push_bind_unseparated(author.trim());
            has_updates = true;
        }
        
        if let Some(description_option) = &request.description {
            separated.push("description = ");
            if let Some(desc) = description_option {
                separated.push_bind_unseparated(desc.trim());
            } else {
                separated.push_bind_unseparated(None::<String>);
            }
            has_updates = true;
        }
        
        if let Some(isbn_option) = &request.isbn {
            separated.push("isbn = ");
            if let Some(isbn_val) = isbn_option {
                separated.push_bind_unseparated(isbn_val.trim());
            } else {
                separated.push_bind_unseparated(None::<String>);
            }
            has_updates = true;
        }
        
        if let Some(price) = &request.price {
            separated.push("price = ");
            separated.push_bind_unseparated(price);
            has_updates = true;
        }
        
        if let Some(language) = &request.language {
            separated.push("language = ");
            separated.push_bind_unseparated(language);
            has_updates = true;
        }
        
        if let Some(is_active) = &request.is_active {
            separated.push("is_active = ");
            separated.push_bind_unseparated(is_active);
            has_updates = true;
        }
        
        if let Some(total_pages) = &request.total_pages {
            separated.push("total_pages = ");
            separated.push_bind_unseparated(total_pages);
            has_updates = true;
        }
        
        if let Some(path) = &pdf_path {
            separated.push("pdf_path = ");
            separated.push_bind_unseparated(path);
            has_updates = true;
        }
        
        if let Some(path) = &cover_path {
            separated.push("cover_path = ");
            separated.push_bind_unseparated(path);
            has_updates = true;
        }
        
        if let Some(size) = &file_size_mb {
            separated.push("file_size_mb = ");
            separated.push_bind_unseparated(size);
            has_updates = true;
        }
        
        // Selalu update timestamp
        separated.push("updated_at = NOW()");
        
        if has_updates {
            query_builder.push(" WHERE id = ");
            query_builder.push_bind(book_id);
            
            let result = query_builder.build().execute(&mut *tx).await;

            match result {
                Ok(query_result) => {
                    if query_result.rows_affected() == 0 {
                        tx.rollback().await?;
                        return Err(DatabaseError::ConcurrentModificationError);
                    }
                }
                Err(e) => {
                    tx.rollback().await?;
                    tracing::error!("Failed to update book {}: {}", book_id, e);
                    return Err(DatabaseError::Connection(e));
                }
            }
        }

        // Update kategori jika ada perubahan
        if let Some(category_ids) = &request.category_ids {
            // Hapus kategori lama
            sqlx::query!("DELETE FROM book_categories WHERE book_id = $1", book_id)
                .execute(&mut *tx)
                .await?;

            // Insert kategori baru
            if !category_ids.is_empty() {
                // Validasi semua kategori ada dan aktif
                let category_count = sqlx::query!(
                    "SELECT COUNT(*) as count FROM categories WHERE id = ANY($1) AND is_active = true",
                    category_ids
                )
                .fetch_one(&mut *tx)
                .await?;

                if category_count.count.unwrap_or(0) != category_ids.len() as i64 {
                    tx.rollback().await?;
                    return Err(DatabaseError::CategoryNotFound);
                }

                let mut query_builder = QueryBuilder::new(
                    "INSERT INTO book_categories (book_id, category_id) "
                );
                
                query_builder.push_values(category_ids.iter(), |mut b, category_id| {
                    b.push_bind(book_id).push_bind(category_id);
                });

                query_builder.build().execute(&mut *tx).await?;
            }
        }

        // Log audit trail
        sqlx::query!(
            r#"
            INSERT INTO audit_logs (action, resource_type, resource_id, details)
            VALUES ('BOOK_UPDATED', 'book', $1, $2)
            "#,
            book_id,
            serde_json::json!({
                "timestamp": Utc::now()
            })
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Soft delete buku (tidak menghapus data fisik)
    /// Hanya mengubah status is_active menjadi false
    pub async fn delete_book(
        pool: &PgPool,
        book_id: Uuid,
    ) -> Result<(), DatabaseError> {
        let mut tx = pool.begin().await?;

        // Ambil info buku sebelum dihapus untuk audit
        let book_info = sqlx::query!(
            "SELECT title, author FROM books WHERE id = $1 AND is_active = true",
            book_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        if book_info.is_none() {
            tx.rollback().await?;
            return Err(DatabaseError::BookNotFound);
        }

        let book = book_info.unwrap();

        // Soft delete dengan update is_active
        let rows_affected = sqlx::query!(
            "UPDATE books SET is_active = false, updated_at = NOW() WHERE id = $1 AND is_active = true",
            book_id
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            tx.rollback().await?;
            return Err(DatabaseError::BookNotFound);
        }

        // Log audit trail untuk tracking
        sqlx::query!(
            r#"
            INSERT INTO audit_logs (action, resource_type, resource_id, details)
            VALUES ('BOOK_DELETED', 'book', $1, $2)
            "#,
            book_id,
            serde_json::json!({
                "title": book.title,
                "author": book.author,
                "deleted_at": Utc::now()
            })
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Increment download counter untuk tracking popularitas
    pub async fn increment_download_count(
        pool: &PgPool,
        book_id: Uuid,
    ) -> Result<(), DatabaseError> {
        sqlx::query!(
            "UPDATE books SET download_count = download_count + 1, updated_at = NOW() WHERE id = $1",
            book_id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Mengambil semua kategori yang aktif
    pub async fn get_all_categories(
        pool: &PgPool,
    ) -> Result<Vec<Category>, DatabaseError> {
        let categories = sqlx::query!(
            r#"
            SELECT 
                id, 
                name, 
                slug, 
                description, 
                is_active as "is_active!", 
                created_at as "created_at!"
            FROM categories 
            WHERE is_active = true 
            ORDER BY name
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(categories.into_iter().map(|row| Category {
            id: row.id,
            name: row.name,
            slug: row.slug,
            description: row.description,
            is_active: row.is_active,
            created_at: row.created_at,
        }).collect())
    }

    pub async fn get_user_library(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Vec<PurchasedBook>, DatabaseError> {
        let rows = sqlx::query!(
            r#"
            SELECT 
                b.id, b.title, b.author, b.description, b.isbn, b.price,
                b.pdf_path, b.cover_path, b.file_size_mb, b.total_pages,
                b.language as "language!", b.is_active as "is_active!",
                b.download_count as "download_count!",
                b.created_at as "created_at!", b.updated_at as "updated_at!",
                up.purchased_at as "purchase_date!",
                up.download_count as "user_download_count!",
                up.last_downloaded_at,
                c.id as "category_id?",
                c.name as "category_name?",
                c.slug as "category_slug?",
                c.description as category_description,
                c.is_active as "category_active?",
                c.created_at as category_created_at
            FROM user_purchases up
            INNER JOIN books b ON up.book_id = b.id
            LEFT JOIN book_categories bc ON b.id = bc.book_id
            LEFT JOIN categories c ON bc.category_id = c.id AND c.is_active = true
            WHERE up.user_id = $1 AND b.is_active = true
            ORDER BY up.purchased_at DESC
            "#,
            user_id
        )
        .fetch_all(pool)
        .await?;

        // Group by book ID
        let mut books_map: HashMap<Uuid, PurchasedBook> = HashMap::new();

        for row in rows {
            let book_entry = books_map.entry(row.id).or_insert_with(|| {
                PurchasedBook {
                    book: Book {
                        id: row.id,
                        title: row.title.clone(),
                        author: row.author.clone(),
                        description: row.description.clone(),
                        isbn: row.isbn.clone(),
                        price: row.price.clone(),
                        pdf_path: row.pdf_path.clone(),
                        cover_path: row.cover_path.clone(),
                        file_size_mb: row.file_size_mb.clone(),
                        total_pages: row.total_pages,
                        language: row.language.clone(),
                        is_active: row.is_active,
                        download_count: row.download_count,
                        created_at: row.created_at,
                        updated_at: row.updated_at,
                    },
                    purchased_at: row.purchase_date,
                    download_count: row.user_download_count,
                    last_downloaded_at: row.last_downloaded_at,
                    categories: Vec::new(),
                }
            });

            if let Some(category_id) = row.category_id {
                if !book_entry.categories.iter().any(|c| c.id == category_id) {
                    book_entry.categories.push(Category {
                        id: category_id,
                        name: row.category_name.unwrap_or_default(),
                        slug: row.category_slug.unwrap_or_default(),
                        description: row.category_description,
                        is_active: row.category_active.unwrap_or(true),
                        created_at: row.category_created_at.unwrap_or(Utc::now()),
                    });
                }
            }
        }

        Ok(books_map.into_values().collect())
    }

    // ===== PREVIEW METHODS =====
    
    /// Mengambil preview data untuk buku
    pub async fn get_book_preview(
        pool: &PgPool,
        book_id: Uuid,
    ) -> Result<Option<BookPreviewData>, DatabaseError> {
        let row = sqlx::query!(
            r#"
            SELECT 
                id, title, preview_url, preview_pages, has_preview, total_pages
            FROM books
            WHERE id = $1 AND is_active = true
            "#,
            book_id
        )
        .fetch_optional(pool)
        .await?;

        match row {
            Some(r) => Ok(Some(BookPreviewData {
                book_id: r.id,
                title: r.title,
                preview_url: r.preview_url,
                preview_pages: r.preview_pages.unwrap_or(0),
                has_preview: r.has_preview.unwrap_or(false),
                total_pages: r.total_pages,
            })),
            None => Err(DatabaseError::BookNotFound),
        }
    }

    // ===== RELATED BOOKS METHODS =====
    
    /// Mengambil buku terkait berdasarkan kategori yang sama
    pub async fn get_related_books(
        pool: &PgPool,
        book_id: Uuid,
        limit: u32,
    ) -> Result<Vec<BookWithCategories>, DatabaseError> {
        let limit = std::cmp::min(limit, 20) as i64;

        let category_ids: Vec<Uuid> = sqlx::query_scalar!(
            "SELECT category_id FROM book_categories WHERE book_id = $1",
            book_id
        )
        .fetch_all(pool)
        .await?;

        if category_ids.is_empty() {
            return Ok(Vec::new());
        }

        let book_ids: Vec<Uuid> = sqlx::query!(
            r#"
            SELECT DISTINCT ON (b.id) 
                b.id,
                b.download_count,
                b.created_at
            FROM books b
            INNER JOIN book_categories bc ON b.id = bc.book_id
            WHERE bc.category_id = ANY($1)
            AND b.id != $2
            AND b.is_active = true
            AND b.pdf_path IS NOT NULL
            ORDER BY b.id, b.download_count DESC, b.created_at DESC
            LIMIT $3
            "#,
            &category_ids[..],
            book_id,
            limit
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| row.id)
        .collect();

        if book_ids.is_empty() {
            return Ok(Vec::new());
        }

        Self::fetch_books_with_categories(pool, book_ids).await
    }

    // ===== REVIEW METHODS =====
    
    /// Mengambil semua review untuk buku dengan info user
    pub async fn get_book_reviews(
        pool: &PgPool,
        book_id: Uuid,
        current_user_id: Option<Uuid>,
    ) -> Result<(Vec<BookReviewWithUser>, ReviewStats), DatabaseError> {
        let reviews_rows = sqlx::query!(
            r#"
            SELECT 
                br.id as "id!",
                br.book_id as "book_id!",
                br.user_id as "user_id!",
                br.rating as "rating!",
                br.comment as "comment!",
                br.helpful_count as "helpful_count!",
                br.created_at as "created_at!",
                br.updated_at as "updated_at!",
                u.full_name as "user_name!",
                u.email as "user_email!"
            FROM book_reviews br
            INNER JOIN users u ON br.user_id = u.id
            WHERE br.book_id = $1
            ORDER BY br.helpful_count DESC, br.created_at DESC
            "#,
            book_id
        )
        .fetch_all(pool)
        .await?;

        let user_helpful_votes: HashSet<Uuid> = if let Some(uid) = current_user_id {
            sqlx::query_scalar!(
                "SELECT review_id FROM review_helpful_votes WHERE user_id = $1",
                uid
            )
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect()
        } else {
            HashSet::new()
        };


        let reviews: Vec<BookReviewWithUser> = reviews_rows
            .into_iter()
            .map(|row| BookReviewWithUser {
                id: row.id,
                book_id: row.book_id,
                user_id: row.user_id,
                user_name: row.user_name,
                user_email: row.user_email,
                rating: row.rating,
                comment: row.comment,
                helpful_count: row.helpful_count,
                created_at: row.created_at,  
                updated_at: row.updated_at,
                can_edit: current_user_id == Some(row.user_id),
                has_voted_helpful: user_helpful_votes.contains(&row.id),
            })
            .collect();

        let stats = Self::calculate_review_stats(pool, book_id).await?;

        Ok((reviews, stats))
    }

    /// Menghitung statistik review untuk buku
    async fn calculate_review_stats(
        pool: &PgPool,
        book_id: Uuid,
    ) -> Result<ReviewStats, DatabaseError> {
        let stats_row = sqlx::query!(
            r#"
            SELECT 
                COUNT(*) as "total!",
                COALESCE(AVG(rating)::double precision, 0.0) as "avg_rating!",
                COUNT(*) FILTER (WHERE rating = 5) as "five_star!",
                COUNT(*) FILTER (WHERE rating = 4) as "four_star!",
                COUNT(*) FILTER (WHERE rating = 3) as "three_star!",
                COUNT(*) FILTER (WHERE rating = 2) as "two_star!",
                COUNT(*) FILTER (WHERE rating = 1) as "one_star!"
            FROM book_reviews
            WHERE book_id = $1
            "#,
            book_id
        )
        .fetch_one(pool)
        .await?;

        Ok(ReviewStats {
            total_reviews: stats_row.total,
            average_rating: stats_row.avg_rating,
            rating_distribution: RatingDistribution {
                five_star: stats_row.five_star,
                four_star: stats_row.four_star,
                three_star: stats_row.three_star,
                two_star: stats_row.two_star,
                one_star: stats_row.one_star,
            },
        })
    }

    /// Membuat review baru untuk buku
    pub async fn create_book_review(
        pool: &PgPool,
        book_id: Uuid,
        user_id: Uuid,
        rating: i32,
        comment: String,
    ) -> Result<BookReview, DatabaseError> {
        // Check ownership
        let has_purchased = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM user_purchases WHERE user_id = $1 AND book_id = $2)",
            user_id,
            book_id
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(false);

        if !has_purchased {
            return Err(DatabaseError::InvalidQuery);
        }

        let existing = sqlx::query!(
            "SELECT id FROM book_reviews WHERE book_id = $1 AND user_id = $2",
            book_id,
            user_id
        )
        .fetch_optional(pool)
        .await?;

        let review = if let Some(existing_review) = existing {
            // Update existing review
            let row = sqlx::query!(
                r#"
                UPDATE book_reviews
                SET rating = $1, comment = $2, updated_at = NOW()
                WHERE id = $3
                RETURNING 
                    id, book_id, user_id, 
                    rating as "rating!",
                    comment as "comment!",
                    helpful_count as "helpful_count!",
                    created_at as "created_at!",
                    updated_at as "updated_at!"
                "#,
                rating,
                comment,
                existing_review.id
            )
            .fetch_one(pool)
            .await?;

            BookReview {
                id: row.id,
                book_id: row.book_id,
                user_id: row.user_id,
                rating: row.rating,
                comment: row.comment,
                helpful_count: row.helpful_count,
                created_at: row.created_at,
                updated_at: row.updated_at,
            }
        } else {
            // Insert new review
            let row = sqlx::query!(
                r#"
                INSERT INTO book_reviews (book_id, user_id, rating, comment)
                VALUES ($1, $2, $3, $4)
                RETURNING 
                    id, book_id, user_id,
                    rating as "rating!",
                    comment as "comment!",
                    helpful_count as "helpful_count!",
                    created_at as "created_at!",
                    updated_at as "updated_at!"
                "#,
                book_id,
                user_id,
                rating,
                comment
            )
            .fetch_one(pool)
            .await?;

            BookReview {
                id: row.id,
                book_id: row.book_id,
                user_id: row.user_id,
                rating: row.rating,
                comment: row.comment,
                helpful_count: row.helpful_count,
                created_at: row.created_at,
                updated_at: row.updated_at,
            }
        };

        Ok(review)
    }

    /// Check apakah user sudah membeli buku
    pub async fn check_user_purchased_book(
        pool: &PgPool,
        user_id: Uuid,
        book_id: Uuid,
    ) -> Result<bool, DatabaseError> {
        let has_purchased = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM user_purchases WHERE user_id = $1 AND book_id = $2)",
            user_id,
            book_id
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(false);

        Ok(has_purchased)
    }

    // ===== FUNGSI ADMIN ANALYTICS =====
    
    /// Mengambil statistik lengkap buku untuk admin dashboard
    /// Termasuk total, aktif/non-aktif, downloads, dan growth metrics
    pub async fn get_admin_book_stats(
        pool: &PgPool,
    ) -> Result<AdminBookStats, DatabaseError> {
        let now = Utc::now();
        let current_month = now.month();
        let current_year = now.year();
    
        // Hitung bulan sebelumnya
        let (prev_month, prev_year) = if current_month == 1 {
            (12, current_year - 1)
        } else {
            (current_month - 1, current_year)
        };
    
        let stats_query = sqlx::query!(
            r#"
            SELECT 
                COUNT(*) as "total_books!",
                COUNT(*) FILTER (WHERE is_active = true) as "active_books!",
                COUNT(*) FILTER (WHERE is_active = false) as "inactive_books!",
                COUNT(*) FILTER (WHERE pdf_path IS NOT NULL) as "books_with_pdf!",
                COUNT(*) FILTER (WHERE cover_path IS NOT NULL) as "books_with_cover!",
                COALESCE(SUM(download_count), 0) as "total_downloads!",
                COUNT(*) FILTER (WHERE EXTRACT(MONTH FROM created_at) = $1 
                                   AND EXTRACT(YEAR FROM created_at) = $2) as "new_books_this_month!",
                COUNT(*) FILTER (WHERE EXTRACT(MONTH FROM created_at) = $3 
                                   AND EXTRACT(YEAR FROM created_at) = $4) as "new_books_last_month!",
                AVG(price) as avg_price,
                SUM(file_size_mb) as total_file_size_mb
            FROM books
            "#,
            current_month as i32,
            current_year as i32,
            prev_month as i32,
            prev_year as i32
        )
        .fetch_one(pool)
        .await?;
    
        let total_books = stats_query.total_books;
        let active_books = stats_query.active_books;
        let inactive_books = stats_query.inactive_books;
        let books_with_pdf = stats_query.books_with_pdf;
        let books_with_cover = stats_query.books_with_cover;
        let total_downloads = stats_query.total_downloads;
        let new_books_this_month = stats_query.new_books_this_month;
        let new_books_last_month = stats_query.new_books_last_month;
        let avg_price = stats_query.avg_price;
        let total_file_size_mb = stats_query.total_file_size_mb;
    
        // Hitung persentase pertumbuhan bulanan
        let monthly_growth_percentage = if new_books_last_month > 0 {
            ((new_books_this_month - new_books_last_month) as f64 / new_books_last_month as f64) * 100.0
        } else if new_books_this_month > 0 {
            100.0
        } else {
            0.0
        };
    
        let language_stats = Self::get_language_statistics(pool).await?;
    
        Ok(AdminBookStats {
            total_books,
            active_books,
            inactive_books,
            books_with_pdf,
            books_with_cover,
            total_downloads,
            new_books_this_month,
            avg_price,
            total_file_size_mb,
            books_by_language: language_stats,
            monthly_growth_percentage,
        })
    }
    
    /// Mengambil statistik distribusi bahasa buku
    async fn get_language_statistics(
        pool: &PgPool,
    ) -> Result<Vec<LanguageStats>, DatabaseError> {
        // Hitung total buku aktif
        let total_row = sqlx::query("SELECT COUNT(*) as total FROM books WHERE is_active = true")
            .fetch_one(pool)
            .await?;
        let total_count: i64 = total_row.get("total");
        
        if total_count == 0 {
            return Ok(Vec::new());
        }
        
        // Ambil distribusi per bahasa
        let rows = sqlx::query(
            r#"
            SELECT 
                language,
                COUNT(*) as book_count
            FROM books 
            WHERE is_active = true
            GROUP BY language
            ORDER BY book_count DESC
            LIMIT 10
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|row| LanguageStats {
            language: row.get("language"),
            book_count: row.get("book_count"),
            percentage: (row.get::<i64, _>("book_count") as f64 / total_count as f64) * 100.0,
        }).collect())
    }

    /// Mengambil top books berdasarkan metric tertentu
    /// Metric: downloads, sales, revenue, recent
    pub async fn get_top_books_by_metric(
        pool: &PgPool,
        metric_type: &str,
        limit: u32,
    ) -> Result<Vec<TopBook>, DatabaseError> {
        let limit = std::cmp::min(limit, 50) as i64;

        let query = match metric_type {
            "downloads" => {
                r#"
                SELECT 
                    b.id, b.title, b.author, b.cover_path, 
                    b.download_count, b.price, b.created_at,
                    b.download_count::bigint as metric_value
                FROM books b
                WHERE b.is_active = true
                ORDER BY b.download_count DESC
                LIMIT $1
                "#
            },
            "sales" => {
                r#"
                SELECT 
                    b.id, b.title, b.author, b.cover_path, 
                    b.download_count, b.price, b.created_at,
                    COALESCE(COUNT(o.id), 0)::bigint as metric_value
                FROM books b
                LEFT JOIN orders o ON b.id = o.book_id AND o.status = 'paid'
                WHERE b.is_active = true
                GROUP BY b.id
                ORDER BY metric_value DESC
                LIMIT $1
                "#
            },
            "revenue" => {
                r#"
                SELECT 
                    b.id, b.title, b.author, b.cover_path, 
                    b.download_count, b.price, b.created_at,
                    COALESCE(SUM(o.amount), 0)::bigint as metric_value
                FROM books b
                LEFT JOIN orders o ON b.id = o.book_id AND o.status = 'paid'
                WHERE b.is_active = true
                GROUP BY b.id
                ORDER BY metric_value DESC
                LIMIT $1
                "#
            },
            "recent" => {
                r#"
                SELECT 
                    b.id, b.title, b.author, b.cover_path, 
                    b.download_count, b.price, b.created_at,
                    EXTRACT(EPOCH FROM b.created_at)::bigint as metric_value
                FROM books b
                WHERE b.is_active = true
                ORDER BY b.created_at DESC
                LIMIT $1
                "#
            },
            _ => return Err(DatabaseError::InvalidMetricType),
        };

        let rows = sqlx::query(query)
            .bind(limit)
            .fetch_all(pool)
            .await?;

        Ok(rows.into_iter().map(|row| TopBook {
            id: row.get("id"),
            title: row.get("title"),
            author: row.get("author"),
            cover_path: row.get("cover_path"),
            download_count: row.get("download_count"),
            price: row.get("price"),
            created_at: row.get("created_at"),
            metric_value: row.get("metric_value"),
            metric_type: metric_type.to_string(),
        }).collect())
    }

    /// Mengambil analytics penjualan untuk chart
    pub async fn get_sales_analytics(
        pool: &PgPool,
        days: u32,
    ) -> Result<Vec<SalesAnalytics>, DatabaseError> {
        let days = std::cmp::min(days, 365) as i32;

        let query = r#"
            SELECT 
                DATE(o.created_at) as sale_date,
                COUNT(*) as sales_count,
                COALESCE(SUM(o.amount), 0) as revenue,
                COUNT(DISTINCT o.book_id) as books_sold
            FROM orders o
            WHERE o.status = 'paid' 
              AND o.created_at >= CURRENT_DATE - INTERVAL '1 day' * $1
            GROUP BY DATE(o.created_at)
            ORDER BY sale_date DESC
        "#;
        
        let rows = sqlx::query(query)
            .bind(days)
            .fetch_all(pool)
            .await?;

        Ok(rows.into_iter().map(|row| {
            let sale_date: chrono::NaiveDate = row.get("sale_date");
            SalesAnalytics {
                date: sale_date.format("%Y-%m-%d").to_string(),
                sales_count: row.get("sales_count"),
                revenue: row.get("revenue"),
                books_sold: row.get("books_sold"),
            }
        }).collect())
    }

    /// Mengambil data untuk chart popular books
    pub async fn get_popular_books_chart_data(
        pool: &PgPool,
        limit: u32,
    ) -> Result<PopularBooksChart, DatabaseError> {
        let limit = std::cmp::min(limit, 10);

        let books = Self::get_top_books_by_metric(pool, "downloads", limit).await?;

        let mut labels = Vec::new();
        let mut data = Vec::new();
        let mut colors = Vec::new();

        // Palet warna untuk chart
        let color_palette = vec![
            "#6366f1", "#8b5cf6", "#06b6d4", "#10b981", "#f59e0b",
            "#ef4444", "#ec4899", "#84cc16", "#f97316", "#6366f1"
        ];

        for (index, book) in books.iter().enumerate() {
            // Truncate judul jika terlalu panjang
            let truncated_title = if book.title.len() > 25 {
                format!("{}...", &book.title[..22])
            } else {
                book.title.clone()
            };

            labels.push(truncated_title);
            data.push(book.download_count as i64);
            colors.push(color_palette[index % color_palette.len()].to_string());
        }

        Ok(PopularBooksChart {
            labels,
            data,
            colors,
        })
    }

    /// Mengambil analytics per kategori
    pub async fn get_category_analytics(
        pool: &PgPool,
    ) -> Result<Vec<CategoryAnalytics>, DatabaseError> {
        let query = r#"
            SELECT 
                c.name as category_name,
                c.slug as category_slug,
                COUNT(DISTINCT b.id) as book_count,
                COALESCE(SUM(b.download_count), 0) as total_downloads,
                COALESCE(SUM(o.total_revenue), 0) as total_revenue,
                AVG(b.price) as avg_price
            FROM categories c
            JOIN book_categories bc ON c.id = bc.category_id
            JOIN books b ON bc.book_id = b.id
            LEFT JOIN (
                SELECT 
                    book_id,
                    SUM(amount) as total_revenue
                FROM orders 
                WHERE status = 'paid'
                GROUP BY book_id
            ) o ON b.id = o.book_id
            WHERE c.is_active = true AND b.is_active = true
            GROUP BY c.id, c.name, c.slug
            ORDER BY total_revenue DESC, book_count DESC
            LIMIT 20
        "#;
        
        let rows = sqlx::query(query)
            .fetch_all(pool)
            .await?;

        Ok(rows.into_iter().map(|row| CategoryAnalytics {
            category_name: row.get("category_name"),
            category_slug: row.get("category_slug"),
            book_count: row.get("book_count"),
            total_downloads: row.get("total_downloads"),
            total_revenue: row.get("total_revenue"),
            avg_price: row.get("avg_price"),
        }).collect())
    }

    /// Mengambil aktivitas buku terbaru untuk dashboard
    /// Menampilkan buku baru, update, dan delete dalam 7 hari terakhir
    pub async fn get_recent_book_activity(
        pool: &PgPool,
        limit: u32,
    ) -> Result<Vec<serde_json::Value>, DatabaseError> {
        let limit = std::cmp::min(limit, 50) as i64;

        // Ambil dari audit_logs dengan LEFT JOIN untuk handle deleted books
        let activity_rows = sqlx::query!(
            r#"
            SELECT 
                al.action,
                al.resource_id as book_id,
                al.details,
                al.created_at as "timestamp!",
                b.title as "title?",        
                b.author as "author?"       
            FROM audit_logs al
            LEFT JOIN books b ON al.resource_id = b.id
            WHERE al.resource_type = 'book'
            AND al.created_at >= CURRENT_DATE - INTERVAL '7 days'
            ORDER BY al.created_at DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(pool)
        .await?;

        Ok(activity_rows.into_iter().map(|row| {
            let title = row.title.unwrap_or_else(|| {
                if let Some(details) = &row.details {
                    details.get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .to_string()
                } else {
                    "Unknown".to_string()
                }
            });
            
            let action_text = match row.action.as_str() {
                "BOOK_CREATED" => format!("Buku baru ditambahkan: {}", title),
                "BOOK_UPDATED" => format!("Buku diupdate: {}", title),
                "BOOK_DELETED" => format!("Buku dihapus: {}", title),
                _ => format!("Aktivitas: {}", row.action)
            };
            
            serde_json::json!({
                "action": row.action,
                "book_id": row.book_id,
                "description": action_text,
                "author": row.author,      
                "timestamp": row.timestamp,
                "details": row.details,
                "icon": match row.action.as_str() {
                    "BOOK_CREATED" => "plus-circle",
                    "BOOK_UPDATED" => "edit",
                    "BOOK_DELETED" => "trash",
                    _ => "activity"
                }
            })
        }).collect())
    }
}