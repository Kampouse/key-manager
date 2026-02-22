use anyhow::Result;
use redis::{AsyncCommands, Client as RedisClient};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;

pub const UNIVERSAL_SUFFIX: &str = "universal";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastData {
    pub receipt_id: String,
    pub action_index: u32,
    pub suffix: String,
    pub data: String, // Base64 encoded
    pub tx_hash: Option<String>,
    pub signer_id: String,
    pub predecessor_id: String,
    pub current_account_id: String,
    pub block_height: u64,
    pub block_timestamp: u64,
    pub shard_id: u32,
    pub receipt_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastDataKv {
    pub receipt_id: String,
    pub action_index: u32,
    pub tx_hash: Option<String>,
    pub signer_id: String,
    pub predecessor_id: String,
    pub current_account_id: String,
    pub block_height: u64,
    pub block_timestamp: u64,
    pub shard_id: u32,
    pub receipt_index: u32,
    pub order_id: u64,
    pub key: String,
    pub value: String,
    pub encrypted_key_id: Option<String>,
}

pub struct RedisDb {
    client: RedisClient,
    chain_id: String,
}

impl RedisDb {
    pub async fn new(chain_id: String) -> Result<Self> {
        let redis_url = env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        
        tracing::info!("Connecting to Redis: {}", redis_url);
        
        let client = redis::Client::open(redis_url.as_str())?;
        
        // Test connection
        let mut conn = client.get_multiplexed_async_connection().await?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        tracing::info!("Redis connection established");
        
        Ok(Self { client, chain_id })
    }
    
    pub async fn test_connection(&self) -> Result<()> {
        self.health_check().await
    }
    
    pub async fn health_check(&self) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        Ok(())
    }
    
    // Key format helpers
    fn checkpoint_key(&self, suffix: &str) -> String {
        format!("checkpoint:{}:{}", self.chain_id, suffix)
    }
    
    fn fastdata_key(&self, suffix: &str, block_height: u64, receipt_id: &str) -> String {
        format!("fastdata:{}:{}:{}:{}", self.chain_id, suffix, block_height, receipt_id)
    }
    
    fn kv_key(&self, predecessor_id: &str, current_account_id: &str, key: &str) -> String {
        format!("kv:{}:{}:{}", predecessor_id, current_account_id, key)
    }
    
    fn kv_history_key(&self, predecessor_id: &str, current_account_id: &str, key: &str) -> String {
        format!("history:{}:{}:{}", predecessor_id, current_account_id, key)
    }
    
    fn accounts_key(&self, current_account_id: &str) -> String {
        format!("accounts:{}", current_account_id)
    }
    
    fn contracts_key(&self, predecessor_id: &str) -> String {
        format!("contracts:{}", predecessor_id)
    }
    
    fn meta_key(&self) -> String {
        format!("meta:{}", self.chain_id)
    }
    
    // Checkpoint operations
    pub async fn get_last_processed_block_height(&self, suffix: &str) -> Result<Option<u64>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.checkpoint_key(suffix);
        let height: Option<String> = conn.get(&key).await?;
        Ok(height.and_then(|h| h.parse().ok()))
    }
    
    pub async fn set_last_processed_block_height(&self, suffix: &str, height: u64) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.checkpoint_key(suffix);
        conn.set(&key, height.to_string()).await?;
        Ok(())
    }
    
    // FastData operations (for main-indexer)
    pub async fn add_data(&self, fastdata: &FastData) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        
        let key = self.fastdata_key(
            &fastdata.suffix,
            fastdata.block_height,
            &fastdata.receipt_id,
        );
        
        let json = serde_json::to_string(fastdata)?;
        conn.set(&key, &json).await?;
        
        // Also update meta block height
        let meta_key = self.meta_key();
        conn.set(&meta_key, fastdata.block_height.to_string()).await?;
        
        Ok(())
    }
    
    // KV operations (for kv-sub-indexer)
    pub async fn add_kv(&self, kv: &FastDataKv) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        
        // Store current value
        let current_key = self.kv_key(&kv.predecessor_id, &kv.current_account_id, &kv.key);
        let stored = StoredKvEntry {
            predecessor_id: kv.predecessor_id.clone(),
            current_account_id: kv.current_account_id.clone(),
            key: kv.key.clone(),
            value: kv.value.clone(),
            block_height: kv.block_height,
            block_timestamp: kv.block_timestamp,
            receipt_id: kv.receipt_id.clone(),
            tx_hash: kv.tx_hash.clone().unwrap_or_default(),
        };
        let json = serde_json::to_string(&stored)?;
        conn.set(&current_key, &json).await?;
        
        // Store history
        let history_key = self.kv_history_key(&kv.predecessor_id, &kv.current_account_id, &kv.key);
        conn.zadd(&history_key, &json, kv.block_height as i64).await?;
        
        // Update accounts set (who wrote to this contract)
        let accounts_key = self.accounts_key(&kv.current_account_id);
        conn.sadd(&accounts_key, &kv.predecessor_id).await?;
        
        // Update contracts set (which contracts this account wrote to)
        let contracts_key = self.contracts_key(&kv.predecessor_id);
        conn.sadd(&contracts_key, &kv.current_account_id).await?;
        
        Ok(())
    }
    
    pub async fn add_kv_batch(&self, kvs: &[FastDataKv]) -> Result<()> {
        for kv in kvs {
            self.add_kv(kv).await?;
        }
        Ok(())
    }
    
    pub async fn set_indexer_block_height(&self, height: u64) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.meta_key();
        conn.set(&key, height.to_string()).await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredKvEntry {
    predecessor_id: String,
    current_account_id: String,
    key: String,
    value: String,
    block_height: u64,
    block_timestamp: u64,
    receipt_id: String,
    tx_hash: String,
}

/// Helper function to compute order_id (same as scylladb module)
pub fn compute_order_id(fastdata: &FastData) -> Result<u64, String> {
    let order_id = ((fastdata.shard_id as u64) & 0xFFFF) << 48
        | ((fastdata.receipt_index as u64) & 0xFFFF) << 32
        | ((fastdata.action_index as u64) & 0xFFFF);
    Ok(order_id)
}

/// Retry helper
pub async fn retry_with_delays<F, Fut, T, E>(delays: &[u64], mut f: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    let mut last_error = None;
    for &delay_secs in delays {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                tracing::warn!("Operation failed, retrying after {}s: {:?}", delay_secs, e);
                last_error = Some(e);
                if delay_secs > 0 {
                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                }
            }
        }
    }
    Err(anyhow::anyhow!("Operation failed after all retries: {:?}", last_error))
}
