/**
 * CLI Example: E2E Encryption with OutLayer TEE (Blockchain Transaction)
 * 
 * Uses real NEAR blockchain transactions to call OutLayer.
 * Costs 0.05 NEAR per wrap/unwrap operation.
 * 
 * Usage: npx tsx examples/e2e-outlayer-tx.ts
 */

import * as nearAPI from 'near-api-js';

const { connect, keyStores, utils } = nearAPI;

const ACCOUNT_ID = 'kampouse.near';
const NETWORK_ID = 'mainnet';

// Simple AES-256-GCM (Node.js crypto)
import { randomBytes, createCipheriv, createDecipheriv } from 'crypto';

function generateKey(): Buffer {
  return randomBytes(32);
}

function keyToBase64(key: Buffer): string {
  return key.toString('base64');
}

function keyFromBase64(b64: string): Buffer {
  return Buffer.from(b64, 'base64');
}

function encrypt(plaintext: string, key: Buffer): string {
  const iv = randomBytes(12);
  const cipher = createCipheriv('aes-256-gcm', key, iv);
  const encrypted = Buffer.concat([
    cipher.update(plaintext, 'utf8'),
    cipher.final(),
  ]);
  const authTag = cipher.getAuthTag();
  return Buffer.concat([iv, encrypted, authTag]).toString('base64');
}

function decrypt(ciphertextB64: string, key: Buffer): string {
  const combined = Buffer.from(ciphertextB64, 'base64');
  const iv = combined.subarray(0, 12);
  const authTag = combined.subarray(combined.length - 16);
  const ciphertext = combined.subarray(12, combined.length - 16);

  const decipher = createDecipheriv('aes-256-gcm', key, iv);
  decipher.setAuthTag(authTag);

  return Buffer.concat([
    decipher.update(ciphertext),
    decipher.final(),
  ]).toString('utf8');
}

// In-memory storage
const store = new Map<string, any>();

