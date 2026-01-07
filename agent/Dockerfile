# Build stage
FROM rust:1.92-slim as builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml ./

# Create dummy main to cache dependencies
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src ./src
COPY migrations ./migrations

# Build the actual binary
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /build/target/release/lightweight-ci /app/

# Copy migrations
COPY --from=builder /build/migrations /app/migrations

# Create data directories
RUN mkdir -p /data/db /data/logs /data/workspace /data/output /data/cache

# Expose ports
# 3000: API + WebSocket
# 8080: Reverse Proxy
EXPOSE 3000 8080

# Set environment variables
ENV DATABASE_URL=sqlite:///data/db/ci.db
ENV RUST_LOG=lightweight_ci=info,tower_http=debug

# Run the binary
CMD ["/app/lightweight-ci"]
