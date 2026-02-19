import type { StorageAdapter, EncryptedEntry } from '../types';

export interface FastKVAdapterConfig {
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
export class FastKVAdapter implements StorageAdapter {
  private readonly apiUrl: string;
  private readonly accountId: string;
  private readonly contractId: string;
  private readonly fetchFn: typeof fetch;

  constructor(config: FastKVAdapterConfig) {
    this.apiUrl = config.apiUrl;
    this.accountId = config.accountId;
    this.contractId = config.contractId || 'contextual.near';
    this.fetchFn = config.fetch || fetch;
  }

  async set(key: string, entry: EncryptedEntry): Promise<{ txHash?: string }> {
    const response = await this.fetchFn(`${this.apiUrl}/v1/kv/set`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        accountId: this.accountId,
        contractId: this.contractId,
        key,
        value: JSON.stringify(entry),
      }),
    });

    if (!response.ok) {
      throw new Error(`FastKV set failed: ${await response.text()}`);
    }

    const data = await response.json();
    return { txHash: data.txHash };
  }

  async get(key: string): Promise<EncryptedEntry | null> {
    const url = new URL(`${this.apiUrl}/v1/kv/get`);
    url.searchParams.set('accountId', this.accountId);
    url.searchParams.set('contractId', this.contractId);
    url.searchParams.set('key', key);
    url.searchParams.set('fields', 'value');

    const response = await this.fetchFn(url.toString());

    if (!response.ok) {
      if (response.status === 404) return null;
      throw new Error(`FastKV get failed: ${await response.text()}`);
    }

    const json = await response.json();
    if (!json.data?.value) return null;

    return JSON.parse(json.data.value);
  }

  async delete(key: string): Promise<{ txHash?: string }> {
    const response = await this.fetchFn(`${this.apiUrl}/v1/kv/delete`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        accountId: this.accountId,
        contractId: this.contractId,
        key,
      }),
    });

    if (!response.ok) {
      throw new Error(`FastKV delete failed: ${await response.text()}`);
    }

    const data = await response.json();
    return { txHash: data.txHash };
  }

  async list(prefix: string = ''): Promise<string[]> {
    const url = new URL(`${this.apiUrl}/v1/kv/query`);
    url.searchParams.set('accountId', this.accountId);
    url.searchParams.set('contractId', this.contractId);
    url.searchParams.set('key_prefix', prefix);
    url.searchParams.set('exclude_deleted', 'true');
    url.searchParams.set('fields', 'key');

    const response = await this.fetchFn(url.toString());

    if (!response.ok) {
      throw new Error(`FastKV query failed: ${await response.text()}`);
    }

    const json = await response.json();
    return (json.data || []).map((e: { key: string }) => e.key);
  }
}
