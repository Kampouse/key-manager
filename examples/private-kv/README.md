# PrivateKV - Maximum Privacy Storage

**Plaintext NEVER leaves your device.**

## How It Works

### Traditional Encryption
```
Client → Plaintext → Server → Encrypt → Store
                    ↑
                    Server sees plaintext!
```

### PrivateKV (Client-Side Encryption)
```
Client:
  1. Generate ephemeral key (local)
  2. Encrypt data (local)
  3. Wrap ephemeral key with TEE
  4. Store: wrapped_key + ciphertext
  
Server/TEE:
  - Never sees plaintext
  - Only unwraps keys (not data)
```

## Architecture

```
┌─────────────────────────────────────────────┐
│            YOUR DEVICE (Client)              │
│                                              │
│  Plaintext: "my secret password"            │
│       ↓                                      │
│  [Encrypt locally with ephemeral key]       │
│       ↓                                      │
│  Ciphertext: "U2FsdGVkX1..."                │
│  Wrapped Key: "kq7R3T..." (from TEE)        │
│                                              │
│  Store: ciphertext + wrapped_key            │
└─────────────────────────────────────────────┘
                    │
                    │ Send ONLY ciphertext + wrapped_key
                    ▼
┌─────────────────────────────────────────────┐
│         SERVER / BLOCKCHAIN                  │
│                                              │
│  Stored: {                                   │
│    "ciphertext": "U2FsdGVkX1...",           │
│    "wrapped_key": "kq7R3T...",              │
│    "key_id": "7bef47124a0b4fe1"             │
│  }                                           │
│                                              │
│  ❌ NEVER SEES PLAINTEXT                     │
└─────────────────────────────────────────────┘
                    │
                    │ Decrypt request (only wrapped_key)
                    ▼
┌─────────────────────────────────────────────┐
│              TEE (Secure)                    │
│                                              │
│  1. Receive: wrapped_key                    │
│  2. Unwrap with group key (CKD-derived)     │
│  3. Return: ephemeral_key (plaintext)       │
│                                              │
│  ❌ NEVER SEES CIPHERTEXT                    │
└─────────────────────────────────────────────┘
```

## Usage

### Install

```bash
npm install @kampouse/private-kv
```

### Encrypt

```typescript
import { PrivateKV } from '@kampouse/private-kv';

const kv = new PrivateKV({
  accountId: "kampouse.near",
  paymentKey: "pk_..."  // Optional: for off-chain execution
});

// Encrypt (plaintext NEVER leaves device)
const encrypted = await kv.encrypt("password", "my secret 123");
console.log(encrypted);
// {
//   wrapped_key: "kq7R3T...",
//   ciphertext: "U2FsdGVkX1...",
//   key_id: "7bef47124a0b4fe1",
//   algorithm: "AES-256-GCM",
//   v: 1
// }

// Store anywhere (FastKV, IPFS, blockchain)
await kv.store("password", encrypted);
```

### Decrypt

```typescript
// Retrieve encrypted entry
const encrypted = await kv.get("password");

// Decrypt (only wrapped_key sent to TEE)
const plaintext = await kv.decrypt(encrypted);
console.log(plaintext);  // "my secret 123"
```

## Privacy Guarantees

### What's Stored

| Location | Plaintext | Ciphertext | Ephemeral Key |
|----------|-----------|------------|---------------|
| Client memory | ✅ Yes (encrypt/decrypt) | ✅ Yes | ✅ Yes |
| Network | ❌ No | ✅ Yes (encrypted) | ❌ No (wrapped) |
| Server | ❌ No | ✅ Yes | ❌ No (wrapped) |
| TEE | ❌ No | ❌ No | ✅ Yes (unwraps only) |
| Blockchain | ❌ No | ✅ Yes | ❌ No (wrapped) |

### What's Visible to Whom

| Party | Sees |
|-------|------|
| **Attacker (network)** | Ciphertext only |
| **Server admin** | Ciphertext only |
| **TEE** | Wrapped key only (not ciphertext) |
| **You** | Everything |

### Security Properties

- ✅ **Forward secrecy**: Each encryption uses new ephemeral key
- ✅ **Key isolation**: Compromised key doesn't affect other encryptions
- ✅ **Access control**: TEE enforces group membership
- ✅ **Audit trail**: TEE logs all unwrap operations
- ✅ **No key storage**: Keys derived from CKD, never stored

