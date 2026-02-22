require('dotenv').config();
const { connect, keyStores, utils } = require('near-api-js');
const { KeyPair } = require('@near-js/crypto');
const { randomBytes, createCipheriv, createDecipheriv } = require('crypto');
const bs58 = require('bs58');

const ACCOUNT_ID = process.env.NEAR_ACCOUNT_ID;
const PRIVATE_KEY = process.env.NEAR_PRIVATE_KEY;

if (!ACCOUNT_ID || !PRIVATE_KEY) {
  console.error('❌ Set NEAR_ACCOUNT_ID and NEAR_PRIVATE_KEY in .env');
  process.exit(1);
}

console.log('🔐 E2E Test with .env Key\n');
console.log(`Account: ${ACCOUNT_ID}`);
console.log(`Key format: ${PRIVATE_KEY.includes(':') ? 'base58' : 'hex'}\n`);

// Convert key if hex
let base58Key = PRIVATE_KEY;
if (!PRIVATE_KEY.includes(':')) {
  const bytes = Buffer.from(PRIVATE_KEY, 'hex');
  base58Key = 'ed25519:' + bs58.encode(bytes);
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
  // Setup key store
  const keyStore = new keyStores.InMemoryKeyStore();
  const keyPair = KeyPair.fromString(base58Key);
  
  // Show public key
  const publicKey = keyPair.getPublicKey().toString();
  console.log(`Public Key: ${publicKey}\n`);

  try {
    await keyStore.setKey('mainnet', ACCOUNT_ID, keyPair);
  } catch (err) {
    console.error('❌ Invalid key format:', err.message);
    process.exit(1);
  }

  // Connect to NEAR
  const near = await connect({
    networkId: 'mainnet',
    nodeUrl: 'https://rpc.mainnet.near.org',
    keyStore
  });

  const account = await near.account(ACCOUNT_ID);
  
  try {
    const balance = await account.getAccountBalance();
    console.log(`Balance: ${utils.format.formatNearAmount(balance.total, 4)} NEAR\n`);
  } catch (err) {
    console.error('❌ Cannot access account - key not registered');
    console.error('   Add this public key to your account:');
    console.error(`   ${publicKey}\n`);
    process.exit(1);
  }

  const testKey = `test-${Date.now()}`;
  const plaintext = 'E2E test from .env! 🔐';

  console.log(`Test Key: ${testKey}`);
  console.log(`Plaintext: "${plaintext}"\n`);

  // 1. Generate AES key
  const aesKey = randomBytes(32);
  console.log('1️⃣ Encrypt locally');
  const ciphertext = encrypt(plaintext, aesKey);

  // 2. Wrap key
  console.log('2️⃣ Wrap key');
  const wrappedKey = mockWrap(aesKey, `mock-${ACCOUNT_ID}`);

  // 3. Create entry
  const entry = {
    wrapped_key: wrappedKey,
    ciphertext,
    key_id: 'mock-key-id',
    algorithm: 'AES-256-GCM',
    v: 1,
  };
  const entryB64 = Buffer.from(JSON.stringify(entry)).toString('base64');

  // 4. Write to blockchain
  console.log('3️⃣ Writing to blockchain...');
  const result = await account.functionCall({
    contractId: 'contextual.near',
    methodName: '__fastdata_kv',
    args: { data: entryB64 },
    gas: '300000000000000',
    attachedDeposit: '0',
  });

  console.log(`\n✅ TX: ${result.transaction.hash}`);
  console.log(`   View: https://explorer.near.org/transactions/${result.transaction.hash}\n`);

  // 5. Decrypt
  console.log('4️⃣ Decrypt locally');
  const decrypted = decrypt(entry.ciphertext, aesKey);
  console.log(`   "${decrypted}"`);
  console.log(`   Matches: ${decrypted === plaintext} ✅\n`);

  console.log('🎉 E2E Test Complete!');
}

main().catch(err => {
  console.error('\n❌ Error:', err.message);
  if (err.type === 'KeyNotFound') {
    console.error('\n💡 Key not registered for this account');
    console.error('   Add it at: https://wallet.near.org');
  }
  process.exit(1);
});
