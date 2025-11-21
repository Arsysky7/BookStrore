-- /pdf-bookstore/database/migrations/009_fix_security_events_table.sql

-- Fix security_events table yang missing di migration
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

-- Add missing indexes
CREATE INDEX IF NOT EXISTS idx_security_events_user_id ON security_events(user_id);
CREATE INDEX IF NOT EXISTS idx_security_events_type ON security_events(event_type);
CREATE INDEX IF NOT EXISTS idx_security_events_created_at ON security_events(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_security_events_success ON security_events(success);

-- Fix existing INET columns
DO $$ 
BEGIN
    -- Convert VARCHAR to INET for ip_address columns
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'security_events' 
        AND column_name = 'ip_address' 
        AND data_type = 'character varying'
    ) THEN
        ALTER TABLE security_events 
        ALTER COLUMN ip_address TYPE INET 
        USING ip_address::INET;
    END IF;
END $$;