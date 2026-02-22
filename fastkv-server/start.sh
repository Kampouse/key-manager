#!/bin/bash
echo "=== Starting fastkv-server ==="
echo "PORT: $PORT"
echo "CHAIN_ID: $CHAIN_ID"
echo "SCYLLA_URL: $SCYLLA_URL"
echo "Current directory: $(pwd)"
echo "Files in target/release:"
ls -la target/release/fastkv-server 2>/dev/null || echo "Binary not found!"
echo "=== Launching app ==="
exec ./target/release/fastkv-server
