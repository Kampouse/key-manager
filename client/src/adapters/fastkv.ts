import type { StorageAdapter, EncryptedEntry } from '../types';
import { $fetch } from 'ofetch';

export interface FastKVAdapterConfig {
  /** FastKV API URL */
  apiUrl: string;
  /** Storage account ID */
  accountId: string;
  /** Storage contract ID (default: contextual.near) */
  contractId?: string;
  /** Custom fetch instance (for testing or custom auth) */
  fetch?: ReturnType<typeof $fetch>;
}

/**
 * FastKV storage adapter for NEAR blockchain
 *
 * Uses ofetch for universal cross-platform fetch support.
 * Works in browser, Node.js, Cloudflare Workers, Deno, and more.
 *
 * @example
 * ```ts
 * const adapter = new FastKVAdapter({
 *   apiUrl: 'https://fastkv-server-production.up.railway.app',
 *   accountId: 'my-account.near',
 * });
 * ```
 *
 * @remarks
 * The near.garden API is read-only for querying. For writes, use:
 * - Production: https://fastkv-server-production.up.railway.app
 * - Or deploy your own FastKV server
 */
export class FastKVAdapter implements StorageAdapter {
  private readonly fetch: any;
  private readonly accountId: string;
  private readonly contractId: string;

  constructor(config: FastKVAdapterConfig) {
    this.accountId = config.accountId;
    this.contractId = config.contractId || 'contextual.near';

    // Use ofetch for universal fetch support
    // Works in browser, Node.js, and edge workers automatically
    this.fetch = config.fetch || $fetch.create({
      baseURL: config.apiUrl,
      headers: {
        'Content-Type': 'application/json',
      },
      onRequestError({ error }: { error: Error }) {
        console.error('FastKV request error:', error);
      },
    });
  }

  async set(key: string, entry: EncryptedEntry): Promise<{ txHash?: string }> {
    try {
      const response: { txHash: string } = await this.fetch('/v1/kv/set', {
        method: 'POST',
        body: {
          accountId: this.accountId,
          contractId: this.contractId,
          key,
          value: JSON.stringify(entry),
        },
      });

      return { txHash: response.txHash };
    } catch (error) {
      if (error instanceof Error) {
        throw new Error(
          `FastKV set failed\n` +
          `Key: ${key}\n` +
          `Error: ${error.message}`
        );
      }
      throw error;
    }
  }

  async get(key: string): Promise<EncryptedEntry | null> {
    try {
      const response: { data: { value: string } } = await this.fetch('/v1/kv/get', {
        method: 'GET',
        query: {
          accountId: this.accountId,
          contractId: this.contractId,
          key,
          fields: 'value',
        },
        onResponse({ response }: { response: { status: number } }) {
          // Handle 404 gracefully
          if (response.status === 404) {
            throw new Error('NOT_FOUND');
          }
        },
      });

      if (!response.data?.value) {
        return null;
      }

      return JSON.parse(response.data.value);
    } catch (error) {
      if (error instanceof Error && error.message === 'NOT_FOUND') {
        return null;
      }

      if (error instanceof Error) {
        throw new Error(
          `FastKV get failed\n` +
          `Key: ${key}\n` +
          `Error: ${error.message}`
        );
      }
      throw error;
    }
  }

  async delete(key: string): Promise<{ txHash?: string }> {
    try {
      const response: { txHash: string } = await this.fetch('/v1/kv/delete', {
        method: 'POST',
        body: {
          accountId: this.accountId,
          contractId: this.contractId,
          key,
        },
      });

      return { txHash: response.txHash };
    } catch (error) {
      if (error instanceof Error) {
        throw new Error(
          `FastKV delete failed\n` +
          `Key: ${key}\n` +
          `Error: ${error.message}`
        );
      }
      throw error;
    }
  }

  async list(prefix: string = ''): Promise<string[]> {
    try {
      const response: { data?: Array<{ key: string }> } = await this.fetch('/v1/kv/query', {
        method: 'GET',
        query: {
          accountId: this.accountId,
          contractId: this.contractId,
          key_prefix: prefix,
          exclude_deleted: 'true',
          fields: 'key',
        },
      });

      return (response.data || []).map((e: { key: string }) => e.key);
    } catch (error) {
      if (error instanceof Error) {
        throw new Error(
          `FastKV query failed\n` +
          `Prefix: ${prefix}\n` +
          `Error: ${error.message}`
        );
      }
      throw error;
    }
  }
}
