# Key Manager Examples

Real-world integration patterns for the OutLayer TEE Key Manager.

## Use Cases

| Example | Description | When to Use |
|---------|-------------|-------------|
| [profile-secrets](./profile-secrets/) | Encrypted user profiles | Social apps, GDPR compliance |
| [shared-vault](./shared-vault/) | Team secrets management | CI/CD, project secrets |
| [wallet-signer](./wallet-signer/) | TEE wallet signer | Crypto wallets, key management |
| [fastkv-integration](./fastkv-integration/) | FastKV storage client | Encrypted key-value store |

## Quick Reference

### Encrypted Value Format
```
enc:AES256:<key_id>:<ciphertext_base64>
```

### Key Manager API
```javascript
// Single operations
{ action: "get_key", group_id, account_id }
{ action: "encrypt", group_id, account_id, plaintext_b64 }
{ action: "decrypt", group_id, account_id, ciphertext_b64 }

// Batch operations (faster)
{ action: "batch_encrypt", group_id, account_id, items: [{key, plaintext_b64}, ...] }
{ action: "batch_decrypt", group_id, account_id, items: [{key, ciphertext_b64}, ...] }
```

### Cost Optimization
```javascript
// ❌ Bad: 3 separate calls
await encrypt("field1");
await encrypt("field2");
await encrypt("field3");

// ✅ Good: 1 batch call
await batch_encrypt([
  { key: "field1", plaintext_b64: "..." },
  { key: "field2", plaintext_b64: "..." },
  { key: "field3", plaintext_b64: "..." }
]);
```

## Performance

| Source | First Call | Cached |
|--------|-----------|--------|
| GitHub | ~30s | ~0.7s |
| WasmUrl | ~1.2s | ~0.9s |

**Cost:** ~$0.005 per cached call
