use fastnear_primitives::near_indexer_primitives::types::{AccountId, BlockHeight};
use fastnear_primitives::near_indexer_primitives::CryptoHash;

use scylla::{DeserializeRow, SerializeRow};

pub const UNIVERSAL_SUFFIX: &str = "*";
pub const BLOCK_HEIGHT_BUCKET_SIZE: u64 = 10_000;

#[derive(Debug, Clone)]
pub struct FastData {
    pub receipt_id: CryptoHash,
    pub action_index: u32,
    pub suffix: String,
    pub data: Vec<u8>,
    pub tx_hash: Option<CryptoHash>,
    pub signer_id: AccountId,
    pub predecessor_id: AccountId,
    pub current_account_id: AccountId,
    pub block_height: BlockHeight,
    pub block_timestamp: u64,
    pub shard_id: u32,
    pub receipt_index: u32,
}

#[derive(Debug, Clone, DeserializeRow, SerializeRow)]
pub(crate) struct FastDataRow {
    pub receipt_id: String,
    pub action_index: i32,
    pub suffix: String,
    pub block_height_bucket: i64,
    pub data: Vec<u8>,
    pub tx_hash: Option<String>,
    pub signer_id: String,
    pub predecessor_id: String,
    pub current_account_id: String,
    pub block_height: i64,
    pub block_timestamp: i64,
    pub shard_id: i32,
    pub receipt_index: i32,
}

impl TryFrom<FastDataRow> for FastData {
    type Error = anyhow::Error;

    fn try_from(row: FastDataRow) -> anyhow::Result<Self> {
        Ok(Self {
            receipt_id: row.receipt_id.parse()
                .map_err(|e| anyhow::anyhow!("Failed to parse receipt_id '{}': {:?}", row.receipt_id, e))?,
            action_index: u32::try_from(row.action_index)
                .map_err(|_| anyhow::anyhow!("Negative action_index in DB: {}", row.action_index))?,
            suffix: row.suffix,
            data: row.data,
            tx_hash: row.tx_hash
                .map(|h| h.parse()
                    .map_err(|e| anyhow::anyhow!("Failed to parse tx_hash '{}': {:?}", h, e)))
                .transpose()?,
            signer_id: row.signer_id.parse()
                .map_err(|e| anyhow::anyhow!("Failed to parse signer_id '{}': {:?}", row.signer_id, e))?,
            predecessor_id: row.predecessor_id.parse()
                .map_err(|e| anyhow::anyhow!("Failed to parse predecessor_id '{}': {:?}", row.predecessor_id, e))?,
            current_account_id: row.current_account_id.parse()
                .map_err(|e| anyhow::anyhow!("Failed to parse current_account_id '{}': {:?}", row.current_account_id, e))?,
            block_height: u64::try_from(row.block_height)
                .map_err(|_| anyhow::anyhow!("Negative block_height in DB: {}", row.block_height))?,
            block_timestamp: u64::try_from(row.block_timestamp)
                .map_err(|_| anyhow::anyhow!("Negative block_timestamp in DB: {}", row.block_timestamp))?,
            shard_id: u32::try_from(row.shard_id)
                .map_err(|_| anyhow::anyhow!("Negative shard_id in DB: {}", row.shard_id))?,
            receipt_index: u32::try_from(row.receipt_index)
                .map_err(|_| anyhow::anyhow!("Negative receipt_index in DB: {}", row.receipt_index))?,
        })
    }
}

impl From<FastData> for FastDataRow {
    fn from(fastdata: FastData) -> Self {
        Self {
            receipt_id: fastdata.receipt_id.to_string(),
            action_index: i32::try_from(fastdata.action_index).expect("action_index exceeds i32"),
            suffix: fastdata.suffix,
            block_height_bucket: i64::try_from(fastdata.block_height / BLOCK_HEIGHT_BUCKET_SIZE).expect("block_height_bucket exceeds i64"),
            data: fastdata.data,
            tx_hash: fastdata.tx_hash.map(|h| h.to_string()),
            signer_id: fastdata.signer_id.to_string(),
            predecessor_id: fastdata.predecessor_id.to_string(),
            current_account_id: fastdata.current_account_id.to_string(),
            block_height: i64::try_from(fastdata.block_height).expect("block_height exceeds i64"),
            block_timestamp: i64::try_from(fastdata.block_timestamp).expect("block_timestamp exceeds i64"),
            shard_id: i32::try_from(fastdata.shard_id).expect("shard_id exceeds i32"),
            receipt_index: i32::try_from(fastdata.receipt_index).expect("receipt_index exceeds i32"),
        }
    }
}
