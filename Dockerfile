# Build stage
FROM rust:1.93-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock* ./

# Create dummy source to build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    echo "" > src/lib.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src ./src

# Build the application
RUN touch src/main.rs src/lib.rs && \
    cargo build --release

# Runtime stage
FROM alpine:3.19

RUN apk add --no-cache ca-certificates tzdata

# Create non-root user
RUN addgroup -S idbuilder && adduser -S idbuilder -G idbuilder

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/idbuilder-worker /app/idbuilder-worker

# Copy default configuration
COPY config /app/config

# Create data directory
RUN mkdir -p /app/data && chown -R idbuilder:idbuilder /app

USER idbuilder

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1

# Run the service
ENTRYPOINT ["/app/idbuilder-worker"]
