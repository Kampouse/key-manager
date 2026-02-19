# Key Manager Examples

Real-world integration patterns for the OutLayer TEE Key Manager.

## TypeScript Clients

All examples include fully-typed TypeScript clients.

### Base Client

```typescript
import { KeyManagerClient } from "../key-manager-client";

const client = new KeyManagerClient({
  paymentKey: "pk_your_payment_key",
});
```

## Use Cases

| Example | Description | When to Use |
|---------|-------------|-------------|
| [profile-secrets](./profile-secrets) | Encrypted user profiles | Social apps, GDPR compliance |
| [shared-vault](./shared-vault) | Team secrets management | CI/CD, project secrets |
| [wallet-signer](./wallet-signer) | TEE wallet signer | Crypto wallets, key management |
| [fastkv-integration](./fastkv-integration) | FastKV storage client | Encrypted key-value store |

## Files

```
examples/
├── types.ts                    # Shared type definitions
├── key-manager-client.ts       # Base client with all actions
├── profile-secrets/
│   ├── profile-client.ts       # Encrypted profiles client
│   ├── client.js               # JavaScript version
│   ├── api-server.js           # Blind API server
│   └── schema.json             # Data model
├── shared-vault/
│   ├── vault-client.ts         # Team vault client
│   └── vault-manager.js        # JavaScript version
├── fastkv-integration/
│   ├── kv-client.ts            # KV storage client
│   └── kv-client.js            # JavaScript version
└── wallet-signer/
    ├── wallet-client.ts        # Wallet signer client
    └── wallet-client.js        # JavaScript version
```

## Quick Reference

### Encrypted Value Format
```
enc:AES256:<key_id>:<ciphertext_base64>
```

### Key Manager API
```typescript
// Single operations
await client.getKey(groupId, accountId);
await client.encrypt(groupId, accountId, plaintext);
await client.decrypt(groupId, accountId, ciphertextB64);

// Batch operations (faster)
await client.batchEncrypt(groupId, accountId, { key1: "value1", key2: "value2" });
await client.batchDecrypt(groupId, accountId, [{ key: "key1", ciphertextB64: "..." }]);
```

### Cost Optimization
```typescript
// ❌ Bad: 3 separate calls
await client.encrypt(groupId, accountId, "value1");
await client.encrypt(groupId, accountId, "value2");
await client.encrypt(groupId, accountId, "value3");

// ✅ Good: 1 batch call
await client.batchEncrypt(groupId, accountId, {
  key1: "value1",
  key2: "value2",
  key3: "value3",
});
```

## Performance

| Source | First Call | Cached |
|--------|-----------|--------|
| GitHub | ~30s | ~0.7s |
| WasmUrl | ~1.2s | ~0.9s |

**Cost:** ~$0.005 per cached call

## Types

See [types.ts](./types.ts) for full type definitions:

```typescript
// Request types
interface GetKeyRequest { action: "get_key"; group_id: string; account_id: string; }
interface EncryptRequest { action: "encrypt"; group_id: string; account_id: string; plaintext_b64: string; }
interface BatchEncryptRequest { action: "batch_encrypt"; items: EncryptItem[]; }

// Response types
interface KeyResponse { key_b64: string; key_id: string; attestation_hash: string; }
interface EncryptResponse { ciphertext_b64: string; key_id: string; }
interface BatchEncryptResponse { key_id: string; items: BatchEncryptItemResult[]; }

// Encrypted value type (branded)
type EncryptedValueString = `enc:AES256:${string}:${string}`;
```
