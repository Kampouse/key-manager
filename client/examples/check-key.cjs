const { KeyPair } = require('@near-js/crypto');
const bs58 = require('bs58');

const PRIVATE_KEY_HEX = '14c9ed74e3fe9185f486a9a40d595bb6f455d8cc89b895b8dc3a18529fa8bf2f';

const privateKeyBytes = Buffer.from(PRIVATE_KEY_HEX, 'hex');
const base58Key = 'ed25519:' + bs58.encode(privateKeyBytes);

const keyPair = KeyPair.fromString(base58Key);
const publicKey = keyPair.getPublicKey().toString();

console.log('Private Key (hex):', PRIVATE_KEY_HEX);
console.log('Private Key (base58):', base58Key);
console.log('Public Key:', publicKey);
console.log('\nCheck if this key has access:');
console.log(`https://explorer.near.org/accounts/kampouse.near?tab=keys`);
