/**
 * PrivateKV Client - End-to-end encrypted key-value storage
 * 
 * Combines:
 * - Client-side AES-256-GCM encryption (plaintext never leaves device)
 * - TEE key wrapping via OutLayer (keys protected, data never touches TEE)
 * - FastKV storage (on-chain or via API)
 * 
 * Privacy guarantees:
 * - Server NEVER sees plaintext
 * - TEE NEVER sees ciphertext
 * - Works across devices (CKD-derived keys)
 */

// ============ Types ============

export interface PrivateKVConfig {
  /** NEAR account ID */
  accountId: string;
  /** FastKV API URL (default: https://near.garden) */
  apiUrl?: string;
  /** OutLayer contract (default: outlayer.near) */
  outlayerContract?: string;
  /** Key manager WASM version (default: v0.3.0) */
  keyManagerVersion?: string;
  /** Group suffix for key derivation (default: "private") */
  groupSuffix?: string;
}

export interface EncryptedEntry {
  wrapped_key: string;
  ciphertext: string;
  key_id: string;
  algorithm: "AES-256-GCM";
  v: 1;
}

export interface StoredEntry extends EncryptedEntry {
  _storedAt?: string;
  _predecessorId?: string;
}

// ============ Main Class ============

export class PrivateKVClient {
  private readonly accountId: string;
  private readonly apiUrl: string;
  private readonly outlayerContract: string;
  private readonly keyManagerVersion: string;
  private readonly groupSuffix: string;
  
  private keyIdCache = new Map<string, string>();

  constructor(config: PrivateKVConfig) {
    this.accountId = config.accountId;
    this.apiUrl = config.apiUrl || "https://near.garden";
    this.outlayerContract = config.outlayerContract || "outlayer.near";
    this.keyManagerVersion = config.keyManagerVersion || "v0.3.0";
    this.groupSuffix = config.groupSuffix || "private";
  }

  // ============ Public API ============

  /**
   * Encrypt and store a value
   * 
   * Flow:
   * 1. Generate ephemeral key locally
   * 2. Encrypt plaintext locally (AES-256-GCM)
   * 3. Wrap ephemeral key with TEE
   * 4. Store ciphertext + wrapped key on FastKV
   */
  async set(key: string, plaintext: string): Promise<{ txHash?: string }> {
    const groupId = this.getGroupId();
    const fullKey = this.getFullKey(key);

    // 1. Generate ephemeral AES-256 key
    const ephemeralKey = await this.generateAESKey();

    // 2. Encrypt plaintext locally
    const ciphertext = await this.encryptAES(plaintext, ephemeralKey);

    // 3. Export ephemeral key
    const ephemeralKeyB64 = await this.exportKey(ephemeralKey);

    // 4. Wrap key with TEE
    const wrappedKey = await this.wrapKey(groupId, ephemeralKeyB64);

    // 5. Build encrypted entry
    const entry: EncryptedEntry = {
      wrapped_key: wrappedKey.wrapped_key_b64,
      ciphertext,
      key_id: wrappedKey.key_id,
      algorithm: "AES-256-GCM",
      v: 1,
    };

    // 6. Store on FastKV
    return await this.storeOnFastKV(fullKey, entry);
  }

  /**
   * Retrieve and decrypt a value
   * 
   * Flow:
   * 1. Retrieve from FastKV (ciphertext + wrapped key)
   * 2. Unwrap key with TEE (only wrapped key sent)
   * 3. Decrypt locally with unwrapped key
   */
  async get(key: string): Promise<string | null> {
    const fullKey = this.getFullKey(key);
    const groupId = this.getGroupId();

    // 1. Retrieve from FastKV
    const entry = await this.getFromFastKV(fullKey);
    if (!entry) return null;

    // 2. Unwrap key with TEE
    const unwrapped = await this.unwrapKey(groupId, entry.wrapped_key);

    // 3. Import key and decrypt locally
    const ephemeralKey = await this.importKey(unwrapped.plaintext_key_b64);
    const plaintext = await this.decryptAES(entry.ciphertext, ephemeralKey);

    return plaintext;
  }

  /**
   * Delete a key (stores null)
   */
  async delete(key: string): Promise<{ txHash?: string }> {
    const fullKey = this.getFullKey(key);
    return await this.deleteFromFastKV(fullKey);
  }

  /**
   * List all keys with given prefix
   */
  async list(prefix: string = ""): Promise<string[]> {
    const fullPrefix = this.getFullKey(prefix);
    const entries = await this.queryFastKV(fullPrefix);
    // Strip the privatekv/accountId/ prefix
    return entries.map(e => e.replace(new RegExp(`^privatekv/${this.accountId}/`), ""));
  }

  /**
   * Get the key ID for this group (public info)
   */
  async getKeyId(): Promise<string> {
    const groupId = this.getGroupId();
    
    if (this.keyIdCache.has(groupId)) {
      return this.keyIdCache.get(groupId)!;
    }

    const response = await this.callTEE("get_group_key_id", {
      group_id: groupId,
      account_id: this.accountId,
    });

    this.keyIdCache.set(groupId, response.key_id);
    return response.key_id;
  }

