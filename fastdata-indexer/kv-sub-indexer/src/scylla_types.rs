use crate::*;
use fastnear_primitives::near_indexer_primitives::types::AccountId;
use fastnear_primitives::near_indexer_primitives::CryptoHash;
use futures::stream::{FuturesUnordered, StreamExt};
use scylla::serialize::row::SerializeRow;
use scylla::statement::batch::{Batch, BatchType};
use scylla::statement::prepared::PreparedStatement;
use scylla::{DeserializeRow, SerializeRow};
use scylladb::{ScyllaDb, SCYLLADB};
use std::collections::{HashMap, HashSet};

pub(crate) const SUFFIX: &str = "kv";
pub(crate) const INDEXER_ID: &str = "kv-1";

/// Detect if a value is encrypted (format: enc:AES256:key_id:ciphertext)
/// Returns Some(key_id) if encrypted, None otherwise
pub fn detect_encrypted(value: &str) -> Option<String> {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() == 4 && parts[0] == "enc" && parts[1] == "AES256" {
        // Remove quotes if present (JSON serialized)
        let key_id = parts[2].trim_matches('"').to_string();
        Some(key_id)
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct FastDataKv {
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
    pub order_id: u64,

    pub key: String,
    pub value: String,
    /// Key ID if value is encrypted (format: enc:AES256:key_id:...)
    pub encrypted_key_id: Option<String>,
}

#[derive(Debug, Clone, DeserializeRow, SerializeRow)]
pub(crate) struct FastDataKvRow {
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
    pub order_id: i64,

    pub key: String,
    pub value: String,
    /// Key ID if encrypted (null if not encrypted)
    pub encrypted_key_id: Option<String>,
}

#[derive(Debug, Clone, SerializeRow)]
pub(crate) struct KvEdgeRow {
    pub edge_type: String,
    pub target: String,
    pub source: String,
    pub current_account_id: String,
    pub block_height: i64,
    pub block_timestamp: i64,
    pub order_id: i64,
    pub value: String,
}

#[derive(Debug, Clone, SerializeRow)]
pub(crate) struct KvByBlockRow {
    pub predecessor_id: String,
    pub current_account_id: String,
    pub block_height: i64,
    pub key: String,
    pub value: String,
    pub block_timestamp: i64,
    pub order_id: i64,
    pub receipt_id: String,
    pub tx_hash: Option<String>,
    pub encrypted_key_id: Option<String>,
}

impl TryFrom<FastDataKvRow> for FastDataKv {
    type Error = anyhow::Error;

    fn try_from(row: FastDataKvRow) -> anyhow::Result<Self> {
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
            order_id: u64::try_from(row.order_id)
                .map_err(|_| anyhow::anyhow!("Negative order_id in DB: {}", row.order_id))?,

            key: row.key,
            value: row.value,
            encrypted_key_id: row.encrypted_key_id,
        })
    }
}

impl From<FastDataKv> for FastDataKvRow {
    fn from(data: FastDataKv) -> Self {
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
            order_id: i64::try_from(data.order_id).expect("order_id exceeds i64"),
            key: data.key,
            value: data.value,
            encrypted_key_id: data.encrypted_key_id,
        }
    }
}

// __fastdata_kv({ "foo/alex.near": "bar", "foo/bob.near": "moo"})
// PRIMARY KEY ((predecessor_id), current_account_id, key)
// "key" >= "foo/" and "key" < "foo/" + '\xff'
// INDEX KEY ((current_account_id), key)

