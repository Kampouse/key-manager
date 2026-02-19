# Key Manager for OutLayer TEE

Encrypted key-value storage using OutLayer TEE for key management.

## Usage

Call via OutLayer HTTPS API:

```bash
curl -X POST https://api.outlayer.fastnear.com/call/your-username/key-manager \
  -H "X-Payment-Key: your_payment_key" \
  -H "Content-Type: application/json" \
  -d '{"action":"get_key","group_id":"test.near/data","account_id":"test.near"}'
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

### `encrypt`
Encrypt data with group key.

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

## Test Locally

```bash
cargo build --release --target wasm32-wasip2
echo '{"action":"get_key","group_id":"test","account_id":"test.near"}' | \
  wasmtime target/wasm32-wasip2/release/key-manager.wasm
```

## Security

- Keys derived from CKD (Confidential Key Derivation)
- AES-256-GCM encryption
- Group-based access control
- Attestation included in responses
