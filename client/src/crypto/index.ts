import type { CryptoAdapter } from '../types';

/**
 * Browser/Web Crypto API implementation
 */
export class BrowserCryptoAdapter implements CryptoAdapter {
  async generateKey(): Promise<CryptoKey> {
    return await crypto.subtle.generateKey(
      { name: 'AES-GCM', length: 256 },
      true,
      ['encrypt', 'decrypt']
    );
  }

  async exportKey(key: CryptoKey): Promise<string> {
    const exported = await crypto.subtle.exportKey('raw', key);
    return this.arrayBufferToBase64(exported);
  }

  async importKey(keyB64: string): Promise<CryptoKey> {
    const keyBytes = this.base64ToArrayBuffer(keyB64);
    return await crypto.subtle.importKey(
      'raw',
      keyBytes.buffer as ArrayBuffer,
      { name: 'AES-GCM', length: 256 },
      false,
      ['decrypt']
    );
  }

  async encrypt(plaintext: string, key: CryptoKey): Promise<string> {
    const iv = crypto.getRandomValues(new Uint8Array(12));
    const encoded = new TextEncoder().encode(plaintext);

    const ciphertext = await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv },
      key,
      encoded
    );

    // Combine IV + ciphertext
    const combined = new Uint8Array(iv.length + ciphertext.byteLength);
    combined.set(iv);
    combined.set(new Uint8Array(ciphertext), iv.length);

    return this.arrayBufferToBase64(combined.buffer as ArrayBuffer);
  }

  async decrypt(ciphertextB64: string, key: CryptoKey): Promise<string> {
    const combined = this.base64ToArrayBuffer(ciphertextB64);
    
    const iv = combined.slice(0, 12);
    const ciphertext = combined.slice(12);

    const plaintext = await crypto.subtle.decrypt(
      { name: 'AES-GCM', iv },
      key,
      ciphertext
    );

    return new TextDecoder().decode(plaintext);
  }

  private arrayBufferToBase64(buffer: ArrayBuffer): string {
    const bytes = new Uint8Array(buffer);
    let binary = '';
    for (let i = 0; i < bytes.byteLength; i++) {
      binary += String.fromCharCode(bytes[i]);
    }
    return btoa(binary);
  }

  private base64ToArrayBuffer(base64: string): Uint8Array {
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    return bytes;
  }
}

/**
 * Node.js crypto implementation
 */
export class NodeCryptoAdapter implements CryptoAdapter {
  private crypto: typeof import('crypto');

  constructor() {
    this.crypto = require('crypto');
  }

  async generateKey(): Promise<Buffer> {
    return this.crypto.randomBytes(32);
  }

  async exportKey(key: Buffer): Promise<string> {
    return key.toString('base64');
  }

  async importKey(keyB64: string): Promise<Buffer> {
    return Buffer.from(keyB64, 'base64');
  }

  async encrypt(plaintext: string, key: Buffer): Promise<string> {
    const iv = this.crypto.randomBytes(12);
    const cipher = this.crypto.createCipheriv('aes-256-gcm', key, iv);
    const encrypted = Buffer.concat([
      cipher.update(plaintext, 'utf8'),
      cipher.final()
    ]);
    const authTag = cipher.getAuthTag();
    return Buffer.concat([iv, encrypted, authTag]).toString('base64');
  }

  async decrypt(ciphertextB64: string, key: Buffer): Promise<string> {
    const combined = Buffer.from(ciphertextB64, 'base64');
    const iv = combined.subarray(0, 12);
    const authTag = combined.subarray(combined.length - 16);
    const ciphertext = combined.subarray(12, combined.length - 16);
    
    const decipher = this.crypto.createDecipheriv('aes-256-gcm', key, iv);
    decipher.setAuthTag(authTag);
    
    return Buffer.concat([
      decipher.update(ciphertext),
      decipher.final()
    ]).toString('utf8');
  }
}

/**
 * Auto-detect and return appropriate crypto adapter for environment
 */
export function createCryptoAdapter(): CryptoAdapter {
  if (typeof crypto !== 'undefined' && crypto.subtle) {
    return new BrowserCryptoAdapter();
  }
  return new NodeCryptoAdapter();
}
