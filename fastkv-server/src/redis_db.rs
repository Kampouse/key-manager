use anyhow::Result;
use redis::{AsyncCommands, Client as RedisClient};
use serde::{Deserialize, Serialize};

use crate::models::{KvEntry, HistoryParams, WritersParams, TimelineParams};

/// Internal stored entry for Redis JSON serialization
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

impl From<StoredKvEntry> for KvEntry {
    fn from(s: StoredKvEntry) -> Self {
        let is_deleted = s.value == "null";
        Self {
            predecessor_id: s.predecessor_id,
            current_account_id: s.current_account_id,
            key: s.key,
            value: s.value,
            block_height: s.block_height,
            block_timestamp: s.block_timestamp,
            receipt_id: s.receipt_id,
            tx_hash: s.tx_hash,
            is_deleted,
        }
    }
}

pub struct RedisDb {
    client: RedisClient,
    chain_id: String,
}

impl RedisDb {
    pub async fn new(chain_id: String) -> Result<Self> {
        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        
        tracing::info!("Connecting to Redis: {}", redis_url);
        
        let client = redis::Client::open(redis_url.as_str())?;
        
        // Test connection
        let mut conn = client.get_multiplexed_async_connection().await?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        tracing::info!("Redis connection established");
        
        Ok(Self { client, chain_id })
    }
    
