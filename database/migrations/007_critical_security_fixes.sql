-- File: /pdf-bookstore/database/migrations/007_critical_security_fixes.sql

-- Add missing composite indexes for performance
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_books_search_composite 
ON books USING gin(to_tsvector('indonesian', title || ' ' || author || ' ' || COALESCE(description, '')));

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_books_price_active_created 
ON books(price, is_active, created_at DESC) WHERE is_active = true;

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_orders_user_status_created 
ON orders(user_id, status, created_at DESC);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_user_purchases_user_book_purchased 
ON user_purchases(user_id, book_id, purchased_at DESC);

-- Add row-level security
ALTER TABLE users ENABLE ROW LEVEL SECURITY;
ALTER TABLE orders ENABLE ROW LEVEL SECURITY;
ALTER TABLE user_purchases ENABLE ROW LEVEL SECURITY;

-- Policy: Users can only see their own data
CREATE POLICY users_own_data ON users
    FOR ALL USING (id = current_setting('app.current_user_id')::uuid);

CREATE POLICY orders_own_data ON orders  
    FOR ALL USING (user_id = current_setting('app.current_user_id')::uuid);

CREATE POLICY purchases_own_data ON user_purchases
    FOR ALL USING (user_id = current_setting('app.current_user_id')::uuid);

-- Add missing constraints for data integrity
ALTER TABLE orders ADD CONSTRAINT orders_amount_positive CHECK (amount > 0);
ALTER TABLE orders ADD CONSTRAINT orders_expires_after_created CHECK (expires_at > created_at);
ALTER TABLE books ADD CONSTRAINT books_pages_positive CHECK (total_pages > 0);
ALTER TABLE user_purchases ADD CONSTRAINT purchases_download_count_positive CHECK (download_count >= 0);

-- Add audit trail for sensitive operations (IF NOT EXISTS untuk prevent duplicate)
CREATE TABLE IF NOT EXISTS security_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    event_type VARCHAR(50) NOT NULL,
    event_data JSONB NOT NULL,
    ip_address INET,
    user_agent TEXT,
    success BOOLEAN NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_security_events_user_id ON security_events(user_id);
