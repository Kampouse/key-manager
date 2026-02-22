use redis_db::{FastData, RedisDb, UNIVERSAL_SUFFIX};

use fastnear_primitives::near_indexer_primitives::types::BlockHeight;
use fastnear_primitives::types::ChainId;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

const FETCHER: &str = "suffix-fetcher";

#[derive(Debug, Clone)]
pub enum SuffixFetcherUpdate {
    FastData(Box<FastData>),
    EndOfRange(BlockHeight),
}

impl From<FastData> for SuffixFetcherUpdate {
    fn from(value: FastData) -> Self {
        Self::FastData(Box::new(value))
    }
}

pub struct SuffixFetcher {
    pub redis_db: Arc<RedisDb>,
    pub chain_id: ChainId,
}

pub struct SuffixFetcherConfig {
    pub suffix: String,
    pub start_block_height: Option<BlockHeight>,
    pub sleep_duration: Duration,
}

impl SuffixFetcher {
    pub async fn new(chain_id: ChainId, redis_db: Option<Arc<RedisDb>>) -> anyhow::Result<Self> {
        let redis_db = match redis_db {
            Some(db) => db,
            None => {
                let db = RedisDb::new(chain_id.to_string())
                    .await
                    .expect("Can't connect to Redis");
                db.test_connection()
                    .await
                    .expect("Can't connect to Redis");
                tracing::info!(target: FETCHER, "Connected to Redis");
                Arc::new(db)
            }
        };
        Ok(Self { redis_db, chain_id })
    }

    pub fn get_redis_db(&self) -> Arc<RedisDb> {
        self.redis_db.clone()
    }

