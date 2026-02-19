/**
 * Shared Vault Manager
 *
 * Manages team secrets with group-based access control.
 * Access determined by contextual.near membership.
 */

const OUTLAYER_API = "https://api.outlayer.fastnear.com";
const FASTDATA_API = "https://fastdata.up.railway.app";

class SharedVault {
  constructor(projectId, accountId, paymentKey) {
    this.projectId = projectId;
    this.accountId = accountId;
    this.groupId = `${projectId}/secrets`;
    this.paymentKey = paymentKey;
  }

  /**
   * Check if current user is a project member
   */
  async checkMembership() {
    const response = await fetch(
      `${FASTDATA_API}/graph/${this.projectId}/members/${this.accountId}`
    );
    const { is_member } = await response.json();
    return is_member;
  }

  /**
   * Store a secret (requires membership)
   */
  async setSecret(key, value) {
    // 1. Verify membership
    if (!await this.checkMembership()) {
      throw new Error(`Not a member of ${this.projectId}`);
    }

    // 2. Encrypt the secret
    const encrypted = await this.callKeyManager({
      action: "encrypt",
      group_id: this.groupId,
      account_id: this.accountId,
      plaintext_b64: btoa(value)
    });

    // 3. Store in FastKV
    await fetch(`${FASTDATA_API}/kv/${this.projectId}/secrets/${key}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        value: `enc:AES256:${encrypted.key_id}:${encrypted.ciphertext_b64}`,
        owner: this.accountId
      })
    });

    return { key, key_id: encrypted.key_id };
  }

  /**
   * Retrieve a secret (requires membership)
   */
  async getSecret(key) {
    // 1. Verify membership
    if (!await this.checkMembership()) {
      throw new Error(`Not a member of ${this.projectId}`);
    }

    // 2. Get encrypted value
    const response = await fetch(
      `${FASTDATA_API}/kv/${this.projectId}/secrets/${key}`
    );
    const { value } = await response.json();

    if (!value || !value.startsWith("enc:AES256:")) {
      throw new Error("Secret not found or not encrypted");
    }

    // 3. Extract ciphertext
    const [, , , ciphertext] = value.split(":");

    // 4. Decrypt
    const decrypted = await this.callKeyManager({
      action: "decrypt",
      group_id: this.groupId,
      account_id: this.accountId,
      ciphertext_b64: ciphertext
    });

    return atob(decrypted.plaintext_b64);
  }

  /**
   * Load all secrets for CI/CD or deployment
   */
  async loadAllSecrets() {
    // 1. List all secrets
    const response = await fetch(
      `${FASTDATA_API}/kv/${this.projectId}/secrets`
    );
    const secrets = await response.json();

    // 2. Batch decrypt
    const items = [];
    for (const [key, value] of Object.entries(secrets)) {
      if (value.startsWith("enc:AES256:")) {
        const [, , , ciphertext] = value.split(":");
        items.push({ key, ciphertext_b64: ciphertext });
      }
    }

    const decrypted = await this.callKeyManager({
      action: "batch_decrypt",
      group_id: this.groupId,
      account_id: this.accountId,
      items
    });

    // 3. Build secrets object
    const result = {};
    for (const item of decrypted.items) {
      if (item.plaintext_utf8) {
        result[item.key] = item.plaintext_utf8;
      }
    }

    return result;
  }

  /**
   * Add team member (project owner only)
   */
  async addMember(newMemberId) {
    await fetch(`${FASTDATA_API}/graph/${this.projectId}/members`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        owner: this.accountId,
        member: newMemberId
      })
    });
  }

  /**
   * Remove team member - instantly revokes access
   */
  async removeMember(memberId) {
    await fetch(
      `${FASTDATA_API}/graph/${this.projectId}/members/${memberId}`,
      { method: "DELETE" }
    );
    // No need to re-encrypt - key derivation is group-based
    // Removed member no longer passes checkMembership()
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

// Usage: CI/CD pipeline loading secrets
async function cicdExample() {
  const vault = new SharedVault(
    "myproject.near",
    process.env.NEAR_ACCOUNT,
    process.env.OUTLAYER_PAYMENT_KEY
  );

  // Load all secrets in one call
  const secrets = await vault.loadAllSecrets();

  console.log("DB_PASSWORD=", secrets.db_password);
  console.log("API_KEY=", secrets.api_key);
  console.log("DEPLOY_TOKEN=", secrets.deploy_token);
}

// Usage: Team management
async function teamExample() {
  const vault = new SharedVault("myproject.near", "owner.near", "pk_...");

  // Add new developer
  await vault.addMember("bob.near");
  console.log("Bob can now access all secrets");

  // Remove developer
  await vault.removeMember("bob.near");
  console.log("Bob's access revoked - no re-encryption needed");
}

export { SharedVault };
