-- /pdf-bookstore/database/migrations/010_payment_service_enhancements.sql
-- Enhancements untuk payment service enterprise

-- ========================= PAYMENT RATE LIMITING =========================
CREATE TABLE IF NOT EXISTS payment_rate_limits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    attempt_count INTEGER DEFAULT 0,
    window_start TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_attempt TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_payment_rate_limits_user_id ON payment_rate_limits(user_id);
CREATE INDEX IF NOT EXISTS idx_payment_rate_limits_window ON payment_rate_limits(window_start);

-- ========================= PAYMENT ANALYTICS =========================
CREATE TABLE IF NOT EXISTS payment_analytics_cache (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    metric_type VARCHAR(50) NOT NULL,
    period VARCHAR(20) NOT NULL,
    date_key DATE NOT NULL,
    data JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(metric_type, period, date_key)
);

CREATE INDEX IF NOT EXISTS idx_payment_analytics_type ON payment_analytics_cache(metric_type);
CREATE INDEX IF NOT EXISTS idx_payment_analytics_period ON payment_analytics_cache(period);
CREATE INDEX IF NOT EXISTS idx_payment_analytics_date ON payment_analytics_cache(date_key);

-- ========================= WEBHOOK DEDUPLICATION =========================
CREATE TABLE IF NOT EXISTS webhook_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id VARCHAR(255) NOT NULL,
    order_id VARCHAR(255) NOT NULL,
    event_type VARCHAR(50) NOT NULL,
    payload JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(transaction_id, event_type)
);

CREATE INDEX IF NOT EXISTS idx_webhook_events_transaction ON webhook_events(transaction_id);
CREATE INDEX IF NOT EXISTS idx_webhook_events_order ON webhook_events(order_id);
CREATE INDEX IF NOT EXISTS idx_webhook_events_created_at ON webhook_events(created_at DESC);

