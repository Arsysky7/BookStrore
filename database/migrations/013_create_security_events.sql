-- Create security_events table if not exists
CREATE TABLE IF NOT EXISTS security_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    event_type VARCHAR(100) NOT NULL,
    event_data JSONB,
    success BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_security_events_user ON security_events(user_id);
CREATE INDEX idx_security_events_type ON security_events(event_type);
CREATE INDEX idx_security_events_created ON security_events(created_at DESC);
CREATE INDEX idx_security_events_success ON security_events(success);

-- Insert some sample events for testing
INSERT INTO security_events (user_id, event_type, event_data, success) 
SELECT 
    id,
    'USER_REGISTERED',
    jsonb_build_object('email', email, 'timestamp', created_at),
    true
FROM users 
WHERE NOT EXISTS (
    SELECT 1 FROM security_events WHERE event_type = 'USER_REGISTERED' AND user_id = users.id
);