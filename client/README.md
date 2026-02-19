# NEAR FastKV Encrypted Storage

End-to-end encrypted key-value storage for **NEAR Protocol** using **FastKV** backend.

> ⚠️ **Infrastructure Required**: This package requires:
> - A NEAR account (for signing transactions)
> - FastKV API endpoint (e.g., https://near.garden)
> - OutLayer TEE contract (outlayer.near on mainnet)
> - NEAR wallet or keypair for transaction signing

Plaintext **never** leaves your device - all encryption happens client-side with AES-256-GCM.

## Related Projects & Deployment

This client works with the following infrastructure:

| Project | Description |
|---------|-------------|
| **[key-manager](https://github.com/Kampouse/key-manager)** | OutLayer TEE Key Manager - Encrypted KV storage with CKD-based keys (Rust) |
| **[fastkv-server](https://github.com/Kampouse/fastkv-server)** | FastKV API server for NEAR blockchain storage |
| **[fastdata-indexer](https://github.com/Kampouse/fastdata-indexer)** | NEAR indexer for FastData/FastKV |

To deploy your own infrastructure:
1. Deploy `fastkv-server` for the storage API
2. Deploy `key-manager` WASM to OutLayer for TEE key wrapping
3. Use this client to interact with your deployment

## Features

- 🔐 **Client-side AES-256-GCM encryption** - plaintext never leaves device
- 🌐 **NEAR Protocol native** - built for NEAR blockchain
- ⚡ **FastKV backend** - fast, decentralized key-value storage
- 🔑 **TEE key wrapping** - keys protected by OutLayer TEE
- 📦 **Full TypeScript support** - Complete type definitions
- 🧪 **Testable** - Mock adapters for testing without infrastructure

## Installation

```bash
npm install near-fastkv-encrypted
```

## Quick Start

```typescript
import { PrivateKV, FastKVAdapter, OutLayerAdapter } from 'near-fastkv-encrypted';

const kv = new PrivateKV({
  accountId: 'your-account.near',
  
  // Storage backend
  storage: new FastKVAdapter({
    apiUrl: 'https://near.garden',
    accountId: 'your-account.near',
  }),
  
  // TEE backend (for key wrapping)
  tee: new OutLayerAdapter({
    signTransaction: async (tx) => {
      // Sign with NEAR wallet or keypair
      return wallet.signAndSendTransaction(tx);
    },
  }),
});

// Encrypt and store
await kv.set('my-secret', 'hello world');

// Retrieve and decrypt
const plaintext = await kv.get('my-secret');
console.log(plaintext); // "hello world"
```

## Architecture

```
┌─────────────────────────────────────────────┐
│            YOUR DEVICE (Client)              │
│                                              │
│  1. Generate ephemeral AES-256 key (local)   │
│  2. Encrypt plaintext locally                │
│  3. Wrap ephemeral key with TEE              │
│  4. Store: wrapped_key + ciphertext          │
└─────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│           Storage Backend (Pluggable)        │
│                                              │
│  Stores only: {                              │
│    "wrapped_key": "...",                     │
│    "ciphertext": "...",                      │
│    "key_id": "..."                           │
│  }                                           │
│                                              │
│  ❌ NEVER SEES PLAINTEXT                     │
└─────────────────────────────────────────────┘
```

## Privacy Guarantees

| Party | Sees Plaintext? | Sees Ciphertext? | Sees Encryption Key? |
|-------|-----------------|------------------|---------------------|
| **Your Device** | ✅ Yes | ✅ Yes | ✅ Yes |
| **Storage Server** | ❌ No | ✅ Yes | ❌ No (wrapped) |
| **TEE (OutLayer)** | ❌ No | ❌ No | ✅ Yes (unwraps only) |
| **Network** | ❌ No | ✅ Yes (encrypted) | ❌ No |

## Built-in Adapters

### Storage Adapters

- **FastKVAdapter** - NEAR blockchain storage via FastKV API
- **MemoryStorageAdapter** - In-memory storage for testing

### TEE Adapters

- **OutLayerAdapter** - NEAR OutLayer TEE for key wrapping
- **MockTEEAdapter** - Mock TEE for testing (NOT secure!)

### Crypto Adapters

- **BrowserCryptoAdapter** - Web Crypto API (browser)
- **NodeCryptoAdapter** - Node.js crypto module
- Auto-detected via `createCryptoAdapter()`

## Custom Adapters

### Custom Storage Backend

```typescript
import type { StorageAdapter, EncryptedEntry } from 'near-fastkv-encrypted';

class S3StorageAdapter implements StorageAdapter {
  async set(key: string, entry: EncryptedEntry) {
    await s3.putObject({ Key: key, Body: JSON.stringify(entry) });
    return {};
  }
  
  async get(key: string) {
    const data = await s3.getObject({ Key: key });
    return data ? JSON.parse(data.Body) : null;
  }
  
  async delete(key: string) {
    await s3.deleteObject({ Key: key });
    return {};
  }
  
  async list(prefix: string) {
    const objects = await s3.listObjects({ Prefix: prefix });
    return objects.map(o => o.Key);
  }
}
```

### Custom TEE Backend

```typescript
import type { TEEAdapter } from 'near-fastkv-encrypted';

class CustomTEEAdapter implements TEEAdapter {
  async wrapKey(groupId: string, plaintextKeyB64: string) {
    const result = await fetch('/tee/wrap', {
      method: 'POST',
      body: JSON.stringify({ groupId, plaintextKeyB64 }),
    });
    return result.json();
  }
  
  async unwrapKey(groupId: string, wrappedKeyB64: string) {
    const result = await fetch('/tee/unwrap', {
      method: 'POST',
      body: JSON.stringify({ groupId, wrappedKeyB64 }),
    });
    return result.json();
  }
}
```

## Testing

```typescript
import { PrivateKV, MemoryStorageAdapter, MockTEEAdapter } from 'near-fastkv-encrypted';

const kv = new PrivateKV({
  accountId: 'test.near',
  storage: new MemoryStorageAdapter(),
  tee: new MockTEEAdapter(),
});

await kv.set('test', 'hello');
const result = await kv.get('test');
console.log(result); // "hello"
```

## API Reference

### `PrivateKV`

#### Constructor

```typescript
new PrivateKV(config: {
  accountId: string;
  storage: StorageAdapter;
  tee: TEEAdapter;
  crypto?: CryptoAdapter;
  namespace?: string;
  groupSuffix?: string;
})
```

#### Methods

- `set(key: string, plaintext: string): Promise<{ txHash?: string }>` - Encrypt and store
- `get(key: string): Promise<string | null>` - Retrieve and decrypt
- `delete(key: string): Promise<{ txHash?: string }>` - Delete a key
- `list(prefix?: string): Promise<string[]>` - List keys
- `getKeyId(): Promise<string>` - Get TEE key ID

## Storage Format

Data stored on backend:

```json
{
  "wrapped_key": "base64-encoded-wrapped-key",
  "ciphertext": "base64-encoded-iv+ciphertext+authTag",
  "key_id": "7bef47124a0b4fe1",
  "algorithm": "AES-256-GCM",
  "v": 1
}
```

Key path: `privatekv/{accountId}/{your-key}`

## Cost (with FastKV/OutLayer)

| Operation | Cost |
|-----------|------|
| Encrypt (wrap key via TEE) | ~0.001 NEAR |
| Decrypt (unwrap key via TEE) | ~0.001 NEAR |
| FastKV storage | ~0.01 NEAR (one-time) |
| FastKV reads | Free (via API) |

## Security Notes

### ✅ Protected Against
- Server compromise (only ciphertext stored)
- Network sniffing (HTTPS + encryption)
- Insider attacks (TEE enforces access)
- Key theft (ephemeral keys, never stored)

### ⚠️ Limitations
- Requires NEAR account (accountability)
- TEE availability (OutLayer dependency)
- Cannot revoke access (re-encrypt with new group instead)

### ❌ Does NOT Protect Against
- Client-side malware (plaintext in memory)
- Compromised NEAR account
- TEE hardware backdoors

## Development

```bash
# Install dependencies
npm install

# Build
npm run build

# Test
npm test

# Publish
npm publish
```

## License

MIT
