/**
 * Shared Vault Manager
 *
 * Team secrets with group-based access control.
 * Access determined by contextual.near membership.
 */

import { KeyManagerClient } from "../key-manager-client";

// ============ Types ============

export interface VaultConfig {
  projectId: string;
  accountId: string;
  paymentKey: string;
  fastDataApi?: string;
}

export interface SecretMetadata {
  key: string;
  created_at: string;
  updated_at: string;
  updated_by: string;
}

export interface TeamMember {
  account_id: string;
  role: "owner" | "admin" | "member";
  added_at: string;
  added_by: string;
}

export interface EncryptedSecret {
  key: string;
  value: string; // enc:AES256:... format
  metadata: SecretMetadata;
}

export interface DecryptedSecret<K extends string = string> {
  key: K;
  value: string;
}

// ============ Client ============

export class SharedVault {
  private readonly keyManager: KeyManagerClient;
  private readonly projectId: string;
  private readonly accountId: string;
  private readonly groupId: string;
  private readonly fastDataApi: string;

  constructor(config: VaultConfig) {
    this.keyManager = new KeyManagerClient({ paymentKey: config.paymentKey });
    this.projectId = config.projectId;
    this.accountId = config.accountId;
    this.groupId = `${config.projectId}/secrets`;
    this.fastDataApi = config.fastDataApi ?? "https://fastdata.up.railway.app";
  }

  /**
   * Check if current user is a project member
   */
  async checkMembership(): Promise<boolean> {
    try {
      const response = await fetch(
        `${this.fastDataApi}/graph/${this.projectId}/members/${this.accountId}`
      );

      if (!response.ok) return false;

      const { is_member } = await response.json() as { is_member: boolean };
      return is_member;
    } catch {
      return false;
    }
  }

  /**
   * Store a secret (requires membership)
   */
  async setSecret<K extends string>(key: K, value: string): Promise<{ key: K; keyId: string }> {
    if (!await this.checkMembership()) {
      throw new VaultError(`Not a member of ${this.projectId}`, "FORBIDDEN");
    }

    // Encrypt
    const { encryptedValue, key_id } = await this.keyManager.encrypt(
      this.groupId,
      this.accountId,
      value
    );

    // Store in FastKV
    await fetch(`${this.fastDataApi}/kv/${this.projectId}/secrets/${key}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        value: encryptedValue,
        owner: this.accountId,
      }),
    });

    return { key, keyId: key_id };
  }

  /**
   * Retrieve a secret (requires membership)
   */
  async getSecret<K extends string>(key: K): Promise<DecryptedSecret<K>> {
    if (!await this.checkMembership()) {
      throw new VaultError(`Not a member of ${this.projectId}`, "FORBIDDEN");
    }

    // Get encrypted value
    const response = await fetch(
      `${this.fastDataApi}/kv/${this.projectId}/secrets/${key}`
    );

    if (!response.ok) {
      throw new VaultError(`Secret '${key}' not found`, "NOT_FOUND");
    }

    const { value } = await response.json() as { value: string };

    // Parse and decrypt
    const parsed = this.keyManager.parseEncryptedValue(value);
    if (!parsed) {
      throw new VaultError("Invalid encrypted value format", "INVALID_FORMAT");
    }

    const { plaintext } = await this.keyManager.decrypt(
      this.groupId,
      this.accountId,
      parsed.ciphertextB64
    );

    return { key, value: plaintext };
  }

  /**
   * Load all secrets (for CI/CD or deployment)
   */
  async loadAllSecrets<K extends string = string>(): Promise<Record<K, string>> {
    if (!await this.checkMembership()) {
      throw new VaultError(`Not a member of ${this.projectId}`, "FORBIDDEN");
    }

    // List all secrets
    const response = await fetch(`${this.fastDataApi}/kv/${this.projectId}/secrets`);
    const secrets = await response.json() as Record<string, string>;

    // Collect ciphertexts
    const decryptItems: Array<{ key: string; ciphertextB64: string }> = [];

    for (const [key, value] of Object.entries(secrets)) {
      const parsed = this.keyManager.parseEncryptedValue(value);
      if (parsed) {
        decryptItems.push({ key, ciphertextB64: parsed.ciphertextB64 });
      }
    }

    // Batch decrypt
    const { plaintexts } = await this.keyManager.batchDecrypt(
      this.groupId,
      this.accountId,
      decryptItems
    );

    return plaintexts as Record<K, string>;
  }

  /**
   * Set multiple secrets at once (batch operation)
   */
  async setSecrets(secrets: Record<string, string>): Promise<{ keyId: string; count: number }> {
    if (!await this.checkMembership()) {
      throw new VaultError(`Not a member of ${this.projectId}`, "FORBIDDEN");
    }

    // Batch encrypt
    const { encryptedValues, key_id } = await this.keyManager.batchEncrypt(
      this.groupId,
      this.accountId,
      secrets
    );

    // Store all
    await fetch(`${this.fastDataApi}/kv/${this.projectId}/secrets/batch`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(encryptedValues),
    });

    return { keyId: key_id, count: Object.keys(secrets).length };
  }

  /**
   * Delete a secret
   */
  async deleteSecret(key: string): Promise<void> {
    await fetch(`${this.fastDataApi}/kv/${this.projectId}/secrets/${key}`, {
      method: "DELETE",
    });
  }

  /**
   * Add team member (project owner only)
   */
  async addMember(memberId: string, role: "admin" | "member" = "member"): Promise<void> {
    await fetch(`${this.fastDataApi}/graph/${this.projectId}/members`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        owner: this.accountId,
        member: memberId,
        role,
      }),
    });
  }

  /**
   * Remove team member - instantly revokes access
   */
  async removeMember(memberId: string): Promise<void> {
    await fetch(`${this.fastDataApi}/graph/${this.projectId}/members/${memberId}`, {
      method: "DELETE",
    });
    // No need to re-encrypt - key derivation is group-based
  }

  /**
   * List team members
   */
  async listMembers(): Promise<TeamMember[]> {
    const response = await fetch(`${this.fastDataApi}/graph/${this.projectId}/members`);
    return await response.json() as TeamMember[];
  }
}

export class VaultError extends Error {
  constructor(
    message: string,
    public readonly code: "FORBIDDEN" | "NOT_FOUND" | "INVALID_FORMAT" | "NETWORK_ERROR"
  ) {
    super(message);
    this.name = "VaultError";
  }
}

// ============ Usage Examples ============

async function cicdExample() {
  const vault = new SharedVault({
    projectId: "myproject.near",
    accountId: process.env.NEAR_ACCOUNT!,
    paymentKey: process.env.OUTLAYER_PAYMENT_KEY!,
  });

  // Load all secrets for deployment
  const secrets = await vault.loadAllSecrets<"db_password" | "api_key" | "deploy_token">();

  console.log("DB_PASSWORD=", secrets.db_password);
  console.log("API_KEY=", secrets.api_key);
  console.log("DEPLOY_TOKEN=", secrets.deploy_token);
}

async function teamExample() {
  const vault = new SharedVault({
    projectId: "myproject.near",
    accountId: "owner.near",
    paymentKey: "pk_...",
  });

  // Add new developer
  await vault.addMember("bob.near", "member");
  console.log("Bob can now access all secrets");

  // Set project secrets
  await vault.setSecrets({
    db_password: "supersecret123",
    api_key: "sk-abc123",
  });

  // Remove developer - instant access revocation
  await vault.removeMember("bob.near");
  console.log("Bob's access revoked - no re-encryption needed");
}

export default SharedVault;
