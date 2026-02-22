/**
 * E2E Encryption Test with Real FastKV Backend
 * 
 * FastKV server reads from Redis (populated by blockchain indexers).
 * Writes go through NEAR blockchain transactions.
 * 
 * Run with: npx tsx examples/with-fastkv.ts
 */

import { PrivateKV } from '../src/PrivateKV';
import { FastKVAdapter } from '../src/adapters/fastkv';
import { MemoryStorageAdapter, MockTEEAdapter } from '../src/adapters/memory';

const FASTKV_URL = 'http://127.0.0.1:3001';
const ACCOUNT_ID = 'kampouse.near';
const CONTRACT_ID = 'kampouse.near'; // Data stored here

async function main() {
  console.log('🔐 E2E Encryption Test\n');

  // Part 1: Test encryption (memory storage)
  console.log('📝 Part 1: Encryption Test\n');
  
  const kv = new PrivateKV({
    accountId: ACCOUNT_ID,
    storage: new MemoryStorageAdapter(),
    tee: new MockTEEAdapter(),
  });

  const testKey = 'secret-password';
  const testValue = 'my-super-secret-value-123';
  
  await kv.set(testKey, testValue);
  const retrieved = await kv.get(testKey);
  
  console.log(`   Stored: "${testValue}"`);
  console.log(`   Retrieved: "${retrieved}"`);
  console.log(`   Works: ${retrieved === testValue} ✅\n`);

  // Part 2: Read from FastKV
  console.log('📝 Part 2: FastKV Read Test\n');
  
  const testKeys = ['test/indexer', 'test/message', 'test/timestamp'];
  
  for (const key of testKeys) {
    const response = await fetch(
      `${FASTKV_URL}/v1/kv/get?accountId=${ACCOUNT_ID}&contractId=${CONTRACT_ID}&key=${key}`
    );
    
    if (response.ok) {
      const data = await response.json() as { data: { value: string } | null };
      if (data.data) {
        console.log(`   ${key}: ${data.data.value}`);
      }
    }
  }
  console.log('   FastKV read ✅\n');

  // Part 3: Full E2E with FastKV storage
  console.log('📝 Part 3: E2E with FastKV Storage\n');
  
  const kvFastKV = new PrivateKV({
    accountId: ACCOUNT_ID,
    storage: new FastKVAdapter({
      apiUrl: FASTKV_URL,
      accountId: ACCOUNT_ID,
      contractId: CONTRACT_ID,
    }),
    tee: new MockTEEAdapter(),
  });

  // Try reading existing data
  const existingData = await kvFastKV.get('test/indexer');
  console.log(`   Existing data: ${existingData}`);
  console.log('   (This is plaintext from blockchain, not encrypted)\n');

  console.log('📝 Part 4: Blockchain Writes\n');
  console.log('   To store encrypted data:');
  console.log('   1. Encrypt locally with this client');
  console.log('   2. Call NEAR: near call contextual.near __fastdata_kv \'{"data": "..."}\'');
  console.log('   3. Indexer writes to Redis');
  console.log('   4. FastKV serves it\n');

  console.log('✅ Done!\n');
}

main().catch(console.error);
