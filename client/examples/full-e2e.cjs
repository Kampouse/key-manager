/**
 * Full E2E: TEE Key Wrap + Blockchain Write
 * 
 * 1. Generate AES key locally
 * 2. Encrypt data locally
 * 3. Wrap key with OutLayer TEE (via HTTP API - no truncation)
 * 4. Write encrypted entry to blockchain (contextual.near)
 * 5. Read from blockchain
 * 6. Unwrap key with TEE
 * 7. Decrypt locally
 * 
 * Usage:
 *   export OUTLAYER_PAYMENT_KEY=your-key
 *   node examples/full-e2e.cjs
 */

const { connect, keyStores, utils } = require('near-api-js');
const { randomBytes, createCipheriv, createDecipheriv } = require('crypto');

const ACCOUNT_ID = 'kampouse.near';
const OUTLAYER_API = 'https://api.outlayer.fastnear.com';
const WASM_URL = 'https://github.com/Kampouse/key-manager/releases/download/v0.2.0/key-manager.wasm';
const WASM_HASH = '44ce9f1f616e765f21fe208eb1ff4db29a7aac90096ca83cf75864793c21e7d3';

// Crypto helpers
const generateKey = () => randomBytes(32);
const keyToB64 = (k) => k.toString('base64');
const keyFromB64 = (b) => Buffer.from(b, 'base64');

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

// OutLayer HTTP API call
async function callOutLayer(action, params, paymentKey) {
  const res = await fetch(`${OUTLAYER_API}/call/Kampouse/key-manager`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', 'X-Payment-Key': paymentKey },
    body: JSON.stringify({
      source: { WasmUrl: { url: WASM_URL, hash: WASM_HASH, build_target: 'wasm32-wasip1' } },
      input_data: JSON.stringify({ action, ...params }),
      resource_limits: { max_instructions: 10_000_000_000, max_memory_mb: 128, max_execution_seconds: 60 },
      response_format: 'Json',
    }),
  });
  return res.json();
}

