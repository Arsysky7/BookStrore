-- /pdf-bookstore/database/migrations/015_add_login_otp.sql

-- Create login OTPs table
CREATE TABLE IF NOT EXISTS login_otps (
    id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    otp_hash VARCHAR(255) NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    used_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- IMPORTANT: One active OTP per user
ALTER TABLE login_otps 
DROP CONSTRAINT IF EXISTS unique_user_otp;

ALTER TABLE login_otps 
ADD CONSTRAINT unique_user_otp UNIQUE (user_id);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_login_otps_user_id ON login_otps(user_id);
CREATE INDEX IF NOT EXISTS idx_login_otps_expires ON login_otps(expires_at);
CREATE INDEX IF NOT EXISTS idx_login_otps_created ON login_otps(created_at DESC);