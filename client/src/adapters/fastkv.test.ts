import { describe, it, expect, beforeEach } from 'vitest';
import { $fetch } from 'ofetch';
import { FastKVAdapter } from './fastkv';

describe('FastKVAdapter', () => {
  it('should create adapter with config', () => {
    const adapter = new FastKVAdapter({
      apiUrl: 'https://near.garden',
      accountId: 'test.near',
      contractId: 'contextual.near',
    });

    expect(adapter).toBeDefined();
  });

  it('should use default contractId when not provided', () => {
    const adapter = new FastKVAdapter({
      apiUrl: 'https://near.garden',
      accountId: 'test.near',
    });

    expect(adapter).toBeDefined();
  });

  it('should accept custom fetch instance', () => {
    const customFetch = $fetch.create({
      baseURL: 'https://near.garden',
    });

    const adapter = new FastKVAdapter({
      apiUrl: 'https://near.garden',
      accountId: 'test.near',
      fetch: customFetch,
    });

    expect(adapter).toBeDefined();
  });
});
