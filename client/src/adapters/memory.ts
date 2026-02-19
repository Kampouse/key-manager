import type { StorageAdapter, EncryptedEntry, TEEAdapter } from '../types';

/**
 * In-memory storage adapter for testing
 */
export class MemoryStorageAdapter implements StorageAdapter {
  private readonly store = new Map<string, EncryptedEntry>();

  async set(key: string, entry: EncryptedEntry): Promise<{ txHash?: string }> {
    this.store.set(key, entry);
    return {};
  }

  async get(key: string): Promise<EncryptedEntry | null> {
    return this.store.get(key) || null;
  }

  async delete(key: string): Promise<{ txHash?: string }> {
    this.store.delete(key);
    return {};
  }

  async list(prefix: string = ''): Promise<string[]> {
    const keys: string[] = [];
    for (const key of this.store.keys()) {
      if (key.startsWith(prefix)) {
        keys.push(key);
      }
    }
    return keys.sort();
  }

  /** Clear all stored data */
  clear(): void {
    this.store.clear();
  }

  /** Get number of stored entries */
  get size(): number {
    return this.store.size;
  }
}

/**
 * Mock TEE adapter for testing (NOT SECURE - do not use in production)
 */
export class MockTEEAdapter implements TEEAdapter {
  private readonly keys = new Map<string, { wrapped: string; plaintext: string }>();
  private keyIdCounter = 0;

  async wrapKey(
    groupId: string,
    plaintextKeyB64: string
  ): Promise<{ wrapped_key_b64: string; key_id: string }> {
    const keyId = this.generateKeyId();
    const wrapped = btoa(`wrapped:${plaintextKeyB64}:${groupId}:${keyId}`);
    
    this.keys.set(wrapped, { wrapped, plaintext: plaintextKeyB64 });
    
    return {
      wrapped_key_b64: wrapped,
      key_id: keyId,
    };
  }

  async unwrapKey(
    groupId: string,
    wrappedKeyB64: string
  ): Promise<{ plaintext_key_b64: string; key_id: string }> {
    const entry = this.keys.get(wrappedKeyB64);
    if (!entry) {
      throw new Error('Key not found');
    }
    
    return {
      plaintext_key_b64: entry.plaintext,
      key_id: this.extractKeyId(wrappedKeyB64),
    };
  }

  async getKeyId(groupId: string): Promise<string> {
    return this.generateKeyId();
  }

  private generateKeyId(): string {
    return `mock-key-${++this.keyIdCounter}`;
  }

  private extractKeyId(wrapped: string): string {
    const parts = atob(wrapped).split(':');
    return parts[3] || 'unknown';
  }
}
