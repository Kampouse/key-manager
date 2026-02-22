import type { TEEAdapter, NEARTransaction } from '../types';

export type Network = 'mainnet' | 'testnet';

export interface OutLayerAdapterConfig {
  /** OutLayer contract ID (default: based on network) */
  contractId?: string;
  /** Network to use (default: mainnet) */
  network?: Network;
  /** Key manager WASM version (default: v0.3.0) */
  keyManagerVersion?: string;
  /** Deposit amount for TEE execution (default: "0.05 NEAR") */
  deposit?: string;
  /** Gas limit for TEE execution (default: "300000000000000") */
  gas?: string;
  /** Function to sign NEAR transactions and return the TEE response */
  signTransaction: (transaction: NEARTransaction) => Promise<Record<string, unknown>>;
}

const DEFAULT_CONTRACTS: Record<Network, string> = {
  mainnet: 'outlayer.near',
  testnet: 'outlayer.testnet',
};

const DEFAULT_DEPOSIT = '0.05 NEAR';
const DEFAULT_GAS = '300000000000000';

/**
 * OutLayer TEE adapter for NEAR
 *
 * Requires a signing function to authorize TEE operations.
 * In browser: use NEAR wallet
 * In Node.js: use keypair or NEAR CLI
 *
 * @example
 * ```ts
 * const adapter = new OutLayerAdapter({
 *   network: 'testnet',
 *   signTransaction: async (tx) => {
 *     // tx.receiverId - the contract to call
 *     // tx.deposit - amount to attach (use this!)
 *     const result = await wallet.signAndSendTransaction({
 *       receiverId: tx.receiverId,
 *       actions: [{
 *         type: 'FunctionCall',
 *         methodName: tx.methodName,
 *         args: tx.args,
 *         gas: tx.gas,
 *         deposit: tx.deposit,
 *       }]
 *     });
 *     return result.transaction.hash;
 *   },
 * });
 * ```
 */
export class OutLayerAdapter implements TEEAdapter {
  private readonly contractId: string;
  private readonly network: Network;
  private readonly keyManagerVersion: string;
  private readonly deposit: string;
  private readonly gas: string;
  private readonly signTransaction: OutLayerAdapterConfig['signTransaction'];
  private readonly keyIdCache = new Map<string, string>();

  constructor(config: OutLayerAdapterConfig) {
    this.network = config.network || 'mainnet';
    this.contractId = config.contractId || DEFAULT_CONTRACTS[this.network];
    this.keyManagerVersion = config.keyManagerVersion || 'v0.3.0';
    this.deposit = config.deposit || DEFAULT_DEPOSIT;
    this.gas = config.gas || DEFAULT_GAS;
    this.signTransaction = config.signTransaction;
  }

  async wrapKey(
    groupId: string,
    plaintextKeyB64: string
  ): Promise<{ wrapped_key_b64: string; key_id: string }> {
    const result = await this.callTEE('wrap_key', {
      group_id: groupId,
      plaintext_key_b64: plaintextKeyB64,
    });
    return result;
  }

  async unwrapKey(
    groupId: string,
    wrappedKeyB64: string
  ): Promise<{ plaintext_key_b64: string; key_id: string }> {
    const result = await this.callTEE('unwrap_key', {
      group_id: groupId,
      wrapped_key_b64: wrappedKeyB64,
    });
    return result;
  }

  async getKeyId(groupId: string): Promise<string> {
    if (this.keyIdCache.has(groupId)) {
      return this.keyIdCache.get(groupId)!;
    }

    const result = await this.callTEE('get_group_key_id', {
      group_id: groupId,
    });

    this.keyIdCache.set(groupId, result.key_id);
    return result.key_id;
  }

  private async callTEE(
    action: string,
    params: Record<string, unknown>
  ): Promise<any> {
    const input = JSON.stringify({ action, ...params });

    const transaction: NEARTransaction = {
      receiverId: this.contractId,
      methodName: 'request_execution',
      args: {
        source: {
          GitHub: {
            repo: 'github.com/Kampouse/key-manager',
            commit: this.keyManagerVersion,
            build_target: 'wasm32-wasip1',
          },
        },
        input_data: input,
        resource_limits: {
          max_instructions: 10000000000,
          max_memory_mb: 128,
          max_execution_seconds: 60,
        },
        response_format: 'Json',
      },
      gas: this.gas,
      deposit: this.deposit,
    };

    const result = await this.signTransaction(transaction);
    return result as { wrapped_key_b64?: string; plaintext_key_b64?: string; key_id: string };
  }
}
