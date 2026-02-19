# FastKV Integration

FastKV provides encrypted storage endpoints that use the Key Manager. Two integration patterns:

## Option 1: Payment Key (Server-side)

Fastest for production - you pay, users don't need NEAR wallet.

```bash
# Set your OutLayer payment key
export OUTLAYER_PAYMENT_KEY=pk_your_key
```

**Endpoints:**
```bash
# Encrypt
curl -X POST https://fastkv.up.railway.app/v1/kv/encrypted/encrypt \
  -H "X-Payment-Key: $OUTLAYER_PAYMENT_KEY" \
  -H "Content-Type: application/json" \
  -d '{"account_id": "alice.near", "value": "secret data"}'

# Response: {"encrypted_value": "enc:AES256:key_id:ciphertext...", "key_id": "..."}

# Decrypt
curl -X POST https://fastkv.up.railway.app/v1/kv/encrypted/decrypt \
  -H "X-Payment-Key: $OUTLAYER_PAYMENT_KEY" \
  -H "Content-Type: application/json" \
  -d '{"account_id": "alice.near", "ciphertext": "enc:AES256:..."}'

# Response: {"plaintext": "secret data", "plaintext_utf8": "secret data", "key_id": "..."}
```

## Option 2: Transaction-based (No Payment Key)

Users sign with their NEAR wallet - you don't pay anything.

```bash
# 1. Get unsigned transaction
curl -X POST https://fastkv.up.railway.app/v1/kv/encrypted/prepare-encrypt \
  -H "Content-Type: application/json" \
  -d '{"account_id": "alice.near", "value": "secret data"}'

# Response:
{
  "transaction": {
    "receiver_id": "outlayer.near",
    "method_name": "request_execution",
    "args": "base64...",
    "deposit": "50000000000000000000000",
    "gas": "300000000000000"
  },
  "submit_url": "https://wallet.near.org/sign"
}

# 2. User signs with wallet (costs ~0.05 NEAR)

# 3. Get result
curl "https://fastkv.up.railway.app/v1/kv/encrypted/result?tx_hash=ABC123..."
```

## TypeScript Client

```typescript
import { EncryptedKV } from "@kampouse/key-manager-client";

const kv = new EncryptedKV({
  accountId: "alice.near",
  paymentKey: "pk_...", // or omit for transaction-based
});

// Encrypt and store
const encrypted = await kv.encrypt("db_password", "super_secret_123");

// Decrypt
const password = await kv.decrypt("db_password");
```

## Cost Comparison

| Method | Who Pays | Cost |
|--------|----------|------|
| Payment Key | You (server) | ~$0.005 per call |
| Transaction | User (wallet) | ~0.05 NEAR (~$0.30) |

## Security

- Keys derived in TEE (CKD)
- Server never sees keys
- AES-256-GCM encryption
- Group-based access control

## Repositories

- **FastKV Server:** https://github.com/Kampouse/fastkv-server
- **Key Manager:** https://github.com/Kampouse/key-manager
