use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use async_trait::async_trait;
use crate::{
    crypto::{Encryptor, serialize_encrypted, deserialize_encrypted},
    outlayer_adapter::TeeAdapter,
    fastkv_adapter::StorageAdapter,
    EncryptedValue, Error, Result,
};

pub struct PrivateKV {
    tee: Arc<dyn TeeAdapter>,
    storage: Arc<dyn StorageAdapter>,
    key_cache: RwLock<HashMap<String, [u8; 32]>>,
}

impl PrivateKV {
    pub fn new(
        tee: impl TeeAdapter + 'static,
        storage: impl StorageAdapter + 'static,
    ) -> Self {
        Self {
            tee: Arc::new(tee),
            storage: Arc::new(storage),
            key_cache: RwLock::new(HashMap::new()),
        }
    }

    async fn get_key(&self, group_id: &str) -> Result<[u8; 32]> {
        {
            let cache = self.key_cache.read().await;
            if let Some(key) = cache.get(group_id) {
                return Ok(*key);
            }
        }
        
        let wrapped_key = self.tee.derive_key(group_id).await?;
        let key_bytes = self.tee.unwrap_key(&wrapped_key).await?;
        
        let mut key = [0u8; 32];
        if key_bytes.len() >= 32 {
            key.copy_from_slice(&key_bytes[..32]);
        } else {
            return Err(Error::KeyDerivation("Key too short".into()));
        }
        
        {
            let mut cache = self.key_cache.write().await;
            cache.insert(group_id.into(), key);
        }
        
        Ok(key)
    }

    async fn encrypt(&self, group_id: &str, plaintext: &str) -> Result<String> {
        let key = self.get_key(group_id).await?;
        let encryptor = Encryptor::new(&key);
        let (iv, ciphertext) = encryptor.encrypt(plaintext.as_bytes())?;
        let encoded = serialize_encrypted(&iv, &ciphertext);
        Ok(format!("enc:AES256:{}:{}", group_id, encoded))
    }

    async fn decrypt(&self, group_id: &str, encrypted: &str) -> Result<String> {
        let encrypted_value = EncryptedValue::parse(encrypted)?;
        let (iv, ciphertext) = deserialize_encrypted(&encrypted_value.ciphertext)?;
        let key = self.get_key(group_id).await?;
        let encryptor = Encryptor::new(&key);
        let plaintext_bytes = encryptor.decrypt(&iv, &ciphertext)?;
        String::from_utf8(plaintext_bytes)
            .map_err(|e| Error::Decryption(format!("Invalid UTF-8: {}", e)))
    }

    pub async fn set(&self, key: &str, value: &str, group_id: &str) -> Result<()> {
        let encrypted = self.encrypt(group_id, value).await?;
        self.storage.set(key, &encrypted, group_id).await
    }

    pub async fn get(&self, key: &str, group_id: &str) -> Result<Option<String>> {
        match self.storage.get(key, group_id).await? {
            Some(encrypted) => {
                let plaintext = self.decrypt(group_id, &encrypted).await?;
                Ok(Some(plaintext))
            }
            None => Ok(None),
        }
    }

    pub async fn delete(&self, key: &str, group_id: &str) -> Result<()> {
        self.storage.delete(key, group_id).await
    }

    pub async fn clear_cache(&self) {
        let mut cache = self.key_cache.write().await;
        cache.clear();
    }
}

#[async_trait]
pub trait KeyValueStore: Send + Sync {
    async fn set(&self, key: &str, value: &str, group_id: &str) -> Result<()>;
    async fn get(&self, key: &str, group_id: &str) -> Result<Option<String>>;
    async fn delete(&self, key: &str, group_id: &str) -> Result<()>;
}

#[async_trait]
impl KeyValueStore for PrivateKV {
    async fn set(&self, key: &str, value: &str, group_id: &str) -> Result<()> {
        self.set(key, value, group_id).await
    }
    
    async fn get(&self, key: &str, group_id: &str) -> Result<Option<String>> {
        self.get(key, group_id).await
    }
    
    async fn delete(&self, key: &str, group_id: &str) -> Result<()> {
        self.delete(key, group_id).await
    }
}