CREATE INDEX IF NOT EXISTS idx_security_events_type ON security_events(event_type);
CREATE INDEX IF NOT EXISTS idx_security_events_created_at ON security_events(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_security_events_success ON security_events(success);

-- Add payment idempotency table
CREATE TABLE IF NOT EXISTS payment_idempotency (
    idempotency_key UUID PRIMARY KEY,
    order_id UUID REFERENCES orders(id) ON DELETE CASCADE,
    payment_result JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_payment_idempotency_order_id ON payment_idempotency(order_id);
CREATE INDEX IF NOT EXISTS idx_payment_idempotency_created_at ON payment_idempotency(created_at);

-- Add rate limiting table
CREATE TABLE IF NOT EXISTS rate_limit_buckets (
    identifier VARCHAR(255) PRIMARY KEY,
    bucket_type VARCHAR(50) NOT NULL,
    tokens INTEGER NOT NULL DEFAULT 0,
    last_refill TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_rate_limit_type ON rate_limit_buckets(bucket_type);
CREATE INDEX IF NOT EXISTS idx_rate_limit_last_refill ON rate_limit_buckets(last_refill);

-- Add session security enhancements 
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS fingerprint VARCHAR(255);
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS is_revoked BOOLEAN DEFAULT FALSE;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS revoked_at TIMESTAMP WITH TIME ZONE;

CREATE INDEX IF NOT EXISTS idx_sessions_fingerprint ON sessions(fingerprint);
CREATE INDEX IF NOT EXISTS idx_sessions_revoked ON sessions(is_revoked) WHERE is_revoked = TRUE;

-- Enhanced audit logging 
ALTER TABLE audit_logs ADD COLUMN IF NOT EXISTS session_id UUID REFERENCES sessions(id) ON DELETE SET NULL;
ALTER TABLE audit_logs ADD COLUMN IF NOT EXISTS risk_score INTEGER DEFAULT 0 CHECK (risk_score >= 0 AND risk_score <= 100);
ALTER TABLE audit_logs ADD COLUMN IF NOT EXISTS requires_review BOOLEAN DEFAULT FALSE;

CREATE INDEX IF NOT EXISTS idx_audit_logs_session_id ON audit_logs(session_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_risk_score ON audit_logs(risk_score DESC);
CREATE INDEX IF NOT EXISTS idx_audit_logs_requires_review ON audit_logs(requires_review) WHERE requires_review = TRUE;

-- Add file integrity tracking
CREATE TABLE IF NOT EXISTS file_integrity (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    file_path VARCHAR(1000) NOT NULL UNIQUE,
    file_hash VARCHAR(64) NOT NULL,
    file_size BIGINT NOT NULL,
    mime_type VARCHAR(100) NOT NULL,
    upload_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    virus_scan_status VARCHAR(20) DEFAULT 'pending' CHECK (virus_scan_status IN ('pending', 'clean', 'infected', 'error')),
    virus_scan_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_file_integrity_path ON file_integrity(file_path);
CREATE INDEX IF NOT EXISTS idx_file_integrity_hash ON file_integrity(file_hash);
CREATE INDEX IF NOT EXISTS idx_file_integrity_virus_status ON file_integrity(virus_scan_status);
CREATE INDEX IF NOT EXISTS idx_file_integrity_upload_user ON file_integrity(upload_user_id);

-- Add book inventory tracking for race condition prevention
CREATE TABLE IF NOT EXISTS book_inventory (
    book_id UUID PRIMARY KEY REFERENCES books(id) ON DELETE CASCADE,
    stock_quantity INTEGER NOT NULL DEFAULT 1,
    reserved_quantity INTEGER NOT NULL DEFAULT 0,
    version INTEGER NOT NULL DEFAULT 1,
    last_updated TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Initialize inventory for existing books
INSERT INTO book_inventory (book_id, stock_quantity)
SELECT id, 999999 FROM books
ON CONFLICT (book_id) DO NOTHING;

CREATE INDEX IF NOT EXISTS idx_book_inventory_stock ON book_inventory(stock_quantity);

-- Create function for safe order processing
CREATE OR REPLACE FUNCTION create_order_atomic(
    p_user_id UUID,
    p_book_id UUID,
    p_amount DECIMAL(12,2),
    p_payment_method VARCHAR(50)
) RETURNS JSON AS $$
DECLARE
    v_order_id UUID;
    v_order_number VARCHAR(50);
    v_book_exists BOOLEAN;
    v_user_has_book BOOLEAN;
    result JSON;
BEGIN
    -- Check if book exists and is active
    SELECT EXISTS(SELECT 1 FROM books WHERE id = p_book_id AND is_active = TRUE) INTO v_book_exists;
    
    IF NOT v_book_exists THEN
        RETURN json_build_object('success', false, 'error', 'BOOK_NOT_FOUND');
    END IF;
    
    -- Check if user already owns this book
    SELECT EXISTS(SELECT 1 FROM user_purchases WHERE user_id = p_user_id AND book_id = p_book_id) INTO v_user_has_book;
    
    IF v_user_has_book THEN
        RETURN json_build_object('success', false, 'error', 'BOOK_ALREADY_PURCHASED');
    END IF;
    
    -- Generate order number
    v_order_number := 'ORD-' || EXTRACT(EPOCH FROM NOW())::bigint || '-' || SUBSTRING(gen_random_uuid()::text, 1, 8);
    
    -- Create order
    INSERT INTO orders (user_id, book_id, order_number, amount, payment_method, expires_at)
    VALUES (p_user_id, p_book_id, v_order_number, p_amount, p_payment_method, NOW() + INTERVAL '24 hours')
    RETURNING id INTO v_order_id;
    
    -- Log the order creation
    INSERT INTO audit_logs (user_id, action, resource_type, resource_id, details)
    VALUES (p_user_id, 'ORDER_CREATED', 'order', v_order_id, 
            json_build_object('book_id', p_book_id, 'amount', p_amount, 'payment_method', p_payment_method));
    
    RETURN json_build_object(
        'success', true, 
        'order_id', v_order_id,
        'order_number', v_order_number
    );
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Create function for safe payment completion dengan idempotent handling
CREATE OR REPLACE FUNCTION complete_payment_atomic(
    p_order_number VARCHAR(50),
    p_transaction_id VARCHAR(255),
    p_webhook_data JSONB
) RETURNS JSONB AS $$
DECLARE
    v_order RECORD;
    v_purchase_exists BOOLEAN;
BEGIN
    -- Lock dan get order
    SELECT * INTO v_order 
    FROM orders 
    WHERE order_number = p_order_number
    FOR UPDATE;
    
    -- Check order exists
    IF NOT FOUND THEN
        RETURN jsonb_build_object(
            'success', false,
            'error', 'ORDER_NOT_FOUND'
        );
    END IF;
    
    -- Check apakah sudah di-process (idempotent check)
    SELECT EXISTS(
        SELECT 1 FROM user_purchases 
        WHERE order_id = v_order.id
    ) INTO v_purchase_exists;
    
    IF v_purchase_exists THEN
        RETURN jsonb_build_object(
            'success', true,
            'error', 'ALREADY_PROCESSED',
            'message', 'Payment sudah diproses sebelumnya'
        );
    END IF;
    
    -- Check apakah order masih pending
    IF v_order.status != 'pending' THEN
        RETURN jsonb_build_object(
            'success', false,
            'error', 'ORDER_NOT_PENDING',
            'current_status', v_order.status
        );
    END IF;
    
    -- Update order status
    UPDATE orders 
    SET status = 'paid', paid_at = NOW(), updated_at = NOW()
    WHERE id = v_order.id;
    
    -- Create user purchase record
    INSERT INTO user_purchases (user_id, book_id, order_id, purchased_at, download_count)
    VALUES (v_order.user_id, v_order.book_id, v_order.id, NOW(), 0);
    
    -- Log payment
    INSERT INTO payment_logs (order_id, transaction_id, transaction_status, webhook_data)
    VALUES (v_order.id, p_transaction_id, 'settlement', p_webhook_data);
    
    -- Log audit
    INSERT INTO audit_logs (user_id, action, resource_type, resource_id, details)
    VALUES (v_order.user_id, 'PAYMENT_COMPLETED', 'order', v_order.id,
            json_build_object('transaction_id', p_transaction_id, 'amount', v_order.amount));
    
    RETURN jsonb_build_object('success', true, 'order_id', v_order.id);
    
EXCEPTION
    WHEN OTHERS THEN
        RETURN jsonb_build_object('success', false, 'error', SQLERRM);
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Add cleanup function for expired orders
CREATE OR REPLACE FUNCTION cleanup_expired_orders() RETURNS INTEGER AS $$
DECLARE
    expired_count INTEGER;
BEGIN
    UPDATE orders 
    SET status = 'expired', updated_at = NOW()
    WHERE status = 'pending' AND expires_at < NOW();
    
    GET DIAGNOSTICS expired_count = ROW_COUNT;
    
    -- Log cleanup action
    INSERT INTO audit_logs (user_id, action, resource_type, details)
    VALUES (NULL, 'EXPIRED_ORDERS_CLEANUP', 'system', 
            json_build_object('expired_count', expired_count));
    
    RETURN expired_count;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;