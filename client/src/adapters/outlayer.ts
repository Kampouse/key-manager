import type { TEEAdapter } from '../types';

export interface OutLayerAdapterConfig {
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
export class OutLayerAdapter implements TEEAdapter {
  private readonly contractId: string;
  private readonly keyManagerVersion: string;
  private readonly signTransaction: OutLayerAdapterConfig['signTransaction'];
  private readonly keyIdCache = new Map<string, string>();

  constructor(config: OutLayerAdapterConfig) {
    this.contractId = config.contractId || 'outlayer.near';
    this.keyManagerVersion = config.keyManagerVersion || 'v0.3.0';
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

    const transaction = {
      contractId: this.contractId,
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
    };

    const result = await this.signTransaction(transaction);
    return result as { wrapped_key_b64?: string; plaintext_key_b64?: string; key_id: string };
  }
}
