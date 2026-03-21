//! Tests for types module

use crate::types::*;

#[test]
fn test_encrypted_value_parse() {
    let valid = "enc:AES256:key123:YWJjZGVmZ2hpamtsbW5vcA==";
    let parsed = EncryptedValue::parse(valid).unwrap();
    
    assert_eq!(parsed.algorithm, "AES256");
    assert_eq!(parsed.key_id, "key123");
    assert_eq!(parsed.ciphertext, "YWJjZGVmZ2hpamtsbW5vcA==");
}

#[test]
fn test_encrypted_value_parse_invalid() {
    // Missing prefix
    assert!(EncryptedValue::parse("AES256:key123:data").is_err());
    
    // Too few parts
    assert!(EncryptedValue::parse("enc:AES256:key123").is_err());
    
    // Wrong format
    assert!(EncryptedValue::parse("invalid").is_err());
}

#[test]
fn test_encrypted_value_serialize() {
    let value = EncryptedValue {
        algorithm: "AES256".into(),
        key_id: "key123".into(),
        ciphertext: "YWJjZGVmZ2hpamtsbW5vcA==".into(),
    };
    
    let serialized = value.to_string();
    assert_eq!(serialized, "enc:AES256:key123:YWJjZGVmZ2hpamtsbW5vcA==");
}

#[test]
fn test_wrapped_key_serialization() {
    let wrapped = WrappedKey {
        encrypted_key: "base64key".into(),
        key_id: "key123".into(),
        wrapping_key_id: "wrap123".into(),
        attestation: None,
    };
    
    let json = serde_json::to_string(&wrapped).unwrap();
    assert!(json.contains("encrypted_key"));
    assert!(json.contains("key123"));
}

#[test]
fn test_attestation() {
    let attestation = Attestation {
        hash: "abc123".into(),
        timestamp: 1234567890,
        platform: "sgx".into(),
    };
    
    let json = serde_json::to_string(&attestation).unwrap();
    assert!(json.contains("abc123"));
    assert!(json.contains("sgx"));
}

#[test]
fn test_storage_response() {
    let json = r#"{"success":true,"value":"test","error":null}"#;
    let response: StorageResponse = serde_json::from_str(json).unwrap();
    
    assert!(response.success);
    assert_eq!(response.value, Some("test".to_string()));
    assert_eq!(response.error, None);
}

#[test]
fn test_storage_response_error() {
    let json = r#"{"success":false,"value":null,"error":"Not found"}"#;
    let response: StorageResponse = serde_json::from_str(json).unwrap();
    
    assert!(!response.success);
    assert_eq!(response.value, None);
    assert_eq!(response.error, Some("Not found".to_string()));
}
