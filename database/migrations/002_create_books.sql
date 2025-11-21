-- /pdf-bookstore/database/migrations/002_create_books.sql

CREATE TABLE books (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title VARCHAR(500) NOT NULL,
    author VARCHAR(300) NOT NULL,
    description TEXT,
    isbn VARCHAR(20) UNIQUE,
    price DECIMAL(12,2) NOT NULL CHECK (price >= 0),
    pdf_path VARCHAR(1000),
    cover_path VARCHAR(1000),
    file_size_mb DECIMAL(8,2),
    total_pages INTEGER,
    language VARCHAR(10) DEFAULT 'id',
    is_active BOOLEAN DEFAULT true,
    download_count INTEGER DEFAULT 0,
    preview_url VARCHAR(1000),
    preview_pages INTEGER DEFAULT 0,
    has_preview BOOLEAN DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_books_title ON books USING gin(to_tsvector('indonesian', title));
CREATE INDEX idx_books_author ON books(author);
CREATE INDEX idx_books_price ON books(price);
CREATE INDEX idx_books_is_active ON books(is_active);
CREATE INDEX idx_books_language ON books(language);
CREATE INDEX idx_books_created_at ON books(created_at DESC);

CREATE INDEX idx_books_has_preview ON books(has_preview) WHERE has_preview = true;

CREATE TRIGGER update_books_updated_at BEFORE UPDATE ON books
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();