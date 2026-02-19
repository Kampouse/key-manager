/**
 * PrivateKV - Client-side encryption with TEE key wrapping
 * 
 * Privacy-first key-value storage where plaintext NEVER leaves your device.
 * 
 * Flow:
 * 1. Generate ephemeral encryption key locally
 * 2. Encrypt data locally (plaintext never sent)
 * 3. Wrap ephemeral key with TEE's group key
 * 4. Store: wrapped_key + ciphertext
 * 5. Decrypt: unwrap key with TEE, decrypt locally
 */

// ============ Types ============

export interface PrivateKVConfig {
  /** NEAR account ID */
  accountId: string;
  /** OutLayer payment key (optional - for off-chain execution) */
  paymentKey?: string;
  /** FastKV API URL */
  apiUrl?: string;
  /** Group ID suffix (default: "private") */
  groupSuffix?: string;
}

export interface EncryptedEntry {
  /** Wrapped encryption key (base64) */
  wrapped_key: string;
  /** Ciphertext (base64) */
  ciphertext: string;
  /** Key ID */
  key_id: string;
  /** Encryption algorithm */
  algorithm: "AES-256-GCM";
  /** Version */
  v: 1;
}

export interface GetGroupKeyIdResponse {
  key_id: string;
  group_id: string;
  algorithm: string;
}

export interface WrapKeyResponse {
  wrapped_key_b64: string;
  key_id: string;
}

export interface UnwrapKeyResponse {
  plaintext_key_b64: string;
  key_id: string;
}

// ============ Main Class ============

export class PrivateKV {
  private readonly accountId: string;
  private readonly paymentKey?: string;
  private readonly apiUrl: string;
  private readonly groupSuffix: string;
  
  // Cache for key IDs
  private keyIdCache = new Map<string, string>();

  constructor(config: PrivateKVConfig) {
    this.accountId = config.accountId;
    this.paymentKey = config.paymentKey;
    this.apiUrl = config.apiUrl || "https://fastkv-server-production-7f40.up.railway.app";
    this.groupSuffix = config.groupSuffix || "private";
  }

  // ============ Public Methods ============

  /**
   * Encrypt and store a value
   * 
   * Plaintext is encrypted locally and NEVER sent to server.
   */
  async encrypt(
    key: string,
    plaintext: string
  ): Promise<EncryptedEntry> {
    const groupId = this.getGroupId();

    // 1. Generate ephemeral AES-256 key
    const ephemeralKey = await this.generateAESKey();

    // 2. Encrypt plaintext locally
    const ciphertext = await this.encryptAES(plaintext, ephemeralKey);

    // 3. Export ephemeral key
    const ephemeralKeyB64 = await this.exportKey(ephemeralKey);

    // 4. Wrap key with TEE
    const wrappedKey = await this.wrapKey(groupId, ephemeralKeyB64);

    // 5. Return encrypted entry (plaintext never sent!)
    return {
      wrapped_key: wrappedKey.wrapped_key_b64,
      ciphertext,
      key_id: wrappedKey.key_id,
      algorithm: "AES-256-GCM",
      v: 1,
    };
  }

  /**
   * Decrypt an encrypted entry
   * 
   * Only the wrapped key is sent to TEE, not the ciphertext.
   * Decryption happens locally.
   */
  async decrypt(entry: EncryptedEntry): Promise<string> {
    const groupId = this.getGroupId();

    // 1. Unwrap key with TEE (only wrapped key sent)
    const unwrapped = await this.unwrapKey(groupId, entry.wrapped_key);

    // 2. Import ephemeral key
    const ephemeralKey = await this.importKey(unwrapped.plaintext_key_b64);

    // 3. Decrypt locally (ciphertext never sent to server)
    const plaintext = await this.decryptAES(entry.ciphertext, ephemeralKey);

    return plaintext;
  }

  /**
   * Store encrypted entry (optional helper)
   */
  async store(key: string, entry: EncryptedEntry): Promise<void> {
    // This would store to FastKV, IPFS, or any storage
    // For now, just serialize
    const serialized = JSON.stringify(entry);
    console.log(`[PrivateKV] Storing ${key}: ${serialized.length} bytes`);
  }

  /**
   * Get group key ID (public info, no auth needed)
   */
  async getGroupKeyId(): Promise<string> {
    const groupId = this.getGroupId();
    
    // Check cache
    if (this.keyIdCache.has(groupId)) {
      return this.keyIdCache.get(groupId)!;
    }

    // Call TEE
    const response = await this.callTEE("get_group_key_id", {
      group_id: groupId,
      account_id: this.accountId,
    });

    const data = response as GetGroupKeyIdResponse;
    this.keyIdCache.set(groupId, data.key_id);
    
    return data.key_id;
  }

  // ============ Private Methods ============

  private getGroupId(): string {
    return `${this.accountId}/${this.groupSuffix}`;
  }

