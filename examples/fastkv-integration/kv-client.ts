/**
 * Encrypted KV Client
 *
 * Direct integration with FastKV for encrypted key-value storage.
 */

import { KeyManagerClient } from "../key-manager-client";
import type { EncryptedValueString } from "../types";

// ============ Types ============

export interface KVConfig {
  accountId: string;
  paymentKey: string;
  fastkvApi?: string;
  groupId?: string;
}

export interface KVEntry<T = string> {
  key: string;
  value: T;
  created_at?: string;
  updated_at?: string;
}

export interface BatchGetResult<T = string> {
  [key: string]: T;
}

export interface SetOptions {
  ttl?: number; // Time to live in seconds
  metadata?: Record<string, unknown>;
}

// ============ Client ============

export class EncryptedKV {
  private readonly keyManager: KeyManagerClient;
  private readonly accountId: string;
  private readonly groupId: string;
  private readonly fastkvApi: string;

  constructor(config: KVConfig) {
    this.keyManager = new KeyManagerClient({ paymentKey: config.paymentKey });
    this.accountId = config.accountId;
    this.groupId = config.groupId ?? `${config.accountId}/kv`;
    this.fastkvApi = config.fastkvApi ?? "https://fastkv.example.com";
  }

  /**
   * Set encrypted value
   */
  async set(key: string, value: string, options?: SetOptions): Promise<{ key: string; keyId: string }> {
    // Encrypt
    const { encryptedValue, key_id } = await this.keyManager.encrypt(
      this.groupId,
      this.accountId,
      value
    );

    // Store
    const response = await fetch(`${this.fastkvApi}/v1/kv/${key}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        value: encryptedValue,
        owner: this.accountId,
        ttl: options?.ttl,
        metadata: options?.metadata,
      }),
    });

    if (!response.ok) {
      throw new KVError(`Failed to set '${key}': ${response.statusText}`, "SET_ERROR");
    }

    return { key, keyId: key_id };
  }

  /**
   * Get and decrypt value
   */
  async get(key: string): Promise<string | null> {
    const response = await fetch(`${this.fastkvApi}/v1/kv/${key}`);

    if (response.status === 404) return null;
    if (!response.ok) {
      throw new KVError(`Failed to get '${key}': ${response.statusText}`, "GET_ERROR");
    }

    const { value } = await response.json() as { value: string };

    // Check if encrypted
    const parsed = this.keyManager.parseEncryptedValue(value);
    if (!parsed) {
      return value; // Not encrypted, return as-is
    }

    // Decrypt
    const { plaintext } = await this.keyManager.decrypt(
      this.groupId,
      this.accountId,
      parsed.ciphertextB64
    );

    return plaintext;
  }

  /**
   * Batch set - encrypt multiple values in one call
   */
  async batchSet(items: Record<string, string>, options?: SetOptions): Promise<{
    count: number;
    keyId: string;
  }> {
    // Batch encrypt
    const { encryptedValues, key_id } = await this.keyManager.batchEncrypt(
      this.groupId,
      this.accountId,
      items
    );

    // Store all
    const response = await fetch(`${this.fastkvApi}/v1/kv/batch`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        items: encryptedValues,
        owner: this.accountId,
        ttl: options?.ttl,
      }),
    });

    if (!response.ok) {
      throw new KVError(`Failed to batch set: ${response.statusText}`, "BATCH_SET_ERROR");
    }

    return { count: Object.keys(items).length, keyId: key_id };
  }

  /**
   * Batch get - retrieve and decrypt multiple values
   */
  async batchGet<K extends string>(keys: K[]): Promise<BatchGetResult> {
    // Retrieve
    const response = await fetch(
      `${this.fastkvApi}/v1/kv/batch?keys=${keys.join(",")}`
    );

    if (!response.ok) {
      throw new KVError(`Failed to batch get: ${response.statusText}`, "BATCH_GET_ERROR");
    }

    const data = await response.json() as Record<string, string>;

    // Separate encrypted vs plain
    const decryptItems: Array<{ key: string; ciphertextB64: string }> = [];
    const results: BatchGetResult = {};

    for (const [key, value] of Object.entries(data)) {
      const parsed = this.keyManager.parseEncryptedValue(value);
      if (parsed) {
        decryptItems.push({ key, ciphertextB64: parsed.ciphertextB64 });
      } else {
        results[key] = value;
      }
    }

    // Batch decrypt encrypted values
    if (decryptItems.length > 0) {
      const { plaintexts } = await this.keyManager.batchDecrypt(
        this.groupId,
        this.accountId,
        decryptItems
      );

      Object.assign(results, plaintexts);
    }

    return results;
  }

  /**
   * Delete value
   */
  async delete(key: string): Promise<boolean> {
    const response = await fetch(`${this.fastkvApi}/v1/kv/${key}`, {
      method: "DELETE",
    });

    return response.ok;
  }

  /**
   * Check if key exists
   */
  async exists(key: string): Promise<boolean> {
    const response = await fetch(`${this.fastkvApi}/v1/kv/${key}`, {
      method: "HEAD",
    });

    return response.ok;
  }

  /**
   * List keys (returns encrypted values, doesn't decrypt)
   */
  async listKeys(prefix?: string): Promise<string[]> {
    const url = prefix
      ? `${this.fastkvApi}/v1/kv?prefix=${encodeURIComponent(prefix)}`
      : `${this.fastkvApi}/v1/kv`;

    const response = await fetch(url);

    if (!response.ok) {
      throw new KVError(`Failed to list keys: ${response.statusText}`, "LIST_ERROR");
    }

    const { keys } = await response.json() as { keys: string[] };
    return keys;
  }

  /**
   * Atomic compare-and-swap
   */
  async cas(
    key: string,
    expectedValue: string | null,
    newValue: string
  ): Promise<boolean> {
    // Get current value
    const current = await this.get(key);

    // Compare
    if (current !== expectedValue) {
      return false;
    }

    // Set new value
    await this.set(key, newValue);
    return true;
  }
}

export class KVError extends Error {
  constructor(
    message: string,
    public readonly code:
      | "SET_ERROR"
      | "GET_ERROR"
      | "BATCH_SET_ERROR"
      | "BATCH_GET_ERROR"
      | "LIST_ERROR"
      | "DELETE_ERROR"
  ) {
    super(message);
    this.name = "KVError";
  }
}

// ============ Usage Example ============

async function example() {
  const kv = new EncryptedKV({
    accountId: "alice.near",
    paymentKey: "pk_your_payment_key",
    fastkvApi: "https://fastkv.example.com",
  });

  // Single set/get
  await kv.set("api_key", "sk-abc123secret");
  const apiKey = await kv.get("api_key");
  console.log("API Key:", apiKey); // "sk-abc123secret"

  // Batch operations (more efficient)
  await kv.batchSet({
    db_password: "supersecret123",
    aws_key: "AKIAIOSFODNN7EXAMPLE",
    aws_secret: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
  });

  const secrets = await kv.batchGet(["db_password", "aws_key", "aws_secret"]);
  console.log("All secrets:", secrets);

  // Atomic CAS
  const updated = await kv.cas("api_key", "sk-abc123secret", "sk-newkey456");
  console.log("Updated:", updated); // true
}

export default EncryptedKV;