## Key Derivation

Keys are derived from CKD (Confidential Key Derivation):

```rust
group_key = SHA256(
  ckd_master_seed +
  group_id +
  "fastkv_encryption_key_v1"
)
```

**Properties:**
- Same group_id = Same group key (always)
- Different device doesn't matter
- No backup needed
- Account recovery = Data recovery

## Device Switching

Works seamlessly because keys are derived, not stored:

### Day 1: Laptop
```typescript
const kv = new PrivateKV({ accountId: "kampouse.near" });
const encrypted = await kv.encrypt("password", "secret");
// Stores: { ciphertext, wrapped_key, key_id }
```

### Day 2: Phone (after laptop dies)
```typescript
const kv = new PrivateKV({ accountId: "kampouse.near" });
const encrypted = await kv.get("password");
const plaintext = await kv.decrypt(encrypted);
// Returns: "secret" ✅
```

**Why it works:** TEE derives the same group key because:
- Same account_id
- Same group_id  
- Same CKD master seed

## Cost

| Operation | Method | Cost |
|-----------|--------|------|
| Get key ID | Off-chain | ~$0.001 |
| Wrap key | TEE execution | ~$0.006 |
| Unwrap key | TEE execution | ~$0.006 |
| **Total per encrypt** | | **~$0.007** |
| **Total per decrypt** | | **~$0.006** |

## Comparison

| Method | Plaintext On-Chain? | Plaintext on Server? | Plaintext on TEE? | Privacy |
|--------|---------------------|---------------------|-------------------|---------|
| Current (on-chain) | ❌ YES | ❌ YES | ❌ YES | 🔴 Public |
| Payment key (off-chain) | ✅ NO | ❌ YES | ❌ YES | 🟡 Medium |
| **PrivateKV** | ✅ NO | ✅ NO | ✅ NO | 🟢 Maximum |

## API Reference

### Constructor

```typescript
new PrivateKV(config: {
  accountId: string;       // Your NEAR account
  paymentKey?: string;     // OutLayer payment key (optional)
  apiUrl?: string;         // FastKV API URL (optional)
  groupSuffix?: string;    // Group suffix (default: "private")
})
```

### Methods

#### `encrypt(key: string, plaintext: string): Promise<EncryptedEntry>`

Encrypts plaintext locally. Returns encrypted entry.

#### `decrypt(entry: EncryptedEntry): Promise<string>`

Decrypts encrypted entry. Only wrapped key sent to TEE.

#### `store(key: string, entry: EncryptedEntry): Promise<void>`

Stores encrypted entry (helper method).

#### `getGroupKeyId(): Promise<string>`

Gets the key ID for the group (public info).

## Examples

### Profile Secrets

```typescript
const kv = new PrivateKV({ accountId: "alice.near" });

// Store private profile data
await kv.store("email", await kv.encrypt("email", "alice@example.com"));
await kv.store("phone", await kv.encrypt("phone", "+1-555-1234"));
await kv.store("address", await kv.encrypt("address", "123 Main St"));

// Retrieve
const email = await kv.decrypt(await kv.get("email"));
```

### Shared Secrets

```typescript
// Alice shares with Bob
const kvAlice = new PrivateKV({ 
  accountId: "alice.near",
  groupSuffix: "shared-with-bob"
});

const encrypted = await kvAlice.encrypt("api_key", "secret123");

// Bob can decrypt (if granted access by TEE)
const kvBob = new PrivateKV({ 
  accountId: "bob.near",
  groupSuffix: "alice/shared-with-bob"
});

const apiKey = await kvBob.decrypt(encrypted);
```

## Security Considerations

### ✅ Protects Against

- Server compromise (only ciphertext stored)
- Network sniffing (HTTPS + encryption)
- Insider attacks (TEE enforces access)
- Key theft (keys never stored)
- Device loss (CKD-derived keys)

### ⚠️ Limitations

- Cannot revoke access (workaround: re-encrypt with new group)
- Requires NEAR account (accountability)
- TEE availability (OutLayer dependency)

### ❌ Does NOT Protect Against

- Client-side malware (plaintext in memory)
- Compromised NEAR account (attacker can decrypt)
- TEE backdoor (trusting hardware manufacturer)

## License

MIT
