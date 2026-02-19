/**
 * Backend-agnostic storage interface
 * Implement this for any storage backend (FastKV, S3, IPFS, etc.)
 */
interface StorageAdapter {
    /** Store an encrypted entry */
    set(key: string, entry: EncryptedEntry): Promise<{
        txHash?: string;
    }>;
    /** Retrieve an encrypted entry */
    get(key: string): Promise<EncryptedEntry | null>;
    /** Delete a key */
    delete(key: string): Promise<{
        txHash?: string;
    }>;
    /** List keys with optional prefix */
    list(prefix?: string): Promise<string[]>;
}
/**
 * TEE (Trusted Execution Environment) adapter interface
 * Implement for different TEE backends (OutLayer, etc.)
 */
interface TEEAdapter {
    /** Wrap a key with TEE */
    wrapKey(groupId: string, plaintextKeyB64: string): Promise<{
        wrapped_key_b64: string;
        key_id: string;
    }>;
    /** Unwrap a key with TEE */
    unwrapKey(groupId: string, wrappedKeyB64: string): Promise<{
        plaintext_key_b64: string;
        key_id: string;
    }>;
    /** Get the key ID for a group */
    getKeyId?(groupId: string): Promise<string>;
}
/**
 * Cryptography adapter interface
 * Implement for different environments (Web Crypto, Node.js crypto, etc.)
 */
