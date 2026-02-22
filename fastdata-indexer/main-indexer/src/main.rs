use dotenvy::dotenv;
use fastnear_neardata_fetcher::fetcher;
use fastnear_primitives::near_indexer_primitives::types::BlockHeight;
use fastnear_primitives::near_primitives::views::{ActionView, ReceiptEnumView};
use fastnear_primitives::types::ChainId;
use futures::stream::{self, StreamExt};
use scylladb::{FastData, ScyllaDb, UNIVERSAL_SUFFIX};
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

const FASTDATA_PREFIX: &str = "__fastdata_";
const PROJECT_ID: &str = "fastdata-indexer";

#[tokio::main]
async fn main() {
    dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("neardata-fetcher=info,fastdata-indexer=info,scylladb=info")),
        )
        .init();

    let chain_id: ChainId = env::var("CHAIN_ID")
        .expect("CHAIN_ID required")
        .try_into()
        .expect("Invalid chain id");

    let scylla_session = ScyllaDb::new_scylla_session()
        .await
        .expect("Can't create scylla session");

    ScyllaDb::test_connection(&scylla_session)
        .await
        .expect("Can't connect to scylla");

    tracing::info!(target: PROJECT_ID, "Connected to Scylla");

    let scylladb = ScyllaDb::new(chain_id, scylla_session, true)
        .await
        .expect("Can't create scylla db");

    let last_processed_block_height = scylladb
        .get_last_processed_block_height(UNIVERSAL_SUFFIX)
        .await
        .expect("Error getting last processed block height");

    tracing::info!(target: PROJECT_ID, "Latest processed block height in DB: {:?}", last_processed_block_height);

    let num_threads = env::var("NUM_THREADS")
        .ok()
        .map(|num_threads| num_threads.parse().expect("Invalid number of threads"))
        .unwrap_or(8u64)
        .max(1);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client");
    let last_block_height = match fetcher::fetch_last_block(&client, chain_id).await {
        Some(block) => block.block.header.height,
        None => {
            tracing::warn!(
                target: PROJECT_ID,
                "Failed to fetch last block from neardata. Using START_BLOCK_HEIGHT env var."
            );
            env::var("START_BLOCK_HEIGHT")
                .expect("Cannot fetch last block and START_BLOCK_HEIGHT not set")
                .parse()
                .expect("Invalid START_BLOCK_HEIGHT")
        }
    };

    tracing::info!(target: PROJECT_ID, "Last neardata block height: {}", last_block_height);

    let start_block_height: BlockHeight = last_processed_block_height
        .map(|h| h + 1)
        .unwrap_or_else(|| {
            env::var("START_BLOCK_HEIGHT")
                .ok()
                .map(|start_block_height| start_block_height.parse().expect("Invalid block height"))
                .unwrap_or(last_block_height)
        });

    let auth_bearer_token = env::var("FASTNEAR_AUTH_BEARER_TOKEN").ok();
    let mut config = fetcher::FetcherConfigBuilder::new()
        .start_block_height(start_block_height)
        .num_threads(num_threads)
        .chain_id(chain_id);
    if let Some(token) = auth_bearer_token.clone() {
        config = config.auth_bearer_token(token);
    }

    let is_running = Arc::new(AtomicBool::new(true));
    let ctrl_c_running = is_running.clone();

    ctrlc::set_handler(move || {
        ctrl_c_running.store(false, Ordering::SeqCst);
        tracing::info!(target: PROJECT_ID, "Received Ctrl+C, starting shutdown...");
    })
    .expect("Error setting Ctrl+C handler");

    let block_update_interval = std::time::Duration::from_millis(
        env::var("BLOCK_UPDATE_INTERVAL_MS")
            .ok()
            .map(|ms| ms.parse().expect("Invalid BLOCK_UPDATE_INTERVAL_MS"))
            .unwrap_or(5000u64),
    );

    tracing::info!(target: PROJECT_ID,
        "Starting {} fetcher with {} threads from height {}. Using auth token: {}",
        chain_id,
        num_threads,
        start_block_height,
        auth_bearer_token.is_some()
    );

    let (sender, mut receiver) = mpsc::channel((num_threads * 10) as _);
    tokio::spawn(fetcher::start_fetcher(
        config.build(),
        sender,
        is_running.clone(),
    ));

    let mut last_block_update = std::time::SystemTime::now();
    let mut consecutive_checkpoint_failures: u32 = 0;
    const MAX_CONSECUTIVE_CHECKPOINT_FAILURES: u32 = 5;
    while let Some(block) = receiver.recv().await {
        let block_height = block.block.header.height;
        let block_timestamp = block.block.header.timestamp;

        tracing::info!(target: PROJECT_ID, "Received block: {}", block_height);

        let mut data = vec![];

        for shard in block.shards {
            for (receipt_index, reo) in shard.receipt_execution_outcomes.into_iter().enumerate() {
                let receipt = reo.receipt;
                let receipt_id = receipt.receipt_id;
                let predecessor_id = receipt.predecessor_id;
                let current_account_id = receipt.receiver_id;
                let tx_hash = reo.tx_hash;
                if let ReceiptEnumView::Action {
                    signer_id, actions, ..
                } = receipt.receipt
                {
                    for (action_index, action) in actions.into_iter().enumerate() {
                        if let ActionView::FunctionCall {
                            method_name, args, ..
                        } = action
                        {
                            if let Some(suffix) = method_name.strip_prefix(FASTDATA_PREFIX) {
                                if suffix.is_empty()
                                    || suffix == UNIVERSAL_SUFFIX
                                    || suffix.len() > 64
                                    || !suffix.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                                {
                                    tracing::warn!(
                                        target: PROJECT_ID,
                                        "Skipping invalid suffix {:?} for receipt {} action {}",
                                        suffix, receipt_id, action_index
                                    );
                                    continue;
                                }
                                data.push(FastData {
                                    receipt_id,
                                    action_index: u32::try_from(action_index).expect("action_index exceeds u32"),
                                    suffix: suffix.to_string(),
                                    data: args.to_vec(),
                                    tx_hash,
                                    signer_id: signer_id.clone(),
                                    predecessor_id: predecessor_id.clone(),
                                    current_account_id: current_account_id.clone(),
                                    block_height,
                                    block_timestamp,
                                    shard_id: shard.shard_id.into(),
                                    receipt_index: u32::try_from(receipt_index).expect("receipt_index exceeds u32"),
                                });
                            }
                        }
                    }
                }
            }
        }

        let current_time = std::time::SystemTime::now();
        let duration = current_time
            .duration_since(last_block_update)
            .unwrap_or(block_update_interval); // Treat as elapsed if clock adjusted
        let mut need_to_save_last_processed_block_height = duration >= block_update_interval;

        if !data.is_empty() {
            tracing::info!(target: PROJECT_ID, "Inserting {} fastdata rows into Scylla", data.len());

            let mut success = false;
            let delays = [0, 1, 2, 4]; // 0 for first attempt, then 1s, 2s, 4s for retries

            for (attempt, &delay_secs) in delays.iter().enumerate() {
                if delay_secs > 0 {
                    tracing::info!(target: PROJECT_ID, "Retrying insertion (attempt {}/{}) after {}s delay", attempt, delays.len() - 1, delay_secs);
                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                }

                let result = stream::iter(data.iter().map(|fastdata| scylladb.add_data(fastdata.clone())))
                    .buffer_unordered(100)
                    .collect::<Vec<_>>()
                    .await
                    .into_iter()
                    .collect::<anyhow::Result<()>>();

                if result.is_ok() {
                    success = true;
                    break;
                } else if let Err(e) = result {
                    tracing::error!(target: PROJECT_ID, "Error inserting data into Scylla (attempt {}): {:?}", attempt + 1, e);
                }
            }

            if !success {
                tracing::error!(
                    target: PROJECT_ID,
                    "Failed to insert data after {} retries. Shutting down to prevent data loss.",
                    delays.len() - 1
                );
                is_running.store(false, Ordering::SeqCst);
                break;
            }

            need_to_save_last_processed_block_height = true;
        }

        if !is_running.load(Ordering::SeqCst) {
            tracing::info!(target: PROJECT_ID, "Shutting down fetcher");
            need_to_save_last_processed_block_height = true;
        }

        if need_to_save_last_processed_block_height {
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
                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                }

                match scylladb.set_last_processed_block_height(UNIVERSAL_SUFFIX, block_height).await {
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
                last_block_update = current_time;
                consecutive_checkpoint_failures = 0;
            } else {
                consecutive_checkpoint_failures += 1;
                last_block_update = current_time; // Prevent retry storm on every subsequent block
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
        }

        if !is_running.load(Ordering::SeqCst) {
            break;
        }
    }

    tracing::info!(target: PROJECT_ID, "Successfully shut down");
}
