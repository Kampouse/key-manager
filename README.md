# Key Manager for OutLayer TEE

TEE-based encryption service running in OutLayer's secure enclave. Keys derived from CKD (Confidential Key Derivation) - nobody knows the key, not even you.

## Project Structure

```
key-manager/
├── contract/          # OutLayer TEE WASM (Rust)
├── client/            # TypeScript/JavaScript client
├── fastdata-indexer/  # NEAR blockchain indexer
├── fastkv-server/     # FastKV API server
└── examples/          # Usage examples
```

## Quick Start

### 1. Deploy Contract WASM

Build the TEE contract:
```bash
cd contract
cargo build --target wasm32-wasip1 --release
# Output: target/wasm32-wasip1/release/key-manager.wasm
```

Deploy to OutLayer:
```bash
# Via OutLayer dashboard or CLI
outlayer deploy --name your-project key-manager.wasm
```

### 2. Install Client

```bash
cd client
npm install
npm run build
```

### 3. Use in Your App

```typescript
import { PrivateKV, FastKVAdapter, OutLayerAdapter } from 'near-fastkv-encrypted';

const kv = new PrivateKV({
  accountId: 'your-account.near',
  storage: new FastKVAdapter({
    apiUrl: 'https://your-fastkv-server.com',
    accountId: 'your-account.near',
  }),
  tee: new OutLayerAdapter({
    network: 'mainnet',
    signTransaction: async (tx) => {
      // Your signing logic
    },
  }),
});

// Encrypt and store
await kv.set('my-secret', 'hello world');

// Retrieve and decrypt
const plaintext = await kv.get('my-secret');
```

See [client/README.md](./client/README.md) for full documentation.

---

## Contract Actions

The WASM contract supports these actions:

### `wrap_key`
Wrap a client AES key with TEE key.

```json
{
  "action": "wrap_key",
  "group_id": "user.near/data",
  "account_id": "user.near",
  "plaintext_key_b64": "base64-encoded-32-byte-key"
}
```

### `unwrap_key`
Unwrap a client AES key.

```json
{
  "action": "unwrap_key",
  "group_id": "user.near/data",
  "account_id": "user.near",
  "wrapped_key_b64": "base64-wrapped-key"
}
```

### `get_key`
Get encryption key for a group.

### `encrypt` / `decrypt`
Encrypt/decrypt data with group key.

### `batch_encrypt` / `batch_decrypt`
Encrypt/decrypt multiple items in one call.

## Examples

See [examples/](./examples) for real-world use cases:

| Example | Description |
|---------|-------------|
| [profile-secrets](./examples/profile-secrets) | Encrypted user profiles (GDPR-friendly) |
| [shared-vault](./examples/shared-vault) | Team secrets with group access |
| [wallet-signer](./examples/wallet-signer) | TEE wallet (keys never leave enclave) |
| [fastkv-integration](./examples/fastkv-integration) | Encrypted key-value storage |

## Performance

| Source | First Call | Cached |
|--------|-----------|--------|
| GitHub | ~30s | ~0.7s |
| **WasmUrl** | ~1.2s | **~0.9s** |

**Cost:** ~$0.005 per cached call

## Test Locally

```bash
cd contract
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

## License

MIT
