const { connect, keyStores, utils } = require('near-api-js');
const { KeyPair } = require('@near-js/crypto');
const { randomBytes, createCipheriv, createDecipheriv } = require('crypto');
const bs58 = require('bs58');

const ACCOUNT_ID = 'kampouse.near';
const PRIVATE_KEY_HEX = '14c9ed74e3fe9185f486a9a40d595bb6f455d8cc89b895b8dc3a18529fa8bf2f';

// Convert hex to base58
const privateKeyBytes = Buffer.from(PRIVATE_KEY_HEX, 'hex');
const base58Key = 'ed25519:' + bs58.encode(privateKeyBytes);

// Crypto helpers
const generateKey = () => randomBytes(32);

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

function mockWrap(key, groupId) {
  const wrapKey = Buffer.alloc(32);
  wrapKey.write(groupId.substring(0, 32));
  const wrapped = Buffer.alloc(key.length);
  for (let i = 0; i < key.length; i++) {
    wrapped[i] = key[i] ^ wrapKey[i % wrapKey.length];
  }
  return wrapped.toString('base64');
}

async function main() {
  console.log('🔐 E2E Test with Custom Key\n');
  console.log('Key (base58):', base58Key.substring(0, 20) + '...\n');

  // Setup key store
  const keyStore = new keyStores.InMemoryKeyStore();
  const keyPair = KeyPair.fromString(base58Key);
  await keyStore.setKey('mainnet', ACCOUNT_ID, keyPair);

  // Connect to NEAR
  const near = await connect({
    networkId: 'mainnet',
    nodeUrl: 'https://rpc.mainnet.near.org',
    keyStore
  });

  const account = await near.account(ACCOUNT_ID);
  const balance = await account.getAccountBalance();
  console.log(`Balance: ${utils.format.formatNearAmount(balance.total, 4)} NEAR\n`);

  const testKey = `test-${Date.now()}`;
  const plaintext = 'Testing E2E with custom signing key! 🔐';

  console.log(`Test Key: ${testKey}`);
  console.log(`Plaintext: "${plaintext}"\n`);

  // 1. Generate AES key
  console.log('1️⃣ Generate AES-256 key');
  const aesKey = generateKey();

  // 2. Encrypt
  console.log('2️⃣ Encrypt locally');
  const ciphertext = encrypt(plaintext, aesKey);
  console.log(`   ${ciphertext.substring(0, 30)}...\n`);

  // 3. Wrap key
  console.log('3️⃣ Wrap key (mock TEE)');
  const groupId = `mock-${ACCOUNT_ID}`;
  const wrappedKey = mockWrap(aesKey, groupId);

  // 4. Create entry
  const entry = {
    wrapped_key: wrappedKey,
    ciphertext,
    key_id: 'mock-key-id',
    algorithm: 'AES-256-GCM',
    v: 1,
  };
  const entryB64 = Buffer.from(JSON.stringify(entry)).toString('base64');

  // 5. Write to blockchain
  console.log('4️⃣ Writing to blockchain...');
  const result = await account.functionCall({
    contractId: 'contextual.near',
    methodName: '__fastdata_kv',
    args: { data: entryB64 },
    gas: '300000000000000',
    attachedDeposit: '0',
  });

  console.log(`\n✅ Written to blockchain!`);
  console.log(`   TX: ${result.transaction.hash}`);
  console.log(`   View: https://explorer.near.org/transactions/${result.transaction.hash}\n`);

  // 6. Decrypt
  console.log('5️⃣ Decrypting locally...');
  const decrypted = decrypt(entry.ciphertext, aesKey);
  console.log(`   Decrypted: "${decrypted}"`);
  console.log(`   Matches: ${decrypted === plaintext} ✅\n`);

  console.log('🎉 E2E Test Complete!');
}

main().catch(console.error);
