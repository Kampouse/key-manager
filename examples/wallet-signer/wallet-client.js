/**
 * TEE Wallet Signer
 *
 * Crypto wallet where private keys are derived inside TEE and never exported.
 * Users sign transactions without ever knowing their private key.
 */

const OUTLAYER_API = "https://api.outlayer.fastnear.com";

class TEEWallet {
  constructor(accountId, paymentKey) {
    this.accountId = accountId;
    this.groupId = `${accountId}/wallet`;
    this.paymentKey = paymentKey;
  }

  /**
   * Derive wallet key - returns public key only, NEVER private key
   */
  async getPublicKey() {
    const result = await this.callKeyManager({
      action: "get_key",
      group_id: this.groupId,
      account_id: this.accountId
    });

    return {
      key_id: result.key_id,
      attestation: result.attestation_hash,
      _note: "Private key never exposed - exists only in TEE"
    };
  }

  /**
   * Sign a transaction
   *
   * NOTE: Would require adding 'sign' action to key-manager with secp256k1
   * This shows the integration pattern.
   */
  async signTransaction(transaction) {
    const txBytes = new TextEncoder().encode(JSON.stringify(transaction));
    const hashBuffer = await crypto.subtle.digest('SHA-256', txBytes);
    const txHash = new Uint8Array(hashBuffer);

    // Would call key-manager with 'sign' action
    // Returns signature, NOT the private key
    console.log("[INFO] Would sign tx in TEE:", {
      tx_hash: Array.from(txHash).map(b => b.toString(16).padStart(2, '0')).join(''),
      _requires: "Add 'sign' action to key-manager"
    });

    return { status: "requires_sign_action" };
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

    return await response.json();
  }
}

export { TEEWallet };