    pub async fn start(
        self,
        config: SuffixFetcherConfig,
        sink: mpsc::Sender<SuffixFetcherUpdate>,
        is_running: Arc<AtomicBool>,
    ) {
        let mut from_block_height = config.start_block_height.unwrap_or(0);
        tracing::info!(target: FETCHER, "Starting suffix fetcher with suffix {:?} from {}", config.suffix, from_block_height);
        
        while is_running.load(Ordering::SeqCst) {
            // Get last processed block height from Redis
            let last_block_height = match self
                .redis_db
                .get_last_processed_block_height(UNIVERSAL_SUFFIX)
                .await
            {
                Ok(height) => height,
                Err(e) => {
                    tracing::error!(
                        target: FETCHER,
                        "Error getting last block height: {:?}. Retrying in 1s...",
                        e
                    );
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };
            
            let Some(last_block_height) = last_block_height else {
                tracing::info!(target: FETCHER, "No last processed block height found");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            };
            
            if from_block_height > last_block_height {
                tracing::debug!(target: FETCHER, "Waiting for new blocks");
                tokio::time::sleep(config.sleep_duration).await;
                continue;
            }
            
            tracing::info!(target: FETCHER, "Fetching blocks from {} to {}", from_block_height, last_block_height);

            // Fetch data from Redis for the range
            let mut range_success = false;
            let mut range_last_block: Option<BlockHeight> = None;
            let delays = [0, 1, 2, 4];

            for (attempt, &delay_secs) in delays.iter().enumerate() {
                let mut last_fastdata_block_height: Option<BlockHeight> = None;

                if delay_secs > 0 {
                    tracing::info!(target: FETCHER, "Retrying range fetch (attempt {}/{}) after {}s delay", attempt, delays.len() - 1, delay_secs);
                    tokio::time::sleep(Duration::from_secs(delay_secs)).await;
                }

                // Stream suffix data from Redis
                let result = self.stream_suffix_data(
                    &config.suffix,
                    from_block_height,
                    last_block_height,
                    &sink,
                    is_running.clone(),
                    &mut last_fastdata_block_height,
                ).await;

                match result {
                    Ok(had_data) => {
                        range_success = true;
                        if let Some(h) = last_fastdata_block_height {
                            range_last_block = Some(h);
                        }
                        tracing::debug!(target: FETCHER, "Range scan complete, had_data={}", had_data);
                        break;
                    }
                    Err(e) => {
                        tracing::error!(target: FETCHER, "Error fetching suffix data (attempt {}): {:?}", attempt + 1, e);
                    }
                }
            }

            if !range_success {
                tracing::error!(
                    target: FETCHER,
                    "Failed to fetch range [{}, {}] after {} retries.",
                    from_block_height, last_block_height, delays.len() - 1
                );
                is_running.store(false, Ordering::SeqCst);
                break;
            }

            // Checkpoint based on actual progress
            if let Some(checkpoint_height) = range_last_block {
                if sink.send(SuffixFetcherUpdate::EndOfRange(checkpoint_height))
                    .await
                    .is_err() {
                    tracing::warn!(target: FETCHER, "Channel closed, stopping");
                    break;
                }
                from_block_height = checkpoint_height + 1;
            } else if is_running.load(Ordering::SeqCst) {
                if sink.send(SuffixFetcherUpdate::EndOfRange(last_block_height))
                    .await
                    .is_err() {
                    tracing::warn!(target: FETCHER, "Channel closed, stopping");
                    break;
                }
                from_block_height = last_block_height + 1;
            }
        }
        tracing::info!(target: FETCHER, "Stopped suffix fetcher");
    }

    async fn stream_suffix_data(
        &self,
        suffix: &str,
        from_block: BlockHeight,
        to_block: BlockHeight,
        sink: &mpsc::Sender<SuffixFetcherUpdate>,
        is_running: Arc<AtomicBool>,
        last_block: &mut Option<BlockHeight>,
    ) -> anyhow::Result<bool> {
        use redis::{AsyncCommands, Client};
        
        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let client = Client::open(redis_url.as_str())?;
        let mut conn = client.get_multiplexed_async_connection().await?;
        
        // Pattern for fastdata keys: fastdata:{chain_id}:{suffix}:{block_height}:{receipt_id}
        let pattern = format!("fastdata:{}:{}:*", self.chain_id, suffix);
        
        let mut cursor: u64 = 0;
        let mut had_data = false;
        
        loop {
            if !is_running.load(Ordering::SeqCst) {
                return Ok(true);
            }
            
            let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await?;
            
            for key in keys {
                tracing::info!(target: FETCHER, "Processing key: {}", key);
                // Parse block height from key
                let parts: Vec<&str> = key.split(':').collect();
                if parts.len() < 5 {
                    tracing::warn!(target: FETCHER, "Key has wrong format: {}", key);
                    continue;
                }
                if let Ok(block_height) = parts[3].parse::<u64>() {
                    tracing::info!(target: FETCHER, "Parsed block_height {} from key", block_height);
                    if block_height < from_block || block_height > to_block {
                        tracing::debug!(target: FETCHER, "Skipping block {} (out of range {}-{})", block_height, from_block, to_block);
                        continue;
                    }
                    
                    // Get the data
                    let data: Option<String> = conn.get(&key).await?;
                    if let Some(json) = data {
                        tracing::info!(target: FETCHER, "Got fastdata JSON for key {}: {} bytes", key, json.len());
                        match serde_json::from_str::<FastData>(&json) {
                            Ok(fastdata) => {
                                tracing::info!(target: FETCHER, "Successfully parsed FastData: block={} receipt={}", fastdata.block_height, fastdata.receipt_id);
                                had_data = true;
                                *last_block = Some(fastdata.block_height);
                                
                                if sink.send(fastdata.into()).await.is_err() {
                                    tracing::warn!(target: FETCHER, "Channel closed, stopping");
                                    return Ok(true);
                                }
                            }
                            Err(e) => {
                                tracing::error!(target: FETCHER, "Failed to parse FastData from {}: {:?}", key, e);
                            }
                        }
                    } else {
                        tracing::warn!(target: FETCHER, "No data found for key {}", key);
                    }
                } else {
                    tracing::warn!(target: FETCHER, "Failed to parse block_height from key {}", key);
                }
            }
            
            cursor = new_cursor;
            if cursor == 0 {
                break;
            }
        }
        
        Ok(had_data)
    }
}
