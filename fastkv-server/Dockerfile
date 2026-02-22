# This Dockerfile is meant to be built from key-manager/ root directory
# Railway Root Directory should be set to "." (repo root), NOT "fastkv-server"

FROM rust:1.88-slim

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy workspace Cargo files for caching
COPY Cargo.toml Cargo.lock* ./
COPY fastkv-server/Cargo.toml ./fastkv-server/
COPY fastkv-server/create-tables.rs ./fastkv-server/

# Create dummy files to build dependencies
RUN mkdir -p src fastkv-server/src && \
    echo "fn main() {}" > src/lib.rs && \
    echo "fn main() {}" > fastkv-server/src/main.rs

# Build dependencies
RUN cargo build --release && rm -rf src fastkv-server/src

# Copy actual source code
COPY src ./src
COPY fastkv-server/src ./fastkv-server/src

# Build fastkv-server
RUN cargo build --release -p fastkv-server

WORKDIR /app/fastkv-server

# Expose port
EXPOSE 3001

# Set environment
ENV CHAIN_ID=mainnet

# Run
CMD ["../target/release/fastkv-server"]
