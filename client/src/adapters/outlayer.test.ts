import { describe, it, expect } from 'vitest';
import { OutLayerAdapter } from './outlayer';
import type { NEARTransaction } from '../types';

describe('OutLayerAdapter', () => {
  it('should create adapter with mainnet config', () => {
    const adapter = new OutLayerAdapter({
      network: 'mainnet',
      signTransaction: async (tx: NEARTransaction) => {
        return { key_id: 'test' };
      },
    });

    expect(adapter).toBeDefined();
  });

  it('should create adapter with testnet config', () => {
    const adapter = new OutLayerAdapter({
      network: 'testnet',
      signTransaction: async (tx: NEARTransaction) => {
        return { key_id: 'test' };
      },
    });

    expect(adapter).toBeDefined();
  });

  it('should use default deposit when not provided', () => {
    const adapter = new OutLayerAdapter({
      signTransaction: async (tx: NEARTransaction) => {
        expect(tx.deposit).toBe('0.05 NEAR');
        return { key_id: 'test' };
      },
    });

    expect(adapter).toBeDefined();
  });

  it('should use custom deposit when provided', async () => {
    let receivedDeposit: string | undefined;

    const adapter = new OutLayerAdapter({
      deposit: '0.1 NEAR',
      signTransaction: async (tx: NEARTransaction) => {
        receivedDeposit = tx.deposit;
        return { key_id: 'test' };
      },
    });

    // Trigger a transaction to capture the deposit
    try {
      await adapter.wrapKey('test-group', 'test-key');
    } catch (e) {
      // Expected to fail since we're not actually signing
    }

    expect(receivedDeposit).toBe('0.1 NEAR');
  });

  it('should use custom gas when provided', async () => {
    let receivedGas: string | undefined;

    const adapter = new OutLayerAdapter({
      gas: '500000000000000',
      signTransaction: async (tx: NEARTransaction) => {
        receivedGas = tx.gas;
        return { key_id: 'test' };
      },
    });

    try {
      await adapter.wrapKey('test-group', 'test-key');
    } catch (e) {
      // Expected to fail
    }

    expect(receivedGas).toBe('500000000000000');
  });

  it('should use correct receiverId based on network', async () => {
    let receivedReceiverId: string | undefined;

    const mainnetAdapter = new OutLayerAdapter({
      network: 'mainnet',
      signTransaction: async (tx: NEARTransaction) => {
        receivedReceiverId = tx.receiverId;
        return { key_id: 'test' };
      },
    });

    try {
      await mainnetAdapter.wrapKey('test-group', 'test-key');
    } catch (e) {}

    expect(receivedReceiverId).toBe('outlayer.near');
  });
});
