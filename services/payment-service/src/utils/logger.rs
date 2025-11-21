// /pdf-bookstore/services/payment-service/src/utils/logger.rs

/// Logger initialization
pub fn init_logger() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("payment_service=debug".parse().unwrap())
                .add_directive("tower_http=debug".parse().unwrap())
        )
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .init();
}