var __require = /* @__PURE__ */ ((x) => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x, {
  get: (a, b) => (typeof require !== "undefined" ? require : a)[b]
}) : x)(function(x) {
  if (typeof require !== "undefined") return require.apply(this, arguments);
  throw Error('Dynamic require of "' + x + '" is not supported');
});

// src/crypto/index.ts
var BrowserCryptoAdapter = class {
  async generateKey() {
    return await crypto.subtle.generateKey(
      { name: "AES-GCM", length: 256 },
      true,
      ["encrypt", "decrypt"]
    );
  }
  async exportKey(key) {
    const exported = await crypto.subtle.exportKey("raw", key);
    return this.arrayBufferToBase64(exported);
  }
  async importKey(keyB64) {
    const keyBytes = this.base64ToArrayBuffer(keyB64);
    return await crypto.subtle.importKey(
      "raw",
      keyBytes.buffer,
      { name: "AES-GCM", length: 256 },
      false,
      ["decrypt"]
    );
  }
  async encrypt(plaintext, key) {
    const iv = crypto.getRandomValues(new Uint8Array(12));
    const encoded = new TextEncoder().encode(plaintext);
    const ciphertext = await crypto.subtle.encrypt(
      { name: "AES-GCM", iv },
      key,
      encoded
    );
    const combined = new Uint8Array(iv.length + ciphertext.byteLength);
    combined.set(iv);
    combined.set(new Uint8Array(ciphertext), iv.length);
    return this.arrayBufferToBase64(combined.buffer);
  }
  async decrypt(ciphertextB64, key) {
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
  arrayBufferToBase64(buffer) {
    const bytes = new Uint8Array(buffer);
    let binary = "";
    for (let i = 0; i < bytes.byteLength; i++) {
      binary += String.fromCharCode(bytes[i]);
    }
    return btoa(binary);
  }
  base64ToArrayBuffer(base64) {
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    return bytes;
  }
};
var NodeCryptoAdapter = class {
  constructor() {
    this.crypto = __require("crypto");
  }
  async generateKey() {
    return this.crypto.randomBytes(32);
  }
  async exportKey(key) {
    return key.toString("base64");
  }
  async importKey(keyB64) {
    return Buffer.from(keyB64, "base64");
  }
  async encrypt(plaintext, key) {
    const iv = this.crypto.randomBytes(12);
    const cipher = this.crypto.createCipheriv("aes-256-gcm", key, iv);
    const encrypted = Buffer.concat([
      cipher.update(plaintext, "utf8"),
      cipher.final()
    ]);
    const authTag = cipher.getAuthTag();
    return Buffer.concat([iv, encrypted, authTag]).toString("base64");
  }
  async decrypt(ciphertextB64, key) {
    const combined = Buffer.from(ciphertextB64, "base64");
    const iv = combined.subarray(0, 12);
    const authTag = combined.subarray(combined.length - 16);
    const ciphertext = combined.subarray(12, combined.length - 16);
    const decipher = this.crypto.createDecipheriv("aes-256-gcm", key, iv);
    decipher.setAuthTag(authTag);
    return Buffer.concat([
      decipher.update(ciphertext),
      decipher.final()
    ]).toString("utf8");
  }
};
function createCryptoAdapter() {
  if (typeof crypto !== "undefined" && crypto.subtle) {
    return new BrowserCryptoAdapter();
  }
  return new NodeCryptoAdapter();
}

// src/PrivateKV.ts
var PrivateKV = class {
  constructor(config) {
    this.keyIdCache = /* @__PURE__ */ new Map();
    this.accountId = config.accountId;
    this.namespace = config.namespace || "privatekv";
    this.groupSuffix = config.groupSuffix || "private";
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
  async set(key, plaintext, options) {
    const groupId = this.getGroupId();
    const fullKey = this.getFullKey(key);
    const ephemeralKey = await this.crypto.generateKey();
    const ciphertext = await this.crypto.encrypt(plaintext, ephemeralKey);
    const ephemeralKeyB64 = await this.crypto.exportKey(ephemeralKey);
    const wrappedKey = await this.tee.wrapKey(groupId, ephemeralKeyB64);
    const entry = {
      wrapped_key: wrappedKey.wrapped_key_b64,
      ciphertext,
      key_id: wrappedKey.key_id,
      algorithm: "AES-256-GCM",
      v: 1
    };
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
  async get(key, options) {
    const fullKey = this.getFullKey(key);
    const groupId = this.getGroupId();
    const entry = await this.storage.get(fullKey);
    if (!entry) return null;
    const unwrapped = await this.tee.unwrapKey(groupId, entry.wrapped_key);
    const ephemeralKey = await this.crypto.importKey(unwrapped.plaintext_key_b64);
    const plaintext = await this.crypto.decrypt(entry.ciphertext, ephemeralKey);
    return plaintext;
  }
  /**
   * Delete a key
   */
  async delete(key) {
    const fullKey = this.getFullKey(key);
    return await this.storage.delete(fullKey);
  }
  /**
   * List all keys with given prefix
   */
  async list(prefix = "") {
    const fullPrefix = this.getFullKey(prefix);
    const entries = await this.storage.list(fullPrefix);
    const prefixPattern = new RegExp(`^${this.namespace}/${this.accountId}/`);
    return entries.map((e) => e.replace(prefixPattern, ""));
  }
  /**
   * Get the key ID for this group (public info)
   */
  async getKeyId() {
    const groupId = this.getGroupId();
    if (this.keyIdCache.has(groupId)) {
      return this.keyIdCache.get(groupId);
    }
    if (this.tee.getKeyId) {
      const keyId = await this.tee.getKeyId(groupId);
      this.keyIdCache.set(groupId, keyId);
      return keyId;
    }
    const ephemeralKey = await this.crypto.generateKey();
    const keyB64 = await this.crypto.exportKey(ephemeralKey);
    const wrapped = await this.tee.wrapKey(groupId, keyB64);
    this.keyIdCache.set(groupId, wrapped.key_id);
    return wrapped.key_id;
  }
  // ============ Private Helpers ============
  getGroupId() {
    return `${this.accountId}/${this.groupSuffix}`;
  }
  getFullKey(key) {
    return `${this.namespace}/${this.accountId}/${key}`;
  }
};

// src/adapters/fastkv.ts
var FastKVAdapter = class {
  constructor(config) {
    this.apiUrl = config.apiUrl;
    this.accountId = config.accountId;
    this.contractId = config.contractId || "contextual.near";
    this.fetchFn = config.fetch || fetch;
  }
  async set(key, entry) {
    const response = await this.fetchFn(`${this.apiUrl}/v1/kv/set`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        accountId: this.accountId,
        contractId: this.contractId,
        key,
        value: JSON.stringify(entry)
      })
    });
    if (!response.ok) {
      throw new Error(`FastKV set failed: ${await response.text()}`);
    }
    const data = await response.json();
    return { txHash: data.txHash };
  }
  async get(key) {
    const url = new URL(`${this.apiUrl}/v1/kv/get`);
    url.searchParams.set("accountId", this.accountId);
    url.searchParams.set("contractId", this.contractId);
    url.searchParams.set("key", key);
    url.searchParams.set("fields", "value");
    const response = await this.fetchFn(url.toString());
    if (!response.ok) {
      if (response.status === 404) return null;
      throw new Error(`FastKV get failed: ${await response.text()}`);
    }
    const json = await response.json();
    if (!json.data?.value) return null;
    return JSON.parse(json.data.value);
  }
  async delete(key) {
    const response = await this.fetchFn(`${this.apiUrl}/v1/kv/delete`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        accountId: this.accountId,
        contractId: this.contractId,
        key
      })
    });
    if (!response.ok) {
      throw new Error(`FastKV delete failed: ${await response.text()}`);
    }
    const data = await response.json();
    return { txHash: data.txHash };
  }
  async list(prefix = "") {
    const url = new URL(`${this.apiUrl}/v1/kv/query`);
    url.searchParams.set("accountId", this.accountId);
    url.searchParams.set("contractId", this.contractId);
    url.searchParams.set("key_prefix", prefix);
    url.searchParams.set("exclude_deleted", "true");
    url.searchParams.set("fields", "key");
    const response = await this.fetchFn(url.toString());
    if (!response.ok) {
      throw new Error(`FastKV query failed: ${await response.text()}`);
    }
    const json = await response.json();
    return (json.data || []).map((e) => e.key);
  }
};

