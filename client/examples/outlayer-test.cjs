require('dotenv').config();
const { randomBytes, createCipheriv, createDecipheriv } = require('crypto');

const ACCOUNT_ID = process.env.NEAR_ACCOUNT_ID || 'kampouse.near';
const PAYMENT_KEY = process.env.OUTLAYER_PAYMENT_KEY;

if (!PAYMENT_KEY) {
  console.error('❌ Set OUTLAYER_PAYMENT_KEY in .env');
  process.exit(1);
}

// Crypto helpers
function encrypt(plaintext, key) {
  const iv = randomBytes(12);
  const cipher = createCipheriv('aes-256-gcm', key, iv);
  const enc = Buffer.concat([cipher.update(plaintext, 'utf8'), cipher.final()]);
  return Buffer.concat([iv, enc, cipher.getAuthTag()]).toString('base64');
}

function decrypt(ciphertextB64, key) {
  const data = Buffer.from(ciphertextB64, 'base64');
  const iv = data.subarray(0, 12);
  const authTag = data.subarray(data.length - 16);
  const ct = data.subarray(12, data.length - 16);
  const dec = createDecipheriv('aes-256-gcm', key, iv);
  dec.setAuthTag(authTag);
  return Buffer.concat([dec.update(ct), dec.final()]).toString('utf8');
}

async function callOutLayer(action, params) {
  const response = await fetch('https://api.outlayer.fastnear.com/call/kampouse.near/key-manager', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-Payment-Key': PAYMENT_KEY
    },
    body: JSON.stringify({
      source: {
        GitHub: {
          repo: 'github.com/Kampouse/key-manager',
          commit: 'v0.3.0',
          build_target: 'wasm32-wasip1'
        }
      },
      input: JSON.stringify({
        action,
        ...params
      }),
      resource_limits: {
        max_memory_mb: 128,
        max_instructions: 10000000000,
        max_execution_seconds: 60
      },
      response_format: 'Json'
    })
  });

  const text = await response.text();
  try {
    return JSON.parse(text);
  } catch {
    return { error: text };
  }
}

async function main() {
  console.log('🔐 OutLayer TEE Test\n');
  console.log(`Account: ${ACCOUNT_ID}`);
  console.log(`Payment Key: ${PAYMENT_KEY.split(':')[0]}:...:${PAYMENT_KEY.split(':')[2].substring(0, 10)}...\n`);

  const groupId = `test-${Date.now()}`;
  const plaintext = 'Hello OutLayer TEE! 🎉';

  // 1. Generate AES key
  console.log('1️⃣ Generate AES-256 key');
  const aesKey = randomBytes(32);
  const plaintextKeyB64 = aesKey.toString('base64');
  console.log(`   ${plaintextKeyB64.substring(0, 20)}...\n`);

  // 2. Encrypt data locally
  console.log('2️⃣ Encrypt locally');
  const ciphertext = encrypt(plaintext, aesKey);
  console.log(`   ${ciphertext.substring(0, 30)}...\n`);

  // 3. Wrap key with OutLayer TEE
  console.log('3️⃣ Wrap key with OutLayer TEE');
  const wrapResult = await callOutLayer('wrap_key', {
    group_id: groupId,
    account_id: ACCOUNT_ID,
    plaintext_key_b64: plaintextKeyB64
  });

  if (wrapResult.error) {
    console.error('❌ Wrap failed:', wrapResult.error);
    process.exit(1);
  }

  console.log(`   Wrapped: ${wrapResult.wrapped_key_b64?.substring(0, 30)}...`);
  console.log(`   Key ID: ${wrapResult.key_id}\n`);

  // 4. Unwrap key with OutLayer TEE
  console.log('4️⃣ Unwrap key with OutLayer TEE');
  const unwrapResult = await callOutLayer('unwrap_key', {
    group_id: groupId,
    account_id: ACCOUNT_ID,
    wrapped_key_b64: wrapResult.wrapped_key_b64
  });

  if (unwrapResult.error) {
    console.error('❌ Unwrap failed:', unwrapResult.error);
    process.exit(1);
  }

  console.log(`   Unwrapped: ${unwrapResult.plaintext_key_b64?.substring(0, 20)}...\n`);

  // 5. Verify round-trip
  console.log('5️⃣ Verify round-trip');
  const unwrappedKey = Buffer.from(unwrapResult.plaintext_key_b64, 'base64');
  
  if (unwrappedKey.equals(aesKey)) {
    console.log('   ✅ Keys match!\n');
  } else {
    console.error('   ❌ Keys do not match!\n');
    process.exit(1);
  }

  // 6. Decrypt
  console.log('6️⃣ Decrypt data');
  const decrypted = decrypt(ciphertext, aesKey);
  console.log(`   "${decrypted}"`);
  console.log(`   Matches: ${decrypted === plaintext} ✅\n`);

  console.log('🎉 OutLayer TEE Test Complete!');
}

main().catch(err => {
  console.error('\n❌ Error:', err.message);
  process.exit(1);
});
