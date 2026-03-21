# Key-Manager Rust Client

Rust client for TEE-based encrypted key-value storage with CKD (Confidential Key Derivation).

## Features

- **Hardware-backed keys**: Keys derived from TEE-protected seed
- **Client-side encryption**: AES-256-GCM encryption before network transmission
- **Group-based access**: Fine-grained permissions per group
- **Attestation**: Verifiable proof of TEE execution
- **Async/await**: Built on tokio for async operations
- **Key caching**: Performance optimization with in-memory cache

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
key-manager-client = { git = "https://github.com/Kampouse/key-manager" }
```

## Quick Start

```rust
use key_manager_client::{PrivateKV, OutLayerAdapter, FastKVAdapter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let kv = PrivateKV::new(
        OutLayerAdapter::new(
            "https://api.outlayer.fastnear.com",
            "wk_your_api_key"
        ),
        FastKVAdapter::new("https://your-fastkv-server.com"),
    );

    // Encrypt and store
    kv.set("api-key", "sk_live_xxxxx", "user.near/private").await?;

    // Retrieve and decrypt
    let plaintext = kv.get("api-key", "user.near/private").await?;
    
    println!("Decrypted: {:?}", plaintext);

    Ok(())
}
```

## API Reference

### PrivateKV

Main client for encrypted key-value operations.

```rust
let kv = PrivateKV::new(tee_adapter, storage_adapter);
```

#### Methods

- `set(key, value, group_id)` - Encrypt and store value
- `get(key, group_id)` - Retrieve and decrypt value
- `delete(key, group_id)` - Delete value
- `clear_cache()` - Clear key cache

### OutLayerAdapter

TEE adapter for key derivation.

```rust
let tee = OutLayerAdapter::new(api_url, api_key);
```

### FastKVAdapter

Storage backend adapter.

```rust
let storage = FastKVAdapter::new(api_url);
```

## Encryption

Values are encrypted with **AES-256-GCM**:
- 256-bit key
- 12-byte random IV
- 128-bit authentication tag

## Security Model

### Protected Against
- ✓ Network observers (ciphertext only)
- ✓ Storage providers (no plaintext)
- ✓ TEE operator (never sees keys)

### Trade-offs
- ⚠ Keys exist in browser memory (XSS risk)
- ⚠ No forward secrecy if key compromised

## Testing

Run tests:

```bash
cargo test
```

Run example:

```bash
cargo run --example basic_usage
```

## License

MIT