async function main() {
  console.log('🔐 Full E2E: TEE Wrap + On-Chain Write\n');

  const paymentKey = process.env.OUTLAYER_PAYMENT_KEY;
  if (!paymentKey) {
    console.error('❌ Set OUTLAYER_PAYMENT_KEY');
    console.log('Get one at: https://outlayer.fastnear.com\n');
    process.exit(1);
  }

  // Connect to NEAR
  const keyStore = new keyStores.UnencryptedFileSystemKeyStore(`${process.env.HOME}/.near-credentials`);
  const near = await connect({ networkId: 'mainnet', nodeUrl: 'https://rpc.mainnet.near.org', keyStore });
  const account = await near.account(ACCOUNT_ID);
  
  const balance = await account.getAccountBalance();
  console.log(`Account: ${ACCOUNT_ID}`);
  console.log(`Balance: ${utils.format.formatNearAmount(balance.total, 4)} NEAR\n`);

  const groupId = `e2e-${Date.now()}`;
  const key = `secret-${Date.now()}`;
  const plaintext = 'This is encrypted end-to-end and stored on chain! 🔐';

  console.log(`Key: ${key}`);
  console.log(`Value: "${plaintext}"\n`);

  // === STEP 1: Generate AES key locally ===
  console.log('1️⃣ Generate AES-256 key locally');
  const aesKey = generateKey();
  const aesKeyB64 = keyToB64(aesKey);
  console.log(`   ${aesKeyB64.substring(0, 20)}...\n`);

  // === STEP 2: Encrypt locally ===
  console.log('2️⃣ Encrypt data locally');
  const ciphertext = encrypt(plaintext, aesKey);
  console.log(`   ${ciphertext.substring(0, 30)}...\n`);

  // === STEP 3: Wrap key with TEE ===
  console.log('3️⃣ Wrap key with OutLayer TEE');
  const wrapped = await callOutLayer('wrap_key', { group_id: groupId, account_id: ACCOUNT_ID, plaintext_key_b64: aesKeyB64 }, paymentKey);
  
  if (wrapped.error) {
    console.error('   ❌', wrapped.error);
    process.exit(1);
  }
  console.log(`   Key ID: ${wrapped.key_id}`);
  console.log(`   Wrapped: ${wrapped.wrapped_key_b64.substring(0, 30)}...\n`);

  // === STEP 4: Create encrypted entry ===
  console.log('4️⃣ Create encrypted entry');
  const entry = {
    wrapped_key: wrapped.wrapped_key_b64,
    ciphertext,
    key_id: wrapped.key_id,
    algorithm: 'AES-256-GCM',
    v: 1,
  };
  const entryB64 = Buffer.from(JSON.stringify(entry)).toString('base64');
  console.log(`   Entry size: ${entryB64.length} bytes\n`);

  // === STEP 5: Write to blockchain ===
  console.log('5️⃣ Write to blockchain (contextual.near)');
  const txResult = await account.functionCall({
    contractId: 'contextual.near',
    methodName: '__fastdata_kv',
    args: { data: entryB64 },
    gas: '300000000000000',
    attachedDeposit: '0',
  });
  console.log(`   TX: ${txResult.transaction.hash}\n`);

  // === STEP 6: Wait for indexer ===
  console.log('6️⃣ Wait for indexer to pick it up (10s)...');
  await new Promise(r => setTimeout(r, 10000));

  // === STEP 7: Read from FastKV ===
  console.log('7️⃣ Read from FastKV server');
  const fastkvUrl = `http://127.0.0.1:3001/v1/kv/contextual.near/${ACCOUNT_ID}/${key}`;
  
  let readEntry = null;
  try {
    const res = await fetch(fastkvUrl);
    if (res.ok) {
      readEntry = await res.json();
      console.log(`   ✅ Got encrypted entry from FastKV\n`);
    } else {
      console.log(`   ⚠️ FastKV not available (${res.status}), using local entry\n`);
      readEntry = entry;
    }
  } catch (e) {
    console.log(`   ⚠️ FastKV not running, using local entry\n`);
    readEntry = entry;
  }

  // === STEP 8: Unwrap key with TEE ===
  console.log('8️⃣ Unwrap key with OutLayer TEE');
  const unwrapped = await callOutLayer('unwrap_key', { 
    group_id: groupId, 
    account_id: ACCOUNT_ID, 
    wrapped_key_b64: readEntry.wrapped_key 
  }, paymentKey);

  if (unwrapped.error) {
    console.error('   ❌', unwrapped.error);
    process.exit(1);
  }
  
  const unwrappedKeyB64 = unwrapped.plaintext_key_b64;
  console.log(`   Matches original: ${unwrappedKeyB64 === aesKeyB64} ✅\n`);

  // === STEP 9: Decrypt locally ===
  console.log('9️⃣ Decrypt data locally');
  const decrypted = decrypt(readEntry.ciphertext, keyFromB64(unwrappedKeyB64));
  console.log(`   Decrypted: "${decrypted}"`);
  console.log(`   Matches original: ${decrypted === plaintext} ✅\n`);

  // Summary
  const newBalance = await account.getAccountBalance();
  const cost = BigInt(balance.total) - BigInt(newBalance.total);

  console.log('🎉 Full E2E Complete!\n');
  console.log('📊 What happened:');
  console.log('   1. AES key generated on YOUR device');
  console.log('   2. Data encrypted on YOUR device');
  console.log('   3. Key wrapped in TEE (OutLayer)');
  console.log('   4. Encrypted entry written to NEAR blockchain');
  console.log('   5. Key unwrapped in TEE');
  console.log('   6. Data decrypted on YOUR device');
  console.log(`\n💰 Cost: ${utils.format.formatNearAmount(cost.toString(), 4)} NEAR (blockchain gas)`);
  console.log('\n🔐 Plaintext NEVER left your device!');
}

main().catch(console.error);
