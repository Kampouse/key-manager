use async_trait::async_trait;
use reqwest::Client;
use crate::{Error, Result, WrappedKey, Attestation, DeriveKeyRequest, DeriveKeyResponse};

pub struct OutLayerAdapter {
    client: Client,
    api_url: String,
    api_key: String,
}

impl OutLayerAdapter {
    pub fn new(api_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_url: api_url.into(),
            api_key: api_key.into(),
        }
    }

    pub async fn derive_key(&self, group_id: &str) -> Result<WrappedKey> {
        let url = format!("{}/v1/key/derive", self.api_url);
        let request = DeriveKeyRequest { group_id: group_id.into() };
        
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(Error::Tee(format!("Key derivation failed: {}", error)));
        }
        
        let derive_response: DeriveKeyResponse = response.json().await
            .map_err(|e| Error::Tee(format!("Parse error: {}", e)))?;
        
        Ok(derive_response.wrapped_key)
    }

    pub async fn unwrap_key(&self, wrapped_key: &WrappedKey) -> Result<Vec<u8>> {
        let key_bytes = base64::decode(&wrapped_key.encrypted_key)?;
        Ok(key_bytes)
    }
}

#[async_trait]
pub trait TeeAdapter: Send + Sync {
    async fn derive_key(&self, group_id: &str) -> Result<WrappedKey>;
    async fn unwrap_key(&self, wrapped_key: &WrappedKey) -> Result<Vec<u8>>;
}

#[async_trait]
impl TeeAdapter for OutLayerAdapter {
    async fn derive_key(&self, group_id: &str) -> Result<WrappedKey> {
        self.derive_key(group_id).await
    }
    
    async fn unwrap_key(&self, wrapped_key: &WrappedKey) -> Result<Vec<u8>> {
        self.unwrap_key(wrapped_key).await
    }
}
