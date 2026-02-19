#!/usr/bin/env node
/**
 * PrivateKV + FastKV Full Integration Test
 * 
 * Demonstrates end-to-end encrypted storage with FastKV backend
 */

const { spawn } = require('child_process');
const crypto = require('crypto');

// ============ Config ============

const CONFIG = {
  accountId: 'kampouse.near',
  apiUrl: 'https://near.garden',
  outlayerContract: 'outlayer.near',
  keyManagerVersion: 'v0.3.0',
  contextualContract: 'contextual.near',
};

// ============ Crypto Helpers ============

function generateKey() {
  return crypto.randomBytes(32);
}

function encryptAES(plaintext, key) {
  const iv = crypto.randomBytes(12);
  const cipher = crypto.createCipheriv('aes-256-gcm', key, iv);
  const encrypted = Buffer.concat([cipher.update(plaintext, 'utf8'), cipher.final()]);
  const authTag = cipher.getAuthTag();
  return Buffer.concat([iv, encrypted, authTag]);
}

function decryptAES(combined, key) {
  const iv = combined.subarray(0, 12);
  const authTag = combined.subarray(combined.length - 16);
  const ciphertext = combined.subarray(12, combined.length - 16);
  
  const decipher = crypto.createDecipheriv('aes-256-gcm', key, iv);
  decipher.setAuthTag(authTag);
  
  return Buffer.concat([decipher.update(ciphertext), decipher.final()]).toString('utf8');
}

function toBase64(buf) {
  return buf.toString('base64');
}

function fromBase64(str) {
  return Buffer.from(str, 'base64');
}

// ============ TEE Operations ============

async function callTEE(action, params) {
  const input = JSON.stringify({ action, ...params });
  
  const args = JSON.stringify({
    source: {
      GitHub: {
        repo: "github.com/Kampouse/key-manager",
        commit: CONFIG.keyManagerVersion,
        build_target: "wasm32-wasip1"
      }
    },
    input_data: input,
    resource_limits: {
      max_instructions: 10000000000,
      max_memory_mb: 128,
      max_execution_seconds: 60
    },
    response_format: "Json"
  });

  return new Promise((resolve, reject) => {
    const proc = spawn('near', [
      'call', CONFIG.outlayerContract, 'request_execution',
      args,
      '--accountId', CONFIG.accountId,
      '--networkId', 'mainnet',
      '--deposit', '0.1',
      '--gas', '300000000000000'
    ], { stdio: ['inherit', 'pipe', 'inherit'] });

    let output = '';
    proc.stdout.on('data', (data) => { output += data; });

    proc.on('close', (code) => {
      if (code !== 0) {
        reject(new Error(`TEE call failed with code ${code}`));
        return;
      }

      // Parse JSON from output (skip NEAR CLI logs)
      const lines = output.split('\n');
      for (const line of lines) {
        try {
          const parsed = JSON.parse(line);
          if (parsed.key_id || parsed.wrapped_key_b64 || wrapped.plaintext_key_b64) {
            resolve(parsed);
            return;
          }
        } catch (e) {}
      }
      
      // Try to extract from last object-looking thing
      const match = output.match(/\{[^{}]*\}/g);
      if (match) {
        try {
          resolve(JSON.parse(match[match.length - 1]));
        } catch (e) {
          reject(new Error(`Failed to parse TEE output: ${output}`));
        }
      } else {
        reject(new Error(`No JSON found in TEE output: ${output}`));
      }
    });
  });
}

async function wrapKey(plaintextKey) {
  const result = await callTEE('wrap_key', {
    group_id: `${CONFIG.accountId}/private`,
    account_id: CONFIG.accountId,
    plaintext_key_b64: toBase64(plaintextKey)
  });
  return result;
}

async function unwrapKey(wrappedKeyB64) {
  const result = await callTEE('unwrap_key', {
    group_id: `${CONFIG.accountId}/private`,
    account_id: CONFIG.accountId,
    wrapped_key_b64: wrappedKeyB64
  });
  return result;
}

// ============ FastKV Operations ============

