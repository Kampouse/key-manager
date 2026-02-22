/**
 * E2E Encryption with OutLayer HTTP API
 * 
 * Uses OutLayer's HTTP API (no blockchain log truncation!)
 * Requires payment key from https://outlayer.fastnear.com
 * 
 * Usage:
 *   export OUTLAYER_PAYMENT_KEY=your-key
 *   npx tsx examples/e2e-outlayer-api.ts
 */

const OUTLAYER_API = 'https://api.outlayer.fastnear.com';
const WASM_URL = 'https://github.com/Kampouse/key-manager/releases/download/v0.2.0/key-manager.wasm';
const WASM_HASH = '44ce9f1f616e765f21fe208eb1ff4db29a7aac90096ca83cf75864793c21e7d3';

async function callOutLayer(action: string, params: Record<string, unknown>, paymentKey: string) {
  const response = await fetch(`${OUTLAYER_API}/call/Kampouse/key-manager`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-Payment-Key': paymentKey,
    },
    body: JSON.stringify({
      source: { WasmUrl: { url: WASM_URL, hash: WASM_HASH, build_target: 'wasm32-wasip1' } },
      input_data: JSON.stringify({ action, ...params }),
      resource_limits: { max_instructions: 10_000_000_000, max_memory_mb: 128, max_execution_seconds: 60 },
      response_format: 'Json',
    }),
  });
  return response.json();
}

import { randomBytes, createCipheriv, createDecipheriv } from 'crypto';

function generateKey() { return randomBytes(32); }
function encrypt(plaintext: string, key: Buffer) {
  const iv = randomBytes(12);
  const cipher = createCipheriv('aes-256-gcm', key, iv);
  const encrypted = Buffer.concat([cipher.update(plaintext, 'utf8'), cipher.final()]);
  return Buffer.concat([iv, encrypted, cipher.getAuthTag()]).toString('base64');
}
function decrypt(ciphertextB64: string, key: Buffer) {
  const combined = Buffer.from(ciphertextB64, 'base64');
  const iv = combined.subarray(0, 12);
  const authTag = combined.subarray(combined.length - 16);
  const ciphertext = combined.subarray(12, combined.length - 16);
  const decipher = createDecipheriv('aes-256-gcm', key, iv);
  decipher.setAuthTag(authTag);
  return Buffer.concat([decipher.update(ciphertext), decipher.final()]).toString('utf8');
}

async function main() {
  console.log('🔐 E2E Encryption with OutLayer HTTP API\n');

  const paymentKey = process.env.OUTLAYER_PAYMENT_KEY;
  if (!paymentKey) {
    console.error('❌ Set OUTLAYER_PAYMENT_KEY env var');
    console.log('\nGet a key at: https://outlayer.fastnear.com');
    process.exit(1);
  }

  const groupId = `test-${Date.now()}`;
  const accountId = 'test.near';
  const plaintext = 'Secret message! 🔐';

  console.log(`Group: ${groupId}`);
  console.log(`Plaintext: "${plaintext}"\n`);

  // 1. Generate key
  console.log('1. Generate AES-256 key locally');
  const key = generateKey();
  const keyB64 = key.toString('base64');
  console.log(`   ${keyB64.substring(0, 20)}...\n`);

  // 2. Wrap with TEE
  console.log('2. Wrap key with OutLayer TEE');
  const wrapped = await callOutLayer('wrap_key', { group_id: groupId, account_id: accountId, plaintext_key_b64: keyB64 }, paymentKey);
  if (wrapped.error) {
    console.error('   ❌', wrapped.error);
    process.exit(1);
  }
  console.log(`   Key ID: ${wrapped.key_id}`);
  console.log(`   Wrapped: ${wrapped.wrapped_key_b64.substring(0, 30)}...\n`);

  // 3. Encrypt locally
  console.log('3. Encrypt data locally');
  const ciphertext = encrypt(plaintext, key);
  console.log(`   ${ciphertext.substring(0, 30)}...\n`);

  // 4. Unwrap with TEE
  console.log('4. Unwrap key with OutLayer TEE');
  const unwrapped = await callOutLayer('unwrap_key', { group_id: groupId, account_id: accountId, wrapped_key_b64: wrapped.wrapped_key_b64 }, paymentKey);
  if (unwrapped.error) {
    console.error('   ❌', unwrapped.error);
    process.exit(1);
  }
  console.log(`   Matches original: ${unwrapped.plaintext_key_b64 === keyB64} ✅\n`);

  // 5. Decrypt locally
  console.log('5. Decrypt data locally');
  const decrypted = decrypt(ciphertext, Buffer.from(unwrapped.plaintext_key_b64, 'base64'));
  console.log(`   Decrypted: "${decrypted}"`);
  console.log(`   Matches: ${decrypted === plaintext} ✅\n`);

  console.log('🎉 E2E encryption complete!\n');
  console.log('🔐 Plaintext NEVER left your device!');
}

main().catch(console.error);
