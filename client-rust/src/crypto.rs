use aes_gcm::{aead::{Aead, KeyInit, OsRng}, Aes256Gcm, Nonce};
use rand::RngCore;
use crate::{Error, Result};

pub struct Encryptor {
    cipher: Aes256Gcm,
}

impl Encryptor {
    pub fn new(key: &[u8; 32]) -> Self {
        Self { cipher: Aes256Gcm::new_from_slice(key).expect("Invalid key") }
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        let mut iv = [0u8; 12];
        OsRng.fill_bytes(&mut iv);
        let nonce = Nonce::from_slice(&iv);
        let ciphertext = self.cipher.encrypt(nonce, plaintext)
            .map_err(|e| Error::Encryption(e.to_string()))?;
        Ok((iv.to_vec(), ciphertext))
    }

    pub fn decrypt(&self, iv: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
        let nonce = Nonce::from_slice(iv);
        self.cipher.decrypt(nonce, ciphertext)
            .map_err(|e| Error::Decryption(e.to_string()))
    }
}

pub fn serialize_encrypted(iv: &[u8], ciphertext: &[u8]) -> String {
    let mut combined = Vec::with_capacity(iv.len() + ciphertext.len());
    combined.extend_from_slice(iv);
    combined.extend_from_slice(ciphertext);
    base64::encode(&combined)
}

pub fn deserialize_encrypted(encoded: &str) -> Result<(Vec<u8>, Vec<u8>)> {
    let decoded = base64::decode(encoded)?;
    if decoded.len() < 12 {
        return Err(Error::InvalidCiphertext("Too short".into()));
    }
    Ok((decoded[..12].to_vec(), decoded[12..].to_vec()))
}
