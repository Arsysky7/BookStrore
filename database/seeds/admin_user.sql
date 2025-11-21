-- /pdf-bookstore/database/seeds/admin_user.sql

-- Clear existing admin/demo users first
DELETE FROM users WHERE email IN ('admin@bookstore.com', 'demo@customer.com');

-- Insert admin and demo customer users with hashed passwords
INSERT INTO users (email, password_hash, full_name, role, is_active, email_verified) 
VALUES 
(
    'admin@bookstore.com', 
    '$argon2id$v=19$m=19456,t=2,p=1$<PASTE_HASH_FROM_TEST_USER_HERE>',
    'System Administrator', 
    'admin', 
    true, 
    true
),
(
    'demo@customer.com',
    '$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHRmb3JEZW1v$/3nh8gN4rKSOQ2YaLvlLEgdMdvVPzpHPcc8QbJyRtmI',
    'Demo Customer', 
    'customer', 
    true, 
    true
) ON CONFLICT (email) DO NOTHING;

-- Password admin: "Test123!@#" | Password demo: "Demo123!"

-- Add constraint kalo belum ada
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'admin_must_be_active'
    ) THEN
        ALTER TABLE users ADD CONSTRAINT admin_must_be_active 
            CHECK (role != 'admin' OR is_active = true);
    END IF;
END $$;

-- Create index kalo belum ada
CREATE INDEX IF NOT EXISTS idx_users_email_active_role 
    ON users(email, is_active, role) WHERE is_active = true;

-- Log ke audit_logs
DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'audit_logs') THEN
        INSERT INTO audit_logs (user_id, action, resource_type, details, ip_address)
        SELECT 
            u.id,
            'ADMIN_USER_CREATED',
            'user',
            json_build_object(
                'email', u.email,
                'role', u.role,
                'created_via', 'database_seed'
            ),
            '127.0.0.1'::inet
        FROM users u 
        WHERE u.email = 'admin@bookstore.com'
        AND NOT EXISTS (
            SELECT 1 FROM audit_logs a 
            WHERE a.user_id = u.id AND a.action = 'ADMIN_USER_CREATED'
        );
    END IF;
END $$;