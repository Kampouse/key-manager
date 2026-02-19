# PrivateKV + FastKV Client

End-to-end encrypted key-value storage where plaintext NEVER leaves your device.

## How It Works

```
┌─────────────────────────────────────────────┐
│            YOUR DEVICE (Client)              │
│                                              │
│  1. Generate ephemeral AES-256 key (local)   │
│  2. Encrypt plaintext locally                │
│  3. Wrap ephemeral key with TEE (OutLayer)   │
│  4. Store: wrapped_key + ciphertext          │
└─────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│              FastKV (Storage)                │
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
| **FastKV Server** | ❌ No | ✅ Yes | ❌ No (wrapped) |
| **TEE (OutLayer)** | ❌ No | ❌ No | ✅ Yes (unwraps only) |
| **Network** | ❌ No | ✅ Yes (encrypted) | ❌ No |

## Installation

```bash
# Clone or copy the client files
npm install  # (if using dependencies)
```

## Usage

### TypeScript/JavaScript (Browser)

```typescript
import { PrivateKVClient } from './PrivateKVClient';

const kv = new PrivateKVClient({
  accountId: 'your-account.near',
  apiUrl: 'https://near.garden'
});

// Encrypt and store
await kv.set('my-secret', 'hello world');
// → Encrypts locally, wraps key with TEE, stores on FastKV

// Retrieve and decrypt
const plaintext = await kv.get('my-secret');
// → Fetches from FastKV, unwraps key with TEE, decrypts locally
console.log(plaintext); // "hello world"
```

### Node.js (CLI)

```bash
# Set a value
./privatekv.js set my-secret "hello world"

# Get a value  
./privatekv.js get my-secret

# Run full test
./privatekv.js test
```

## API

### `set(key: string, plaintext: string): Promise<void>`

Encrypts plaintext and stores on FastKV.

**Flow:**
1. Generate ephemeral AES-256 key locally
2. Encrypt plaintext locally (AES-256-GCM)
3. Wrap ephemeral key with TEE
4. Store encrypted entry on FastKV

### `get(key: string): Promise<string | null>`

Retrieves and decrypts a value.

**Flow:**
1. Fetch encrypted entry from FastKV
2. Unwrap key with TEE (only wrapped key sent)
3. Decrypt locally with unwrapped key

### `delete(key: string): Promise<void>`

Deletes a key (stores `null`).

### `list(prefix?: string): Promise<string[]>`

Lists keys with optional prefix filter.

## Storage Format

Data stored on FastKV:

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

## Cost

| Operation | Cost |
|-----------|------|
| Encrypt (wrap key via TEE) | ~0.001 NEAR |
| Decrypt (unwrap key via TEE) | ~0.001 NEAR |
| FastKV storage | ~0.01 NEAR (one-time) |
| FastKV reads | Free (via API) |

## Requirements

- **NEAR Account** - For signing transactions
- **NEAR CLI** - For Node.js client (`near login`)
- **Web Crypto API** - For browser client

## Security Notes

### ✅ Protected Against
- Server compromise (only ciphertext stored)
- Network sniffing (HTTPS + encryption)
- Insider attacks (TEE enforces access)
- Key theft (ephemeral keys, never stored)
- Device loss (CKD-derived keys)

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
# Run test
node privatekv.js test

# Check FastKV health
curl https://near.garden/health
```

## Related

- [key-manager](https://github.com/Kampouse/key-manager) - TEE WASM module
- [fastkv-server](https://github.com/Kampouse/fastkv-server) - FastKV API
- [contextual.near](https://github.com/MultiAgency/fastnear-contract) - Storage contract