  // ============ Private Helpers ============

  private getGroupId(): string {
    return `${this.accountId}/${this.groupSuffix}`;
  }

  private getFullKey(key: string): string {
    return `privatekv/${this.accountId}/${key}`;
  }

  // --- TEE Operations ---

  private async wrapKey(
    groupId: string,
    plaintextKeyB64: string
  ): Promise<{ wrapped_key_b64: string; key_id: string }> {
    return await this.callTEE("wrap_key", {
      group_id: groupId,
      account_id: this.accountId,
      plaintext_key_b64: plaintextKeyB64,
    });
  }

  private async unwrapKey(
    groupId: string,
    wrappedKeyB64: string
  ): Promise<{ plaintext_key_b64: string; key_id: string }> {
    return await this.callTEE("unwrap_key", {
      group_id: groupId,
      account_id: this.accountId,
      wrapped_key_b64: wrappedKeyB64,
    });
  }

  private async callTEE(
    action: string,
    params: Record<string, unknown>
  ): Promise<any> {
    const input = JSON.stringify({ action, ...params });

    // Build NEAR transaction for OutLayer
    const response = await fetch(`${this.apiUrl}/v1/kv/encrypted/prepare-tee-call`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        source: {
          GitHub: {
            repo: "github.com/Kampouse/key-manager",
            commit: this.keyManagerVersion,
            build_target: "wasm32-wasip1",
          },
        },
        input_data: input,
        resource_limits: {
          max_instructions: 10000000000,
          max_memory_mb: 128,
          max_execution_seconds: 60,
        },
        response_format: "Json",
      }),
    });

    if (!response.ok) {
      throw new Error(`TEE call failed: ${await response.text()}`);
    }

    const { transaction } = await response.json();
    
    // In browser with wallet, request signature
    // In Node.js with keypair, sign directly
    return { needsSignature: true, transaction };
  }

  // --- FastKV Operations ---

  private async storeOnFastKV(
    key: string,
    entry: EncryptedEntry
  ): Promise<{ txHash?: string }> {
    // Option 1: Use FastKV API (if available)
    // const response = await fetch(`${this.apiUrl}/v1/kv/set`, { ... });
    
    // Option 2: Write directly to contextual.near
    // In production, this uses NEAR wallet or keypair
    
    // For now, return the entry for manual storage
    console.log(`[PrivateKV] Store this on FastKV:`);
    console.log(`  Key: ${key}`);
    console.log(`  Value: ${JSON.stringify(entry)}`);
    
    return { txHash: undefined };
  }

  private async getFromFastKV(key: string): Promise<EncryptedEntry | null> {
    const response = await fetch(
      `${this.apiUrl}/v1/kv/get?accountId=${this.accountId}&contractId=contextual.near&key=${encodeURIComponent(key)}&fields=value`
    );

    if (!response.ok) {
      if (response.status === 404) return null;
      throw new Error(`FastKV get failed: ${await response.text()}`);
    }

    const json = await response.json();
    const data = json.data;
    if (!data || !data.value) return null;

    // Parse the JSON value
    return JSON.parse(data.value);
  }

  private async deleteFromFastKV(key: string): Promise<{ txHash?: string }> {
    console.log(`[PrivateKV] Delete key: ${key}`);
    return { txHash: undefined };
  }

  private async queryFastKV(prefix: string): Promise<string[]> {
    const response = await fetch(
      `${this.apiUrl}/v1/kv/query?accountId=${this.accountId}&contractId=contextual.near&key_prefix=${encodeURIComponent(prefix)}&exclude_deleted=true&fields=key`
    );

    if (!response.ok) {
      throw new Error(`FastKV query failed: ${await response.text()}`);
    }

    const json = await response.json();
    return (json.data || []).map((e: any) => e.key);
  }

  // --- Crypto Helpers ---

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

  private async encryptAES(plaintext: string, key: CryptoKey): Promise<string> {
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

  private async decryptAES(ciphertextB64: string, key: CryptoKey): Promise<string> {
    const combined = this.base64ToArrayBuffer(ciphertextB64);
    
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

// ============ Node.js Helper ============

/**
 * For Node.js environments, provides a way to execute TEE calls
 * via NEAR CLI instead of wallet signature
 */
export class PrivateKVNodeClient extends PrivateKVClient {
  private nearCliPath: string;

  constructor(config: PrivateKVConfig & { nearCliPath?: string }) {
    super(config);
    this.nearCliPath = config.nearCliPath || "near";
  }

  /**
   * Execute TEE call via NEAR CLI
   * Requires NEAR CLI to be installed and logged in
   */
  async callTEE(
    action: string,
    params: Record<string, unknown>
  ): Promise<any> {
    const input = JSON.stringify({ action, ...params });
    
    // This would spawn a child process to call near CLI
    // For actual implementation, see the full example
    
    console.log(`[PrivateKVNode] Would execute:`);
    console.log(`  near call ${this.outlayerContract} request_execution '...'`);
    
    throw new Error("Node.js execution requires child_process implementation");
  }
}

export default PrivateKVClient;
