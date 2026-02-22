use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use dotenvy::dotenv;
use fastnear_primitives::near_indexer_primitives::types::BlockHeight;
use fastnear_primitives::types::ChainId;
use redis_db::{FastData, FastDataKv, RedisDb};
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use suffix_fetcher::{SuffixFetcher, SuffixFetcherConfig, SuffixFetcherUpdate};
use tokio::sync::mpsc;

const PROJECT_ID: &str = "kv-sub-indexer";
const SUFFIX: &str = "kv";
const INDEXER_ID: &str = "kv-sub-indexer";
const MAX_NUM_KEYS: usize = 256;
const MAX_KEY_LENGTH: usize = 1024;

fn parse_kv_entries(fastdata: &FastData) -> Vec<FastDataKv> {
    // Decode base64 data
    let decoded_data = match BASE64.decode(&fastdata.data) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(target: PROJECT_ID, "Failed to decode base64 data: {:?}", e);
            return vec![];
        }
    };
    
    let json_value = match serde_json::from_slice::<serde_json::Value>(&decoded_data) {
        Ok(v) => v,
        Err(_) => {
            tracing::debug!(target: PROJECT_ID, "Received invalid Key-Value Fastdata");
            return vec![];
        }
    };
    let json_object = match json_value.as_object() {
        Some(o) => o,
        None => {
            tracing::debug!(target: PROJECT_ID, "Received invalid Key-Value Fastdata");
            return vec![];
        }
    };
    if json_object.len() > MAX_NUM_KEYS {
        tracing::warn!(
            target: PROJECT_ID,
            "Dropping Key-Value Fastdata with {} keys (max {}) for receipt {} action {}",
            json_object.len(), MAX_NUM_KEYS, fastdata.receipt_id, fastdata.action_index
        );
        return vec![];
    }

    let order_id = compute_order_id(fastdata);
    let mut entries = Vec::new();
    for (key, value) in json_object {
        if key.len() > MAX_KEY_LENGTH {
            tracing::debug!(target: PROJECT_ID, "Received Key-Value Fastdata with invalid key length: {}", key.len());
            continue;
        }
        if key.is_empty() || key.chars().any(|c| c.is_control()) {
            tracing::debug!(target: PROJECT_ID, "Skipping KV key with invalid characters");
            continue;
        }
        let serialized_value = match serde_json::to_string(value) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    target: PROJECT_ID,
                    "Failed to serialize JSON value for key {}: {:?}. Skipping entry.",
                    key, e
                );
                continue;
            }
        };
        
        let encrypted_key_id = detect_encrypted(&serialized_value);
        
        entries.push(FastDataKv {
            receipt_id: fastdata.receipt_id.clone(),
            action_index: fastdata.action_index,
            tx_hash: fastdata.tx_hash.clone(),
            signer_id: fastdata.signer_id.clone(),
            predecessor_id: fastdata.predecessor_id.clone(),
            current_account_id: fastdata.current_account_id.clone(),
            block_height: fastdata.block_height,
            block_timestamp: fastdata.block_timestamp,
            shard_id: fastdata.shard_id,
            receipt_index: fastdata.receipt_index,
            order_id,
            key: key.clone(),
            value: serialized_value,
            encrypted_key_id,
        });
    }
    entries
}

fn compute_order_id(fastdata: &FastData) -> u64 {
    ((fastdata.shard_id as u64) * 100_000 + fastdata.receipt_index as u64) * 1_000 + fastdata.action_index as u64
}

fn detect_encrypted(value: &str) -> Option<String> {
    let value = value.trim_matches('"');
    if let Some(rest) = value.strip_prefix("enc:AES256:") {
        let parts: Vec<&str> = rest.splitn(2, ':').collect();
        if parts.len() == 2 {
            return Some(parts[0].to_string());
        }
    }
    None
}