async function storeOnFastKV(key, value) {
  const args = JSON.stringify({
    [key]: JSON.stringify(value)
  });

  return new Promise((resolve, reject) => {
    const proc = spawn('near', [
      'call', CONFIG.contextualContract, '__fastdata_kv',
      args,
      '--accountId', CONFIG.accountId,
      '--networkId', 'mainnet',
      '--deposit', '0.01'
    ], { stdio: 'inherit' });

    proc.on('close', (code) => {
      if (code !== 0) {
        reject(new Error(`FastKV store failed with code ${code}`));
      } else {
        resolve();
      }
    });
  });
}

async function getFromFastKV(key) {
  const url = `${CONFIG.apiUrl}/v1/kv/get?accountId=${CONFIG.accountId}&contractId=${CONFIG.contextualContract}&key=${encodeURIComponent(key)}&fields=value`;
  
  const response = await fetch(url);
  if (!response.ok) {
    if (response.status === 404) return null;
    throw new Error(`FastKV get failed: ${response.statusText}`);
  }
  
  const json = await response.json();
  const data = json.data;
  if (!data || !data.value) return null;
  
  return JSON.parse(data.value);
}

// ============ Main API ============

async function set(key, plaintext) {
  console.log(`\n📤 Encrypting "${plaintext}" for key "${key}"...`);
  
  // 1. Generate ephemeral key
  const ephemeralKey = generateKey();
  
  // 2. Encrypt locally
  const ciphertext = encryptAES(plaintext, ephemeralKey);
  
  // 3. Wrap key with TEE
  console.log('  🔐 Wrapping key with TEE...');
  const wrapped = await wrapKey(ephemeralKey);
  
  // 4. Build encrypted entry
  const entry = {
    wrapped_key: wrapped.wrapped_key_b64,
    ciphertext: toBase64(ciphertext),
    key_id: wrapped.key_id,
    algorithm: 'AES-256-GCM',
    v: 1
  };
  
  // 5. Store on FastKV
  const fullKey = `privatekv/${CONFIG.accountId}/${key}`;
  console.log(`  💾 Storing on FastKV (${fullKey})...`);
  await storeOnFastKV(fullKey, entry);
  
  console.log('  ✅ Done!');
  return entry;
}

async function get(key) {
  console.log(`\n📥 Retrieving key "${key}"...`);
  
  // 1. Get from FastKV
  const fullKey = `privatekv/${CONFIG.accountId}/${key}`;
  console.log(`  💾 Fetching from FastKV (${fullKey})...`);
  const entry = await getFromFastKV(fullKey);
  
  if (!entry) {
    console.log('  ❌ Key not found');
    return null;
  }
  
  // 2. Unwrap key with TEE
  console.log('  🔓 Unwrapping key with TEE...');
  const unwrapped = await unwrapKey(entry.wrapped_key);
  
  // 3. Decrypt locally
  const ephemeralKey = fromBase64(unwrapped.plaintext_key_b64);
  const ciphertext = fromBase64(entry.ciphertext);
  const plaintext = decryptAES(ciphertext, ephemeralKey);
  
  console.log(`  ✅ Decrypted: "${plaintext}"`);
  return plaintext;
}

// ============ CLI ============

async function main() {
  const [cmd, ...args] = process.argv.slice(2);
  
  switch (cmd) {
    case 'set':
      if (args.length < 2) {
        console.log('Usage: privatekv.js set <key> <plaintext>');
        process.exit(1);
      }
      await set(args[0], args.slice(1).join(' '));
      break;
      
    case 'get':
      if (args.length < 1) {
        console.log('Usage: privatekv.js get <key>');
        process.exit(1);
      }
      await get(args[0]);
      break;
      
    case 'test':
      console.log('=== PrivateKV + FastKV Integration Test ===\n');
      await set('greeting', 'hello world');
      await get('greeting');
      console.log('\n=== Test Complete ===');
      break;
      
    default:
      console.log(`
PrivateKV + FastKV Client

Usage:
  privatekv.js set <key> <plaintext>  - Encrypt and store
  privatekv.js get <key>              - Retrieve and decrypt
  privatekv.js test                   - Run full test
`);
      process.exit(1);
  }
}

main().catch(console.error);
