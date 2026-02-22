FROM rust:1.88-slim

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy workspace Cargo files for caching
COPY Cargo.toml Cargo.lock* ./
COPY rust-toolchain.toml ./

# Create dummy files to build dependencies
RUN mkdir -p key-manager/src fastkv-server/src && \
    echo "fn main() {}" > key-manager/src/lib.rs && \
    echo "fn main() {}" > fastkv-server/src/main.rs && \
    echo "fn main() {}" > fastkv-server/create-tables.rs

# Build dependencies
RUN cargo build --release && rm -rf key-manager fastkv-server

# Copy actual source code
COPY key-manager ./key-manager
COPY fastkv-server ./fastkv-server

# Build fastkv-server
RUN cargo build --release -p fastkv-server

# Expose port
EXPOSE 3001

# Set environment
ENV CHAIN_ID=mainnet

# Run
CMD ["./target/release/fastkv-server"]
