/**
 * Encrypted Profile Client
 *
 * Social app where users store private data (email, phone, bio).
 * Server NEVER has access to decryption keys - GDPR/HIPAA compliant.
 */

import { KeyManagerClient, KeyManagerError } from "../key-manager-client";
import type { EncryptedValueString } from "../types";

// ============ Types ============

export interface PublicProfile {
  name: string;
  avatar?: string;
  bio?: string;
}

export interface PrivateProfileData {
  email: string;
  phone?: string;
  address?: AddressData;
  [key: string]: string | AddressData | undefined;
}

export interface AddressData {
  street?: string;
  city: string;
  state?: string;
  zip: string;
  country?: string;
}

export interface StoredProfile {
  account_id: string;
  public: PublicProfile;
  private_encrypted: Record<string, EncryptedValueString>;
  updated_at: string;
}

export interface FullProfile {
  account_id: string;
  public: PublicProfile;
  private: PrivateProfileData;
}

export interface ProfileClientConfig {
  accountId: string;
  paymentKey: string;
  apiBaseUrl?: string;
}

// ============ Client ============

export class EncryptedProfileClient {
  private readonly keyManager: KeyManagerClient;
  private readonly groupId: string;
  private readonly accountId: string;
  private readonly apiBaseUrl: string;

  constructor(config: ProfileClientConfig) {
    this.keyManager = new KeyManagerClient({ paymentKey: config.paymentKey });
    this.groupId = `${config.accountId}/private`;
    this.accountId = config.accountId;
    this.apiBaseUrl = config.apiBaseUrl ?? "/api";
  }

  /**
   * Save profile with encrypted private fields
   */
  async saveProfile(
    publicData: PublicProfile,
    privateData: PrivateProfileData
  ): Promise<{ success: boolean }> {
    // 1. Batch encrypt all private fields
    const { encryptedValues } = await this.keyManager.batchEncrypt(
      this.groupId,
      this.accountId,
      this.serializePrivateData(privateData)
    );

    // 2. Send to API (server sees only encrypted blobs)
    const response = await fetch(`${this.apiBaseUrl}/profile`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        account_id: this.accountId,
        public: publicData,
        private_encrypted: encryptedValues,
      } as Omit<StoredProfile, "updated_at">),
    });

    if (!response.ok) {
      throw new ProfileError(`Failed to save profile: ${response.statusText}`);
    }

    return { success: true };
  }

  /**
   * Load profile with decrypted private fields
   */
  async loadProfile(): Promise<FullProfile> {
    // 1. Get encrypted record from API
    const response = await fetch(`${this.apiBaseUrl}/profile/${this.accountId}`);

    if (!response.ok) {
      throw new ProfileError(`Failed to load profile: ${response.statusText}`);
    }

    const stored: StoredProfile = await response.json();

    // 2. Extract ciphertexts
    const decryptItems: Array<{ key: string; ciphertextB64: string }> = [];

    for (const [key, value] of Object.entries(stored.private_encrypted)) {
      const parsed = this.keyManager.parseEncryptedValue(value);
      if (parsed) {
        decryptItems.push({ key, ciphertextB64: parsed.ciphertextB64 });
      }
    }

    // 3. Batch decrypt
    const { plaintexts } = await this.keyManager.batchDecrypt(
      this.groupId,
      this.accountId,
      decryptItems
    );

    // 4. Reconstruct private fields
    const privateData = this.deserializePrivateData(plaintexts);

    return {
      account_id: this.accountId,
      public: stored.public,
      private: privateData,
    };
  }

  /**
   * Update single private field
   */
  async updatePrivateField<K extends keyof PrivateProfileData>(
    field: K,
    value: PrivateProfileData[K]
  ): Promise<void> {
    const serialized = typeof value === "string" ? value : JSON.stringify(value);
    const { encryptedValue } = await this.keyManager.encrypt(
      this.groupId,
      this.accountId,
      serialized
    );

    await fetch(`${this.apiBaseUrl}/profile/${this.accountId}/field/${field}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ value: encryptedValue }),
    });
  }

  /**
   * Delete profile (GDPR "Right to be Forgotten")
   */
  async deleteProfile(): Promise<void> {
    const response = await fetch(`${this.apiBaseUrl}/profile/${this.accountId}`, {
      method: "DELETE",
    });

    if (!response.ok) {
      throw new ProfileError(`Failed to delete profile: ${response.statusText}`);
    }
  }

  // ============ Private Helpers ============

  private serializePrivateData(data: PrivateProfileData): Record<string, string> {
    const result: Record<string, string> = {};

    for (const [key, value] of Object.entries(data)) {
      if (value !== undefined) {
        result[key] = typeof value === "string" ? value : JSON.stringify(value);
      }
    }

    return result;
  }

  private deserializePrivateData(plaintexts: Record<string, string>): PrivateProfileData {
    const result: PrivateProfileData = { email: "" };

    for (const [key, value] of Object.entries(plaintexts)) {
      // Try to parse as JSON first (for objects like address)
      try {
        const parsed = JSON.parse(value);
        result[key] = parsed;
      } catch {
        result[key] = value;
      }
    }

    return result;
  }
}

export class ProfileError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ProfileError";
  }
}

// ============ Usage Example ============

async function example() {
  const client = new EncryptedProfileClient({
    accountId: "alice.near",
    paymentKey: "pk_your_payment_key",
    apiBaseUrl: "https://api.example.com",
  });

  // Save profile
  await client.saveProfile(
    {
      name: "Alice",
      avatar: "https://example.com/avatar.jpg",
      bio: "Building on NEAR",
    },
    {
      email: "alice@example.com",
      phone: "+1-555-1234",
      address: {
        city: "New York",
        zip: "10001",
        country: "USA",
      },
    }
  );

  // Load profile (private data decrypted)
  const profile = await client.loadProfile();
  console.log("Email:", profile.private.email);
  console.log("Address:", profile.private.address?.city);

  // Update single field
  await client.updatePrivateField("phone", "+1-555-9999");
}

export default EncryptedProfileClient;
