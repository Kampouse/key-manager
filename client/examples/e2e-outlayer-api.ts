/**
 * CLI Example: E2E Encryption with OutLayer HTTP API
 * 
 * Uses OutLayer's HTTP API (cheaper than blockchain calls)
 * Full end-to-end encryption flow.
 * 
 * Usage: 
 *   export OUTLAYER_PAYMENT_KEY=your-key
 *   npx tsx examples/e2e-outlayer-api.ts
 */

const OUTLAYER_API = 'https://api.outlayer.fastnear.com';
const WASM_URL = 'https://github.com/Kampouse/key-manager/releases/download/v0.2.0/key-manager.wasm';
const WASM_HASH = '44ce9f1f616e765f21fe208eb1ff4db29a7aac90096ca83cf75864793c21e7d3';

interface OutLayerResponse {
  wrapped_key_b64?: string;
  plaintext_key_b64?: string;
  key_id?: string;
  ciphertext_b64?: string;
  plaintext_b64?: string;
  plaintext_utf8?: string;
  error?: string;
}

async function callOutLayer(
  action: string,
  params: Record<string, unknown>,
  paymentKey: string
): Promise<OutLayerResponse> {
  const response = await fetch(`${OUTLAYER_API}/call/Kampouse/key-manager`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-Payment-Key': paymentKey,
    },
    body: JSON.stringify({
      source: {
        WasmUrl: {
          url: WASM_URL,
          hash: WASM_HASH,
          build_target: 'wasm32-wasip1',
        },
      },
      input_data: JSON.stringify({ action, ...params }),
      resource_limits: {
        max_instructions: 10_000_000_000,
        max_memory_mb: 128,
        max_execution_seconds: 60,
      },
      response_format: 'Json',
    }),
  });

  return response.json();
}

// Simple AES-256-GCM encryption (Node.js)
import { randomBytes, createCipheriv, createDecipheriv } from 'crypto';

function generateKey(): Buffer {
  return randomBytes(32);
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

async function main() {
  console.log('🔐 E2E Encryption with OutLayer HTTP API\n');

  const paymentKey = process.env.OUTLAYER_PAYMENT_KEY;
  if (!paymentKey) {
    console.error('❌ Set OUTLAYER_PAYMENT_KEY env var');
    console.log('\nGet a payment key from: https://outlayer.fastnear.com');
    process.exit(1);
  }

  const groupId = 'test-group-' + Date.now();
  const accountId = 'test-account.near';

  console.log(`Group: ${groupId}`);
  console.log(`Account: ${accountId}\n`);

  // Step 1: Generate local AES key
  console.log('📝 Step 1: Generate local AES-256 key');
  const localKey = generateKey();
  const localKeyB64 = localKey.toString('base64');
  console.log(`   Key: ${localKeyB64.substring(0, 20)}...\n`);

  // Step 2: Wrap key with OutLayer TEE
  console.log('📝 Step 2: Wrap key with OutLayer TEE');
  const wrapResult = await callOutLayer('wrap_key', {
    group_id: groupId,
    plaintext_key_b64: localKeyB64,
  }, paymentKey);

  if (wrapResult.error) {
    console.error('   ❌ Error:', wrapResult.error);
    process.exit(1);
  }

  const wrappedKey = wrapResult.wrapped_key_b64!;
  const keyId = wrapResult.key_id!;
  console.log(`   Key ID: ${keyId}`);
  console.log(`   Wrapped: ${wrappedKey.substring(0, 30)}...\n`);

  // Step 3: Encrypt data locally
  console.log('📝 Step 3: Encrypt data locally');
  const plaintext = 'This is a secret message! 🔐';
  const ciphertext = encrypt(plaintext, localKey);
  console.log(`   Plaintext: "${plaintext}"`);
  console.log(`   Ciphertext: ${ciphertext.substring(0, 30)}...\n`);

  // Step 4: Store encrypted data (simulated - in real use, store on blockchain)
  console.log('📝 Step 4: Store encrypted data');
  const encryptedEntry = {
    wrapped_key: wrappedKey,
    ciphertext,
    key_id: keyId,
    algorithm: 'AES-256-GCM',
    v: 1,
  };
  console.log('   Entry:', JSON.stringify({
    ...encryptedEntry,
    wrapped_key: wrappedKey.substring(0, 20) + '...',
    ciphertext: ciphertext.substring(0, 20) + '...',
  }, null, 2));
  console.log('   (In production: store on blockchain via contextual.near)\n');

  // Step 5: Unwrap key with OutLayer TEE
  console.log('📝 Step 5: Unwrap key with OutLayer TEE');
  const unwrapResult = await callOutLayer('unwrap_key', {
    group_id: groupId,
    wrapped_key_b64: wrappedKey,
  }, paymentKey);

  if (unwrapResult.error) {
    console.error('   ❌ Error:', unwrapResult.error);
    process.exit(1);
  }

  const unwrappedKeyB64 = unwrapResult.plaintext_key_b64!;
  console.log(`   Unwrapped: ${unwrappedKeyB64.substring(0, 20)}...`);
  console.log(`   Matches original: ${unwrappedKeyB64 === localKeyB64} ✅\n`);

  // Step 6: Decrypt data locally
  console.log('📝 Step 6: Decrypt data locally');
  const unwrappedKey = Buffer.from(unwrappedKeyB64, 'base64');
  const decrypted = decrypt(ciphertext, unwrappedKey);
  console.log(`   Decrypted: "${decrypted}"`);
  console.log(`   Matches original: ${decrypted === plaintext} ✅\n`);

  console.log('🎉 E2E encryption test complete!\n');
  console.log('📊 Summary:');
  console.log('   - Key generated locally: ✅');
  console.log('   - Key wrapped by OutLayer TEE: ✅');
  console.log('   - Data encrypted locally: ✅');
  console.log('   - Key unwrapped by OutLayer TEE: ✅');
  console.log('   - Data decrypted locally: ✅');
  console.log('\n🔐 Plaintext NEVER left your device!');
}

main().catch(console.error);
