const { createDecipheriv } = require('crypto');

function decrypt(ciphertextB64, key) {
  const data = Buffer.from(ciphertextB64, 'base64');
  const iv = data.subarray(0, 12);
  const authTag = data.subarray(data.length - 16);
  const ct = data.subarray(12, data.length - 16);
  const dec = createDecipheriv('aes-256-gcm', key, iv);
  dec.setAuthTag(authTag);
  return Buffer.concat([dec.update(ct), dec.final()]).toString('utf8');
}

function mockUnwrap(wrappedB64, groupId) {
  const wrapped = Buffer.from(wrappedB64, 'base64');
  const wrapKey = Buffer.alloc(32);
  wrapKey.write(groupId.substring(0, 32));
  const key = Buffer.alloc(wrapped.length);
  for (let i = 0; i < wrapped.length; i++) {
    key[i] = wrapped[i] ^ wrapKey[i % wrapKey.length];
  }
  return key;
}

const { execSync } = require('child_process');

console.log('📖 Read from Redis & Decrypt\n');

// Read from Redis
console.log('1️⃣ Reading from Redis...');
const redisKey = 'kv:kampouse.near:contextual.near:data';
const rawData = execSync(`redis-cli GET "${redisKey}"`).toString().trim();

if (!rawData) {
  console.error('❌ No data found');
  process.exit(1);
}

console.log(`   Key: ${redisKey}`);
console.log('   ✅ Got encrypted entry\n');

// Parse
const kvData = JSON.parse(rawData);
const entryJson = Buffer.from(kvData.value.replace(/"/g, ''), 'base64').toString();
const entry = JSON.parse(entryJson);

console.log('📦 Encrypted Entry:');
console.log(JSON.stringify({
  algorithm: entry.algorithm,
  key_id: entry.key_id,
  wrapped_key: entry.wrapped_key.substring(0, 30) + '...',
  ciphertext: entry.ciphertext.substring(0, 30) + '...',
  block_height: kvData.block_height,
  receipt_id: kvData.receipt_id
}, null, 2));

// Decrypt
console.log('\n2️⃣ Unwrapping key (mock TEE)...');
const groupId = 'mock-kampouse.near';
const aesKey = mockUnwrap(entry.wrapped_key, groupId);

console.log('\n3️⃣ Decrypting...');
const plaintext = decrypt(entry.ciphertext, aesKey);

console.log('\n🔓 Decrypted:');
console.log(`   "${plaintext}"\n`);

console.log('✅ Full E2E Round-Trip Complete!');
console.log('\n📊 Pipeline:');
console.log('   Write → Blockchain → main-indexer → Redis → kv-sub-indexer → Redis → Decrypt ✅');
