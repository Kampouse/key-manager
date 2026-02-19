import type { 
  PrivateKVConfig, 
  EncryptedEntry, 
  SetOptions, 
  GetOptions, 
  GetResult 
} from './types';
import { createCryptoAdapter } from './crypto';
import type { CryptoAdapter } from './types';

/**
 * PrivateKV - Backend-agnostic end-to-end encrypted key-value storage
 * 
 * Features:
 * - Client-side AES-256-GCM encryption
 * - Pluggable storage backends (FastKV, S3, IPFS, etc.)
 * - Pluggable TEE backends (OutLayer, etc.)
 * - Works in browser and Node.js
 * - Full TypeScript support
 * 
 * @example
 * ```typescript
 * const kv = new PrivateKV({
 *   accountId: 'user.near',
 *   storage: new FastKVAdapter({ apiUrl: 'https://api.example.com' }),
 *   tee: new OutLayerAdapter({ contractId: 'outlayer.near' })
 * });
 * 
 * await kv.set('secret', 'my data');
 * const data = await kv.get('secret');
 * ```
 */
export class PrivateKV {
  private readonly accountId: string;
  private readonly namespace: string;
  private readonly groupSuffix: string;
  private readonly storage: PrivateKVConfig['storage'];
  private readonly tee: PrivateKVConfig['tee'];
  private readonly crypto: CryptoAdapter;
  private readonly keyIdCache = new Map<string, string>();

  constructor(config: PrivateKVConfig) {
    this.accountId = config.accountId;
    this.namespace = config.namespace || 'privatekv';
    this.groupSuffix = config.groupSuffix || 'private';
    this.storage = config.storage;
    this.tee = config.tee;
    this.crypto = config.crypto || createCryptoAdapter();
  }

  // ============ Public API ============

  /**
   * Encrypt and store a value
   * 
   * Flow:
   * 1. Generate ephemeral key locally
   * 2. Encrypt plaintext locally (AES-256-GCM)
   * 3. Wrap ephemeral key with TEE
   * 4. Store ciphertext + wrapped key
   */
  async set(key: string, plaintext: string, options?: SetOptions): Promise<{ txHash?: string }> {
    const groupId = this.getGroupId();
    const fullKey = this.getFullKey(key);

    // 1. Generate ephemeral AES-256 key
    const ephemeralKey = await this.crypto.generateKey();

    // 2. Encrypt plaintext locally
    const ciphertext = await this.crypto.encrypt(plaintext, ephemeralKey);

    // 3. Export ephemeral key
    const ephemeralKeyB64 = await this.crypto.exportKey(ephemeralKey);

    // 4. Wrap key with TEE
    const wrappedKey = await this.tee.wrapKey(groupId, ephemeralKeyB64);

    // 5. Build encrypted entry
    const entry: EncryptedEntry = {
      wrapped_key: wrappedKey.wrapped_key_b64,
      ciphertext,
      key_id: wrappedKey.key_id,
      algorithm: 'AES-256-GCM',
      v: 1,
    };

    // 6. Store via adapter
    return await this.storage.set(fullKey, entry);
  }

  /**
   * Retrieve and decrypt a value
   * 
   * Flow:
   * 1. Retrieve ciphertext + wrapped key
   * 2. Unwrap key with TEE (only wrapped key sent)
   * 3. Decrypt locally with unwrapped key
   */
  async get(key: string, options?: GetOptions): Promise<string | null> {
    const fullKey = this.getFullKey(key);
    const groupId = this.getGroupId();

    // 1. Retrieve from storage
    const entry = await this.storage.get(fullKey);
    if (!entry) return null;

    // 2. Unwrap key with TEE
    const unwrapped = await this.tee.unwrapKey(groupId, entry.wrapped_key);

    // 3. Import key and decrypt locally
    const ephemeralKey = await this.crypto.importKey(unwrapped.plaintext_key_b64);
    const plaintext = await this.crypto.decrypt(entry.ciphertext, ephemeralKey);

    return plaintext;
  }

  /**
   * Delete a key
   */
  async delete(key: string): Promise<{ txHash?: string }> {
    const fullKey = this.getFullKey(key);
    return await this.storage.delete(fullKey);
  }

  /**
   * List all keys with given prefix
   */
  async list(prefix: string = ''): Promise<string[]> {
    const fullPrefix = this.getFullKey(prefix);
    const entries = await this.storage.list(fullPrefix);
    // Strip the namespace/accountId/ prefix
    const prefixPattern = new RegExp(`^${this.namespace}/${this.accountId}/`);
    return entries.map(e => e.replace(prefixPattern, ''));
  }

  /**
   * Get the key ID for this group (public info)
   */
  async getKeyId(): Promise<string> {
    const groupId = this.getGroupId();
    
    if (this.keyIdCache.has(groupId)) {
      return this.keyIdCache.get(groupId)!;
    }

    if (this.tee.getKeyId) {
      const keyId = await this.tee.getKeyId(groupId);
      this.keyIdCache.set(groupId, keyId);
      return keyId;
    }

    // Generate a key to get the key ID
    const ephemeralKey = await this.crypto.generateKey();
    const keyB64 = await this.crypto.exportKey(ephemeralKey);
    const wrapped = await this.tee.wrapKey(groupId, keyB64);
    this.keyIdCache.set(groupId, wrapped.key_id);
    return wrapped.key_id;
  }

  // ============ Private Helpers ============

  private getGroupId(): string {
    return `${this.accountId}/${this.groupSuffix}`;
  }

  private getFullKey(key: string): string {
    return `${this.namespace}/${this.accountId}/${key}`;
  }
}

export default PrivateKV;
