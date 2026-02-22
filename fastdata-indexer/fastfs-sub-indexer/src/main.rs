mod fastfs;
mod scylla_types;

use crate::fastfs::FastfsData;
use crate::scylla_types::{
    add_fastfs_fastdata, create_tables, prepare_insert_query, FastfsFastData,
};
use dotenvy::dotenv;
use fastnear_primitives::near_indexer_primitives::types::BlockHeight;
use fastnear_primitives::types::ChainId;
use scylla::statement::prepared::PreparedStatement;
use scylladb::{retry_with_delays, ScyllaDb};
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use suffix_fetcher::{SuffixFetcher, SuffixFetcherConfig, SuffixFetcherUpdate};
use tokio::sync::mpsc;

const PROJECT_ID: &str = "fastfs-sub-indexer";
const SUFFIX: &str = "fastfs";
const INDEXER_ID: &str = "fastfs_v2";

async fn write_with_retries(
    scylladb: &ScyllaDb,
    insert_query: &PreparedStatement,
    fastdata: &FastfsFastData,
) -> anyhow::Result<()> {
    retry_with_delays(&[1, 2, 4], || {
        add_fastfs_fastdata(scylladb, insert_query, fastdata)
    }).await
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("fastfs-sub-indexer=info,scylladb=info,suffix-fetcher=info")),
        )
        .init();

    let chain_id: ChainId = env::var("CHAIN_ID")
        .expect("CHAIN_ID required")
        .try_into()
        .expect("Invalid chain id");

    let fetcher = SuffixFetcher::new(chain_id, None)
        .await
        .expect("Can't create suffix fetcher");

    let scylladb = fetcher.get_scylladb();

    create_tables(&scylladb)
        .await
        .expect("Error creating tables");

    let insert_query = prepare_insert_query(&scylladb)
        .await
        .expect("Error preparing insert query");

    let last_processed_block_height = scylladb
        .get_last_processed_block_height(INDEXER_ID)
        .await
        .expect("Error getting last processed block height");

    let start_block_height: BlockHeight = last_processed_block_height
        .map(|h| h + 1)
        .unwrap_or_else(|| {
            env::var("START_BLOCK_HEIGHT")
                .ok()
                .map(|start_block_height| start_block_height.parse().expect("Invalid block height"))
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
        "Starting {:?} {} fetcher from height {} with indexer ID {}",
        SUFFIX,
        chain_id,
        start_block_height,
        INDEXER_ID
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

    let mut consecutive_checkpoint_failures: u32 = 0;
    const MAX_CONSECUTIVE_CHECKPOINT_FAILURES: u32 = 5;
    while let Some(update) = receiver.recv().await {
        match update {
            SuffixFetcherUpdate::FastData(fastdata) => {
                tracing::info!(target: PROJECT_ID, "Received fastdata: {} {} {}", fastdata.block_height, fastdata.receipt_id, fastdata.action_index);
                let value: FastfsData = match borsh::from_slice(&fastdata.data) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!(target: PROJECT_ID, "Failed to deserialize borsh data for receipt {} action {}: {:?}, skipping", fastdata.receipt_id, fastdata.action_index, e);
                        continue;
                    }
                };
                {
                    match value {
                        FastfsData::Simple(simple_fastfs) => {
                            if !simple_fastfs.is_valid() {
                                tracing::warn!(
                                    target: PROJECT_ID,
                                    "Invalid SimpleFastfs data for receipt {} action {}, skipping",
                                    fastdata.receipt_id, fastdata.action_index
                                );
                            } else {
                                let (mime_type, content) = simple_fastfs
                                    .content
                                    .map(|c| (Some(c.mime_type), Some(c.content)))
                                    .unwrap_or((None, None));
                                let full_size = content
                                    .as_ref()
                                    .map(|c| u32::try_from(c.len()).expect("content length exceeds u32"))
                                    .unwrap_or(0);
                                let fastfs_fastdata = FastfsFastData {
                                    receipt_id: fastdata.receipt_id,
                                    action_index: fastdata.action_index,
                                    tx_hash: fastdata.tx_hash,
                                    signer_id: fastdata.signer_id,
                                    predecessor_id: fastdata.predecessor_id,
                                    current_account_id: fastdata.current_account_id,
                                    block_height: fastdata.block_height,
                                    block_timestamp: fastdata.block_timestamp,
                                    shard_id: fastdata.shard_id,
                                    receipt_index: fastdata.receipt_index,
                                    mime_type,
                                    relative_path: simple_fastfs.relative_path,
                                    content,
                                    offset: 0,
                                    full_size,
                                    nonce: 0,
                                };
                                tracing::info!(target: PROJECT_ID, "FastFS data {} bytes: {}/{}/{}", fastfs_fastdata.content.as_ref().map(|v| v.len()).unwrap_or(0), fastfs_fastdata.predecessor_id, fastfs_fastdata.current_account_id, fastfs_fastdata.relative_path);

                                if let Err(e) = write_with_retries(&scylladb, &insert_query, &fastfs_fastdata).await {
                                    tracing::error!(
                                        target: PROJECT_ID,
                                        "Failed to write FastFS data after retries. Shutting down to prevent data loss: {:?}", e
                                    );
                                    is_running.store(false, Ordering::SeqCst);
                                    break;
                                }
                            }
                        }
                        FastfsData::Partial(partial_fs) => {
                            if !partial_fs.is_valid() {
                                tracing::warn!(
                                    target: PROJECT_ID,
                                    "Invalid PartialFastfs data for receipt {} action {}, skipping",
                                    fastdata.receipt_id, fastdata.action_index
                                );
                            } else {
                                let fastfs_fastdata = FastfsFastData {
                                    receipt_id: fastdata.receipt_id,
                                    action_index: fastdata.action_index,
                                    tx_hash: fastdata.tx_hash,
                                    signer_id: fastdata.signer_id,
                                    predecessor_id: fastdata.predecessor_id,
                                    current_account_id: fastdata.current_account_id,
                                    block_height: fastdata.block_height,
                                    block_timestamp: fastdata.block_timestamp,
                                    shard_id: fastdata.shard_id,
                                    receipt_index: fastdata.receipt_index,
                                    mime_type: Some(partial_fs.mime_type),
                                    relative_path: partial_fs.relative_path,
                                    content: Some(partial_fs.content_chunk),
                                    offset: partial_fs.offset,
                                    full_size: partial_fs.full_size,
                                    nonce: partial_fs.nonce,
                                };
                                tracing::info!(target: PROJECT_ID, "FastFS partial data {} bytes: {}/{}/{} offset {}", fastfs_fastdata.content.as_ref().map(|v| v.len()).unwrap_or(0), fastfs_fastdata.predecessor_id, fastfs_fastdata.current_account_id, fastfs_fastdata.relative_path, fastfs_fastdata.offset);

                                if let Err(e) = write_with_retries(&scylladb, &insert_query, &fastfs_fastdata).await {
                                    tracing::error!(
                                        target: PROJECT_ID,
                                        "Failed to write FastFS partial data after retries. Shutting down to prevent data loss: {:?}", e
                                    );
                                    is_running.store(false, Ordering::SeqCst);
                                    break;
                                }
                            }
                        }
                    };
                }
            }
            SuffixFetcherUpdate::EndOfRange(block_height) => {
                tracing::info!(target: PROJECT_ID, "Saving last processed block height: {}", block_height);

                // Retry checkpoint write with delays
                let checkpoint_delays = [1, 2, 4];
                let mut checkpoint_success = false;

                for (attempt, &delay_secs) in checkpoint_delays.iter().enumerate() {
                    if attempt > 0 {
                        tracing::warn!(
                            target: PROJECT_ID,
                            "Retrying checkpoint write (attempt {}/{}) after {}s delay",
                            attempt + 1, checkpoint_delays.len(), delay_secs
                        );
                        tokio::time::sleep(Duration::from_secs(delay_secs)).await;
                    }

                    match scylladb.set_last_processed_block_height(INDEXER_ID, block_height).await {
                        Ok(_) => {
                            checkpoint_success = true;
                            break;
                        }
                        Err(e) => {
                            tracing::error!(
                                target: PROJECT_ID,
                                "Checkpoint write failed (attempt {}): {:?}",
                                attempt + 1, e
                            );
                        }
                    }
                }

                if checkpoint_success {
                    consecutive_checkpoint_failures = 0;
                } else {
                    consecutive_checkpoint_failures += 1;
                    tracing::error!(
                        target: PROJECT_ID,
                        "Checkpoint failed ({}/{} consecutive). Will reprocess from last checkpoint on restart.",
                        consecutive_checkpoint_failures, MAX_CONSECUTIVE_CHECKPOINT_FAILURES
                    );
                    if consecutive_checkpoint_failures >= MAX_CONSECUTIVE_CHECKPOINT_FAILURES {
                        tracing::error!(
                            target: PROJECT_ID,
                            "Too many consecutive checkpoint failures. Shutting down."
                        );
                        is_running.store(false, Ordering::SeqCst);
                    }
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