pub(crate) async fn create_tables(scylla_db: &ScyllaDb) -> anyhow::Result<()> {
    let queries = [
        "CREATE TABLE IF NOT EXISTS s_kv (
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
            order_id bigint,

            key text,
            value text,
            encrypted_key_id text,
            PRIMARY KEY ((predecessor_id), current_account_id, key, block_height, order_id)
        )",
        "CREATE TABLE IF NOT EXISTS s_kv_last (
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
            order_id bigint,

            key text,
            value text,
            encrypted_key_id text,
            PRIMARY KEY ((predecessor_id), current_account_id, key)
        )",
        "CREATE MATERIALIZED VIEW IF NOT EXISTS mv_kv_key AS
            SELECT * FROM s_kv
            WHERE key IS NOT NULL AND block_height IS NOT NULL AND order_id IS NOT NULL AND predecessor_id IS NOT NULL AND current_account_id IS NOT NULL
            PRIMARY KEY((key), block_height, order_id, predecessor_id, current_account_id)
        ",
        "CREATE MATERIALIZED VIEW IF NOT EXISTS mv_kv_cur_key AS
            SELECT * FROM s_kv
            WHERE current_account_id IS NOT NULL AND key IS NOT NULL AND block_height IS NOT NULL AND order_id IS NOT NULL AND predecessor_id IS NOT NULL
            PRIMARY KEY((current_account_id), key, block_height, order_id, predecessor_id)
        ",
        "CREATE TABLE IF NOT EXISTS kv_accounts (
            current_account_id text,
            key text,
            predecessor_id text,
            PRIMARY KEY ((current_account_id), key, predecessor_id)
        )",
        "CREATE TABLE IF NOT EXISTS kv_edges (
            edge_type text,
            target text,
            source text,
            current_account_id text,
            block_height bigint,
            block_timestamp bigint,
            order_id bigint,
            value text,
            PRIMARY KEY ((edge_type, target), source)
        )",
        "CREATE TABLE IF NOT EXISTS kv_reverse (
            current_account_id text,
            key text,
            predecessor_id text,
            receipt_id text,
            action_index int,
            tx_hash text,
            signer_id text,
            block_height bigint,
            block_timestamp bigint,
            shard_id int,
            receipt_index int,
            order_id bigint,
            value text,
            PRIMARY KEY ((current_account_id, key), predecessor_id)
        )",
        "CREATE TABLE IF NOT EXISTS all_accounts (
            predecessor_id text PRIMARY KEY,
            last_block_height bigint,
            last_block_timestamp bigint
        )",
        "CREATE TABLE IF NOT EXISTS s_kv_by_block (
            predecessor_id text,
            current_account_id text,
            block_height bigint,
            key text,
            value text,
            block_timestamp bigint,
            order_id bigint,
            receipt_id text,
            tx_hash text,
            encrypted_key_id text,
            PRIMARY KEY ((predecessor_id, current_account_id), block_height, key)
        ) WITH CLUSTERING ORDER BY (block_height DESC, key ASC)",
    ];
    for query in queries.iter() {
        tracing::debug!(target: SCYLLADB, "Creating table: {}", query);
        scylla_db.scylla_session.query_unpaged(*query, &[]).await?;
    }
    Ok(())
}

pub(crate) async fn prepare_kv_insert_query(
    scylla_db: &ScyllaDb,
) -> anyhow::Result<PreparedStatement> {
    ScyllaDb::prepare_query(
        &scylla_db.scylla_session,
    "INSERT INTO s_kv (receipt_id, action_index, tx_hash, signer_id, predecessor_id, current_account_id, block_height, block_timestamp, shard_id, receipt_index, order_id, key, value, encrypted_key_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        scylla::frame::types::Consistency::LocalQuorum,
    )
    .await
}

pub(crate) async fn prepare_kv_last_insert_query(
    scylla_db: &ScyllaDb,
) -> anyhow::Result<PreparedStatement> {
    ScyllaDb::prepare_query(
        &scylla_db.scylla_session,
        "INSERT INTO s_kv_last (receipt_id, action_index, tx_hash, signer_id, predecessor_id, current_account_id, block_height, block_timestamp, shard_id, receipt_index, order_id, key, value, encrypted_key_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        scylla::frame::types::Consistency::LocalQuorum,
    )
        .await
}

pub(crate) async fn prepare_kv_accounts_insert_query(
    scylla_db: &ScyllaDb,
) -> anyhow::Result<PreparedStatement> {
    ScyllaDb::prepare_query(
        &scylla_db.scylla_session,
        "INSERT INTO kv_accounts (current_account_id, key, predecessor_id) VALUES (?, ?, ?)",
        scylla::frame::types::Consistency::LocalQuorum,
    )
    .await
}

pub(crate) async fn prepare_kv_edges_insert_query(
    scylla_db: &ScyllaDb,
) -> anyhow::Result<PreparedStatement> {
    ScyllaDb::prepare_query(
        &scylla_db.scylla_session,
        "INSERT INTO kv_edges (edge_type, target, source, current_account_id, block_height, block_timestamp, order_id, value) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        scylla::frame::types::Consistency::LocalQuorum,
    )
    .await
}

