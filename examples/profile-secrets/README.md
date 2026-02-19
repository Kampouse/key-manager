# Use Case 1: Encrypted User Profiles

**Scenario:** Social app where users store private data (email, phone, bio). You as the app developer want to prove you can NEVER access their private info.

**Architecture:**
```
Frontend (User) → Key Manager (TEE) → FastKV (Encrypted Storage)
                      ↓
              Key never leaves TEE
```

## Files

- `schema.json` - Data model
- `client.js` - Frontend SDK
- `api-server.js` - Your backend (blind to secrets)

## Why This Matters

- GDPR/HIPAA compliance: You literally cannot decrypt user data
- Trust: Users know their secrets are safe
- Data breach proof: Stealing DB = useless encrypted blobs