-- ========================= REFUND TRACKING =========================
CREATE TABLE IF NOT EXISTS refunds (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID REFERENCES orders(id) ON DELETE CASCADE,
    refund_id VARCHAR(255) UNIQUE,
    amount DECIMAL(12,2) NOT NULL,
    reason TEXT,
    status VARCHAR(50) DEFAULT 'pending',
    refunded_by UUID REFERENCES users(id) ON DELETE SET NULL,
    refunded_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_refunds_order_id ON refunds(order_id);
CREATE INDEX IF NOT EXISTS idx_refunds_status ON refunds(status);

-- ========================= ENHANCED ATOMIC FUNCTIONS =========================

-- Function untuk create order dengan idempotency check
CREATE OR REPLACE FUNCTION create_order_with_idempotency(
    p_user_id UUID,
    p_book_id UUID,
    p_amount DECIMAL(12,2),
    p_payment_method VARCHAR(50),
    p_idempotency_key VARCHAR(255) DEFAULT NULL
) RETURNS JSONB AS $$
DECLARE
    v_order_id UUID;
    v_order_number VARCHAR(50);
    v_expires_at TIMESTAMP WITH TIME ZONE;
BEGIN
    -- Check book exists
    IF NOT EXISTS (SELECT 1 FROM books WHERE id = p_book_id AND is_active = true) THEN
        RETURN jsonb_build_object('success', false, 'error', 'BOOK_NOT_FOUND');
    END IF;
    
    -- Check duplicate purchase
    IF EXISTS (SELECT 1 FROM user_purchases WHERE user_id = p_user_id AND book_id = p_book_id) THEN
        RETURN jsonb_build_object('success', false, 'error', 'BOOK_ALREADY_PURCHASED');
    END IF;
    
    -- Generate order number
    v_order_number := 'ORD-' || TO_CHAR(NOW(), 'YYYYMMDD') || '-' || 
                      LPAD(FLOOR(RANDOM() * 999999)::TEXT, 6, '0');
    v_expires_at := NOW() + INTERVAL '24 hours';
    v_order_id := gen_random_uuid();
    
    -- Insert order dengan idempotency_key
    INSERT INTO orders (
        id, user_id, book_id, order_number, amount, 
        status, payment_method, expires_at, idempotency_key,
        created_at, updated_at
    ) VALUES (
        v_order_id, p_user_id, p_book_id, v_order_number, p_amount,
        'pending', p_payment_method, v_expires_at, p_idempotency_key,
        NOW(), NOW()
    );
    
    RETURN jsonb_build_object(
        'success', true,
        'order_id', v_order_id::TEXT,
        'order_number', v_order_number
    );
    
EXCEPTION
    WHEN unique_violation THEN
        RETURN jsonb_build_object('success', false, 'error', 'DUPLICATE_ORDER');
    WHEN OTHERS THEN
        RETURN jsonb_build_object('success', false, 'error', SQLERRM);
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Function untuk handle webhook deduplication
CREATE OR REPLACE FUNCTION process_webhook_event(
    p_transaction_id VARCHAR(255),
    p_order_id VARCHAR(255),
    p_event_type VARCHAR(50),
    p_payload JSONB
) RETURNS JSON AS $$
DECLARE
    v_existing_event UUID;
BEGIN
    -- Check if event already processed
    SELECT id INTO v_existing_event
    FROM webhook_events
    WHERE transaction_id = p_transaction_id 
      AND event_type = p_event_type;
    
    IF v_existing_event IS NOT NULL THEN
        RETURN json_build_object(
            'success', true,
            'message', 'Event already processed',
            'duplicate', true
        );
    END IF;
    
    -- Insert new event
    INSERT INTO webhook_events (transaction_id, order_id, event_type, payload)
    VALUES (p_transaction_id, p_order_id, p_event_type, p_payload);
    
    RETURN json_build_object(
        'success', true,
        'message', 'Event processed successfully',
        'duplicate', false
    );
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Function untuk calculate payment analytics
CREATE OR REPLACE FUNCTION calculate_payment_analytics(
    p_period VARCHAR(20),
    p_days INTEGER DEFAULT 30
) RETURNS TABLE (
    date_key DATE,
    orders_count BIGINT,
    total_revenue DECIMAL,
    avg_order_value DECIMAL,
    payment_methods JSONB
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        DATE(o.created_at) as date_key,
        COUNT(*) as orders_count,
        SUM(o.amount) as total_revenue,
        AVG(o.amount) as avg_order_value,
        jsonb_object_agg(
            COALESCE(o.payment_method, 'unknown'),
            COUNT(*)
        ) as payment_methods
    FROM orders o
    WHERE o.status = 'paid'
      AND o.created_at >= CURRENT_DATE - INTERVAL '1 day' * p_days
    GROUP BY DATE(o.created_at)
    ORDER BY date_key DESC;
END;
$$ LANGUAGE plpgsql STABLE;

-- ========================= TRIGGERS =========================

-- Trigger untuk update analytics cache
CREATE OR REPLACE FUNCTION update_analytics_cache() RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status = 'paid' AND OLD.status != 'paid' THEN
        UPDATE payment_analytics_cache
        SET updated_at = NOW() - INTERVAL '1 hour'
        WHERE date_key = DATE(NEW.created_at);
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trigger_update_analytics_cache ON orders;
CREATE TRIGGER trigger_update_analytics_cache
    AFTER UPDATE OF status ON orders
    FOR EACH ROW
    EXECUTE FUNCTION update_analytics_cache();

-- ========================= INDEXES FOR PERFORMANCE =========================

CREATE INDEX IF NOT EXISTS idx_orders_user_status_created 
    ON orders(user_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_orders_status_created_amount 
    ON orders(status, created_at DESC, amount) 
    WHERE status = 'paid';

CREATE INDEX IF NOT EXISTS idx_payment_logs_order_created 
    ON payment_logs(order_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_orders_pending_expires 
    ON orders(expires_at) 
    WHERE status = 'pending';

-- ========================= SECURITY POLICIES =========================

ALTER TABLE orders ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS orders_user_policy ON orders;
CREATE POLICY orders_user_policy ON orders
    FOR SELECT
    USING (user_id = current_setting('app.current_user_id', true)::uuid 
           OR current_setting('app.current_user_role', true) = 'admin');

DROP POLICY IF EXISTS orders_insert_policy ON orders;
CREATE POLICY orders_insert_policy ON orders
    FOR INSERT
    WITH CHECK (true);

DROP POLICY IF EXISTS orders_update_policy ON orders;
CREATE POLICY orders_update_policy ON orders
    FOR UPDATE
    USING (true);

-- ========================= GRANTS =========================

GRANT SELECT, INSERT, UPDATE ON ALL TABLES IN SCHEMA public TO bookstore_user;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO bookstore_user;
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO bookstore_user;