use async_trait::async_trait;
use reqwest::Client;
use crate::{Error, Result, StorageResponse, GetRequest, SetRequest};

pub struct FastKVAdapter {
    client: Client,
    api_url: String,
}

impl FastKVAdapter {
    pub fn new(api_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_url: api_url.into(),
        }
    }

    pub async fn get(&self, key: &str, group_id: &str) -> Result<Option<String>> {
        let url = format!("{}/v1/kv/get", self.api_url);
        let request = GetRequest {
            key: key.into(),
            group_id: group_id.into(),
        };
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(Error::Storage(format!("Get failed: {}", error)));
        }
        
        let storage_response: StorageResponse = response.json().await
            .map_err(|e| Error::Storage(format!("Parse error: {}", e)))?;
        
        if !storage_response.success {
            return Err(Error::Storage(
                storage_response.error.unwrap_or_else(|| "Unknown error".into())
            ));
        }
        
        Ok(storage_response.value)
    }

    pub async fn set(&self, key: &str, value: &str, group_id: &str) -> Result<()> {
        let url = format!("{}/v1/kv/set", self.api_url);
        let request = SetRequest {
            key: key.into(),
            value: value.into(),
            group_id: group_id.into(),
        };
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(Error::Storage(format!("Set failed: {}", error)));
        }
        
        Ok(())
    }

    pub async fn delete(&self, key: &str, group_id: &str) -> Result<()> {
        let url = format!("{}/v1/kv/delete", self.api_url);
        let request = GetRequest {
            key: key.into(),
            group_id: group_id.into(),
        };
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(Error::Storage(format!("Delete failed: {}", error)));
        }
        
        Ok(())
    }
}

#[async_trait]
pub trait StorageAdapter: Send + Sync {
    async fn get(&self, key: &str, group_id: &str) -> Result<Option<String>>;
    async fn set(&self, key: &str, value: &str, group_id: &str) -> Result<()>;
    async fn delete(&self, key: &str, group_id: &str) -> Result<()>;
}

#[async_trait]
impl StorageAdapter for FastKVAdapter {
    async fn get(&self, key: &str, group_id: &str) -> Result<Option<String>> {
        self.get(key, group_id).await
    }
    
    async fn set(&self, key: &str, value: &str, group_id: &str) -> Result<()> {
        self.set(key, value, group_id).await
    }
    
    async fn delete(&self, key: &str, group_id: &str) -> Result<()> {
        self.delete(key, group_id).await
    }
}