pub(crate) async fn prepare_kv_edges_delete_query(
    scylla_db: &ScyllaDb,
) -> anyhow::Result<PreparedStatement> {
    ScyllaDb::prepare_query(
        &scylla_db.scylla_session,
        "DELETE FROM kv_edges WHERE edge_type = ? AND target = ? AND source = ?",
        scylla::frame::types::Consistency::LocalQuorum,
    )
    .await
}

pub(crate) async fn prepare_kv_reverse_insert_query(
    scylla_db: &ScyllaDb,
) -> anyhow::Result<PreparedStatement> {
    ScyllaDb::prepare_query(
        &scylla_db.scylla_session,
        "INSERT INTO kv_reverse (current_account_id, key, predecessor_id, receipt_id, action_index, tx_hash, signer_id, block_height, block_timestamp, shard_id, receipt_index, order_id, value) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        scylla::frame::types::Consistency::LocalQuorum,
    )
    .await
}

pub(crate) async fn prepare_all_accounts_insert_query(
    scylla_db: &ScyllaDb,
) -> anyhow::Result<PreparedStatement> {
    ScyllaDb::prepare_query(
        &scylla_db.scylla_session,
        "INSERT INTO all_accounts (predecessor_id, last_block_height, last_block_timestamp) VALUES (?, ?, ?)",
        scylla::frame::types::Consistency::LocalQuorum,
    )
    .await
}

pub(crate) async fn prepare_kv_by_block_insert_query(
    scylla_db: &ScyllaDb,
) -> anyhow::Result<PreparedStatement> {
    ScyllaDb::prepare_query(
        &scylla_db.scylla_session,
        "INSERT INTO s_kv_by_block (predecessor_id, current_account_id, block_height, key, value, block_timestamp, order_id, receipt_id, tx_hash, encrypted_key_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        scylla::frame::types::Consistency::LocalQuorum,
    )
    .await
}

/// Returns true if the value represents a logical deletion of a KV entry.
/// Currently only JSON null (serialized as the 4-char string "null") counts.
pub(crate) fn is_edge_deletion(value: &str) -> bool {
    value == "null"
}

/// Auto-detects a graph edge from a KV key.
/// Key must have at least 2 `/`-separated segments.
/// Last segment = target, all preceding segments joined by `/` = edge_type.
/// Source is always `predecessor_id` (not derived from the key).
pub(crate) fn extract_edge(key: &str, predecessor_id: &str) -> Option<(String, String, String)> {
    let key = key.trim_start_matches('/');
    let segments: Vec<&str> = key.split('/').collect();
    if segments.len() < 2 {
        return None;
    }
    if predecessor_id.is_empty() {
        return None;
    }
    let target = segments[segments.len() - 1];
    if target.is_empty() {
        return None;
    }
    let edge_type = segments[..segments.len() - 1].join("/");
    if edge_type.is_empty() {
        return None;
    }
    Some((edge_type, predecessor_id.to_string(), target.to_string()))
}

const BATCH_CHUNK_SIZE: usize = 100;

pub(crate) struct KvQueries {
    pub kv_insert: PreparedStatement,
    pub kv_last_insert: PreparedStatement,
    pub kv_accounts_insert: PreparedStatement,
    pub kv_edges_insert: PreparedStatement,
    pub kv_edges_delete: PreparedStatement,
    pub kv_reverse_insert: PreparedStatement,
    pub all_accounts_insert: PreparedStatement,
    pub kv_by_block_insert: PreparedStatement,
}

