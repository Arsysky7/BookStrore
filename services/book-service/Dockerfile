# /pdf-bookstore/services/book-service/Dockerfile

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
RUN apk add --no-cache ca-certificates libgcc openssl imagemagick file

# Create non-root user
RUN addgroup -g 1002 -S appuser && \
    adduser -S -D -H -u 1002 -h /app -s /sbin/nologin -G appuser appuser

WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/book-service /app/book-service

# Create storage directories
RUN mkdir -p /app/storage/books /app/storage/covers /app/storage/uploads && \
    chown -R appuser:appuser /app && \
    chmod +x /app/book-service

USER appuser
EXPOSE 3002

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:3002/health || exit 1

ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

CMD ["/app/book-service"]