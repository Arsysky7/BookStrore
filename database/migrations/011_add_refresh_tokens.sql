-- /pdf-bookstore/database/migrations/011_add_refresh_tokens.sql

-- Table untuk refresh tokens dengan device tracking
CREATE TABLE IF NOT EXISTS refresh_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(255) NOT NULL UNIQUE,
    device_fingerprint VARCHAR(255),
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    last_used_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    is_revoked BOOLEAN DEFAULT FALSE,
    revoked_at TIMESTAMP WITH TIME ZONE,
    revoked_reason VARCHAR(255)
);

-- Indexes untuk performance
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_user_id ON refresh_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_token_hash ON refresh_tokens(token_hash);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_expires_at ON refresh_tokens(expires_at);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_is_revoked ON refresh_tokens(is_revoked);

-- Table untuk blacklist access tokens yang di-revoke sebelum expire
CREATE TABLE IF NOT EXISTS token_blacklist (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    token_jti VARCHAR(255) NOT NULL UNIQUE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    blacklisted_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    reason VARCHAR(255)
);

CREATE INDEX IF NOT EXISTS idx_token_blacklist_jti ON token_blacklist(token_jti);
CREATE INDEX IF NOT EXISTS idx_token_blacklist_expires_at ON token_blacklist(expires_at);

-- Cleanup function untuk expired tokens dengan detail lengkap (MERGED dari migration 016)
CREATE OR REPLACE FUNCTION cleanup_expired_tokens() RETURNS JSONB AS $$
DECLARE
    otp_deleted INTEGER;
    email_deleted INTEGER;
    password_deleted INTEGER;
    refresh_deleted INTEGER;
    blacklist_deleted INTEGER;
BEGIN
    -- Clean expired OTPs (older than 1 hour)
    DELETE FROM login_otps 
    WHERE expires_at < NOW() - INTERVAL '1 hour';
    GET DIAGNOSTICS otp_deleted = ROW_COUNT;
    
    -- Clean expired email verification tokens (older than 7 days)
    DELETE FROM email_verification_tokens 
    WHERE expires_at < NOW() - INTERVAL '7 days';
    GET DIAGNOSTICS email_deleted = ROW_COUNT;
    
    -- Clean used password reset tokens (older than 1 day)
    DELETE FROM password_reset_tokens 
    WHERE expires_at < NOW() - INTERVAL '1 day' OR used_at IS NOT NULL;
    GET DIAGNOSTICS password_deleted = ROW_COUNT;
    
    -- Clean expired refresh tokens
    DELETE FROM refresh_tokens 
    WHERE expires_at < NOW() AND is_revoked = FALSE;
    GET DIAGNOSTICS refresh_deleted = ROW_COUNT;
    
    -- Clean expired blacklisted tokens
    DELETE FROM token_blacklist 
    WHERE expires_at < NOW();
    GET DIAGNOSTICS blacklist_deleted = ROW_COUNT;
    
    -- Log cleanup ke audit_logs
    INSERT INTO audit_logs (action, resource_type, details)
    VALUES (
        'TOKEN_CLEANUP', 
        'system',
        jsonb_build_object(
            'otp_deleted', otp_deleted,
            'email_deleted', email_deleted,
            'password_deleted', password_deleted,
            'refresh_deleted', refresh_deleted,
            'blacklist_deleted', blacklist_deleted,
            'timestamp', NOW()
        )
    );
    
    RETURN jsonb_build_object(
        'success', true,
        'otp_deleted', otp_deleted,
        'email_deleted', email_deleted,
        'password_deleted', password_deleted,
        'refresh_deleted', refresh_deleted,
        'blacklist_deleted', blacklist_deleted,
        'total_deleted', otp_deleted + email_deleted + password_deleted + refresh_deleted + blacklist_deleted
    );
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;