pub(crate) async fn add_kv_rows(
    scylla_db: &ScyllaDb,
    queries: &KvQueries,
    rows: &[FastDataKv],
    last_processed_block_height: Option<BlockHeight>,
) -> anyhow::Result<()> {
    let kv_rows: Vec<FastDataKvRow> = rows
        .iter()
        .cloned()
        .map(FastDataKvRow::from)
        .collect();
    // Deduplicate kv_rows by key, keeping the index of the row with the highest order_id
    let mut kv_last_map: HashMap<(&str, &str, &str), usize> = HashMap::new();
    for (i, row) in kv_rows.iter().enumerate() {
        let dedup_key = (
            row.predecessor_id.as_str(),
            row.current_account_id.as_str(),
            row.key.as_str(),
        );
        match kv_last_map.get(&dedup_key) {
            Some(&existing_idx) if (kv_rows[existing_idx].block_height, kv_rows[existing_idx].order_id) >= (row.block_height, row.order_id) => {}
            _ => {
                kv_last_map.insert(dedup_key, i);
            }
        }
    }
    let mut kv_last_indices: Vec<usize> = kv_last_map.into_values().collect();
    kv_last_indices.sort_by(|&a, &b| {
        (kv_rows[b].block_height, kv_rows[b].order_id).cmp(&(kv_rows[a].block_height, kv_rows[a].order_id))
    });

    // Write kv_rows in chunks
    for chunk in kv_rows.chunks(BATCH_CHUNK_SIZE) {
        let mut batch = Batch::new(BatchType::Unlogged);
        let mut values: Vec<&dyn SerializeRow> = vec![];
        for kv in chunk {
            batch.append_statement(queries.kv_insert.clone());
            values.push(kv);
        }
        scylla_db.scylla_session.batch(&batch, values).await?;
    }

    // Write s_kv_by_block in chunks (same rows as s_kv, repartitioned for timeline queries)
    for chunk in kv_rows.chunks(BATCH_CHUNK_SIZE) {
        let mut batch = Batch::new(BatchType::Unlogged);
        let mut values: Vec<&dyn SerializeRow> = vec![];
        let by_block_rows: Vec<KvByBlockRow> = chunk.iter().map(|r| KvByBlockRow {
            predecessor_id: r.predecessor_id.clone(),
            current_account_id: r.current_account_id.clone(),
            block_height: r.block_height,
            key: r.key.clone(),
            value: r.value.clone(),
            block_timestamp: r.block_timestamp,
            order_id: r.order_id,
            receipt_id: r.receipt_id.clone(),
            tx_hash: r.tx_hash.clone(),
            encrypted_key_id: r.encrypted_key_id.clone(),
        }).collect();
        for row in &by_block_rows {
            batch.append_statement(queries.kv_by_block_insert.clone());
            values.push(row);
        }
        scylla_db.scylla_session.batch(&batch, values).await?;
    }

    // Pre-compute USING TIMESTAMP values for kv_last entries (bail early on overflow).
    // Encode (block_height, order_id) into a single i64 timestamp for deterministic LWW.
    // Max order_id ≈ shard_count * 100_000 * 1_000 ≈ 600M (6 shards). With 1B multiplier,
    // block_height can reach ~9.2B before i64 overflow (current NEAR height: ~150M).
    let kv_last_timestamps: Vec<(usize, i64)> = kv_last_indices.iter()
        .map(|&i| {
            let row = &kv_rows[i];
            let ts = row.block_height.checked_mul(1_000_000_000)
                .and_then(|v| v.checked_add(row.order_id))
                .ok_or_else(|| anyhow::anyhow!(
                    "USING TIMESTAMP overflow: block_height={}, order_id={}",
                    row.block_height, row.order_id
                ))?;
            Ok((i, ts))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    // Pre-compute kv_accounts unique tuples
    let account_tuples_vec: Vec<(String, String, String)> = {
        let set: HashSet<(&str, &str, &str)> = kv_last_indices.iter()
            .map(|&i| {
                let row = &kv_rows[i];
                (row.current_account_id.as_str(), row.key.as_str(), row.predecessor_id.as_str())
            })
            .collect();
        set.into_iter().map(|(a, k, p)| (a.to_string(), k.to_string(), p.to_string())).collect()
    };

    // Pre-compute edge writes (inserts and deletes with timestamps)
    let mut edge_inserts: Vec<(KvEdgeRow, i64)> = Vec::new();
    let mut edge_deletes: Vec<((String, String, String), i64)> = Vec::new();
    {
        let mut edge_dedup: HashSet<(String, String, String)> = HashSet::new();
        for &(idx, ts) in &kv_last_timestamps {
            let row = &kv_rows[idx];
            if let Some((edge_type, source, target)) = extract_edge(&row.key, &row.predecessor_id) {
                let dedup_key = (edge_type.clone(), target.clone(), source.clone());
                if !edge_dedup.insert(dedup_key) { continue; }
                if is_edge_deletion(&row.value) {
                    edge_deletes.push(((edge_type, target, source), ts));
                } else {
                    edge_inserts.push((KvEdgeRow {
                        edge_type,
                        target,
                        source,
                        current_account_id: row.current_account_id.clone(),
                        block_height: row.block_height,
                        block_timestamp: row.block_timestamp,
                        order_id: row.order_id,
                        value: row.value.clone(),
                    }, ts));
                }
            }
        }
    }

    // Pre-compute all_accounts writes: (predecessor_id, block_height, block_timestamp, ts)
    let all_accounts_writes: Vec<(String, i64, i64, i64)> = {
        let mut account_max: HashMap<&str, &FastDataKvRow> = HashMap::new();
        for &idx in &kv_last_indices {
            let row = &kv_rows[idx];
            account_max
                .entry(row.predecessor_id.as_str())
                .and_modify(|existing| {
                    if (row.block_height, row.order_id) > (existing.block_height, existing.order_id) {
                        *existing = row;
                    }
                })
                .or_insert(row);
        }
        account_max.into_values().map(|row| {
            let ts = row.block_height.checked_mul(1_000_000_000)
                .and_then(|v| v.checked_add(row.order_id))
                .ok_or_else(|| anyhow::anyhow!(
                    "all_accounts USING TIMESTAMP overflow: block_height={}, order_id={}",
                    row.block_height, row.order_id
                ))?;
            Ok((row.predecessor_id.clone(), row.block_height, row.block_timestamp, ts))
        }).collect::<anyhow::Result<Vec<_>>>()?
    };

    let session = &scylla_db.scylla_session;

    // Run independent table writes concurrently via try_join!.
    // Within each group, use FuturesUnordered for concurrent individual writes.
    tokio::try_join!(
        // kv_last + kv_reverse: USING TIMESTAMP per-row writes
        async {
            let mut futs = FuturesUnordered::new();
            for &(idx, ts) in &kv_last_timestamps {
                let kv_last = &kv_rows[idx];
                let mut last_stmt = queries.kv_last_insert.clone();
                last_stmt.set_timestamp(Some(ts));
                let mut rev_stmt = queries.kv_reverse_insert.clone();
                rev_stmt.set_timestamp(Some(ts));
                futs.push(async move {
                    session.execute_unpaged(&last_stmt, kv_last).await?;
                    let rev_row = (
                        &kv_last.current_account_id, &kv_last.key, &kv_last.predecessor_id,
                        &kv_last.receipt_id, &kv_last.action_index, &kv_last.tx_hash,
                        &kv_last.signer_id, &kv_last.block_height, &kv_last.block_timestamp,
                        &kv_last.shard_id, &kv_last.receipt_index, &kv_last.order_id, &kv_last.value,
                    );
                    session.execute_unpaged(&rev_stmt, rev_row).await?;
                    Ok::<(), anyhow::Error>(())
                });
            }
            while let Some(r) = futs.next().await { r?; }
            Ok::<(), anyhow::Error>(())
        },
        // kv_accounts: batched writes (already efficient)
        async {
            for chunk in account_tuples_vec.chunks(BATCH_CHUNK_SIZE) {
                let mut batch = Batch::new(BatchType::Unlogged);
                let mut values: Vec<&dyn SerializeRow> = vec![];
                for tuple in chunk {
                    batch.append_statement(queries.kv_accounts_insert.clone());
                    values.push(tuple);
                }
                session.batch(&batch, values).await?;
            }
            Ok::<(), anyhow::Error>(())
        },
        // kv_edges: USING TIMESTAMP per-row inserts and deletes
        async {
            // Edge inserts
            let mut futs = FuturesUnordered::new();
            for (edge, ts) in &edge_inserts {
                let mut stmt = queries.kv_edges_insert.clone();
                stmt.set_timestamp(Some(*ts));
                futs.push(async move {
                    session.execute_unpaged(&stmt, edge).await?;
                    Ok::<(), anyhow::Error>(())
                });
            }
            while let Some(r) = futs.next().await { r?; }
            // Edge deletes
            let mut del_futs = FuturesUnordered::new();
            for ((edge_type, target, source), ts) in &edge_deletes {
                let mut stmt = queries.kv_edges_delete.clone();
                stmt.set_timestamp(Some(*ts));
                del_futs.push(async move {
                    session.execute_unpaged(&stmt, (edge_type, target, source)).await?;
                    Ok::<(), anyhow::Error>(())
                });
            }
            while let Some(r) = del_futs.next().await { r?; }
            Ok::<(), anyhow::Error>(())
        },
        // all_accounts: USING TIMESTAMP per-row writes
        async {
            let mut futs = FuturesUnordered::new();
            for (predecessor_id, block_height, block_timestamp, ts) in &all_accounts_writes {
                let mut stmt = queries.all_accounts_insert.clone();
                stmt.set_timestamp(Some(*ts));
                futs.push(async move {
                    session.execute_unpaged(&stmt, (predecessor_id, block_height, block_timestamp)).await?;
                    Ok::<(), anyhow::Error>(())
                });
            }
            while let Some(r) = futs.next().await { r?; }
            Ok::<(), anyhow::Error>(())
        }
    )?;

    // Write checkpoint only when a block height is provided
    if let Some(height) = last_processed_block_height {
        let checkpoint_row = (INDEXER_ID.to_string(), i64::try_from(height).expect("checkpoint height exceeds i64"));
        scylla_db
            .scylla_session
            .execute_unpaged(
                &scylla_db.insert_last_processed_block_height_query,
                checkpoint_row,
            )
            .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_edge_follow() {
        assert_eq!(
            extract_edge("legion/follow/bob.near", "alice.near"),
            Some(("legion/follow".into(), "alice.near".into(), "bob.near".into()))
        );
    }

    #[test]
    fn test_extract_edge_graph_follow() {
        assert_eq!(
            extract_edge("graph/follow/bob.near", "alice.near"),
            Some(("graph/follow".into(), "alice.near".into(), "bob.near".into()))
        );
    }

    #[test]
    fn test_extract_edge_single_segment_type() {
        assert_eq!(
            extract_edge("follow/bob.near", "alice.near"),
            Some(("follow".into(), "alice.near".into(), "bob.near".into()))
        );
    }

    #[test]
    fn test_extract_edge_deep_type() {
        assert_eq!(
            extract_edge("a/b/c/bob.near", "alice.near"),
            Some(("a/b/c".into(), "alice.near".into(), "bob.near".into()))
        );
    }

    #[test]
    fn test_extract_edge_empty_predecessor() {
        assert_eq!(extract_edge("graph/follow/bob.near", ""), None);
    }

    #[test]
    fn test_extract_edge_too_few_segments() {
        assert_eq!(extract_edge("bob.near", "alice.near"), None);
    }

    #[test]
    fn test_extract_edge_non_account_target() {
        assert_eq!(
            extract_edge("like/post/12345", "alice.near"),
            Some(("like/post".into(), "alice.near".into(), "12345".into()))
        );
    }

    #[test]
    fn test_extract_edge_profile_name() {
        assert_eq!(
            extract_edge("profile/name", "alice.near"),
            Some(("profile".into(), "alice.near".into(), "name".into()))
        );
    }

    #[test]
    fn test_extract_edge_empty_target() {
        assert_eq!(extract_edge("legion/follow/", "alice.near"), None);
    }

    #[test]
    fn test_extract_edge_empty_source() {
        assert_eq!(extract_edge("graph/follow/bob.near", ""), None);
    }

    #[test]
    fn test_extract_edge_no_slash() {
        assert_eq!(extract_edge("justadata", "alice.near"), None);
    }

    #[test]
    fn test_extract_edge_custom_subscribe() {
        assert_eq!(
            extract_edge("custom/subscribe/channel.near", "alice.near"),
            Some(("custom/subscribe".into(), "alice.near".into(), "channel.near".into()))
        );
    }

    #[test]
    fn test_extract_edge_leading_slash() {
        assert_eq!(
            extract_edge("/graph/alice.near", "bob.near"),
            Some(("graph".into(), "bob.near".into(), "alice.near".into()))
        );
    }

    #[test]
    fn test_is_edge_deletion_null() {
        assert!(is_edge_deletion("null"));
    }

    #[test]
    fn test_is_edge_deletion_non_null() {
        assert!(!is_edge_deletion("\"hello\""));
    }

    #[test]
    fn test_is_edge_deletion_null_string_value() {
        // JSON string "null" (\"null\") is NOT JSON null (null)
        assert!(!is_edge_deletion("\"null\""));
    }
}
