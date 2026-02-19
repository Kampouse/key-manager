/**
 * TEE Wallet Signer
 *
 * Crypto wallet where private keys are derived inside TEE and never exported.
 * Users sign transactions without ever knowing their private key.
 */

import { KeyManagerClient } from "../key-manager-client";

// ============ Types ============

export interface WalletConfig {
  accountId: string;
  paymentKey: string;
}

export interface WalletInfo {
  accountId: string;
  keyId: string;
  attestation: string;
}

export interface NearTransaction {
  receiverId: string;
  actions: NearAction[];
  nonce?: number;
  blockHash?: string;
}

export type NearAction =
  | { type: "Transfer"; amount: string }
  | { type: "FunctionCall"; methodName: string; args: string; gas: string; deposit: string }
  | { type: "CreateAccount" }
  | { type: "DeleteAccount"; beneficiaryId: string };

export interface SignedTransaction {
  transaction: NearTransaction;
  signature: string;
  publicKey: string;
  attestation: string;
}

export interface SignatureResult {
  signature: string;
  publicKey: string;
  attestation: string;
}

// ============ Client ============

export class TEEWallet {
  private readonly keyManager: KeyManagerClient;
  private readonly accountId: string;
  private readonly groupId: string;
  private keyId: string | null = null;

  constructor(config: WalletConfig) {
    this.keyManager = new KeyManagerClient({ paymentKey: config.paymentKey });
    this.accountId = config.accountId;
    this.groupId = `${config.accountId}/wallet`;
  }

  /**
   * Get wallet info - returns public key only, NEVER private key
   */
  async getInfo(): Promise<WalletInfo> {
    const { key_id, attestation_hash } = await this.keyManager.getKey(
      this.groupId,
      this.accountId
    );

    this.keyId = key_id;

    return {
      accountId: this.accountId,
      keyId: key_id,
      attestation: attestation_hash,
    };
  }

  /**
   * Sign a transaction
   *
   * NOTE: Current key-manager doesn't have signing capability.
   * This shows the integration pattern for when it's added.
   */
  async signTransaction(transaction: NearTransaction): Promise<SignedTransaction> {
    // 1. Get key info
    const info = await this.getInfo();

    // 2. Serialize transaction
    const txBytes = this.serializeTransaction(transaction);

    // 3. Hash transaction
    const txHash = await this.hashBytes(txBytes);

    // 4. Sign in TEE
    // NOTE: This would require adding 'sign' action to key-manager
    // See: https://github.com/Kampouse/key-manager/issues
    const signatureResult = await this.signInTEE(txHash);

    return {
      transaction,
      signature: signatureResult.signature,
      publicKey: info.keyId, // In production, this would be the secp256k1 public key
      attestation: signatureResult.attestation,
    };
  }

  /**
   * Sign a message (for authentication)
   */
  async signMessage(message: string): Promise<SignatureResult> {
    const info = await this.getInfo();
    const messageBytes = new TextEncoder().encode(message);
    const hash = await this.hashBytes(messageBytes);

    return this.signInTEE(hash);
  }

  /**
   * Verify a signature (client-side)
   */
  verifySignature(message: string, signature: string, publicKey: string): boolean {
    // In production, would use secp256k1 ECDSA verification
    // For now, placeholder
    console.log("[INFO] Would verify signature:", {
      message,
      signature: signature.slice(0, 20) + "...",
      publicKey,
    });

    return true; // Placeholder
  }

  // ============ Private Methods ============

  private async signInTEE(hash: Uint8Array): Promise<SignatureResult> {
    // NOTE: This would call key-manager with a 'sign' action
    // Current implementation shows the API pattern

    const hashHex = Array.from(hash)
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("");

    console.log("[INFO] Would sign in TEE:", {
      hash: hashHex,
      _note: "Requires 'sign' action in key-manager",
      _implementation: "Add secp256k1 ECDSA to WASM",
    });

    // Placeholder return - would come from TEE
    return {
      signature: "PLACEHOLDER_SIG",
      publicKey: this.keyId ?? "unknown",
      attestation: "pending_implementation",
    };
  }

  private serializeTransaction(tx: NearTransaction): Uint8Array {
    // Simplified - real implementation would use borsh serialization
    const serialized = JSON.stringify({
      receiver_id: tx.receiverId,
      actions: tx.actions.map((action) => {
        switch (action.type) {
          case "Transfer":
            return { Transfer: { deposit: action.amount } };
          case "FunctionCall":
            return {
              FunctionCall: {
                method_name: action.methodName,
                args: action.args,
                gas: action.gas,
                deposit: action.deposit,
              },
            };
          case "CreateAccount":
            return { CreateAccount: {} };
          case "DeleteAccount":
            return { DeleteAccount: { beneficiary_id: action.beneficiaryId } };
        }
      }),
      nonce: tx.nonce,
      block_hash: tx.blockHash,
    });

    return new TextEncoder().encode(serialized);
  }

  private async hashBytes(bytes: Uint8Array): Promise<Uint8Array> {
    // SHA-256 hash
    const hashBuffer = await crypto.subtle.digest("SHA-256", bytes);
    return new Uint8Array(hashBuffer);
  }
}

// ============ Security Benefits ============

export const WALLET_SECURITY_BENEFITS = `
🔒 TEE Wallet Security Benefits:

1. Phishing Resistant
   - User clicks fake link? Attacker gets nothing
   - Key never leaves TEE, cannot be exported

2. Malware Resistant
   - Keylogger on device? Gets password, not key
   - Key derived fresh each time in TEE

3. No Backup Needed
   - Key derived from CKD, not generated and stored
   - Lose device? Derive same key from CKD on new device

4. Auditable
   - Every signature has attestation
   - Proof that signature came from TEE, not leaked key

5. Upgradeable Code
   - Can update wallet logic without changing keys
   - CKD ensures same key across code updates
`;

// ============ Usage Example ============

async function example() {
  const wallet = new TEEWallet({
    accountId: "alice.near",
    paymentKey: "pk_your_payment_key",
  });

  // Get wallet info (public key only)
  const info = await wallet.getInfo();
  console.log("Wallet:", info.accountId);
  console.log("Key ID:", info.keyId);
  console.log("Attestation:", info.attestation);

  // Sign a transfer
  const signedTx = await wallet.signTransaction({
    receiverId: "bob.near",
    actions: [
      { type: "Transfer", amount: "1000000000000000000000000" }, // 1 NEAR
    ],
  });

  console.log("Signed transaction:", signedTx);

  // Sign a message for authentication
  const authSig = await wallet.signMessage("Login to app.near");
  console.log("Auth signature:", authSig.signature);
}

export default TEEWallet;
