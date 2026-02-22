use crate::*;
use fastnear_primitives::near_indexer_primitives::types::AccountId;
use fastnear_primitives::near_indexer_primitives::CryptoHash;
use scylla::statement::prepared::PreparedStatement;
use scylla::{DeserializeRow, SerializeRow};
use scylladb::{ScyllaDb, SCYLLADB};

#[derive(Debug, Clone)]
pub struct FastfsFastData {
    pub receipt_id: CryptoHash,
    pub action_index: u32,
    pub tx_hash: Option<CryptoHash>,
    pub signer_id: AccountId,
    pub predecessor_id: AccountId,
    pub current_account_id: AccountId,
    pub block_height: BlockHeight,
    pub block_timestamp: u64,
    pub shard_id: u32,
    pub receipt_index: u32,
    pub mime_type: Option<String>,
    pub relative_path: String,
    pub content: Option<Vec<u8>>,
    pub offset: u32,
    pub full_size: u32,
    pub nonce: u32,
}

#[derive(Debug, Clone, DeserializeRow, SerializeRow)]
pub(crate) struct FastfsFastDataRow {
    pub receipt_id: String,
    pub action_index: i32,
    pub tx_hash: Option<String>,
    pub signer_id: String,
    pub predecessor_id: String,
    pub current_account_id: String,
    pub block_height: i64,
    pub block_timestamp: i64,
    pub shard_id: i32,
    pub receipt_index: i32,
    pub mime_type: Option<String>,
    pub relative_path: String,
    pub content: Option<Vec<u8>>,
    pub offset: i32,
    pub full_size: i32,
    pub nonce: i32,
}

impl TryFrom<FastfsFastDataRow> for FastfsFastData {
    type Error = anyhow::Error;

    fn try_from(row: FastfsFastDataRow) -> anyhow::Result<Self> {
        Ok(Self {
            receipt_id: row.receipt_id.parse()
                .map_err(|e| anyhow::anyhow!("Failed to parse receipt_id '{}': {:?}", row.receipt_id, e))?,
            action_index: u32::try_from(row.action_index)
                .map_err(|_| anyhow::anyhow!("Negative action_index in DB: {}", row.action_index))?,
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
            mime_type: row.mime_type,
            relative_path: row.relative_path,
            content: row.content,
            offset: u32::try_from(row.offset)
                .map_err(|_| anyhow::anyhow!("Negative offset in DB: {}", row.offset))?,
            full_size: u32::try_from(row.full_size)
                .map_err(|_| anyhow::anyhow!("Negative full_size in DB: {}", row.full_size))?,
            nonce: u32::try_from(row.nonce)
                .map_err(|_| anyhow::anyhow!("Negative nonce in DB: {}", row.nonce))?,
        })
    }
}

impl From<FastfsFastData> for FastfsFastDataRow {
    fn from(data: FastfsFastData) -> Self {
        Self {
            receipt_id: data.receipt_id.to_string(),
            action_index: i32::try_from(data.action_index).expect("action_index exceeds i32"),
            tx_hash: data.tx_hash.map(|h| h.to_string()),
            signer_id: data.signer_id.to_string(),
            predecessor_id: data.predecessor_id.to_string(),
            current_account_id: data.current_account_id.to_string(),
            block_height: i64::try_from(data.block_height).expect("block_height exceeds i64"),
            block_timestamp: i64::try_from(data.block_timestamp).expect("block_timestamp exceeds i64"),
            shard_id: i32::try_from(data.shard_id).expect("shard_id exceeds i32"),
            receipt_index: i32::try_from(data.receipt_index).expect("receipt_index exceeds i32"),
            mime_type: data.mime_type,
            relative_path: data.relative_path,
            content: data.content,
            offset: i32::try_from(data.offset).expect("offset exceeds i32"),
            full_size: i32::try_from(data.full_size).expect("full_size exceeds i32"),
            nonce: i32::try_from(data.nonce).expect("nonce exceeds i32"),
        }
    }
}

pub(crate) async fn create_tables(scylla_db: &ScyllaDb) -> anyhow::Result<()> {
    let queries = [
        "CREATE TABLE IF NOT EXISTS s_fastfs_v2 (
            receipt_id text,
            action_index int,
            tx_hash text,
            signer_id text,
            predecessor_id text,
            current_account_id text,
            block_height bigint,
            block_timestamp bigint,
            shard_id int,
            receipt_index int,
            mime_type text,
            relative_path text,
            content blob,
            offset int,
            full_size int,
            nonce int,
            PRIMARY KEY ((predecessor_id), current_account_id, relative_path, nonce, offset)
        )",
        // Secondary indexes on high-cardinality columns removed (ScyllaDB anti-pattern).
        // Use dedicated lookup tables if tx_hash/receipt_id queries are needed.
    ];
    for query in queries.iter() {
        tracing::debug!(target: SCYLLADB, "Creating table: {}", query);
        scylla_db.scylla_session.query_unpaged(*query, &[]).await?;
    }
    Ok(())
}

pub(crate) async fn prepare_insert_query(
    scylla_db: &ScyllaDb,
) -> anyhow::Result<PreparedStatement> {
    ScyllaDb::prepare_query(
        &scylla_db.scylla_session,
        "INSERT INTO s_fastfs_v2 (receipt_id, action_index, tx_hash, signer_id, predecessor_id, current_account_id, block_height, block_timestamp, shard_id, receipt_index, mime_type, relative_path, content, offset, full_size, nonce) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        scylla::frame::types::Consistency::LocalQuorum,
    )
        .await
}

pub(crate) async fn add_fastfs_fastdata(
    scylla_db: &ScyllaDb,
    insert_query: &PreparedStatement,
    fastdata: &FastfsFastData,
) -> anyhow::Result<()> {
    scylla_db
        .scylla_session
        .execute_unpaged(insert_query, FastfsFastDataRow::from(fastdata.clone()))
        .await?;
    Ok(())
}
