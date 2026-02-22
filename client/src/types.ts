// ============ Core Types ============

/**
 * Backend-agnostic storage interface
 * Implement this for any storage backend (FastKV, S3, IPFS, etc.)
 */
export interface StorageAdapter {
  /** Store an encrypted entry */
  set(key: string, entry: EncryptedEntry): Promise<{ txHash?: string }>;
  
  /** Retrieve an encrypted entry */
  get(key: string): Promise<EncryptedEntry | null>;
  
  /** Delete a key */
  delete(key: string): Promise<{ txHash?: string }>;
  
  /** List keys with optional prefix */
  list(prefix?: string): Promise<string[]>;
}

/**
 * TEE (Trusted Execution Environment) adapter interface
 * Implement for different TEE backends (OutLayer, etc.)
 */
export interface TEEAdapter {
  /** Wrap a key with TEE */
  wrapKey(groupId: string, plaintextKeyB64: string): Promise<{ wrapped_key_b64: string; key_id: string }>;
  
  /** Unwrap a key with TEE */
  unwrapKey(groupId: string, wrappedKeyB64: string): Promise<{ plaintext_key_b64: string; key_id: string }>;
  
  /** Get the key ID for a group */
  getKeyId?(groupId: string): Promise<string>;
}

/**
 * Cryptography adapter interface
 * Implement for different environments (Web Crypto, Node.js crypto, etc.)
 */
export interface CryptoAdapter {
  /** Generate a new AES-256 key */
  generateKey(): Promise<CryptoKey | Buffer>;
  
  /** Export key to base64 */
  exportKey(key: CryptoKey | Buffer): Promise<string>;
  
  /** Import key from base64 */
  importKey(keyB64: string): Promise<CryptoKey | Buffer>;
  
  /** Encrypt plaintext with AES-256-GCM */
  encrypt(plaintext: string, key: CryptoKey | Buffer): Promise<string>;
  
  /** Decrypt ciphertext with AES-256-GCM */
  decrypt(ciphertextB64: string, key: CryptoKey | Buffer): Promise<string>;
}

/**
 * Encrypted entry format
 */
export interface EncryptedEntry {
  wrapped_key: string;
  ciphertext: string;
  key_id: string;
  algorithm: "AES-256-GCM";
  v: 1;
}

/**
 * Configuration for PrivateKV client
 */
export interface PrivateKVConfig {
  /** Account ID (user identifier) */
  accountId: string;
  
  /** Storage adapter (required) */
  storage: StorageAdapter;
  
  /** TEE adapter (required) */
  tee: TEEAdapter;
  
  /** Crypto adapter (optional, defaults to environment-specific) */
  crypto?: CryptoAdapter;
  
  /** Key namespace prefix (default: "privatekv") */
  namespace?: string;
  
  /** Group suffix for key derivation (default: "private") */
  groupSuffix?: string;
}

/**
 * Options for individual operations
 */
export interface SetOptions {
  /** Additional metadata to store */
  metadata?: Record<string, unknown>;
}

export interface GetOptions {
  /** Include metadata in response */
  includeMetadata?: boolean;
}

export interface GetResult {
  plaintext: string;
  metadata?: Record<string, unknown>;
}

// ============ NEAR Transaction Types ============

/**
 * NEAR transaction interface for TEE operations
 * Matches standard NEAR SDK transaction structure
 */
export interface NEARTransaction {
  /** Contract account ID to call (receiverId in NEAR SDK) */
  receiverId: string;
  /** Method name to call */
  methodName: string;
  /** Method arguments */
  args: Record<string, unknown>;
  /** Gas limit in yoctoNEAR (optional) */
  gas?: string;
  /** Attached deposit (e.g., "0.05 NEAR") */
  deposit?: string;
}

/**
 * Result returned by signTransaction callback
 */
export type TransactionResult = string; // transaction hash
