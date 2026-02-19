/**
 * Encrypted Profile Client SDK
 *
 * Frontend library for managing user profiles with encrypted private data.
 * The server NEVER has access to decryption keys.
 */

const OUTLAYER_API = "https://api.outlayer.fastnear.com";
const KEY_MANAGER_WASM = "https://github.com/Kampouse/key-manager/releases/download/v0.2.0/key-manager.wasm";
const KEY_MANAGER_HASH = "44ce9f1f616e765f21fe208eb1ff4db29a7aac90096ca83cf75864793c21e7d3";

class EncryptedProfileClient {
  constructor(accountId, paymentKey) {
    this.accountId = accountId;
    this.paymentKey = paymentKey;
    this.groupId = `${accountId}/private`;
  }

  /**
   * Save user profile - encrypts private fields client-side
   */
  async saveProfile(profile) {
    const { public: publicFields, private: privateFields } = profile;

    // 1. Batch encrypt all private fields in ONE call
    const items = Object.entries(privateFields).map(([key, value]) => ({
      key,
      plaintext_b64: btoa(JSON.stringify(value))
    }));

    const encrypted = await this.callKeyManager({
      action: "batch_encrypt",
      group_id: this.groupId,
      account_id: this.accountId,
      items
    });

    // 2. Build encrypted record
    const encryptedRecord = {};
    for (const item of encrypted.items) {
      encryptedRecord[item.key] = `enc:AES256:${encrypted.key_id}:${item.ciphertext_b64}`;
    }

    // 3. Send to your API (server sees only encrypted blobs)
    const response = await fetch("/api/profile", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        account_id: this.accountId,
        public: publicFields,
        private_encrypted: encryptedRecord
      })
    });

    return response.json();
  }

  /**
   * Load user profile - decrypts private fields
   */
  async loadProfile() {
    // 1. Get encrypted record from your API
    const response = await fetch(`/api/profile/${this.accountId}`);
    const { public: publicFields, private_encrypted } = await response.json();

    // 2. Extract ciphertexts
    const items = [];
    for (const [key, value] of Object.entries(private_encrypted)) {
      if (value.startsWith("enc:AES256:")) {
        const [, , , ciphertext] = value.split(":");
        items.push({ key, ciphertext_b64: ciphertext });
      }
    }

    // 3. Batch decrypt
    const decrypted = await this.callKeyManager({
      action: "batch_decrypt",
      group_id: this.groupId,
      account_id: this.accountId,
      items
    });

    // 4. Reconstruct private fields
    const privateFields = {};
    for (const item of decrypted.items) {
      if (item.plaintext_utf8) {
        privateFields[item.key] = JSON.parse(item.plaintext_utf8);
      }
    }

    return { public: publicFields, private: privateFields };
  }

  /**
   * Call OutLayer Key Manager
   */
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
            url: KEY_MANAGER_WASM,
            hash: KEY_MANAGER_HASH,
            build_target: "wasm32-wasip1"
          }
        },
        input_data: JSON.stringify(request),
        resource_limits: {
          max_instructions: 10000000000,
          max_memory_mb: 128,
          max_execution_seconds: 60
        },
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
  const client = new EncryptedProfileClient("alice.near", "pk_your_payment_key");

  // Save profile with encrypted private data
  await client.saveProfile({
    public: {
      name: "Alice",
      avatar: "https://example.com/avatar.jpg"
    },
    private: {
      email: "alice@example.com",
      phone: "+1-555-1234",
      address: { city: "New York", zip: "10001" }
    }
  });

  // Load profile (private data decrypted)
  const profile = await client.loadProfile();
  console.log(profile.private.email); // "alice@example.com"
}

export { EncryptedProfileClient };
