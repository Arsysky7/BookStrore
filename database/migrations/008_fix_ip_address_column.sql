-- /pdf-bookstore/database/migrations/008_fix_ip_address_column.sql
DO $$ 
BEGIN
    -- Ganti user_sessions ke sessions
    IF EXISTS (
        SELECT 1 
        FROM information_schema.columns 
        WHERE table_name = 'sessions' 
        AND column_name = 'ip_address' 
        AND data_type != 'inet'
    ) THEN
        ALTER TABLE sessions ADD COLUMN ip_address_temp TEXT;
        UPDATE sessions SET ip_address_temp = ip_address::text WHERE ip_address IS NOT NULL;
        ALTER TABLE sessions DROP COLUMN ip_address;
        ALTER TABLE sessions ADD COLUMN ip_address INET;
        UPDATE sessions SET ip_address = ip_address_temp::inet 
        WHERE ip_address_temp IS NOT NULL 
        AND ip_address_temp ~ '^((25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$';
        ALTER TABLE sessions DROP COLUMN ip_address_temp;
    END IF;
    
    -- Tabel security_events tetap sama
    IF EXISTS (
        SELECT 1 
        FROM information_schema.columns 
        WHERE table_name = 'security_events' 
        AND column_name = 'ip_address' 
        AND data_type != 'inet'
    ) THEN
        ALTER TABLE security_events ADD COLUMN ip_address_temp TEXT;
        UPDATE security_events SET ip_address_temp = ip_address::text WHERE ip_address IS NOT NULL;
        ALTER TABLE security_events DROP COLUMN ip_address;
        ALTER TABLE security_events ADD COLUMN ip_address INET;
        UPDATE security_events SET ip_address = ip_address_temp::inet 
        WHERE ip_address_temp IS NOT NULL 
        AND ip_address_temp ~ '^((25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$';
        ALTER TABLE security_events DROP COLUMN ip_address_temp;
    END IF;
    
    -- Tabel audit_logs tetap sama
    IF EXISTS (
        SELECT 1 
        FROM information_schema.columns 
        WHERE table_name = 'audit_logs' 
        AND column_name = 'ip_address' 
        AND data_type != 'inet'
    ) THEN
        ALTER TABLE audit_logs ADD COLUMN ip_address_temp TEXT;
        UPDATE audit_logs SET ip_address_temp = ip_address::text WHERE ip_address IS NOT NULL;
        ALTER TABLE audit_logs DROP COLUMN ip_address;
        ALTER TABLE audit_logs ADD COLUMN ip_address INET;
        UPDATE audit_logs SET ip_address = ip_address_temp::inet 
        WHERE ip_address_temp IS NOT NULL 
        AND ip_address_temp ~ '^((25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$';
        ALTER TABLE audit_logs DROP COLUMN ip_address_temp;
    END IF;
END $$;

-- Ensure indexes exist (FIXED: sessions bukan user_sessions)
CREATE INDEX IF NOT EXISTS idx_sessions_ip_address ON sessions(ip_address);
CREATE INDEX IF NOT EXISTS idx_security_events_ip_address ON security_events(ip_address);
CREATE INDEX IF NOT EXISTS idx_audit_logs_ip_address ON audit_logs(ip_address);