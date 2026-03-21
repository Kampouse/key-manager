//! Integration tests for Key-Manager client

use key_manager_client::*;

#[tokio::test]
async fn test_encrypt_decrypt_roundtrip() {
    let key = [42u8; 32];
    let encryptor = Encryptor::new(&key);
    
    let plaintext = b"Hello, World!";
    let (iv, ciphertext) = encryptor.encrypt(plaintext).unwrap();
    let decrypted = encryptor.decrypt(&iv, &ciphertext).unwrap();
    
    assert_eq!(decrypted, plaintext);
}

#[tokio::test]
async fn test_serialize_deserialize() {
    let iv = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    let ciphertext = vec![13, 14, 15, 16, 17, 18];
    
    let serialized = serialize_encrypted(&iv, &ciphertext);
    let (deserialized_iv, deserialized_ciphertext) = 
        deserialize_encrypted(&serialized).unwrap();
    
    assert_eq!(iv, deserialized_iv);
    assert_eq!(ciphertext, deserialized_ciphertext);
}

#[tokio::test]
async fn test_encrypted_value_parse() {
    let encrypted = "enc:AES256:key123:YWJjZGVmZ2hpamtsbW5vcA==";
    let parsed = EncryptedValue::parse(encrypted).unwrap();
    
    assert_eq!(parsed.algorithm, "AES256");
    assert_eq!(parsed.key_id, "key123");
    assert_eq!(parsed.ciphertext, "YWJjZGVmZ2hpamtsbW5vcA==");
}

#[tokio::test]
async fn test_different_plaintexts() {
    let key = [0u8; 32];
    let encryptor = Encryptor::new(&key);
    
    let plaintexts = vec![
        b"short".to_vec(),
        b"This is a longer message".to_vec(),
        vec![0u8; 1000],
        (0..255).collect::<Vec<u8>>(),
    ];
    
    for plaintext in plaintexts {
        let (iv, ciphertext) = encryptor.encrypt(&plaintext).unwrap();
        let decrypted = encryptor.decrypt(&iv, &ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}

#[tokio::test]
async fn test_unique_ivs() {
    let key = [0u8; 32];
    let encryptor = Encryptor::new(&key);
    
    let plaintext = b"Same message";
    let (iv1, _) = encryptor.encrypt(plaintext).unwrap();
    let (iv2, _) = encryptor.encrypt(plaintext).unwrap();
    
    assert_ne!(iv1, iv2);
}

#[tokio::test]
async fn test_decrypt_with_wrong_key() {
    let key1 = [1u8; 32];
    let key2 = [2u8; 32];
    
    let encryptor1 = Encryptor::new(&key1);
    let encryptor2 = Encryptor::new(&key2);
    
    let plaintext = b"Secret message";
    let (iv, ciphertext) = encryptor1.encrypt(plaintext).unwrap();
    
    let result = encryptor2.decrypt(&iv, &ciphertext);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_tampered_ciphertext() {
    let key = [0u8; 32];
    let encryptor = Encryptor::new(&key);
    
    let plaintext = b"Original message";
    let (iv, mut ciphertext) = encryptor.encrypt(plaintext).unwrap();
    
    if !ciphertext.is_empty() {
        ciphertext[0] ^= 0xFF;
    }
    
    let result = encryptor.decrypt(&iv, &ciphertext);
    assert!(result.is_err());
}