    pub async fn health_check(&self) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        Ok(())
    }
    
    // Key format helpers
    fn kv_key(&self, predecessor_id: &str, current_account_id: &str, key: &str) -> String {
        format!("kv:{}:{}:{}", predecessor_id, current_account_id, key)
    }
    
    fn kv_prefix(&self, predecessor_id: &str, current_account_id: &str) -> String {
        format!("kv:{}:{}:", predecessor_id, current_account_id)
    }
    
    fn history_key(&self, predecessor_id: &str, current_account_id: &str, key: &str) -> String {
        format!("history:{}:{}:{}", predecessor_id, current_account_id, key)
    }
    
    fn accounts_key(&self, current_account_id: &str) -> String {
        format!("accounts:{}", current_account_id)
    }
    
    fn contracts_key(&self, predecessor_id: &str) -> String {
        format!("contracts:{}", predecessor_id)
    }
    
    fn meta_key(&self, suffix: &str) -> String {
        format!("meta:{}", suffix)
    }
    
    // Core read operations
    
    pub async fn get_kv(
        &self,
        predecessor_id: &str,
        current_account_id: &str,
        key: &str,
    ) -> Result<Option<KvEntry>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key_str = self.kv_key(predecessor_id, current_account_id, key);
        
        let data: Option<String> = conn.get(&key_str).await?;
        
        match data {
            Some(json) => {
                let stored: StoredKvEntry = serde_json::from_str(&json)?;
                Ok(Some(stored.into()))
            }
            None => Ok(None),
        }
    }
    
    pub async fn get_kv_last(
        &self,
        predecessor_id: &str,
        current_account_id: &str,
        key: &str,
    ) -> Result<Option<KvEntry>> {
        self.get_kv(predecessor_id, current_account_id, key).await
    }
    
    pub async fn query_kv_with_pagination(
        &self,
        params: &crate::models::QueryParams,
    ) -> Result<(Vec<KvEntry>, bool, usize)> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let prefix = self.kv_prefix(&params.predecessor_id, &params.current_account_id);
        
        let pattern = if let Some(ref prefix_filter) = params.key_prefix {
            format!("{}{}*", prefix, prefix_filter)
        } else {
            format!("{}*", prefix)
        };
        
        let (_new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(0u64)
            .arg("MATCH")
            .arg(&pattern)
            .arg("COUNT")
            .arg((params.limit + 1) as i64)
            .query_async(&mut conn)
            .await?;
        
        let has_more = keys.len() > params.limit;
        let keys: Vec<String> = keys.into_iter().take(params.limit).collect();
        let dropped = 0usize;
        
        let mut entries = Vec::new();
        for key in keys {
            let data: Option<String> = conn.get(&key).await?;
            if let Some(json) = data {
                if let Ok(stored) = serde_json::from_str::<StoredKvEntry>(&json) {
                    entries.push(stored.into());
                }
            }
        }
        
        Ok((entries, has_more, dropped))
    }
    
    pub async fn query_writers(
        &self,
        params: &WritersParams,
    ) -> Result<(Vec<KvEntry>, bool, bool, usize)> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        
        let pattern = if let Some(ref account_id) = params.predecessor_id {
            format!("kv:{}:{}:{}*", account_id, params.current_account_id, params.key)
        } else {
            format!("kv:*:{}:{}*", params.current_account_id, params.key)
        };
        
        let (_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(0u64)
            .arg("MATCH")
            .arg(&pattern)
            .arg("COUNT")
            .arg((params.limit + 1) as i64)
            .query_async(&mut conn)
            .await?;
        
        let has_more = keys.len() > params.limit;
        let keys: Vec<String> = keys.into_iter().take(params.limit).collect();
        
        let mut entries = Vec::new();
        for key in keys {
            let data: Option<String> = conn.get(&key).await?;
            if let Some(json) = data {
                if let Ok(stored) = serde_json::from_str::<StoredKvEntry>(&json) {
                    entries.push(stored.into());
                }
            }
        }
        
        Ok((entries, has_more, false, 0))
    }
    
    pub async fn query_accounts_by_contract(
        &self,
        contract_id: &str,
        _key: Option<&str>,
        limit: usize,
        _offset: usize,
        _after_account: Option<&str>,
    ) -> Result<(Vec<String>, bool, bool, usize)> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.accounts_key(contract_id);
        
        let members: Vec<String> = conn.smembers(&key).await?;
        
        let has_more = members.len() > limit;
        let accounts: Vec<String> = members.into_iter().take(limit).collect();
        
        Ok((accounts, has_more, false, 0))
    }
    
    pub async fn query_all_accounts(
        &self,
        limit: usize,
        _after_account: Option<&str>,
    ) -> Result<(Vec<String>, bool, usize)> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        
        let (_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(0u64)
            .arg("MATCH")
            .arg("accounts:*")
            .arg("COUNT")
            .arg(limit as i64)
            .query_async(&mut conn)
            .await?;
        
        let accounts: Vec<String> = keys
            .into_iter()
            .filter_map(|k| k.strip_prefix("accounts:").map(|s| s.to_string()))
            .take(limit)
            .collect();
        
        Ok((accounts, false, 0))
    }
    
    pub async fn query_contracts_by_account(
        &self,
        account_id: &str,
        limit: usize,
        _after_contract: Option<&str>,
    ) -> Result<(Vec<String>, bool, usize)> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.contracts_key(account_id);
        
        let members: Vec<String> = conn.smembers(&key).await?;
        let has_more = members.len() > limit;
        
        Ok((members.into_iter().take(limit).collect(), has_more, 0))
    }
    
    pub async fn query_all_contracts(
        &self,
        limit: usize,
        _after_contract: Option<&str>,
    ) -> Result<(Vec<String>, bool, usize)> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        
        let (_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(0u64)
            .arg("MATCH")
            .arg("contracts:*")
            .arg("COUNT")
            .arg(limit as i64)
            .query_async(&mut conn)
            .await?;
        
        let contracts: Vec<String> = keys
            .into_iter()
            .filter_map(|k| k.strip_prefix("contracts:").map(|s| s.to_string()))
            .take(limit)
            .collect();
        
        Ok((contracts, false, 0))
    }
    
    pub async fn get_kv_at_block(
        &self,
        predecessor_id: &str,
        current_account_id: &str,
        key: &str,
        block_height: u64,
    ) -> Result<Option<KvEntry>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let history_key = self.history_key(predecessor_id, current_account_id, key);
        
        let entries: Vec<(i64, String)> = conn
            .zrangebyscore_withscores(&history_key, 0, block_height as i64)
            .await?;
        
        if let Some((_, json)) = entries.last() {
            let stored: StoredKvEntry = serde_json::from_str(json)?;
            return Ok(Some(stored.into()));
        }
        
        self.get_kv(predecessor_id, current_account_id, key).await
    }
    
    pub async fn get_kv_history(
        &self,
        params: &HistoryParams,
    ) -> Result<(Vec<KvEntry>, bool, bool, Option<String>)> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let history_key = self.history_key(&params.predecessor_id, &params.current_account_id, &params.key);
        
        let start = params.from_block.unwrap_or(0);
        let end = params.to_block.unwrap_or(i64::MAX);
        
        let entries: Vec<(i64, String)> = conn
            .zrangebyscore_withscores(&history_key, start, end)
            .await?;
        
        let has_more = entries.len() > params.limit;
        
        let history_entries: Vec<KvEntry> = entries
            .into_iter()
            .take(params.limit)
            .filter_map(|(_, json)| {
                let stored: StoredKvEntry = serde_json::from_str(&json).ok()?;
                Some(stored.into())
            })
            .collect();
        
        let next_cursor = None;
        Ok((history_entries, has_more, false, next_cursor))
    }
    
    pub async fn get_kv_timeline(
        &self,
        params: &TimelineParams,
    ) -> Result<(Vec<KvEntry>, bool, bool, usize, Option<String>)> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let history_key = self.history_key(&params.predecessor_id, &params.current_account_id, "");
        
        let start = params.from_block.unwrap_or(0);
        let end = params.to_block.unwrap_or(i64::MAX);
        
        let entries: Vec<(i64, String)> = conn
            .zrangebyscore_withscores(&history_key, start, end)
            .await?;
        
        let has_more = entries.len() > params.limit;
        
        let timeline_entries: Vec<KvEntry> = entries
            .into_iter()
            .take(params.limit)
            .filter_map(|(_, json)| {
                let stored: StoredKvEntry = serde_json::from_str(&json).ok()?;
                Some(stored.into())
            })
            .collect();
        
        Ok((timeline_entries, has_more, false, 0, None))
    }
    
    pub async fn query_edges(
        &self,
        _edge_type: &str,
        _target: &str,
        limit: usize,
        _offset: usize,
        _after_source: Option<&str>,
    ) -> Result<(Vec<crate::models::EdgeSourceEntry>, bool, usize)> {
        // TODO: Implement edges storage in Redis
        Ok((Vec::new(), false, 0))
    }
    
    pub async fn count_edges(&self, _edge_type: &str, _target: &str) -> Result<usize> {
        // TODO: Implement edges count in Redis
        Ok(0)
    }
    
    pub async fn get_indexer_block_height(&self) -> Result<Option<u64>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.meta_key(&self.chain_id);
        
        let height: Option<String> = conn.get(&key).await?;
        Ok(height.and_then(|h| h.parse().ok()))
    }
    
    // Write operations (for indexer use)
    
    pub async fn set_kv(&self, entry: &KvEntry) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        
        let key = self.kv_key(&entry.predecessor_id, &entry.current_account_id, &entry.key);
        let stored = StoredKvEntry {
            predecessor_id: entry.predecessor_id.clone(),
            current_account_id: entry.current_account_id.clone(),
            key: entry.key.clone(),
            value: entry.value.clone(),
            block_height: entry.block_height,
            block_timestamp: entry.block_timestamp,
            receipt_id: entry.receipt_id.clone(),
            tx_hash: entry.tx_hash.clone(),
        };
        
        let json = serde_json::to_string(&stored)?;
        conn.set(&key, &json).await?;
        
        // Also add to accounts set
        let accounts_key = self.accounts_key(&entry.current_account_id);
        conn.sadd(&accounts_key, &entry.predecessor_id).await?;
        
        // Also add to contracts set
        let contracts_key = self.contracts_key(&entry.predecessor_id);
        conn.sadd(&contracts_key, &entry.current_account_id).await?;
        
        Ok(())
    }
    
    pub async fn set_kv_history(
        &self,
        predecessor_id: &str,
        current_account_id: &str,
        key: &str,
        entry: &KvEntry,
    ) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        
        let history_key = self.history_key(predecessor_id, current_account_id, key);
        let stored = StoredKvEntry {
            predecessor_id: entry.predecessor_id.clone(),
            current_account_id: entry.current_account_id.clone(),
            key: entry.key.clone(),
            value: entry.value.clone(),
            block_height: entry.block_height,
            block_timestamp: entry.block_timestamp,
            receipt_id: entry.receipt_id.clone(),
            tx_hash: entry.tx_hash.clone(),
        };
        
        let json = serde_json::to_string(&stored)?;
        conn.zadd(&history_key, &json, entry.block_height as i64).await?;
        
        Ok(())
    }
    
    pub async fn set_indexer_block_height(&self, height: u64) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.meta_key(&self.chain_id);
        
        conn.set(&key, height.to_string()).await?;
        Ok(())
    }
}
