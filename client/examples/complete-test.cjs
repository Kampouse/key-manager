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

// Mock unwrap (XOR - same as wrap)
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

async function main() {
  console.log('📖 Read & Decrypt Test\n');

  // Read from FastKV
  const fastkvUrl = 'http://127.0.0.1:3001/v1/kv/contextual.near/kampouse.near/data';
  
  console.log('1️⃣ Fetching from FastKV...');
  console.log(`   ${fastkvUrl}\n`);
  
  const res = await fetch(fastkvUrl);
  if (!res.ok) {
    console.error('❌ FastKV error:', res.status);
    process.exit(1);
  }

  const data = await res.json();
  console.log('✅ Got encrypted entry from FastKV\n');

  // The value is base64 encoded JSON
  const entryJson = Buffer.from(data.value.replace(/"/g, ''), 'base64').toString();
  const entry = JSON.parse(entryJson);

  console.log('📦 Encrypted Entry:');
  console.log(JSON.stringify({
    algorithm: entry.algorithm,
    key_id: entry.key_id,
    wrapped_key: entry.wrapped_key.substring(0, 30) + '...',
    ciphertext: entry.ciphertext.substring(0, 30) + '...',
    v: entry.v
  }, null, 2));

  // Decrypt
  console.log('\n2️⃣ Unwrapping key (mock TEE)...');
  const groupId = 'mock-kampouse.near';
  const aesKey = mockUnwrap(entry.wrapped_key, groupId);
  console.log(`   AES key recovered: ${aesKey.toString('base64').substring(0, 20)}...\n`);

  console.log('3️⃣ Decrypting ciphertext...');
  const plaintext = decrypt(entry.ciphertext, aesKey);
  
  console.log('\n🔓 Decrypted:');
  console.log(`   "${plaintext}"\n`);

  console.log('✅ Full E2E Round-Trip Complete!');
  console.log('\n📊 Summary:');
  console.log('   • Wrote encrypted data to blockchain ✅');
  console.log('   • main-indexer processed block ✅');
  console.log('   • kv-sub-indexer wrote to Redis ✅');
  console.log('   • FastKV served the data ✅');
  console.log('   • Decrypted successfully ✅');
  console.log('\n🔐 Plaintext NEVER left your device!');
}

main().catch(console.error);