interface CryptoAdapter {
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
interface EncryptedEntry {
    wrapped_key: string;
    ciphertext: string;
    key_id: string;
    algorithm: "AES-256-GCM";
    v: 1;
}
/**
 * Configuration for PrivateKV client
 */
interface PrivateKVConfig {
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
interface SetOptions {
    /** Additional metadata to store */
    metadata?: Record<string, unknown>;
}
interface GetOptions {
    /** Include metadata in response */
    includeMetadata?: boolean;
}
interface GetResult {
    plaintext: string;
    metadata?: Record<string, unknown>;
}

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
declare class PrivateKV {
    private readonly accountId;
    private readonly namespace;
    private readonly groupSuffix;
    private readonly storage;
    private readonly tee;
    private readonly crypto;
    private readonly keyIdCache;
    constructor(config: PrivateKVConfig);
    /**
     * Encrypt and store a value
     *
     * Flow:
     * 1. Generate ephemeral key locally
     * 2. Encrypt plaintext locally (AES-256-GCM)
     * 3. Wrap ephemeral key with TEE
     * 4. Store ciphertext + wrapped key
     */
    set(key: string, plaintext: string, options?: SetOptions): Promise<{
        txHash?: string;
    }>;
    /**
     * Retrieve and decrypt a value
     *
     * Flow:
     * 1. Retrieve ciphertext + wrapped key
     * 2. Unwrap key with TEE (only wrapped key sent)
     * 3. Decrypt locally with unwrapped key
     */
    get(key: string, options?: GetOptions): Promise<string | null>;
    /**
     * Delete a key
     */
    delete(key: string): Promise<{
        txHash?: string;
    }>;
    /**
     * List all keys with given prefix
     */
    list(prefix?: string): Promise<string[]>;
    /**
     * Get the key ID for this group (public info)
     */
    getKeyId(): Promise<string>;
    private getGroupId;
    private getFullKey;
}

/**
 * Browser/Web Crypto API implementation
 */
declare class BrowserCryptoAdapter implements CryptoAdapter {
    generateKey(): Promise<CryptoKey>;
    exportKey(key: CryptoKey): Promise<string>;
    importKey(keyB64: string): Promise<CryptoKey>;
    encrypt(plaintext: string, key: CryptoKey): Promise<string>;
    decrypt(ciphertextB64: string, key: CryptoKey): Promise<string>;
    private arrayBufferToBase64;
    private base64ToArrayBuffer;
}
/**
 * Node.js crypto implementation
 */
declare class NodeCryptoAdapter implements CryptoAdapter {
    private crypto;
    constructor();
    generateKey(): Promise<Buffer>;
    exportKey(key: Buffer): Promise<string>;
    importKey(keyB64: string): Promise<Buffer>;
    encrypt(plaintext: string, key: Buffer): Promise<string>;
    decrypt(ciphertextB64: string, key: Buffer): Promise<string>;
}
/**
 * Auto-detect and return appropriate crypto adapter for environment
 */
declare function createCryptoAdapter(): CryptoAdapter;

interface FastKVAdapterConfig {
    /** FastKV API URL */
    apiUrl: string;
    /** Storage account ID */
    accountId: string;
    /** Storage contract ID (default: contextual.near) */
    contractId?: string;
    /** Custom fetch function (for testing or custom auth) */
    fetch?: typeof fetch;
}
/**
 * FastKV storage adapter for NEAR blockchain
 */
declare class FastKVAdapter implements StorageAdapter {
    private readonly apiUrl;
    private readonly accountId;
    private readonly contractId;
    private readonly fetchFn;
    constructor(config: FastKVAdapterConfig);
    set(key: string, entry: EncryptedEntry): Promise<{
        txHash?: string;
    }>;
    get(key: string): Promise<EncryptedEntry | null>;
    delete(key: string): Promise<{
        txHash?: string;
    }>;
    list(prefix?: string): Promise<string[]>;
}

interface OutLayerAdapterConfig {
    /** OutLayer contract ID */
    contractId?: string;
    /** Key manager WASM version */
    keyManagerVersion?: string;
    /** Function to sign NEAR transactions */
    signTransaction: (transaction: unknown) => Promise<unknown>;
}
/**
 * OutLayer TEE adapter for NEAR
 *
 * Requires a signing function to authorize TEE operations.
 * In browser: use NEAR wallet
 * In Node.js: use keypair or NEAR CLI
 */
declare class OutLayerAdapter implements TEEAdapter {
    private readonly contractId;
    private readonly keyManagerVersion;
    private readonly signTransaction;
    private readonly keyIdCache;
    constructor(config: OutLayerAdapterConfig);
    wrapKey(groupId: string, plaintextKeyB64: string): Promise<{
        wrapped_key_b64: string;
        key_id: string;
    }>;
    unwrapKey(groupId: string, wrappedKeyB64: string): Promise<{
        plaintext_key_b64: string;
        key_id: string;
    }>;
    getKeyId(groupId: string): Promise<string>;
    private callTEE;
}

/**
 * In-memory storage adapter for testing
 */
declare class MemoryStorageAdapter implements StorageAdapter {
    private readonly store;
    set(key: string, entry: EncryptedEntry): Promise<{
        txHash?: string;
    }>;
    get(key: string): Promise<EncryptedEntry | null>;
    delete(key: string): Promise<{
        txHash?: string;
    }>;
    list(prefix?: string): Promise<string[]>;
    /** Clear all stored data */
    clear(): void;
    /** Get number of stored entries */
    get size(): number;
}
/**
 * Mock TEE adapter for testing (NOT SECURE - do not use in production)
 */
declare class MockTEEAdapter implements TEEAdapter {
    private readonly keys;
    private keyIdCounter;
    wrapKey(groupId: string, plaintextKeyB64: string): Promise<{
        wrapped_key_b64: string;
        key_id: string;
    }>;
    unwrapKey(groupId: string, wrappedKeyB64: string): Promise<{
        plaintext_key_b64: string;
        key_id: string;
    }>;
    getKeyId(groupId: string): Promise<string>;
    private generateKeyId;
    private extractKeyId;
}

export { BrowserCryptoAdapter, type CryptoAdapter, type EncryptedEntry, FastKVAdapter, type FastKVAdapterConfig, type GetOptions, type GetResult, MemoryStorageAdapter, MockTEEAdapter, NodeCryptoAdapter, OutLayerAdapter, type OutLayerAdapterConfig, PrivateKV, type PrivateKVConfig, type SetOptions, type StorageAdapter, type TEEAdapter, createCryptoAdapter };
