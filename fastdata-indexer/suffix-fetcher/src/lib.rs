use scylladb::{FastData, ScyllaDb, UNIVERSAL_SUFFIX};

use fastnear_primitives::near_indexer_primitives::types::BlockHeight;
use fastnear_primitives::types::ChainId;
use futures::StreamExt;
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
    pub scylladb: Arc<ScyllaDb>,
}

/// Configuration for the `SuffixFetcher`.
///
/// This struct defines the parameters used to configure the behavior of the fetcher.
pub struct SuffixFetcherConfig {
    /// The suffix to be fetched. This is a unique identifier used to determine
    /// the specific data.
    pub suffix: String,

    /// The optional starting block height for the fetcher. If provided, the fetcher
    /// will begin retrieving data from this block height. If `None`, defaults to 0.
    pub start_block_height: Option<BlockHeight>,

    /// The duration for which the fetcher will sleep while waiting for the next universal last
    /// processed block height. Consider using around 500ms.
    pub sleep_duration: Duration,
}

impl SuffixFetcher {
    pub async fn new(chain_id: ChainId, scylladb: Option<Arc<ScyllaDb>>) -> anyhow::Result<Self> {
        let scylladb = match scylladb {
            Some(scylladb) => scylladb,
            None => {
                let scylla_session = ScyllaDb::new_scylla_session()
                    .await
                    .expect("Can't create scylla session");
                ScyllaDb::test_connection(&scylla_session)
                    .await
                    .expect("Can't connect to scylla");
                tracing::info!(target: FETCHER, "Connected to Scylla");

                Arc::new(ScyllaDb::new(chain_id, scylla_session, false).await?)
            }
        };
        Ok(Self { scylladb })
    }

    pub fn get_scylladb(&self) -> Arc<ScyllaDb> {
        self.scylladb.clone()
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
            let last_block_height = match self
                .scylladb
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

            let mut range_success = false;
            let mut range_last_block: Option<BlockHeight> = None; // Tracks actual progress across retries
            let delays = [0, 1, 2, 4]; // 0 for first attempt, then 1s, 2s, 4s for retries

            for (attempt, &delay_secs) in delays.iter().enumerate() {
                // Reset per-attempt state to avoid stale values corrupting retry logic (CRIT-6)
                let mut last_fastdata_block_height: Option<BlockHeight> = None;

                if delay_secs > 0 {
                    tracing::info!(target: FETCHER, "Retrying range fetch (attempt {}/{}) after {}s delay", attempt, delays.len() - 1, delay_secs);
                    tokio::time::sleep(Duration::from_secs(delay_secs)).await;
                }

                let mut stream = match self
                    .scylladb
                    .get_suffix_data(&config.suffix, from_block_height, last_block_height)
                    .await
                {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!(
                            target: FETCHER,
                            "Error getting suffix data (attempt {}): {:?}",
                            attempt + 1, e
                        );
                        // Continue to next retry attempt
                        continue;
                    }
                };

                let mut had_error = false;
                loop {
                    let fastdata = match stream.next().await {
                        Some(item) => item,
                        None => break, // Stream exhausted normally
                    };
                    if !is_running.load(Ordering::SeqCst) {
                        range_success = true;
                        break;
                    }
                    match fastdata {
                        Ok(fastdata) => {
                            if let Some(last_fastdata_block_height) = last_fastdata_block_height {
                                if fastdata.block_height > last_fastdata_block_height
                                    && sink.send(SuffixFetcherUpdate::EndOfRange(
                                        last_fastdata_block_height,
                                    ))
                                    .await
                                    .is_err()
                                {
                                    tracing::warn!(target: FETCHER, "Channel closed, stopping");
                                    range_success = true;
                                    break;
                                }
                            }
                            last_fastdata_block_height = Some(fastdata.block_height);
                            if sink.send(fastdata.into())
                                .await
                                .is_err() {
                                tracing::warn!(target: FETCHER, "Channel closed, stopping");
                                range_success = true;
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::error!(target: FETCHER, "Error fetching fastdata (attempt {}): {:?}", attempt + 1, e);
                            had_error = true;
                            break;
                        }
                    }
                }

                // Propagate actual progress to outer scope for accurate checkpointing
                if let Some(h) = last_fastdata_block_height {
                    range_last_block = Some(h);
                }

                if !had_error {
                    range_success = true;
                    break;
                }
            }

            if !range_success {
                tracing::error!(
                    target: FETCHER,
                    "Failed to fetch range [{}, {}] after {} retries. Halting to prevent data loss.",
                    from_block_height, last_block_height, delays.len() - 1
                );
                is_running.store(false, Ordering::SeqCst);
                break;
            }

            // Checkpoint based on actual progress
            if let Some(checkpoint_height) = range_last_block {
                // Rows were processed — checkpoint at last processed block
                if sink.send(SuffixFetcherUpdate::EndOfRange(checkpoint_height))
                    .await
                    .is_err() {
                    tracing::warn!(target: FETCHER, "Channel closed, stopping");
                    break;
                }
                from_block_height = checkpoint_height + 1;
            } else if is_running.load(Ordering::SeqCst) {
                // Empty range (no rows existed) and not interrupted — safe to advance
                if sink.send(SuffixFetcherUpdate::EndOfRange(last_block_height))
                    .await
                    .is_err() {
                    tracing::warn!(target: FETCHER, "Channel closed, stopping");
                    break;
                }
                from_block_height = last_block_height + 1;
            }
            // If !is_running && no rows processed: Ctrl-C before progress — don't advance checkpoint
        }
        tracing::info!(target: FETCHER, "Stopped suffix fetcher");
    }
}
