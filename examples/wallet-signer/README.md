# Use Case 3: TEE Wallet Signer

**Scenario:** Crypto wallet app where private keys NEVER leave the TEE. Users can sign transactions without ever having access to their private key.

**Architecture:**
```
User → "Sign this tx" → Key Manager (TEE)
                              ↓
                      Derive key from CKD
                              ↓
                      Sign inside TEE
                              ↓
                      Return signature (not key!)
```

## Why This Matters

- **Key never exported:** Even the user doesn't know their private key
- **Cannot be stolen:** Phishing gets nothing - key never leaves TEE
- **Upgradeable:** Change code, same key (CKD persistence)
- **Recoverable:** Key derived from CKD, not stored

## Security Model

1. **Key Derivation:** `private_key = CKD(user_account_id)`
2. **Signing:** Happens inside TEE, returns only signature
3. **Storage:** No key storage needed - always derived fresh

**Note:** Current implementation uses mock signing. Production would use proper secp256k1 ECDSA.
