/**
 * Key Manager Client
 *
 * Base client for calling OutLayer TEE Key Manager
 */

import type {
  KeyManagerConfig,
  KeyManagerRequest,
  OutLayerRequest,
  KeyResponse,
  EncryptResponse,
  DecryptResponse,
  MembershipResponse,
  BatchEncryptRequest,
  BatchEncryptResponse,
  BatchDecryptRequest,
  BatchDecryptResponse,
  EncryptedValueString,
} from "./types";

const DEFAULT_CONFIG = {
  outlayerApi: "https://api.outlayer.fastnear.com",
  wasmUrl: "https://github.com/Kampouse/key-manager/releases/download/v0.2.0/key-manager.wasm",
  wasmHash: "44ce9f1f616e765f21fe208eb1ff4db29a7aac90096ca83cf75864793c21e7d3",
  buildTarget: "wasm32-wasip1" as const,
};

export class KeyManagerClient {
  private readonly config: Required<Omit<KeyManagerConfig, "paymentKey">> & { paymentKey: string };

  constructor(config: KeyManagerConfig) {
    this.config = {
      outlayerApi: config.outlayerApi ?? DEFAULT_CONFIG.outlayerApi,
      paymentKey: config.paymentKey,
      wasmUrl: config.wasmUrl ?? DEFAULT_CONFIG.wasmUrl,
      wasmHash: config.wasmHash ?? DEFAULT_CONFIG.wasmHash,
      buildTarget: config.buildTarget ?? DEFAULT_CONFIG.buildTarget,
    };
  }

  /**
   * Get encryption key for a group
   */
  async getKey(groupId: string, accountId: string): Promise<KeyResponse> {
    return this.call<KeyResponse>({
      action: "get_key",
      group_id: groupId,
      account_id: accountId,
    });
  }

  /**
   * Encrypt data with group key
   */
  async encrypt(
    groupId: string,
    accountId: string,
    plaintext: string
  ): Promise<EncryptResponse & { encryptedValue: EncryptedValueString }> {
    const result = await this.call<EncryptResponse>({
      action: "encrypt",
      group_id: groupId,
      account_id: accountId,
      plaintext_b64: this.toBase64(plaintext),
    });

    return {
      ...result,
      encryptedValue: `enc:AES256:${result.key_id}:${result.ciphertext_b64}` as EncryptedValueString,
    };
  }

  /**
   * Decrypt data with group key
   */
  async decrypt(
    groupId: string,
    accountId: string,
    ciphertextB64: string
  ): Promise<DecryptResponse & { plaintext: string }> {
    const result = await this.call<DecryptResponse>({
      action: "decrypt",
      group_id: groupId,
      account_id: accountId,
      ciphertext_b64: ciphertextB64,
    });

    return {
      ...result,
      plaintext: result.plaintext_utf8 ?? this.fromBase64(result.plaintext_b64),
    };
  }

  /**
   * Batch encrypt multiple items
   */
  async batchEncrypt(
    groupId: string,
    accountId: string,
    items: Record<string, string>
  ): Promise<BatchEncryptResponse & { encryptedValues: Record<string, EncryptedValueString> }> {
    const result = await this.call<BatchEncryptResponse>({
      action: "batch_encrypt",
      group_id: groupId,
      account_id: accountId,
      items: Object.entries(items).map(([key, value]) => ({
        key,
        plaintext_b64: this.toBase64(value),
      })),
    });

    const encryptedValues: Record<string, EncryptedValueString> = {};
    for (const item of result.items) {
      if (!item.error) {
        encryptedValues[item.key] = `enc:AES256:${result.key_id}:${item.ciphertext_b64}` as EncryptedValueString;
      }
    }

    return { ...result, encryptedValues };
  }

  /**
   * Batch decrypt multiple items
   */
  async batchDecrypt(
    groupId: string,
    accountId: string,
    items: Array<{ key: string; ciphertextB64: string }>
  ): Promise<BatchDecryptResponse & { plaintexts: Record<string, string> }> {
    const result = await this.call<BatchDecryptResponse>({
      action: "batch_decrypt",
      group_id: groupId,
      account_id: accountId,
      items: items.map((item) => ({
        key: item.key,
        ciphertext_b64: item.ciphertextB64,
      })),
    });

    const plaintexts: Record<string, string> = {};
    for (const item of result.items) {
      if (!item.error && item.plaintext_utf8) {
        plaintexts[item.key] = item.plaintext_utf8;
      }
    }

    return { ...result, plaintexts };
  }

  /**
   * Verify group membership
   */
  async verifyMembership(
    groupId: string,
    accountId: string
  ): Promise<MembershipResponse> {
    return this.call<MembershipResponse>({
      action: "verify_membership",
      group_id: groupId,
      account_id: accountId,
    });
  }

  /**
   * Parse encrypted value string
   */
  parseEncryptedValue(value: string): { keyId: string; ciphertextB64: string } | null {
    if (!value.startsWith("enc:AES256:")) return null;

    const parts = value.split(":");
    if (parts.length !== 4) return null;

    return {
      keyId: parts[2],
      ciphertextB64: parts[3],
    };
  }

  // ============ Private Methods ============

  private async call<T>(request: KeyManagerRequest): Promise<T> {
    const outlayerRequest: OutLayerRequest = {
      source: {
        WasmUrl: {
          url: this.config.wasmUrl,
          hash: this.config.wasmHash,
          build_target: this.config.buildTarget,
        },
      },
      input_data: JSON.stringify(request),
      resource_limits: {
        max_instructions: 10_000_000_000,
        max_memory_mb: 128,
        max_execution_seconds: 60,
      },
      response_format: "Json",
    };

    const response = await fetch(`${this.config.outlayerApi}/call/Kampouse/key-manager`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Payment-Key": this.config.paymentKey,
      },
      body: JSON.stringify(outlayerRequest),
    });

    const result = await response.json();

    if (result.error) {
      throw new KeyManagerError(result.error, result.code ?? 500);
    }

    return result as T;
  }

  private toBase64(str: string): string {
    if (typeof btoa === "function") {
      return btoa(str);
    }
    return Buffer.from(str).toString("base64");
  }

  private fromBase64(b64: string): string {
    if (typeof atob === "function") {
      return atob(b64);
    }
    return Buffer.from(b64, "base64").toString();
  }
}

export class KeyManagerError extends Error {
  constructor(
    message: string,
    public readonly code: number
  ) {
    super(message);
    this.name = "KeyManagerError";
  }
}

export default KeyManagerClient;
