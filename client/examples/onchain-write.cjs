/**
 * Write Encrypted Data On-Chain
 * 
 * - Uses mock TEE (no OutLayer payment needed)
 * - Writes encrypted entry to contextual.near
 * - Real blockchain transaction
 * 
 * Usage: node examples/onchain-write.cjs
 */

const { connect, keyStores, utils } = require('near-api-js');
const { randomBytes, createCipheriv, createDecipheriv } = require('crypto');

const ACCOUNT_ID = 'kampouse.near';

// Crypto
const generateKey = () => randomBytes(32);
const keyToB64 = (k) => k.toString('base64');
const keyFromB64 = (b) => Buffer.from(b, 'base64');

function encrypt(plaintext, key) {
  const iv = randomBytes(12);
  const cipher = createCipheriv('aes-256-gcm', key, iv);
  const enc = Buffer.concat([cipher.update(plaintext, 'utf8'), cipher.final()]);
  return Buffer.concat([iv, enc, cipher.getAuthTag()]).toString('base64');
}

// Simple XOR wrap (mock TEE - NOT SECURE, for demo only)
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
  console.log('🔐 Write Encrypted Data On-Chain\n');

  // Connect to NEAR
  const keyStore = new keyStores.UnencryptedFileSystemKeyStore(`${process.env.HOME}/.near-credentials`);
  const near = await connect({ networkId: 'mainnet', nodeUrl: 'https://rpc.mainnet.near.org', keyStore });
  const account = await near.account(ACCOUNT_ID);
  
  const balance = await account.getAccountBalance();
  console.log(`Account: ${ACCOUNT_ID}`);
  console.log(`Balance: ${utils.format.formatNearAmount(balance.total, 4)} NEAR\n`);

  // Data to encrypt
  const key = `test-${Date.now()}`;
  const plaintext = 'Secret data stored on NEAR blockchain! 🔐';

  console.log(`Key: ${key}`);
  console.log(`Plaintext: "${plaintext}"\n`);

  // 1. Generate AES key
  console.log('1️⃣ Generate AES-256 key locally');
  const aesKey = generateKey();
  console.log(`   ${keyToB64(aesKey).substring(0, 20)}...\n`);

  // 2. Encrypt
  console.log('2️⃣ Encrypt data locally');
  const ciphertext = encrypt(plaintext, aesKey);
  console.log(`   ${ciphertext.substring(0, 30)}...\n`);

  // 3. Create encrypted entry
  console.log('3️⃣ Create encrypted entry');
  const groupId = `mock-${ACCOUNT_ID}`;
  const wrappedKey = mockWrap(aesKey, groupId);
  
  const entry = {
    wrapped_key: wrappedKey,
    ciphertext,
    key_id: 'mock-key-id',
    algorithm: 'AES-256-GCM',
    v: 1,
  };
  
  const entryB64 = Buffer.from(JSON.stringify(entry)).toString('base64');
  console.log(`   Entry size: ${entryB64.length} bytes\n`);

  // 4. Write to blockchain
  console.log('4️⃣ Writing to blockchain (contextual.near)...');
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

  // 5. Show what was stored
  console.log('📦 Encrypted entry stored on chain:');
  console.log(JSON.stringify({
    ...entry,
    wrapped_key: wrappedKey.substring(0, 20) + '...',
    ciphertext: ciphertext.substring(0, 20) + '...',
  }, null, 2));

  // Cost
  const newBalance = await account.getAccountBalance();
  const cost = BigInt(balance.total) - BigInt(newBalance.total);
  console.log(`\n💰 Cost: ${utils.format.formatNearAmount(cost.toString(), 6)} NEAR`);
  console.log('\n🔐 Your plaintext NEVER left your device!');
}

main().catch(console.error);