// src/adapters/outlayer.ts
var OutLayerAdapter = class {
  constructor(config) {
    this.keyIdCache = /* @__PURE__ */ new Map();
    this.contractId = config.contractId || "outlayer.near";
    this.keyManagerVersion = config.keyManagerVersion || "v0.3.0";
    this.signTransaction = config.signTransaction;
  }
  async wrapKey(groupId, plaintextKeyB64) {
    const result = await this.callTEE("wrap_key", {
      group_id: groupId,
      plaintext_key_b64: plaintextKeyB64
    });
    return result;
  }
  async unwrapKey(groupId, wrappedKeyB64) {
    const result = await this.callTEE("unwrap_key", {
      group_id: groupId,
      wrapped_key_b64: wrappedKeyB64
    });
    return result;
  }
  async getKeyId(groupId) {
    if (this.keyIdCache.has(groupId)) {
      return this.keyIdCache.get(groupId);
    }
    const result = await this.callTEE("get_group_key_id", {
      group_id: groupId
    });
    this.keyIdCache.set(groupId, result.key_id);
    return result.key_id;
  }
  async callTEE(action, params) {
    const input = JSON.stringify({ action, ...params });
    const transaction = {
      contractId: this.contractId,
      methodName: "request_execution",
      args: {
        source: {
          GitHub: {
            repo: "github.com/Kampouse/key-manager",
            commit: this.keyManagerVersion,
            build_target: "wasm32-wasip1"
          }
        },
        input_data: input,
        resource_limits: {
          max_instructions: 1e10,
          max_memory_mb: 128,
          max_execution_seconds: 60
        },
        response_format: "Json"
      }
    };
    const result = await this.signTransaction(transaction);
    return result;
  }
};