async function main() {
  console.log('🔐 E2E Encryption with OutLayer TEE (Blockchain)\n');
  console.log(`Account: ${ACCOUNT_ID}`);
  console.log(`Network: ${NETWORK_ID}\n`);

  // Connect to NEAR
  const keyStore = new keyStores.UnencryptedFileSystemKeyStore(
    `${process.env.HOME}/.near-credentials`
  );

  const near = await connect({
    networkId: NETWORK_ID,
    nodeUrl: 'https://rpc.mainnet.near.org',
    keyStore,
  });

  const account = await near.account(ACCOUNT_ID);
  const balance = await account.getAccountBalance();
  console.log(`Balance: ${utils.format.formatNearAmount(balance.total, 4)} NEAR\n`);

  // Test data
  const groupId = `test-group-${Date.now()}`;
  const testPlaintext = 'This is encrypted end-to-end via blockchain! 🔐';

  console.log('📝 E2E Encryption Test\n');
  console.log(`Group: ${groupId}`);
  console.log(`Plaintext: "${testPlaintext}"\n`);

  // Step 1: Generate key locally
  console.log('Step 1: Generate AES-256 key locally');
  const localKey = generateKey();
  const localKeyB64 = keyToBase64(localKey);
  console.log(`   Key: ${localKeyB64.substring(0, 20)}...\n`);

  // Step 2: Wrap key with OutLayer
  console.log('Step 2: Wrap key with OutLayer TEE (0.05 NEAR)');
  const wrapResult = await account.functionCall({
    contractId: 'outlayer.near',
    methodName: 'request_execution',
    args: {
      source: {
        GitHub: {
          repo: 'github.com/Kampouse/key-manager',
          commit: 'v0.3.0',
          build_target: 'wasm32-wasip1',
        },
      },
      input_data: JSON.stringify({
        action: 'wrap_key',
        group_id: groupId,
        plaintext_key_b64: localKeyB64,
      }),
      resource_limits: {
        max_instructions: 10000000000,
        max_memory_mb: 128,
        max_execution_seconds: 60,
      },
      response_format: 'Json',
    },
    gas: '300000000000000',
    attachedDeposit: utils.format.parseNearAmount('0.05')!,
  });

  console.log(`   TX: ${wrapResult.transaction.hash}`);
  
  // Parse wrapped key from result
  const wrappedData = parseTEEResult(wrapResult);
  if (!wrappedData.wrapped_key_b64) {
    console.error('   ❌ No wrapped_key in response');
    console.log('   Logs:', wrapResult.receipts_outcome[0]?.outcome?.logs);
    process.exit(1);
  }
  
  const wrappedKey = wrappedData.wrapped_key_b64;
  const keyId = wrappedData.key_id || 'unknown';
  console.log(`   Key ID: ${keyId}`);
  console.log(`   Wrapped: ${wrappedKey.substring(0, 30)}...\n`);

  // Step 3: Encrypt locally
  console.log('Step 3: Encrypt data locally (AES-256-GCM)');
  const ciphertext = encrypt(testPlaintext, localKey);
  console.log(`   Ciphertext: ${ciphertext.substring(0, 30)}...\n`);

  // Step 4: Store (simulated)
  console.log('Step 4: Store encrypted entry');
  const entry = {
    wrapped_key: wrappedKey,
    ciphertext,
    key_id: keyId,
    algorithm: 'AES-256-GCM',
    v: 1,
  };
  store.set('test-secret', entry);
  console.log('   Stored in memory (in prod: blockchain)\n');

  // Step 5: Unwrap key with OutLayer
  console.log('Step 5: Unwrap key with OutLayer TEE (0.05 NEAR)');
  const unwrapResult = await account.functionCall({
    contractId: 'outlayer.near',
    methodName: 'request_execution',
    args: {
      source: {
        GitHub: {
          repo: 'github.com/Kampouse/key-manager',
          commit: 'v0.3.0',
          build_target: 'wasm32-wasip1',
        },
      },
      input_data: JSON.stringify({
        action: 'unwrap_key',
        group_id: groupId,
        wrapped_key_b64: wrappedKey,
      }),
      resource_limits: {
        max_instructions: 10000000000,
        max_memory_mb: 128,
        max_execution_seconds: 60,
      },
      response_format: 'Json',
    },
    gas: '300000000000000',
    attachedDeposit: utils.format.parseNearAmount('0.05')!,
  });

  console.log(`   TX: ${unwrapResult.transaction.hash}`);
  
  const unwrappedData = parseTEEResult(unwrapResult);
  if (!unwrappedData.plaintext_key_b64) {
    console.error('   ❌ No plaintext_key in response');
    console.log('   Logs:', unwrapResult.receipts_outcome[0]?.outcome?.logs);
    process.exit(1);
  }

  const unwrappedKeyB64 = unwrappedData.plaintext_key_b64;
  console.log(`   Unwrapped: ${unwrappedKeyB64.substring(0, 20)}...`);
  console.log(`   Matches original: ${unwrappedKeyB64 === localKeyB64} ✅\n`);

  // Step 6: Decrypt locally
  console.log('Step 6: Decrypt data locally');
  const unwrappedKey = keyFromBase64(unwrappedKeyB64);
  const decrypted = decrypt(ciphertext, unwrappedKey);
  console.log(`   Decrypted: "${decrypted}"`);
  console.log(`   Matches original: ${decrypted === testPlaintext} ✅\n`);

  // Summary
  const newBalance = await account.getAccountBalance();
  const cost = BigInt(balance.total) - BigInt(newBalance.total);
  
  console.log('🎉 E2E Encryption Complete!\n');
  console.log('📊 Summary:');
  console.log('   - Key generated locally: ✅');
  console.log('   - Wrapped by OutLayer TEE: ✅');
  console.log('   - Encrypted locally: ✅');
  console.log('   - Unwrapped by OutLayer TEE: ✅');
  console.log('   - Decrypted locally: ✅');
  console.log(`   - Total cost: ${utils.format.formatNearAmount(cost.toString(), 4)} NEAR`);
  console.log('\n🔐 Plaintext NEVER left your device!');
}

function parseTEEResult(result: any): any {
  // Check logs for return_data
  for (const outcome of result.receipts_outcome) {
    for (const log of outcome.outcome.logs) {
      if (log.includes('return_data')) {
        try {
          const parsed = JSON.parse(log);
          if (parsed.return_data) {
            return JSON.parse(parsed.return_data);
          }
        } catch {}
      }
      // Try direct parse
      if (log.includes('wrapped_key') || log.includes('plaintext_key')) {
        try {
          return JSON.parse(log);
        } catch {}
      }
    }
  }
  return {};
}

main().catch(console.error);
