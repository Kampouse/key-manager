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

## Installation

```bash
npm install @kampouse/key-manager-client
# or
yarn add @kampouse/key-manager-client
```

## TypeScript Usage

```typescript
import { KeyManagerClient } from "@kampouse/key-manager-client";

const client = new KeyManagerClient({
  paymentKey: "pk_your_payment_key",
});

// Encrypt data
const { encryptedValue, key_id } = await client.encrypt(
  "alice.near/private",
  "alice.near",
  "secret data"
);

// Decrypt data
const { plaintext } = await client.decrypt(
  "alice.near/private",
  "alice.near",
  ciphertextB64
);

// Batch operations (faster)
const { encryptedValues } = await client.batchEncrypt(
  "alice.near/data",
  "alice.near",
  {
    email: "alice@example.com",
    phone: "+1-555-1234",
  }
);
```

## Actions

### `get_key`
Get encryption key for a group.

```typescript
const { key_b64, key_id, attestation_hash } = await client.getKey(
  "alice.near/private",
  "alice.near"
);
```

### `encrypt`
Encrypt data with group key (AES-256-GCM).

```typescript
const { ciphertext_b64, key_id, encryptedValue } = await client.encrypt(
  "alice.near/private",
  "alice.near",
  "secret data"
);
// encryptedValue = "enc:AES256:key_id:ciphertext_b64"
```

### `decrypt`
Decrypt data with group key.

```typescript
const { plaintext, plaintext_utf8 } = await client.decrypt(
  "alice.near/private",
  "alice.near",
  ciphertextB64
);
// plaintext_utf8 = "secret data" (if valid UTF-8)
```

### `batch_encrypt`
Encrypt multiple items in one call.

```typescript
const { encryptedValues, key_id } = await client.batchEncrypt(
  "alice.near/data",
  "alice.near",
  {
    name: "Alice",
    email: "alice@example.com",
    phone: "+1-555-1234",
  }
);
```

### `batch_decrypt`
Decrypt multiple items in one call.

```typescript
const { plaintexts } = await client.batchDecrypt(
  "alice.near/data",
  "alice.near",
  [
    { key: "name", ciphertextB64: "..." },
    { key: "email", ciphertextB64: "..." },
  ]
);
// plaintexts = { name: "Alice", email: "alice@example.com" }
```

## Examples

See [examples/](./examples) for real-world use cases:

| Example | Description |
|---------|-------------|
| [profile-secrets](./examples/profile-secrets) | Encrypted user profiles (GDPR-friendly) |
| [shared-vault](./examples/shared-vault) | Team secrets with group access |
| [wallet-signer](./examples/wallet-signer) | TEE wallet (keys never leave enclave) |
| [fastkv-integration](./examples/fastkv-integration) | Encrypted key-value storage |

## FastKV Integration

FastKV provides encrypted storage endpoints using the Key Manager.

**Quick start:**
```bash
# Encrypt
curl -X POST https://fastkv.up.railway.app/v1/kv/encrypted/encrypt \
  -H "X-Payment-Key: pk_..." \
  -d '{"account_id": "alice.near", "value": "secret"}'

# Decrypt
curl -X POST https://fastkv.up.railway.app/v1/kv/encrypted/decrypt \
  -H "X-Payment-Key: pk_..." \
  -d '{"account_id": "alice.near", "ciphertext": "enc:AES256:..."}'
```

See [docs/FASTKV_INTEGRATION.md](./docs/FASTKV_INTEGRATION.md) for full documentation.

## Performance

| Source | First Call | Cached |
|--------|-----------|--------|
| GitHub | ~30s | ~0.7s |
| **WasmUrl** | ~1.2s | **~0.9s** |

**Cost:** ~$0.005 per cached call

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

For integration with storage backends:

```
enc:AES256:<key_id>:<ciphertext_base64>
```

Example:
```
enc:AES256:01560100ddd39635:hM60OnrQ6W2yUKX0QvTCo8iSz2f4g3tElMzA5Fj93ig4
```

## API Reference

See [types.ts](./examples/types.ts) for full type definitions.

## License

MIT
