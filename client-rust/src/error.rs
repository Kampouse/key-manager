use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Encryption: {0}")]
    Encryption(String),
    #[error("Decryption: {0}")]
    Decryption(String),
    #[error("Key derivation: {0}")]
    KeyDerivation(String),
    #[error("Invalid ciphertext: {0}")]
    InvalidCiphertext(String),
    #[error("Storage: {0}")]
    Storage(String),
    #[error("TEE: {0}")]
    Tee(String),
    #[error("Serialization: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Base64: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    #[error("Attestation: {0}")]
    Attestation(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
}