async fn flush_rows(
    redis_db: &RedisDb,
    rows: &[FastDataKv],
    checkpoint: Option<BlockHeight>,
) -> anyhow::Result<()> {
    redis_db.add_kv_batch(rows).await?;
    if let Some(height) = checkpoint {
        redis_db.set_last_processed_block_height(INDEXER_ID, height).await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("kv-sub-indexer=info,redis_db=info,suffix-fetcher=info")),
        )
        .init();

    let chain_id: ChainId = env::var("CHAIN_ID")
        .expect("CHAIN_ID required")
        .try_into()
        .expect("Invalid chain id");

    let redis_db = Arc::new(RedisDb::new(chain_id.to_string())
        .await
        .expect("Can't connect to Redis"));

    redis_db.test_connection()
        .await
        .expect("Can't connect to Redis");

    tracing::info!(target: PROJECT_ID, "Connected to Redis");

    let fetcher = SuffixFetcher::new(chain_id, Some(redis_db.clone()))
        .await
        .expect("Can't create suffix fetcher");

    let last_processed_block_height = redis_db
        .get_last_processed_block_height(INDEXER_ID)
        .await
        .expect("Error getting last processed block height");

    let start_block_height: BlockHeight = last_processed_block_height
        .map(|h| h + 1)
        .unwrap_or_else(|| {
            env::var("START_BLOCK_HEIGHT")
                .ok()
                .map(|s| s.parse().expect("Invalid block height"))
                .unwrap_or(0)
        });

    let is_running = Arc::new(AtomicBool::new(true));
    let ctrl_c_running = is_running.clone();

    ctrlc::set_handler(move || {
        ctrl_c_running.store(false, Ordering::SeqCst);
        tracing::info!(target: PROJECT_ID, "Received Ctrl+C, starting shutdown...");
    })
    .expect("Error setting Ctrl+C handler");

    tracing::info!(target: PROJECT_ID,
        "Starting {} {} fetcher from height {}",
        SUFFIX,
        chain_id,
        start_block_height,
    );

    let (sender, mut receiver) = mpsc::channel(100);
    tokio::spawn(fetcher.start(
        SuffixFetcherConfig {
            suffix: SUFFIX.to_string(),
            start_block_height: Some(start_block_height),
            sleep_duration: Duration::from_millis(500),
        },
        sender,
        is_running.clone(),
    ));

    let mut rows: Vec<FastDataKv> = vec![];
    while let Some(update) = receiver.recv().await {
        match update {
            SuffixFetcherUpdate::FastData(fastdata) => {
                tracing::info!(target: PROJECT_ID, "Received fastdata: {} {} {}", fastdata.block_height, fastdata.receipt_id, fastdata.action_index);

                let new_entries = parse_kv_entries(&fastdata);
                rows.extend(new_entries);

                if rows.len() >= 10_000 {
                    tracing::info!(target: PROJECT_ID, "Early flush at {} rows", rows.len());
                    let current_rows = std::mem::take(&mut rows);

                    if let Err(e) = flush_rows(&redis_db, &current_rows, None).await {
                        tracing::error!(target: PROJECT_ID,
                            "Failed to write data. Shutting down to prevent data loss: {:?}", e
                        );
                        is_running.store(false, Ordering::SeqCst);
                        break;
                    }
                }
            }
            SuffixFetcherUpdate::EndOfRange(block_height) => {
                tracing::info!(target: PROJECT_ID, "Saving last processed block height {} with {} rows", block_height, rows.len());
                let current_rows = std::mem::take(&mut rows);

                if let Err(e) = flush_rows(&redis_db, &current_rows, Some(block_height)).await {
                    tracing::error!(target: PROJECT_ID,
                        "Failed to write data. Shutting down to prevent data loss: {:?}", e
                    );
                    is_running.store(false, Ordering::SeqCst);
                    break;
                }

                if !is_running.load(Ordering::SeqCst) {
                    tracing::info!(target: PROJECT_ID, "Shutting down...");
                    break;
                }
            }
        };
    }

    tracing::info!(target: PROJECT_ID, "Successfully shut down");
}
