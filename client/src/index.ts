// Main class
export { PrivateKV } from './PrivateKV';

// Types
export type {
  StorageAdapter,
  TEEAdapter,
  CryptoAdapter,
  EncryptedEntry,
  PrivateKVConfig,
  SetOptions,
  GetOptions,
  GetResult,
} from './types';

// Crypto adapters
export { BrowserCryptoAdapter, NodeCryptoAdapter, createCryptoAdapter } from './crypto';

// Storage adapters
export { FastKVAdapter } from './adapters/fastkv';
export type { FastKVAdapterConfig } from './adapters/fastkv';

// TEE adapters
export { OutLayerAdapter } from './adapters/outlayer';
export type { OutLayerAdapterConfig } from './adapters/outlayer';

// Testing utilities
export { MemoryStorageAdapter, MockTEEAdapter } from './adapters/memory';