  private async wrapKey(
    groupId: string,
    plaintextKeyB64: string
  ): Promise<WrapKeyResponse> {
    const response = await this.callTEE("wrap_key", {
      group_id: groupId,
      account_id: this.accountId,
      plaintext_key_b64: plaintextKeyB64,
    });

    return response as WrapKeyResponse;
  }

  private async unwrapKey(
    groupId: string,
    wrappedKeyB64: string
  ): Promise<UnwrapKeyResponse> {
    const response = await this.callTEE("unwrap_key", {
      group_id: groupId,
      account_id: this.accountId,
      wrapped_key_b64: wrappedKeyB64,
    });

    return response as UnwrapKeyResponse;
  }

  private async callTEE(
    action: string,
    params: Record<string, unknown>
  ): Promise<unknown> {
    // If payment key provided, use off-chain API
    if (this.paymentKey) {
      return await this.callOffChain(action, params);
    }

    // Otherwise, prepare on-chain transaction
    return await this.prepareTransaction(action, params);
  }

  private async callOffChain(
    action: string,
    params: Record<string, unknown>
  ): Promise<unknown> {
    const response = await fetch(`${this.apiUrl}/v1/kv/encrypted/tee-call`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Payment-Key": this.paymentKey!,
      },
      body: JSON.stringify({
        action,
        ...params,
      }),
    });

    if (!response.ok) {
      const error = await response.text();
      throw new Error(`TEE call failed: ${error}`);
    }

    return await response.json();
  }

  private async prepareTransaction(
    action: string,
    params: Record<string, unknown>
  ): Promise<unknown> {
    const response = await fetch(
      `${this.apiUrl}/v1/kv/encrypted/prepare-tee-call`,
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          action,
          ...params,
        }),
      }
    );

    if (!response.ok) {
      const error = await response.text();
      throw new Error(`Failed to prepare transaction: ${error}`);
    }

    const { transaction } = await response.json();
    
    // Return transaction for user to sign
    return {
      needsSignature: true,
      transaction,
      message: "Sign this transaction to execute in TEE",
    };
  }

  // ============ Crypto Helpers ============

  private async generateAESKey(): Promise<CryptoKey> {
    return await crypto.subtle.generateKey(
      { name: "AES-GCM", length: 256 },
      true,
      ["encrypt", "decrypt"]
    );
  }

  private async exportKey(key: CryptoKey): Promise<string> {
    const exported = await crypto.subtle.exportKey("raw", key);
    return this.arrayBufferToBase64(exported);
  }

  private async importKey(keyB64: string): Promise<CryptoKey> {
    const keyBytes = this.base64ToArrayBuffer(keyB64);
    return await crypto.subtle.importKey(
      "raw",
      keyBytes,
      { name: "AES-GCM", length: 256 },
      false,
      ["decrypt"]
    );
  }

  private async encryptAES(
    plaintext: string,
    key: CryptoKey
  ): Promise<string> {
    const iv = crypto.getRandomValues(new Uint8Array(12));
    const encoded = new TextEncoder().encode(plaintext);

    const ciphertext = await crypto.subtle.encrypt(
      { name: "AES-GCM", iv },
      key,
      encoded
    );

    // Combine IV + ciphertext
    const combined = new Uint8Array(iv.length + ciphertext.byteLength);
    combined.set(iv);
    combined.set(new Uint8Array(ciphertext), iv.length);

    return this.arrayBufferToBase64(combined.buffer as ArrayBuffer);
  }

  private async decryptAES(
    ciphertextB64: string,
    key: CryptoKey
  ): Promise<string> {
    const combined = this.base64ToArrayBuffer(ciphertextB64);
    
    // Extract IV and ciphertext
    const iv = combined.slice(0, 12);
    const ciphertext = combined.slice(12);

    const plaintext = await crypto.subtle.decrypt(
      { name: "AES-GCM", iv },
      key,
      ciphertext
    );

    return new TextDecoder().decode(plaintext);
  }

  private arrayBufferToBase64(buffer: ArrayBuffer): string {
    const bytes = new Uint8Array(buffer);
    let binary = "";
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

// ============ Usage Example ============

/*
async function example() {
  const kv = new PrivateKV({
    accountId: "kampouse.near",
    paymentKey: "pk_...", // Optional: for off-chain execution
  });

  // Encrypt (plaintext never leaves device)
  const encrypted = await kv.encrypt("password", "my secret 123");
  console.log("Encrypted:", encrypted);
  // { wrapped_key: "...", ciphertext: "...", key_id: "..." }

  // Store (optional - could be FastKV, IPFS, etc.)
  await kv.store("password", encrypted);

  // Decrypt (only wrapped key sent to TEE)
  const plaintext = await kv.decrypt(encrypted);
  console.log("Decrypted:", plaintext); // "my secret 123"
}
*/

export default PrivateKV;
