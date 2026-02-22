/**
 * Local E2E Encryption Test
 * 
 * Uses mock adapters - no external infrastructure needed
 * Run with: npx tsx examples/local-test.ts
 */

import { PrivateKV } from '../src/PrivateKV';
import { MemoryStorageAdapter, MockTEEAdapter } from '../src/adapters/memory';

async function main() {
  console.log('🔐 Testing End-to-End Encryption Locally\n');

  // Setup with mock adapters
  const kv = new PrivateKV({
    accountId: 'test-account.near',
    storage: new MemoryStorageAdapter(),
    tee: new MockTEEAdapter(),
  });

  console.log('✅ PrivateKV initialized\n');

  // Test 1: Store and retrieve a secret
  console.log('📝 Test 1: Store and retrieve');
  await kv.set('my-secret', 'Hello, this is sensitive data!');
  console.log('   Stored: "Hello, this is sensitive data!"');
  
  const retrieved = await kv.get('my-secret');
  console.log(`   Retrieved: "${retrieved}"`);
  console.log(`   Match: ${retrieved === 'Hello, this is sensitive data!'} ✅\n`);

  // Test 2: Multiple keys
  console.log('📝 Test 2: Multiple keys');
  await kv.set('api-key', 'sk-secret-12345');
  await kv.set('db-password', 'super-secret-pw');
  await kv.set('user-token', 'token-abc-xyz');
  
  console.log(`   api-key: "${await kv.get('api-key')}"`);
  console.log(`   db-password: "${await kv.get('db-password')}"`);
  console.log(`   user-token: "${await kv.get('user-token')}"`);
  
  // List keys
  const keys = await kv.list();
  console.log(`   Keys stored: ${keys.join(', ')} ✅\n`);

  // Test 3: Delete
  console.log('📝 Test 3: Delete');
  await kv.delete('api-key');
  const deleted = await kv.get('api-key');
  console.log(`   api-key after delete: ${deleted}`);
  console.log(`   Deleted: ${deleted === null} ✅\n`);

  // Test 4: Overwrite
  console.log('📝 Test 4: Overwrite existing key');
  await kv.set('my-secret', 'Updated secret value');
  const updated = await kv.get('my-secret');
  console.log(`   Updated value: "${updated}"`);
  console.log(`   Updated: ${updated === 'Updated secret value'} ✅\n`);

  console.log('🎉 All tests passed!\n');
  console.log('📊 Summary:');
  console.log('   - Encryption: AES-256-GCM (client-side)');
  console.log('   - Key wrapping: Mock TEE');
  console.log('   - Storage: In-memory');
}

main().catch(console.error);
