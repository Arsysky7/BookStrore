-- /pdf-bookstore/database/migrations/016_create_reviews_table.sql

-- Tabel untuk review buku
CREATE TABLE IF NOT EXISTS book_reviews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    book_id UUID NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    rating INTEGER NOT NULL CHECK (rating >= 1 AND rating <= 5),
    comment TEXT NOT NULL CHECK (length(comment) >= 10 AND length(comment) <= 1000),
    helpful_count INTEGER DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    CONSTRAINT unique_user_book_review UNIQUE (book_id, user_id)
);

-- Tabel untuk tracking helpful votes
CREATE TABLE IF NOT EXISTS review_helpful_votes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    review_id UUID NOT NULL REFERENCES book_reviews(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    CONSTRAINT unique_user_review_vote UNIQUE (review_id, user_id)
);

-- Indexes untuk performance
CREATE INDEX idx_book_reviews_book_id ON book_reviews(book_id);
CREATE INDEX idx_book_reviews_user_id ON book_reviews(user_id);
CREATE INDEX idx_book_reviews_rating ON book_reviews(rating);
CREATE INDEX idx_book_reviews_created_at ON book_reviews(created_at DESC);
CREATE INDEX idx_review_helpful_votes_review ON review_helpful_votes(review_id);
CREATE INDEX idx_review_helpful_votes_user ON review_helpful_votes(user_id);

-- Trigger untuk update updated_at
CREATE TRIGGER update_book_reviews_updated_at 
    BEFORE UPDATE ON book_reviews
    FOR EACH ROW 
    EXECUTE FUNCTION update_updated_at_column();

-- Function untuk auto-update helpful_count ketika ada vote
CREATE OR REPLACE FUNCTION update_review_helpful_count()
RETURNS TRIGGER AS $$
BEGIN
    IF (TG_OP = 'INSERT') THEN
        UPDATE book_reviews 
        SET helpful_count = helpful_count + 1 
        WHERE id = NEW.review_id;
    ELSIF (TG_OP = 'DELETE') THEN
        UPDATE book_reviews 
        SET helpful_count = GREATEST(helpful_count - 1, 0)
        WHERE id = OLD.review_id;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_helpful_count
    AFTER INSERT OR DELETE ON review_helpful_votes
    FOR EACH ROW
    EXECUTE FUNCTION update_review_helpful_count();

-- Audit log untuk reviews
CREATE OR REPLACE FUNCTION log_review_audit()
RETURNS TRIGGER AS $$
BEGIN
    IF (TG_OP = 'INSERT') THEN
        INSERT INTO audit_logs (action, resource_type, resource_id, user_id, details)
        VALUES ('REVIEW_CREATED', 'review', NEW.id, NEW.user_id, 
                json_build_object('book_id', NEW.book_id, 'rating', NEW.rating));
    ELSIF (TG_OP = 'UPDATE') THEN
        INSERT INTO audit_logs (action, resource_type, resource_id, user_id, details)
        VALUES ('REVIEW_UPDATED', 'review', NEW.id, NEW.user_id,
                json_build_object('old_rating', OLD.rating, 'new_rating', NEW.rating));
    ELSIF (TG_OP = 'DELETE') THEN
        INSERT INTO audit_logs (action, resource_type, resource_id, user_id, details)
        VALUES ('REVIEW_DELETED', 'review', OLD.id, OLD.user_id,
                json_build_object('book_id', OLD.book_id));
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_log_review_audit
    AFTER INSERT OR UPDATE OR DELETE ON book_reviews
    FOR EACH ROW
    EXECUTE FUNCTION log_review_audit();