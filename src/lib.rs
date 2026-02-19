//! Key Manager for OutLayer TEE
//!
//! This WASM module runs inside OutLayer's TEE and handles:
//! - Key derivation from CKD (Confidential Key Derivation)
//! - Encryption/decryption of FastKV values
//! - Membership verification

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// CKD master key - in real OutLayer, this is derived from hardware
/// In production, this comes from OutLayer's CKD mechanism
const CKD_MASTER_SEED: &[u8] = b"outlayer_ckd_master_seed_do_not_use_in_prod";

/// Request types
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Request {
    GetKey {
        group_id: String,
        account_id: String,
    },
    GetGroupKeyId {
        group_id: String,
        account_id: String,
    },
    WrapKey {
        group_id: String,
        account_id: String,
        plaintext_key_b64: String,
    },
    UnwrapKey {
        group_id: String,
        account_id: String,
        wrapped_key_b64: String,
    },
    Encrypt {
        group_id: String,
        account_id: String,
        plaintext_b64: String,
    },
    Decrypt {
        group_id: String,
        account_id: String,
        ciphertext_b64: String,
    },
    VerifyMembership {
        group_id: String,
        account_id: String,
    },
    BatchEncrypt {
        group_id: String,
        account_id: String,
        items: Vec<EncryptItem>,
    },
    BatchDecrypt {
        group_id: String,
        account_id: String,
        items: Vec<DecryptItem>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EncryptItem {
    pub key: String,
    pub plaintext_b64: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DecryptItem {
    pub key: String,
    pub ciphertext_b64: String,
}

/// Response types
#[derive(Debug, Serialize, Deserialize)]
pub struct KeyResponse {
    pub key_b64: String,
    pub key_id: String,
    pub group_id: String,
    pub attestation_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupKeyIdResponse {
    pub key_id: String,
    pub group_id: String,
    pub algorithm: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WrapKeyResponse {
    pub wrapped_key_b64: String,
    pub key_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnwrapKeyResponse {
    pub plaintext_key_b64: String,
    pub key_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EncryptResponse {
    pub ciphertext_b64: String,
    pub key_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DecryptResponse {
    pub plaintext_b64: String,
    pub plaintext_utf8: Option<String>,
    pub key_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MembershipResponse {
    pub is_member: bool,
    pub group_id: String,
    pub account_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchEncryptResponse {
    pub key_id: String,
    pub items: Vec<BatchEncryptItemResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchEncryptItemResult {
    pub key: String,
    pub ciphertext_b64: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchDecryptResponse {
    pub key_id: String,
    pub items: Vec<BatchDecryptItemResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchDecryptItemResult {
    pub key: String,
    pub plaintext_b64: String,
    pub plaintext_utf8: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: u32,
}

/// Derive a group-specific key from CKD master
pub fn derive_group_key(master_seed: &[u8], group_id: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(master_seed);
    hasher.update(group_id.as_bytes());
    hasher.update(b"fastkv_encryption_key_v1");

    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// Generate key ID from group_id (deterministic)
pub fn key_id_for_group(group_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(group_id.as_bytes());
    hasher.update(b"key_id_v1");
    hex::encode(&hasher.finalize()[..8])
}

/// Generate attestation hash (in real OutLayer, this comes from Intel TDX)
pub fn generate_attestation(input: &str, output: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"outlayer_attestation_v1");
    hasher.update(input.as_bytes());
    hasher.update(output.as_bytes());
    hex::encode(&hasher.finalize()[..16])
}

/// Encrypt data with AES-256-GCM
pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;

    // Generate random IV
    let iv_bytes: [u8; 12] = {
        // In real OutLayer TEE, use hardware RNG
        // For testing, we use a deterministic approach
        let mut hasher = Sha256::new();
        hasher.update(plaintext);
        hasher.update(key);
        let result = hasher.finalize();
        let mut iv = [0u8; 12];
        iv.copy_from_slice(&result[..12]);
        iv
    };

    let nonce = Nonce::from_slice(&iv_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("Encryption failed: {}", e))?;

    // Format: IV (12) + ciphertext + auth_tag (16, included in ciphertext)
    let mut result = iv_bytes.to_vec();
    result.extend(ciphertext);
    Ok(result)
}

/// Decrypt data with AES-256-GCM
pub fn decrypt(ciphertext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
    if ciphertext.len() < 12 + 16 {
        return Err("Ciphertext too short".to_string());
    }

    let iv = &ciphertext[..12];
    let encrypted_data = &ciphertext[12..];

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let nonce = Nonce::from_slice(iv);

    cipher
        .decrypt(nonce, encrypted_data)
        .map_err(|e| format!("Decryption failed: {}", e))
}

/// Check membership (mock implementation)
/// In production, this would query contextual.near contract
pub fn check_membership(group_id: &str, account_id: &str) -> bool {
    // Mock: everyone is a member of their own groups
    // Real implementation: query contextual.near is_member view
    group_id.starts_with(&account_id.split('.').next().unwrap_or(""))
        || account_id.ends_with(".near")
        || account_id.ends_with(".testnet")
}

/// Main entry point for OutLayer WASM execution
pub fn execute(input: &str) -> String {
    let request: Result<Request, _> = serde_json::from_str(input);

    match request {
        Ok(req) => match req {
            Request::GetKey { group_id, account_id } => {
                handle_get_key(&group_id, &account_id)
            }
            Request::GetGroupKeyId { group_id, account_id } => {
                handle_get_group_key_id(&group_id, &account_id)
            }
            Request::WrapKey {
                group_id,
                account_id,
                plaintext_key_b64,
            } => handle_wrap_key(&group_id, &account_id, &plaintext_key_b64),
            Request::UnwrapKey {
                group_id,
                account_id,
                wrapped_key_b64,
            } => handle_unwrap_key(&group_id, &account_id, &wrapped_key_b64),
            Request::Encrypt {
                group_id,
                account_id,
                plaintext_b64,
            } => handle_encrypt(&group_id, &account_id, &plaintext_b64),
            Request::Decrypt {
                group_id,
                account_id,
                ciphertext_b64,
            } => handle_decrypt(&group_id, &account_id, &ciphertext_b64),
            Request::VerifyMembership {
                group_id,
                account_id,
            } => handle_verify_membership(&group_id, &account_id),
            Request::BatchEncrypt {
                group_id,
                account_id,
                items,
            } => handle_batch_encrypt(&group_id, &account_id, &items),
            Request::BatchDecrypt {
                group_id,
                account_id,
                items,
            } => handle_batch_decrypt(&group_id, &account_id, &items),
        },
        Err(e) => error_response(&format!("Invalid request: {}", e), 400),
    }
}

fn handle_get_key(group_id: &str, account_id: &str) -> String {
    // Check membership first
    if !check_membership(group_id, account_id) {
        return error_response("Not a group member", 403);
    }

    let key = derive_group_key(CKD_MASTER_SEED, group_id);
    let key_id = key_id_for_group(group_id);

    let response = KeyResponse {
        key_b64: BASE64.encode(key),
        key_id,
        group_id: group_id.to_string(),
        attestation_hash: generate_attestation(
            &format!("get_key:{}:{}", group_id, account_id),
            &BASE64.encode(key),
        ),
    };

    serde_json::to_string(&response).unwrap_or_else(|e| error_response(&e.to_string(), 500))
}

fn handle_get_group_key_id(group_id: &str, account_id: &str) -> String {
    // No membership check needed - key_id is public info
    let key_id = key_id_for_group(group_id);

    let response = GroupKeyIdResponse {
        key_id,
        group_id: group_id.to_string(),
        algorithm: "AES-256-GCM".to_string(),
    };

    serde_json::to_string(&response).unwrap_or_else(|e| error_response(&e.to_string(), 500))
}

fn handle_wrap_key(group_id: &str, account_id: &str, plaintext_key_b64: &str) -> String {
    // Check membership - only members can wrap keys
    if !check_membership(group_id, account_id) {
        return error_response("Not a group member", 403);
    }

    // Decode the plaintext key (client's ephemeral encryption key)
    let plaintext_key = match BASE64.decode(plaintext_key_b64) {
        Ok(k) => k,
        Err(e) => return error_response(&format!("Invalid base64 key: {}", e), 400),
    };

    // Validate key length (32 bytes for AES-256)
    if plaintext_key.len() != 32 {
        return error_response("Key must be 32 bytes", 400);
    }

    // Get group key
    let group_key = derive_group_key(CKD_MASTER_SEED, group_id);

    // Wrap the plaintext key (encrypt with group key)
    let wrapped_key = match encrypt(&plaintext_key, &group_key) {
        Ok(w) => w,
        Err(e) => return error_response(&e, 500),
    };

    let response = WrapKeyResponse {
        wrapped_key_b64: BASE64.encode(&wrapped_key),
        key_id: key_id_for_group(group_id),
    };

    serde_json::to_string(&response).unwrap_or_else(|e| error_response(&e.to_string(), 500))
}

fn handle_unwrap_key(group_id: &str, account_id: &str, wrapped_key_b64: &str) -> String {
    // Check membership - only members can unwrap keys
    if !check_membership(group_id, account_id) {
        return error_response("Not a group member", 403);
    }

    // Decode the wrapped key
    let wrapped_key = match BASE64.decode(wrapped_key_b64) {
        Ok(k) => k,
        Err(e) => return error_response(&format!("Invalid base64 wrapped key: {}", e), 400),
    };

    // Get group key
    let group_key = derive_group_key(CKD_MASTER_SEED, group_id);

    // Unwrap (decrypt with group key)
    let plaintext_key = match decrypt(&wrapped_key, &group_key) {
        Ok(k) => k,
        Err(e) => return error_response(&e, 500),
    };

    let response = UnwrapKeyResponse {
        plaintext_key_b64: BASE64.encode(&plaintext_key),
        key_id: key_id_for_group(group_id),
    };

    serde_json::to_string(&response).unwrap_or_else(|e| error_response(&e.to_string(), 500))
}

fn handle_encrypt(group_id: &str, account_id: &str, plaintext_b64: &str) -> String {
    if !check_membership(group_id, account_id) {
        return error_response("Not a group member", 403);
    }

    let plaintext = match BASE64.decode(plaintext_b64) {
        Ok(p) => p,
        Err(e) => return error_response(&format!("Invalid base64 plaintext: {}", e), 400),
    };

    let key = derive_group_key(CKD_MASTER_SEED, group_id);
    let ciphertext = match encrypt(&plaintext, &key) {
        Ok(c) => c,
        Err(e) => return error_response(&e, 500),
    };

    let response = EncryptResponse {
        ciphertext_b64: BASE64.encode(&ciphertext),
        key_id: key_id_for_group(group_id),
    };

    serde_json::to_string(&response).unwrap_or_else(|e| error_response(&e.to_string(), 500))
}

fn handle_decrypt(group_id: &str, account_id: &str, ciphertext_b64: &str) -> String {
    if !check_membership(group_id, account_id) {
        return error_response("Not a group member", 403);
    }

    let ciphertext = match BASE64.decode(ciphertext_b64) {
        Ok(c) => c,
        Err(e) => return error_response(&format!("Invalid base64 ciphertext: {}", e), 400),
    };

    let key = derive_group_key(CKD_MASTER_SEED, group_id);
    let plaintext = match decrypt(&ciphertext, &key) {
        Ok(p) => p,
        Err(e) => return error_response(&e, 500),
    };

    let plaintext_b64 = BASE64.encode(&plaintext);
    let plaintext_utf8 = String::from_utf8(plaintext.clone()).ok();

    let response = DecryptResponse {
        plaintext_b64,
        plaintext_utf8,
        key_id: key_id_for_group(group_id),
    };

    serde_json::to_string(&response).unwrap_or_else(|e| error_response(&e.to_string(), 500))
}

fn handle_verify_membership(group_id: &str, account_id: &str) -> String {
    let is_member = check_membership(group_id, account_id);

    let response = MembershipResponse {
        is_member,
        group_id: group_id.to_string(),
        account_id: account_id.to_string(),
    };

    serde_json::to_string(&response).unwrap_or_else(|e| error_response(&e.to_string(), 500))
}

fn handle_batch_encrypt(group_id: &str, account_id: &str, items: &[EncryptItem]) -> String {
    if !check_membership(group_id, account_id) {
        return error_response("Not a group member", 403);
    }

    let key = derive_group_key(CKD_MASTER_SEED, group_id);
    let key_id = key_id_for_group(group_id);

    let results: Vec<BatchEncryptItemResult> = items
        .iter()
        .map(|item| {
            match BASE64.decode(&item.plaintext_b64) {
                Ok(plaintext) => match encrypt(&plaintext, &key) {
                    Ok(ciphertext) => BatchEncryptItemResult {
                        key: item.key.clone(),
                        ciphertext_b64: BASE64.encode(&ciphertext),
                        error: None,
                    },
                    Err(e) => BatchEncryptItemResult {
                        key: item.key.clone(),
                        ciphertext_b64: String::new(),
                        error: Some(e),
                    },
                },
                Err(e) => BatchEncryptItemResult {
                    key: item.key.clone(),
                    ciphertext_b64: String::new(),
                    error: Some(format!("Invalid base64: {}", e)),
                },
            }
        })
        .collect();

    let response = BatchEncryptResponse { key_id, items: results };
    serde_json::to_string(&response).unwrap_or_else(|e| error_response(&e.to_string(), 500))
}

fn handle_batch_decrypt(group_id: &str, account_id: &str, items: &[DecryptItem]) -> String {
    if !check_membership(group_id, account_id) {
        return error_response("Not a group member", 403);
    }

    let key = derive_group_key(CKD_MASTER_SEED, group_id);
    let key_id = key_id_for_group(group_id);

    let results: Vec<BatchDecryptItemResult> = items
        .iter()
        .map(|item| {
            match BASE64.decode(&item.ciphertext_b64) {
                Ok(ciphertext) => match decrypt(&ciphertext, &key) {
                    Ok(plaintext) => {
                        let plaintext_b64 = BASE64.encode(&plaintext);
                        let plaintext_utf8 = String::from_utf8(plaintext).ok();
                        BatchDecryptItemResult {
                            key: item.key.clone(),
                            plaintext_b64,
                            plaintext_utf8,
                            error: None,
                        }
                    }
                    Err(e) => BatchDecryptItemResult {
                        key: item.key.clone(),
                        plaintext_b64: String::new(),
                        plaintext_utf8: None,
                        error: Some(e),
                    },
                },
                Err(e) => BatchDecryptItemResult {
                    key: item.key.clone(),
                    plaintext_b64: String::new(),
                    plaintext_utf8: None,
                    error: Some(format!("Invalid base64: {}", e)),
                },
            }
        })
        .collect();

    let response = BatchDecryptResponse { key_id, items: results };
    serde_json::to_string(&response).unwrap_or_else(|e| error_response(&e.to_string(), 500))
}

fn error_response(message: &str, code: u32) -> String {
    serde_json::to_string(&ErrorResponse {
        error: message.to_string(),
        code,
    })
    .unwrap_or_else(|_| r#"{"error":"Internal error","code":500}"#.to_string())
}

/// WASM entry point
#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;

    #[no_mangle]
    pub extern "C" fn outlayer_main(input_ptr: *const u8, input_len: usize) -> *mut u8 {
        let input = unsafe { std::slice::from_raw_parts(input_ptr, input_len) };
        let input_str = std::str::from_utf8(input).unwrap_or("");

        let output = execute(input_str);
        let output_bytes = output.into_bytes();

        let result = output_bytes.leak();
        result.as_mut_ptr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn test_derive_group_key() {
        let key1 = derive_group_key(b"master", "group1");
        let key2 = derive_group_key(b"master", "group2");
        let key1_again = derive_group_key(b"master", "group1");

        // Different groups have different keys
        assert_ne!(key1, key2);
        // Same group always gets same key
        assert_eq!(key1, key1_again);
    }

    #[test]
    fn test_encrypt_decrypt() {
        let mut rng = rand::thread_rng();
        let mut key = [0u8; 32];
        rng.fill(&mut key);

        let plaintext = b"Hello, encrypted world!";
        let ciphertext = encrypt(plaintext, &key).expect("Encryption failed");

        // Ciphertext should be longer (IV + auth tag)
        assert!(ciphertext.len() > plaintext.len());

        let decrypted = decrypt(&ciphertext, &key).expect("Decryption failed");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_with_wrong_key_fails() {
        let mut rng = rand::thread_rng();
        let mut key1 = [0u8; 32];
        let mut key2 = [0u8; 32];
        rng.fill(&mut key1);
        rng.fill(&mut key2);

        let plaintext = b"Secret message";
        let ciphertext = encrypt(plaintext, &key1).expect("Encryption failed");

        let result = decrypt(&ciphertext, &key2);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_get_key() {
        let input = r#"{"action":"get_key","group_id":"alice.near/private","account_id":"alice.near"}"#;
        let output = execute(input);

        let response: KeyResponse = serde_json::from_str(&output).expect("Invalid response");
        assert_eq!(response.group_id, "alice.near/private");
        assert!(!response.key_b64.is_empty());
    }

    #[test]
    fn test_execute_encrypt_decrypt_roundtrip() {
        let plaintext = b"Hello NEAR!";
        let plaintext_b64 = BASE64.encode(plaintext);

        // Encrypt
        let encrypt_input = serde_json::to_string(&Request::Encrypt {
            group_id: "alice.near/data".to_string(),
            account_id: "alice.near".to_string(),
            plaintext_b64: plaintext_b64.clone(),
        })
        .unwrap();

        let encrypt_output = execute(&encrypt_input);
        let encrypt_resp: EncryptResponse =
            serde_json::from_str(&encrypt_output).expect("Encrypt failed");

        // Decrypt
        let decrypt_input = serde_json::to_string(&Request::Decrypt {
            group_id: "alice.near/data".to_string(),
            account_id: "alice.near".to_string(),
            ciphertext_b64: encrypt_resp.ciphertext_b64,
        })
        .unwrap();

        let decrypt_output = execute(&decrypt_input);
        let decrypt_resp: DecryptResponse =
            serde_json::from_str(&decrypt_output).expect("Decrypt failed");

        // Verify roundtrip
        let decrypted = BASE64.decode(&decrypt_resp.plaintext_b64).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_membership_check() {
        // Mock: .near and .testnet accounts are members
        assert!(check_membership("anygroup", "alice.near"));
        assert!(check_membership("anygroup", "bob.testnet"));

        // Group starting with account name is accessible
        assert!(check_membership("alice/private", "alice.near"));
    }

    #[test]
    fn test_key_id_deterministic() {
        let id1 = key_id_for_group("alice.near/data");
        let id2 = key_id_for_group("alice.near/data");
        let id3 = key_id_for_group("bob.near/data");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_attestation_generation() {
        let att1 = generate_attestation("input1", "output1");
        let att2 = generate_attestation("input1", "output1");
        let att3 = generate_attestation("input2", "output1");

        assert_eq!(att1, att2);
        assert_ne!(att1, att3);
        assert_eq!(att1.len(), 32); // 16 bytes hex
    }
}
