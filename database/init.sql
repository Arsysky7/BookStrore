-- /pdf-bookstore/database/init.sql

\echo 'Starting database initialization...'

\i /docker-entrypoint-initdb.d/migrations/001_create_users.sql
\echo 'Users table created.'

\i /docker-entrypoint-initdb.d/migrations/002_create_books.sql
\echo 'Books table created.'

\i /docker-entrypoint-initdb.d/migrations/003_create_categories.sql
\echo 'Categories table created.'

\i /docker-entrypoint-initdb.d/migrations/004_create_orders.sql
\echo 'Orders table created.'

\i /docker-entrypoint-initdb.d/migrations/005_create_payments.sql
\echo 'Payments table created.'

\i /docker-entrypoint-initdb.d/migrations/006_create_sessions.sql
\echo 'Sessions table created.'

\i /docker-entrypoint-initdb.d/migrations/007_critical_security_fixes.sql
\echo 'Critical security fixes applied.'

\i /docker-entrypoint-initdb.d/migrations/008_fix_ip_address_column.sql
\echo 'IP address column fixes applied.'

\i /docker-entrypoint-initdb.d/migrations/009_fix_security_events_table.sql
\i /docker-entrypoint-initdb.d/migrations/010_payment_service_enhancements.sql
\i /docker-entrypoint-initdb.d/migrations/011_add_refresh_tokens.sql
\i /docker-entrypoint-initdb.d/migrations/012_add_password_reset_and_verification.sql
\i /docker-entrypoint-initdb.d/migrations/013_create_security_events.sql
\i /docker-entrypoint-initdb.d/migrations/014_complete_missing_tables.sql
\i /docker-entrypoint-initdb.d/migrations/015_add_login_otp.sql



\i /docker-entrypoint-initdb.d/seeds/categories.sql
\echo 'Categories data seeded.'

\i /docker-entrypoint-initdb.d/seeds/admin_user.sql
\echo 'Admin user created.'

\i /docker-entrypoint-initdb.d/seeds/books.sql
\echo 'Sample books data seeded.'

\echo 'Database initialization completed successfully!'
\echo 'Total tables: 12 (includes security tables)'
\echo 'Ready for microservices connection.'