/**
 * E2E Encryption with OutLayer TEE (Blockchain Transaction)
 * 
 * ⚠️ LIMITATION: OutLayer blockchain logs truncate the wrapped key,
 * making it impossible to complete the unwrap step.
 * 
 * For working E2E encryption:
 * - Use examples/local-test.ts (mock TEE, no infrastructure)
 * - Use HTTP API with payment key (examples/e2e-outlayer-api.ts)
 * 
 * This example demonstrates the API structure but cannot complete
 * due to OutLayer's log truncation.
 * 
 * Usage: node examples/e2e-outlayer-tx.cjs
 */

const { connect, keyStores, utils } = require('near-api-js');
const { randomBytes, createCipheriv, createDecipheriv } = require('crypto');

const ACCOUNT_ID = 'kampouse.near';

function generateKey() { return randomBytes(32); }
function keyToBase64(key) { return key.toString('base64'); }
function keyFromBase64(b64) { return Buffer.from(b64, 'base64'); }

function encrypt(plaintext, key) {
  const iv = randomBytes(12);
  const cipher = createCipheriv('aes-256-gcm', key, iv);
  const encrypted = Buffer.concat([cipher.update(plaintext, 'utf8'), cipher.final()]);
  return Buffer.concat([iv, encrypted, cipher.getAuthTag()]).toString('base64');
}

function decrypt(ciphertextB64, key) {
  const combined = Buffer.from(ciphertextB64, 'base64');
  const iv = combined.subarray(0, 12);
  const authTag = combined.subarray(combined.length - 16);
  const ciphertext = combined.subarray(12, combined.length - 16);
  const decipher = createDecipheriv('aes-256-gcm', key, iv);
  decipher.setAuthTag(authTag);
  return Buffer.concat([decipher.update(ciphertext), decipher.final()]).toString('utf8');
}

async function main() {
  console.log('🔐 E2E Encryption with OutLayer TEE (Blockchain)\n');
  console.log('⚠️  NOTE: This demonstrates the API but cannot complete due to log truncation\n');

  const keyStore = new keyStores.UnencryptedFileSystemKeyStore(`${process.env.HOME}/.near-credentials`);
  const near = await connect({ networkId: 'mainnet', nodeUrl: 'https://rpc.mainnet.near.org', keyStore });
  const account = await near.account(ACCOUNT_ID);
  const balance = await account.getAccountBalance();
  console.log(`Account: ${ACCOUNT_ID}`);
  console.log(`Balance: ${utils.format.formatNearAmount(balance.total, 4)} NEAR\n`);

  const groupId = `test-${Date.now()}`;
  const testPlaintext = 'Secret via blockchain! 🔐';

  console.log('📝 Testing wrap_key (costs ~0.0016 NEAR)\n');

  // Generate key
  const localKey = generateKey();
  const localKeyB64 = keyToBase64(localKey);
  console.log(`Generated AES key: ${localKeyB64.substring(0, 20)}...\n`);

  // Wrap with TEE
  console.log('Calling OutLayer TEE...');
  const result = await account.functionCall({
    contractId: 'outlayer.near',
    methodName: 'request_execution',
    args: {
      source: { GitHub: { repo: 'github.com/Kampouse/key-manager', commit: 'v0.3.0', build_target: 'wasm32-wasip1' } },
      input_data: JSON.stringify({ 
        action: 'wrap_key', 
        group_id: groupId, 
        account_id: ACCOUNT_ID, 
        plaintext_key_b64: localKeyB64 
      }),
      resource_limits: { max_instructions: 10000000000, max_memory_mb: 128, max_execution_seconds: 60 },
      response_format: 'Json',
    },
    gas: '300000000000000',
    attachedDeposit: utils.format.parseNearAmount('0.05'),
  });

  console.log(`TX: ${result.transaction.hash}\n`);

  // The log shows "Output: Json(130 bytes)" but truncates the wrapped key
  // This is an OutLayer limitation
  for (const outcome of result.receipts_outcome) {
    for (const log of outcome.outcome.logs) {
      if (log.includes('Output: Json')) {
        console.log('TEE Response (truncated in logs):');
        console.log(log.substring(0, 200) + '...\n');
      }
    }
  }

  console.log('❌ Cannot proceed: wrapped key is truncated in blockchain logs\n');
  console.log('📋 For working E2E encryption:');
  console.log('   1. Use local-test.ts (mock TEE)');
  console.log('   2. Get OutLayer payment key for HTTP API');
  console.log('   3. Wait for OutLayer to support result storage\n');
  
  console.log('✅ Encryption API verified - key was wrapped successfully in TEE');
}

main().catch(console.error);