// src/adapters/memory.ts
var MemoryStorageAdapter = class {
  constructor() {
    this.store = /* @__PURE__ */ new Map();
  }
  async set(key, entry) {
    this.store.set(key, entry);
    return {};
  }
  async get(key) {
    return this.store.get(key) || null;
  }
  async delete(key) {
    this.store.delete(key);
    return {};
  }
  async list(prefix = "") {
    const keys = [];
    for (const key of this.store.keys()) {
      if (key.startsWith(prefix)) {
        keys.push(key);
      }
    }
    return keys.sort();
  }
  /** Clear all stored data */
  clear() {
    this.store.clear();
  }
  /** Get number of stored entries */
  get size() {
    return this.store.size;
  }
};
var MockTEEAdapter = class {
  constructor() {
    this.keys = /* @__PURE__ */ new Map();
    this.keyIdCounter = 0;
  }
  async wrapKey(groupId, plaintextKeyB64) {
    const keyId = this.generateKeyId();
    const wrapped = btoa(`wrapped:${plaintextKeyB64}:${groupId}:${keyId}`);
    this.keys.set(wrapped, { wrapped, plaintext: plaintextKeyB64 });
    return {
      wrapped_key_b64: wrapped,
      key_id: keyId
    };
  }
  async unwrapKey(groupId, wrappedKeyB64) {
    const entry = this.keys.get(wrappedKeyB64);
    if (!entry) {
      throw new Error("Key not found");
    }
    return {
      plaintext_key_b64: entry.plaintext,
      key_id: this.extractKeyId(wrappedKeyB64)
    };
  }
  async getKeyId(groupId) {
    return this.generateKeyId();
  }
  generateKeyId() {
    return `mock-key-${++this.keyIdCounter}`;
  }
  extractKeyId(wrapped) {
    const parts = atob(wrapped).split(":");
    return parts[3] || "unknown";
  }
};
export {
  BrowserCryptoAdapter,
  FastKVAdapter,
  MemoryStorageAdapter,
  MockTEEAdapter,
  NodeCryptoAdapter,
  OutLayerAdapter,
  PrivateKV,
  createCryptoAdapter
};
