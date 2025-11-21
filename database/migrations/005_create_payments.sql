-- /pdf-bookstore/database/migrations/005_create_payments.sql

CREATE TABLE payment_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID REFERENCES orders(id) ON DELETE CASCADE,
    transaction_id VARCHAR(255),
    payment_type VARCHAR(100),
    gross_amount DECIMAL(12,2),
    transaction_status VARCHAR(50),
    fraud_status VARCHAR(50),
    settlement_time TIMESTAMP WITH TIME ZONE,
    webhook_data JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE user_purchases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    book_id UUID REFERENCES books(id) ON DELETE CASCADE,
    order_id UUID REFERENCES orders(id) ON DELETE CASCADE,
    purchased_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    download_count INTEGER DEFAULT 0,
    last_downloaded_at TIMESTAMP WITH TIME ZONE,
    UNIQUE(user_id, book_id)
);

CREATE INDEX idx_payment_logs_order_id ON payment_logs(order_id);
CREATE INDEX idx_payment_logs_transaction_id ON payment_logs(transaction_id);
CREATE INDEX idx_payment_logs_created_at ON payment_logs(created_at DESC);
CREATE INDEX idx_user_purchases_user_id ON user_purchases(user_id);
CREATE INDEX idx_user_purchases_book_id ON user_purchases(book_id);
CREATE INDEX idx_user_purchases_purchased_at ON user_purchases(purchased_at DESC);