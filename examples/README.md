# Key Manager Examples

Real-world integration patterns for the OutLayer TEE Key Manager.

## TypeScript Clients

All examples use the `PrivateKV` class from `near-fastkv-encrypted`.

### Base Client

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
```

## Use Cases

| Example | Description | When to Use |
|---------|-------------|-------------|
| [profile-secrets](./profile-secrets) | Encrypted user profiles | Social apps, GDPR compliance |
| [shared-vault](./shared-vault) | Team secrets management | CI/CD, project secrets |
| [wallet-signer](./wallet-signer) | TEE wallet signer | Crypto wallets, key management |
| [fastkv-integration](./fastkv-integration) | FastKV storage client | Encrypted key-value store |

## Files

- `profile-secrets/` - User profile encryption with GDPR support
- `shared-vault/` - Team secrets with group access control
- `wallet-signer/` - TEE-based wallet signing
- `fastkv-integration/` - Full FastKV client example

## Running Examples

Each example is self-contained. Install dependencies and run:

```bash
cd examples/profile-secrets
npm install
npm start
```

## Testing

For testing without infrastructure, use mock adapters:

```typescript
import { PrivateKV, MemoryStorageAdapter, MockTEEAdapter } from 'near-fastkv-encrypted';

const kv = new PrivateKV({
  accountId: 'test.near',
  storage: new MemoryStorageAdapter(),
  tee: new MockTEEAdapter(),
});
```
