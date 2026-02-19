# Use Case 4: FastKV Integration

**Scenario:** Direct integration with FastKV encrypted storage layer. FastKV handles storage, Key Manager handles encryption.

**Architecture:**
```
Client → Key Manager (encrypt) → FastKV API (store)
Client ← Key Manager (decrypt) ← FastKV API (retrieve)
```

## Encrypted Value Format

```
enc:AES256:<key_id>:<ciphertext_base64>
```

FastKV stores this string as-is. Key Manager handles the crypto.
