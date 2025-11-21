# /pdf-bookstore/services/payment-service/Dockerfile

# Build stage
FROM rust:1.75-alpine as builder

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev pkgconfig sqlite-dev

WORKDIR /app
COPY Cargo.toml Cargo.lock ./

# Cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# Build application
COPY src ./src
RUN cargo build --release

# Production stage
FROM alpine:3.19 as production

# Install runtime dependencies
RUN apk add --no-cache ca-certificates libgcc openssl curl tzdata

# Create non-root user
RUN addgroup -g 1003 -S appuser && \
    adduser -S -D -H -u 1003 -h /app -s /sbin/nologin -G appuser appuser

WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/payment-service /app/payment-service

# Create directories
RUN mkdir -p /app/logs && \
    chown -R appuser:appuser /app && \
    chmod +x /app/payment-service

USER appuser
EXPOSE 3003

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3003/health || exit 1

ENV RUST_LOG=info
ENV RUST_BACKTRACE=1
ENV TZ=Asia/Jakarta

CMD ["/app/payment-service"]