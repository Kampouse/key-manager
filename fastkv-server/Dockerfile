FROM rust:1.88-slim

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy Cargo files first for better caching
COPY Cargo.toml Cargo.lock* ./
COPY rust-toolchain.toml ./

# Create dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# Copy actual source
COPY src ./src
COPY create-tables.rs ./
COPY start.sh ./

# Build for real
RUN cargo build --release && chmod +x start.sh

# Expose port
EXPOSE 3001

# Set environment
ENV CHAIN_ID=mainnet

# Run
CMD ["./start.sh"]
