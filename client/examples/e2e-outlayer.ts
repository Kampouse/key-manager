/**
 * CLI Example: End-to-End Encryption with OutLayer TEE
 * 
 * Full flow:
 * 1. Generate AES key locally
 * 2. Encrypt data locally
 * 3. Wrap key with OutLayer TEE (0.05 NEAR)
 * 4. Store on blockchain via contextual.near
 * 5. Indexer writes to Redis
 * 6. FastKV serves encrypted data
 * 7. Unwrap key with OutLayer
 * 8. Decrypt locally
 * 
 * Usage: npx tsx examples/e2e-outlayer.ts
 * 
 * Requires:
 * - NEAR account with funds (0.05 NEAR per wrap/unwrap)
 * - Private key in NEAR_PRIVATE_KEY env var (or ~/.near-credentials)
 * - fastkv-server running locally
 */

import { PrivateKV } from '../src/PrivateKV';
import { FastKVAdapter } from '../src/adapters/fastkv';
import { OutLayerAdapter } from '../src/adapters/outlayer';
import * as nearAPI from 'near-api-js';
import { KeyPairEd25519 } from 'near-api-js';

const FASTKV_URL = process.env.FASTKV_URL || 'http://127.0.0.1:3001';
const ACCOUNT_ID = process.env.NEAR_ACCOUNT_ID || 'kampouse.near';
const PRIVATE_KEY = process.env.NEAR_PRIVATE_KEY;

// Or load from credentials
async function getCredentials(accountId: string) {
  if (PRIVATE_KEY) {
    return { accountId, keyPair: KeyPairEd25519.fromString(PRIVATE_KEY) };
  }
  
  // Try loading from ~/.near-credentials
  const fs = await import('fs');
  const path = await import('path');
  const os = await import('os');
  
  const credPath = path.join(
    os.homedir(), 
    '.near-credentials', 
    'mainnet',
    `${accountId}.json`
  );
  
  if (fs.existsSync(credPath)) {
    const cred = JSON.parse(fs.readFileSync(credPath, 'utf8'));
    return { 
      accountId, 
      keyPair: KeyPairEd25519.fromString(cred.private_key) 
    };
  }
  
  throw new Error(
    'No credentials found. Set NEAR_PRIVATE_KEY env var or ' +
    `create ${credPath}`
  );
}

async function main() {
  console.log('🔐 E2E Encryption with OutLayer TEE\n');
  console.log(`Account: ${ACCOUNT_ID}`);
  console.log(`FastKV: ${FASTKV_URL}\n`);

  // Get credentials
  const { keyPair } = await getCredentials(ACCOUNT_ID);
  console.log('✅ Credentials loaded\n');

  // Setup NEAR connection
  const near = await nearAPI.connect({
    networkId: 'mainnet',
    nodeUrl: 'https://rpc.mainnet.near.org',
    keyStore: {
      getKey: async () => keyPair,
      setKey: async () => {},
      clear: async () => {},
      getNetworks: async () => ['mainnet'],
      getKeyIds: async () => [ACCOUNT_ID],
    } as any,
  });

  const account = await near.account(ACCOUNT_ID);
  const balance = await account.getAccountBalance();
  console.log(`Balance: ${nearAPI.utils.format.formatNearAmount(balance.total, 4)} NEAR\n`);

  // Create OutLayer adapter with real signing
  const tee = new OutLayerAdapter({
    network: 'mainnet',
    deposit: '0.05 NEAR',
    signTransaction: async (tx) => {
      console.log(`   Calling ${tx.methodName} on ${tx.receiverId}...`);
      
      const result = await account.functionCall({
        contractId: tx.receiverId,
        methodName: tx.methodName,
        args: tx.args,
        gas: tx.gas || '300000000000000',
        attachedDeposit: nearAPI.utils.format.parseNearAmount(tx.deposit || '0.05')!,
      });

      console.log(`   TX: ${result.transaction.hash}`);
      
      // Parse TEE response from logs
      for (const outcome of result.receipts_outcome) {
        for (const log of outcome.outcome.logs) {
          if (log.includes('wrapped_key') || log.includes('plaintext_key')) {
            try {
              return JSON.parse(log);
            } catch {}
          }
        }
      }
      
      // Return transaction hash as fallback
      return { txHash: result.transaction.hash };
    },
  });

  // Create FastKV adapter for storage
  const storage = new FastKVAdapter({
    apiUrl: FASTKV_URL,
    accountId: ACCOUNT_ID,
    contractId: 'contextual.near',
  });

  // Initialize PrivateKV
  const kv = new PrivateKV({
    accountId: ACCOUNT_ID,
    storage,
    tee,
  });

  console.log('✅ PrivateKV initialized\n');

  // Test: Wrap a key with OutLayer
  console.log('📝 Test: Wrap key with OutLayer TEE\n');
  
  const testKey = `e2e-test-${Date.now()}`;
  const testValue = 'This is encrypted end-to-end!';
  
  console.log(`Key: ${testKey}`);
  console.log(`Value: "${testValue}"\n`);

  console.log('Step 1: Encrypting locally (AES-256-GCM)...');
  console.log('Step 2: Wrapping key with OutLayer (costs 0.05 NEAR)...\n');
  
  try {
    await kv.set(testKey, testValue);
    console.log('✅ Stored encrypted data!\n');
  } catch (error) {
    console.error('❌ Error:', error);
    return;
  }

  // Read it back
  console.log('Step 3: Reading from FastKV...');
  console.log('Step 4: Unwrapping key with OutLayer (costs 0.05 NEAR)...\n');
  
  try {
    const retrieved = await kv.get(testKey);
    console.log(`Retrieved: "${retrieved}"\n`);
    console.log(`Match: ${retrieved === testValue} ${retrieved === testValue ? '✅' : '❌'}\n`);
  } catch (error) {
    console.error('❌ Error:', error);
  }

  console.log('🎉 E2E encryption test complete!');
}

main().catch(console.error);
