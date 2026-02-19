/**
 * FastKV + Key Manager Integration
 *
 * Complete client for encrypted key-value storage.
 */

const OUTLAYER_API = "https://api.outlayer.fastnear.com";
const FASTKV_API = "https://fastkv.example.com"; // Your FastKV server

class EncryptedKV {
  constructor(accountId, paymentKey) {
    this.accountId = accountId;
    this.paymentKey = paymentKey;
    this.groupId = `${accountId}/kv`;
    this.keyId = null; // Cached key ID
  }

  /**
   * Set encrypted value
   */
  async set(key, value) {
    // 1. Encrypt
    const encrypted = await this.callKeyManager({
      action: "encrypt",
      group_id: this.groupId,
      account_id: this.accountId,
      plaintext_b64: btoa(value)
    });

    // 2. Store with format: enc:AES256:key_id:ciphertext
    const storedValue = `enc:AES256:${encrypted.key_id}:${encrypted.ciphertext_b64}`;

    await fetch(`${FASTKV_API}/v1/kv/${key}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ value: storedValue, owner: this.accountId })
    });

    return { key, key_id: encrypted.key_id };
  }

  /**
   * Get and decrypt value
   */
  async get(key) {
    // 1. Retrieve
    const response = await fetch(`${FASTKV_API}/v1/kv/${key}`);
    const { value } = await response.json();

    if (!value || !value.startsWith("enc:AES256:")) {
      return value; // Not encrypted, return as-is
    }

    // 2. Parse format
    const [, , keyId, ciphertext] = value.split(":");

    // 3. Decrypt
    const decrypted = await this.callKeyManager({
      action: "decrypt",
      group_id: this.groupId,
      account_id: this.accountId,
      ciphertext_b64: ciphertext
    });

    return atob(decrypted.plaintext_b64);
  }

  /**
   * Batch set - encrypt multiple values in one call
   */
  async batchSet(items) {
    // 1. Batch encrypt
    const encryptItems = Object.entries(items).map(([key, value]) => ({
      key,
      plaintext_b64: btoa(value)
    }));

    const encrypted = await this.callKeyManager({
      action: "batch_encrypt",
      group_id: this.groupId,
      account_id: this.accountId,
      items: encryptItems
    });

    // 2. Build batch request
    const batchData = {};
    for (const item of encrypted.items) {
      batchData[item.key] = `enc:AES256:${encrypted.key_id}:${item.ciphertext_b64}`;
    }

    // 3. Store all
    await fetch(`${FASTKV_API}/v1/kv/batch`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(batchData)
    });

    return { count: items.length, key_id: encrypted.key_id };
  }

  /**
   * Batch get - retrieve and decrypt multiple values
   */
  async batchGet(keys) {
    // 1. Batch retrieve
    const response = await fetch(`${FASTKV_API}/v1/kv/batch?keys=${keys.join(",")}`);
    const data = await response.json();

    // 2. Collect encrypted items
    const decryptItems = [];
    const keyMap = new Map();

    for (const [key, value] of Object.entries(data)) {
      if (value?.startsWith("enc:AES256:")) {
        const [, , , ciphertext] = value.split(":");
        decryptItems.push({ key, ciphertext_b64: ciphertext });
        keyMap.set(key, "encrypted");
      } else {
        keyMap.set(key, value);
      }
    }

    // 3. Batch decrypt
    if (decryptItems.length > 0) {
      const decrypted = await this.callKeyManager({
        action: "batch_decrypt",
        group_id: this.groupId,
        account_id: this.accountId,
        items: decryptItems
      });

      for (const item of decrypted.items) {
        if (item.plaintext_utf8) {
          keyMap.set(item.key, item.plaintext_utf8);
        }
      }
    }

    // 4. Return as object
    const result = {};
    for (const [key, value] of keyMap) {
      result[key] = value;
    }
    return result;
  }

  /**
   * Delete value
   */
  async delete(key) {
    await fetch(`${FASTKV_API}/v1/kv/${key}`, { method: "DELETE" });
  }

  async callKeyManager(request) {
    const response = await fetch(`${OUTLAYER_API}/call/Kampouse/key-manager`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Payment-Key": this.paymentKey
      },
      body: JSON.stringify({
        source: {
          WasmUrl: {
            url: "https://github.com/Kampouse/key-manager/releases/download/v0.2.0/key-manager.wasm",
            hash: "44ce9f1f616e765f21fe208eb1ff4db29a7aac90096ca83cf75864793c21e7d3",
            build_target: "wasm32-wasip1"
          }
        },
        input_data: JSON.stringify(request),
        resource_limits: { max_instructions: 10000000000, max_memory_mb: 128, max_execution_seconds: 60 },
        response_format: "Json"
      })
    });

    const result = await response.json();
    if (result.error) throw new Error(result.error);
    return result;
  }
}

// Usage Example
async function example() {
  const kv = new EncryptedKV("alice.near", "pk_your_payment_key");

  // Single set/get
  await kv.set("api_key", "sk-abc123secret");
  const apiKey = await kv.get("api_key");
  console.log("API Key:", apiKey); // "sk-abc123secret"

  // Batch operations (more efficient)
  await kv.batchSet({
    db_password: "supersecret123",
    aws_key: "AKIAIOSFODNN7EXAMPLE",
    aws_secret: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
  });

  const secrets = await kv.batchGet(["db_password", "aws_key", "aws_secret"]);
  console.log("All secrets:", secrets);
}

export { EncryptedKV };
