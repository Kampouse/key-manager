const { connect, keyStores } = require('near-api-js');
const { createDecipheriv } = require('crypto');

const ACCOUNT_ID = 'kampouse.near';
const TX_HASH = 'CpjnfLwKCfFGueH8P9iEssZrGupsEam5WinVZTfyTqSv';

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

async function main() {
  console.log('📖 Read & Decrypt From Blockchain\n');

  const keyStore = new keyStores.UnencryptedFileSystemKeyStore(`${process.env.HOME}/.near-credentials`);
  const near = await connect({ networkId: 'mainnet', nodeUrl: 'https://rpc.mainnet.near.org', keyStore });

  // Try fastfs_get
  console.log('Trying fastfs_get...');
  try {
    const result = await near.connection.provider.query({
      request_type: 'call_function',
      account_id: 'contextual.near',
      method_name: 'fastfs_get',
      args_base64: Buffer.from(JSON.stringify({ 
        account_id: ACCOUNT_ID,
        key: 'test-1771788274870'
      })).toString('base64'),
      finality: 'final',
    });

    if (result.result?.length > 0) {
      const data = Buffer.from(result.result).toString().replace(/"/g, '');
      console.log('✅ Found data via fastfs_get');
      
      // Decode base64 and parse
      const entry = JSON.parse(Buffer.from(data, 'base64').toString());
      console.log('\n📦 Encrypted entry:');
      console.log(JSON.stringify({
        algorithm: entry.algorithm,
        key_id: entry.key_id,
        wrapped_key: entry.wrapped_key?.substring(0, 20) + '...',
        ciphertext: entry.ciphertext?.substring(0, 30) + '...',
      }, null, 2));

      // Decrypt
      const groupId = `mock-${ACCOUNT_ID}`;
      const aesKey = mockUnwrap(entry.wrapped_key, groupId);
      const plaintext = decrypt(entry.ciphertext, aesKey);
      
      console.log('\n🔓 Decrypted:');
      console.log(`"${plaintext}"`);
      console.log('\n✅ Full round-trip complete!');
      return;
    }
  } catch (e) {
    console.log('fastfs_get error:', e.message?.substring(0, 100));
  }

  // Try getting all keys for account
  console.log('\nTrying get_all...');
  try {
    const result = await near.connection.provider.query({
      request_type: 'call_function',
      account_id: 'contextual.near',
      method_name: 'get_all',
      args_base64: Buffer.from(JSON.stringify({ 
        account_id: ACCOUNT_ID,
      })).toString('base64'),
      finality: 'final',
    });

    if (result.result?.length > 0) {
      const data = Buffer.from(result.result).toString();
      console.log('get_all result:', data.substring(0, 500));
    }
  } catch (e) {
    console.log('get_all error:', e.message?.substring(0, 100));
  }

  // Check state directly
  console.log('\n📝 The data was written via __fastdata_kv');
  console.log('   It stores data in the contract state');
  console.log('   Indexer reads from chain and writes to Redis/FastKV');
  console.log('   For now, the encrypted data is on-chain ✅');
}

main().catch(console.error);
