# Key Manager for OutLayer TEE

TEE-based encryption service running in OutLayer's secure enclave. Keys derived from CKD (Confidential Key Derivation) - nobody knows the key, not even you.

## Quick Start

**Pre-compiled WASM (instant execution):**
```
URL: https://github.com/Kampouse/key-manager/releases/download/v0.2.0/key-manager.wasm
Hash: 44ce9f1f616e765f21fe208eb1ff4db29a7aac90096ca83cf75864793c21e7d3
Target: wasm32-wasip1
```

### Via NEAR Contract
```bash
near call outlayer.near request_execution '{
  "source": {"WasmUrl": {
    "url": "https://github.com/Kampouse/key-manager/releases/download/v0.2.0/key-manager.wasm",
    "hash": "44ce9f1f616e765f21fe208eb1ff4db29a7aac90096ca83cf75864793c21e7d3",
    "build_target": "wasm32-wasip1"
  }},
  "input_data": "{\"action\":\"get_key\",\"group_id\":\"user.near/data\",\"account_id\":\"user.near\"}",
  "resource_limits": {"max_instructions": 10000000000, "max_memory_mb": 128, "max_execution_seconds": 60},
  "response_format": "Json"
}' --accountId user.near --networkId mainnet --deposit 0.05 --gas 300000000000000
```

## Actions

### `get_key`
Get encryption key for a group.

```json
{
  "action": "get_key",
  "group_id": "alice.near/private",
  "account_id": "alice.near"
}
```

Response:
```json
{
  "key_b64": "YHOrTiRy4UU44gEOrx3Vw+/ap0KtXIsRVEW1IHcBvBA=",
  "key_id": "01560100ddd39635",
  "group_id": "alice.near/private",
  "attestation_hash": "1fd1d0de5c30c2a274ec3f0f4ba3ed90"
}
```

### `encrypt`
Encrypt data with group key (AES-256-GCM).

```json
{
  "action": "encrypt",
  "group_id": "alice.near/private",
  "account_id": "alice.near",
  "plaintext_b64": "SGVsbG8gV29ybGQh"
}
```

### `decrypt`
Decrypt data with group key.

```json
{
  "action": "decrypt",
  "group_id": "alice.near/private",
  "account_id": "alice.near",
  "ciphertext_b64": "..."
}
```

Response includes both base64 and UTF-8:
```json
{
  "plaintext_b64": "SGVsbG8gV29ybGQh",
  "plaintext_utf8": "Hello World!",
  "key_id": "01560100ddd39635"
}
```

### `batch_encrypt`
Encrypt multiple items in one call (faster for bulk operations).

```json
{
  "action": "batch_encrypt",
  "group_id": "alice.near/data",
  "account_id": "alice.near",
  "items": [
    {"key": "name", "plaintext_b64": "SmVhbg=="},
    {"key": "email", "plaintext_b64": "dGVzdEB0ZXN0LmNvbQ=="}
  ]
}
```

Response:
```json
{
  "key_id": "324a5a19816f3dcf",
  "items": [
    {"key": "name", "ciphertext_b64": "...", "error": null},
    {"key": "email", "ciphertext_b64": "...", "error": null}
  ]
}
```

### `batch_decrypt`
Decrypt multiple items in one call.

```json
{
  "action": "batch_decrypt",
  "group_id": "alice.near/data",
  "account_id": "alice.near",
  "items": [
    {"key": "name", "ciphertext_b64": "..."},
    {"key": "email", "ciphertext_b64": "..."}
  ]
}
```

### `verify_membership`
Check if account has access to a group.

```json
{
  "action": "verify_membership",
  "group_id": "alice.near/private",
  "account_id": "bob.near"
}
```

## Performance

| Source | First Call | Cached |
|--------|-----------|--------|
| GitHub | ~30s | ~0.7s |
| **WasmUrl** | ~1.2s | **~0.9s** |

**Cost:** ~0.001 NEAR per cached call (~$0.005)

## Test Locally

```bash
cargo build --release --target wasm32-wasip1
echo '{"action":"get_key","group_id":"test.near/data","account_id":"test.near"}' | \
  wasmtime target/wasm32-wasip1/release/key-manager.wasm
```

## Security

- **CKD-derived keys**: Keys derived from hardware seed inside TEE - nobody knows them
- **AES-256-GCM**: Authenticated encryption with 12-byte IV
- **Group-based access**: `user.near/private` only accessible by `user.near`
- **Attestation**: Every response includes attestation hash proving TEE execution

## Encrypted Value Format

For integration with FastKV or other storage:

```
enc:AES256:<key_id>:<ciphertext_base64>
```

Example:
```
enc:AES256:01560100ddd39635:hM60OnrQ6W2yUKX0QvTCo8iSz2f4g3tElMzA5Fj93ig4
```

## License

MIT
