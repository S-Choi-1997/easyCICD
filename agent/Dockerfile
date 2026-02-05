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

# Copy migrations first and create checksum to invalidate cache when migrations change
COPY migrations ./migrations
RUN find migrations -type f -exec sha256sum {} \; | sort > /tmp/migrations.sum

# Copy actual source code
COPY src ./src

# Build the actual binary
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:trixie-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /build/target/release/lightweight-ci /app/

# Copy migrations
COPY --from=builder /build/migrations /app/migrations

# Create data directories
RUN mkdir -p /data/easycicd

# Expose ports
# 3000: API + WebSocket
# 8080: Reverse Proxy
EXPOSE 3000 8080

# Run the binary
CMD ["/app/lightweight-ci"]
