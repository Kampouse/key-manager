use scylla::client::session::Session;
use scylla::client::session_builder::SessionBuilder;
use scylla::errors::NextRowError;
use scylla::statement::prepared::PreparedStatement;

use crate::models::{
    bigint_to_u64, AccountsParams, ContractAccountRow, ContractKeyRow, ContractRow, EdgeRow, EdgeSourceEntry,
    HistoryParams, KvEntry, KvHistoryRow, KvRow, KvTimelineRow, QueryParams, TimelineParams,
    WritersParams, MAX_DEDUP_SCAN,
};
use fastnear_primitives::types::ChainId;
use futures::stream::StreamExt;
use futures::Stream;
use rustls::pki_types::pem::PemObject;
use rustls::{ClientConfig, RootCertStore};
use std::collections::HashSet;
use std::env;
use std::sync::Arc;

/// Outcome of a paginated stream collection.
#[derive(Debug)]
pub struct PageResult<T> {
    pub items: Vec<T>,
    pub has_more: bool,
    pub truncated: bool,
    pub dropped_rows: usize,
}

/// Collects rows from a typed stream with standard pagination semantics.
///
/// **Overfetch mode** (`scan_cap = None`):
///   Skips `offset` valid items (those where `transform` returns `Some`),
///   collects up to `limit + 1`, sets `has_more` from overfetch.
///
/// **Scan-cap mode** (`scan_cap = Some(cap)`):
///   Collects ALL valid items up to `cap` raw rows scanned.
///   Sets `truncated = true` if cap hit. Does NOT apply offset/limit
///   (caller post-sorts then slices). `has_more` is left as `false`.
pub async fn collect_page<T, R, S, F>(
    stream: &mut S,
    limit: usize,
    offset: usize,
    scan_cap: Option<usize>,
    mut transform: F,
) -> PageResult<T>
where
    S: Stream<Item = Result<R, NextRowError>> + Unpin,
    F: FnMut(R) -> Option<T>,
{
    let mut items = match scan_cap {
        None => Vec::with_capacity(limit + 1),
        Some(_) => Vec::new(),
    };
    let mut dropped_rows = 0usize;
    let mut skipped = 0usize;
    let mut scanned = 0usize;
    let mut truncated = false;

    while let Some(row_result) = stream.next().await {
        // Scan-cap check (before deser — matches current behavior)
        if let Some(cap) = scan_cap {
            scanned += 1;
            if scanned > cap {
                truncated = true;
                break;
            }
        }

        let row = match row_result {
            Ok(r) => r,
            Err(e) => {
                dropped_rows += 1;
                tracing::warn!(
                    target: "fastkv-server",
                    error = %e,
                    "Failed to deserialize row"
                );
                continue;
            }
        };

        // Transform + filter (None = filtered out, not counted for offset/limit)
        let item = match transform(row) {
            Some(item) => item,
            None => continue,
        };

        // Offset skip (overfetch mode only)
        if scan_cap.is_none() && skipped < offset {
            skipped += 1;
            continue;
        }

        items.push(item);

        // Early break at limit+1 (overfetch mode only)
        if scan_cap.is_none() && items.len() > limit {
            break;
        }
    }

    let has_more = if scan_cap.is_none() {
        let over = items.len() > limit;
        items.truncate(limit);
        over
    } else {
        false // caller computes after post-sort + slice
    };

    PageResult {
        items,
        has_more,
        truncated,
        dropped_rows,
    }
}

/// Validate that a CQL identifier (keyspace/table name) contains only safe characters.
pub(crate) fn validate_identifier(name: &str, label: &str) -> anyhow::Result<()> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        anyhow::bail!(
            "{} must contain only alphanumeric characters and underscores: {}",
            label,
            name
        );
    }
    Ok(())
}

pub struct ScyllaDb {
    get_kv: PreparedStatement,
    get_kv_last: PreparedStatement,
    query_kv_no_prefix: PreparedStatement,
    query_kv_cursor: PreparedStatement,
    pub(crate) reverse_kv: PreparedStatement,
    reverse_list: PreparedStatement,
    reverse_list_cursor: PreparedStatement,
    history_asc: PreparedStatement,
    history_desc: PreparedStatement,
    get_kv_at_block: PreparedStatement,
    timeline_asc: PreparedStatement,
    timeline_desc: PreparedStatement,
    accounts_by_contract: PreparedStatement,
    accounts_by_contract_key: PreparedStatement,
    accounts_all: PreparedStatement,
    accounts_all_cursor: PreparedStatement,
    contracts_all: PreparedStatement,
    contracts_all_cursor: PreparedStatement,
    contracts_by_account: PreparedStatement,
    edges_list: PreparedStatement,
    edges_list_cursor: PreparedStatement,
    edges_count: PreparedStatement,
    prefix_query: PreparedStatement,
    prefix_cursor_query: PreparedStatement,
    meta_query: PreparedStatement,

    pub scylla_session: Session,
    pub table_name: String,
    pub history_table_name: String,
    pub reverse_view_name: String,
    pub kv_accounts_table_name: String,
    pub all_accounts_table_name: String,
    pub kv_edges_table_name: String,
    pub kv_reverse_table_name: String,
}

pub fn create_rustls_client_config() -> anyhow::Result<Arc<ClientConfig>> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .expect("Failed to install default provider");
    }

    let ca_cert_path =
        env::var("SCYLLA_SSL_CA").map_err(|_| anyhow::anyhow!("SCYLLA_SSL_CA required for TLS"))?;
    let ca_certs = rustls::pki_types::CertificateDer::from_pem_file(&ca_cert_path)
        .map_err(|e| anyhow::anyhow!("Failed to load CA cert from '{}': {}", ca_cert_path, e))?;

    let mut root_store = RootCertStore::empty();
    root_store
        .add(ca_certs)
        .map_err(|e| anyhow::anyhow!("Failed to add CA certs to root store: {}", e))?;

    let config = match (
        env::var("SCYLLA_SSL_CERT").ok(),
        env::var("SCYLLA_SSL_KEY").ok(),
    ) {
        (Some(cert_path), Some(key_path)) => {
            let client_certs = rustls::pki_types::CertificateDer::from_pem_file(&cert_path)
                .map_err(|e| {
                    anyhow::anyhow!("Failed to load client cert from '{}': {}", cert_path, e)
                })?;
            let client_key =
                rustls::pki_types::PrivateKeyDer::from_pem_file(&key_path).map_err(|e| {
                    anyhow::anyhow!("Failed to load client key from '{}': {}", key_path, e)
                })?;
            ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_client_auth_cert(vec![client_certs], client_key)
                .map_err(|e| anyhow::anyhow!("Failed to create client config with mTLS: {}", e))?
        }
        _ => ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    };

    Ok(Arc::new(config))
}

impl ScyllaDb {
    pub async fn new_scylla_session() -> anyhow::Result<Session> {
        let scylla_url = env::var("SCYLLA_URL").expect("SCYLLA_URL must be set");
        let scylla_username = env::var("SCYLLA_USERNAME").expect("SCYLLA_USERNAME must be set");
        let scylla_password = env::var("SCYLLA_PASSWORD").expect("SCYLLA_PASSWORD must be set");

        let tls_config = match env::var("SCYLLA_SSL_CA").ok() {
            Some(_) => Some(create_rustls_client_config()?),
            None => None,
        };

        let session: Session = SessionBuilder::new()
            .known_node(scylla_url)
            .connection_timeout(std::time::Duration::from_secs(5))
            .tls_context(tls_config)
            .authenticator_provider(Arc::new(
                scylla::authentication::PlainTextAuthenticator::new(
                    scylla_username,
                    scylla_password,
                ),
            ))
            .build()
            .await?;

        Ok(session)
    }

    pub async fn test_connection(scylla_session: &Session) -> anyhow::Result<()> {
        let mut stmt = scylla::statement::Statement::new("SELECT now() FROM system.local");
        stmt.set_request_timeout(Some(std::time::Duration::from_secs(10)));
        scylla_session.query_unpaged(stmt, &[]).await?;
        Ok(())
    }

    pub async fn health_check(&self) -> anyhow::Result<()> {
        // Simple query to verify connection
        let mut stmt = scylla::statement::Statement::new("SELECT now() FROM system.local");
        stmt.set_request_timeout(Some(std::time::Duration::from_secs(10)));
        self.scylla_session.query_unpaged(stmt, &[]).await?;
        Ok(())
    }

    pub async fn new(chain_id: ChainId, scylla_session: Session) -> anyhow::Result<Self> {
        // Support custom keyspace or use default pattern
        let keyspace = env::var("KEYSPACE").unwrap_or_else(|_| format!("fastdata_{chain_id}"));

        validate_identifier(&keyspace, "KEYSPACE")?;

        scylla_session.use_keyspace(&keyspace, false).await?;

        // Support custom table names
        let table_name = env::var("TABLE_NAME").unwrap_or_else(|_| "s_kv_last".to_string());
        let history_table_name =
            env::var("HISTORY_TABLE_NAME").unwrap_or_else(|_| "s_kv".to_string());
        let reverse_view_name =
            env::var("REVERSE_VIEW_NAME").unwrap_or_else(|_| "mv_kv_cur_key".to_string());
        let kv_accounts_table_name =
            env::var("KV_ACCOUNTS_TABLE_NAME").unwrap_or_else(|_| "kv_accounts".to_string());
        let all_accounts_table_name =
            env::var("ALL_ACCOUNTS_TABLE_NAME").unwrap_or_else(|_| "all_accounts".to_string());
        let kv_edges_table_name =
            env::var("KV_EDGES_TABLE_NAME").unwrap_or_else(|_| "kv_edges".to_string());
        let kv_reverse_table_name =
            env::var("KV_REVERSE_TABLE_NAME").unwrap_or_else(|_| "kv_reverse".to_string());

        validate_identifier(&table_name, "TABLE_NAME")?;
        validate_identifier(&history_table_name, "HISTORY_TABLE_NAME")?;
        validate_identifier(&reverse_view_name, "REVERSE_VIEW_NAME")?;
        validate_identifier(&kv_accounts_table_name, "KV_ACCOUNTS_TABLE_NAME")?;
        validate_identifier(&all_accounts_table_name, "ALL_ACCOUNTS_TABLE_NAME")?;
        validate_identifier(&kv_edges_table_name, "KV_EDGES_TABLE_NAME")?;
        validate_identifier(&kv_reverse_table_name, "KV_REVERSE_TABLE_NAME")?;

        let columns = "predecessor_id, current_account_id, key, value, block_height, block_timestamp, receipt_id, tx_hash";
        let history_columns = "predecessor_id, current_account_id, key, block_height, order_id, value, block_timestamp, receipt_id, tx_hash, signer_id, shard_id, receipt_index, action_index";
        let timeline_columns = "predecessor_id, current_account_id, block_height, key, order_id, value, block_timestamp, receipt_id, tx_hash";

        Ok(Self {
            get_kv: Self::prepare_query(
                &scylla_session,
                &format!("SELECT {} FROM {} WHERE predecessor_id = ? AND current_account_id = ? AND key = ?", columns, table_name),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            get_kv_last: Self::prepare_query(
                &scylla_session,
                &format!("SELECT value FROM {} WHERE predecessor_id = ? AND current_account_id = ? AND key = ?", table_name),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            query_kv_no_prefix: Self::prepare_query(
                &scylla_session,
                &format!("SELECT {} FROM {} WHERE predecessor_id = ? AND current_account_id = ?", columns, table_name),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            query_kv_cursor: Self::prepare_query(
                &scylla_session,
                &format!("SELECT {} FROM {} WHERE predecessor_id = ? AND current_account_id = ? AND key > ?", columns, table_name),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            reverse_kv: Self::prepare_query(
                &scylla_session,
                &format!("SELECT {} FROM {} WHERE current_account_id = ? AND key = ? ORDER BY block_height DESC, order_id DESC, predecessor_id DESC", columns, reverse_view_name),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            reverse_list: Self::prepare_query(
                &scylla_session,
                &format!("SELECT {} FROM {} WHERE current_account_id = ? AND key = ?", columns, kv_reverse_table_name),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            reverse_list_cursor: Self::prepare_query(
                &scylla_session,
                &format!("SELECT {} FROM {} WHERE current_account_id = ? AND key = ? AND predecessor_id > ?", columns, kv_reverse_table_name),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            history_asc: Self::prepare_query(
                &scylla_session,
                &format!("SELECT {} FROM {} WHERE predecessor_id = ? AND current_account_id = ? AND key = ? AND block_height >= ? AND block_height <= ? ORDER BY block_height ASC, order_id ASC", history_columns, history_table_name),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            history_desc: Self::prepare_query(
                &scylla_session,
                &format!("SELECT {} FROM {} WHERE predecessor_id = ? AND current_account_id = ? AND key = ? AND block_height >= ? AND block_height <= ? ORDER BY block_height DESC, order_id DESC", history_columns, history_table_name),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            get_kv_at_block: Self::prepare_query(
                &scylla_session,
                &format!("SELECT {} FROM {} WHERE predecessor_id = ? AND current_account_id = ? AND key = ? AND block_height = ?", history_columns, history_table_name),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            timeline_desc: Self::prepare_query(
                &scylla_session,
                &format!("SELECT {} FROM s_kv_by_block WHERE predecessor_id = ? AND current_account_id = ? AND block_height >= ? AND block_height <= ? ORDER BY block_height DESC, key ASC", timeline_columns),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            timeline_asc: Self::prepare_query(
                &scylla_session,
                &format!("SELECT {} FROM s_kv_by_block WHERE predecessor_id = ? AND current_account_id = ? AND block_height >= ? AND block_height <= ? ORDER BY block_height ASC, key DESC", timeline_columns),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            // LocalQuorum for kv_accounts: this table is populated asynchronously so
            // LocalOne reads could return stale/partial results after recent writes.
            accounts_by_contract: Self::prepare_query(
                &scylla_session,
                &format!("SELECT predecessor_id FROM {} WHERE current_account_id = ?", kv_accounts_table_name),
                scylla::frame::types::Consistency::LocalQuorum,
            ).await?,
            accounts_by_contract_key: Self::prepare_query(
                &scylla_session,
                &format!("SELECT predecessor_id FROM {} WHERE current_account_id = ? AND key = ?", kv_accounts_table_name),
                scylla::frame::types::Consistency::LocalQuorum,
            ).await?,
            accounts_all: Self::prepare_query(
                &scylla_session,
                &format!("SELECT predecessor_id FROM {}", all_accounts_table_name),
                scylla::frame::types::Consistency::LocalQuorum,
            ).await?,
            accounts_all_cursor: Self::prepare_query(
                &scylla_session,
                &format!("SELECT predecessor_id FROM {} WHERE TOKEN(predecessor_id) > TOKEN(?)", all_accounts_table_name),
                scylla::frame::types::Consistency::LocalQuorum,
            ).await?,
            contracts_all: Self::prepare_query(
                &scylla_session,
                &format!("SELECT current_account_id FROM {}", kv_accounts_table_name),
                scylla::frame::types::Consistency::LocalQuorum,
            ).await?,
            contracts_all_cursor: Self::prepare_query(
                &scylla_session,
                &format!("SELECT current_account_id FROM {} WHERE TOKEN(current_account_id) > TOKEN(?)", kv_accounts_table_name),
                scylla::frame::types::Consistency::LocalQuorum,
            ).await?,
            contracts_by_account: Self::prepare_query(
                &scylla_session,
                &format!("SELECT current_account_id, key FROM {} WHERE predecessor_id = ?", table_name),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            edges_list: Self::prepare_query(
                &scylla_session,
                &format!("SELECT source, block_height FROM {} WHERE edge_type = ? AND target = ?", kv_edges_table_name),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            edges_list_cursor: Self::prepare_query(
                &scylla_session,
                &format!("SELECT source, block_height FROM {} WHERE edge_type = ? AND target = ? AND source > ?", kv_edges_table_name),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            edges_count: Self::prepare_query(
                &scylla_session,
                &format!("SELECT COUNT(*) FROM {} WHERE edge_type = ? AND target = ?", kv_edges_table_name),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            prefix_query: Self::prepare_query(
                &scylla_session,
                &format!("SELECT {} FROM {} WHERE predecessor_id = ? AND current_account_id = ? AND key >= ? AND key < ?", columns, table_name),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            prefix_cursor_query: Self::prepare_query(
                &scylla_session,
                &format!("SELECT {} FROM {} WHERE predecessor_id = ? AND current_account_id = ? AND key > ? AND key < ?", columns, table_name),
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            meta_query: Self::prepare_query(
                &scylla_session,
                "SELECT last_processed_block_height FROM meta WHERE suffix = ?",
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            scylla_session,
            table_name,
            history_table_name,
            reverse_view_name,
            kv_accounts_table_name,
            all_accounts_table_name,
            kv_edges_table_name,
            kv_reverse_table_name,
        })
    }

    pub async fn prepare_query(
        scylla_db_session: &Session,
        query_text: &str,
        consistency: scylla::frame::types::Consistency,
    ) -> anyhow::Result<PreparedStatement> {
        let mut query = scylla::statement::Statement::new(query_text);
        query.set_consistency(consistency);
        query.set_request_timeout(Some(std::time::Duration::from_secs(10)));
        Ok(scylla_db_session.prepare(query).await?)
    }

    pub async fn get_kv(
        &self,
        predecessor_id: &str,
        current_account_id: &str,
        key: &str,
    ) -> anyhow::Result<Option<KvEntry>> {
        let result = self
            .scylla_session
            .execute_unpaged(&self.get_kv, (predecessor_id, current_account_id, key))
            .await?
            .into_rows_result()?;

        let entry = result
            .rows::<KvRow>()?
            .next()
            .transpose()?
            .map(KvEntry::from);

        Ok(entry)
    }

    pub async fn get_kv_last(
        &self,
        predecessor_id: &str,
        current_account_id: &str,
        key: &str,
    ) -> anyhow::Result<Option<String>> {
        let result = self
            .scylla_session
            .execute_unpaged(&self.get_kv_last, (predecessor_id, current_account_id, key))
            .await?
            .into_rows_result()?;

        let value = result
            .rows::<(Option<String>,)>()?
            .next()
            .transpose()?
            .and_then(|row| row.0);

        Ok(value)
    }

    /// Query writers for a key under a contract using the kv_reverse table.
    /// predecessor_id is the clustering key so rows are naturally deduplicated.
    /// Supports cursor pagination via `after_account`.
    /// Optionally filters to a specific writer (predecessor_id).
    /// Returns (entries, has_more, truncated, dropped_rows).
    pub async fn query_writers(
        &self,
        params: &WritersParams,
    ) -> anyhow::Result<(Vec<KvEntry>, bool, bool, usize)> {
        let mut rows_stream = match &params.after_account {
            Some(cursor) => self
                .scylla_session
                .execute_iter(
                    self.reverse_list_cursor.clone(),
                    (&params.current_account_id, &params.key, cursor),
                )
                .await?
                .rows_stream::<KvRow>()?,
            None => self
                .scylla_session
                .execute_iter(
                    self.reverse_list.clone(),
                    (&params.current_account_id, &params.key),
                )
                .await?
                .rows_stream::<KvRow>()?,
        };

        let exclude_deleted = params.exclude_deleted.unwrap_or(false);
        let pred_filter = params.predecessor_id.clone();
        let offset = effective_offset(params.after_account.as_deref(), params.offset);
        let page = collect_page(
            &mut rows_stream,
            params.limit,
            offset,
            None,
            |row: KvRow| {
                let entry = KvEntry::from(row);
                if let Some(ref pred) = pred_filter {
                    if entry.predecessor_id != *pred {
                        return None;
                    }
                }
                if exclude_deleted && entry.value == "null" {
                    return None;
                }
                Some(entry)
            },
        )
        .await;

        Ok((page.items, page.has_more, page.truncated, page.dropped_rows))
    }

    pub async fn query_accounts(
        &self,
        params: &AccountsParams,
    ) -> anyhow::Result<(Vec<String>, bool, usize)> {
        // Use kv_reverse table for CQL-level cursor pagination
        let mut rows_stream = match &params.after_account {
            Some(cursor) => self
                .scylla_session
                .execute_iter(
                    self.reverse_list_cursor.clone(),
                    (&params.current_account_id, &params.key, cursor),
                )
                .await?
                .rows_stream::<KvRow>()?,
            None => self
                .scylla_session
                .execute_iter(
                    self.reverse_list.clone(),
                    (&params.current_account_id, &params.key),
                )
                .await?
                .rows_stream::<KvRow>()?,
        };

        let exclude_deleted = params.exclude_deleted.unwrap_or(false);
        let offset = effective_offset(params.after_account.as_deref(), params.offset);
        let page = collect_page(
            &mut rows_stream,
            params.limit,
            offset,
            None,
            |row: KvRow| {
                if exclude_deleted && row.value == "null" {
                    return None;
                }
                Some(row.predecessor_id)
            },
        )
        .await;

        Ok((page.items, page.has_more, page.dropped_rows))
    }

    /// Returns `(accounts, has_more, truncated, dropped_rows)`.
    /// `truncated` is true if scan hit MAX_DEDUP_SCAN.
    pub async fn query_accounts_by_contract(
        &self,
        contract_id: &str,
        key: Option<&str>,
        limit: usize,
        offset: usize,
        after_account: Option<&str>,
    ) -> anyhow::Result<(Vec<String>, bool, bool, usize)> {
        let needs_dedup = key.is_none();

        let mut rows_stream = match key {
            Some(k) => self
                .scylla_session
                .execute_iter(self.accounts_by_contract_key.clone(), (contract_id, k))
                .await?
                .rows_stream::<ContractAccountRow>()?,
            None => self
                .scylla_session
                .execute_iter(self.accounts_by_contract.clone(), (contract_id,))
                .await?
                .rows_stream::<ContractAccountRow>()?,
        };

        let mut seen = HashSet::new();
        let mut accounts = Vec::new();
        let target_count = offset + limit + 1;
        let mut truncated = false;
        let mut dropped_rows = 0usize;

        while let Some(row_result) = rows_stream.next().await {
            if needs_dedup && seen.len() >= MAX_DEDUP_SCAN {
                truncated = true;
                break;
            }

            let row = match row_result {
                Ok(row) => row,
                Err(e) => {
                    dropped_rows += 1;
                    tracing::warn!(
                        target: "fastkv-server",
                        error = %e,
                        "Failed to deserialize row in query_accounts_by_contract"
                    );
                    continue;
                }
            };

            if needs_dedup {
                if seen.contains(&row.predecessor_id) {
                    continue;
                }
                seen.insert(row.predecessor_id.clone());
            }

            // Apply cursor: skip accounts <= after_account
            if let Some(cursor) = after_account {
                if row.predecessor_id.as_str() <= cursor {
                    continue;
                }
            }

            accounts.push(row.predecessor_id);

            if accounts.len() >= target_count {
                break;
            }
        }

        let result: Vec<String> = if after_account.is_some() {
            // When using cursor, no offset skipping needed
            accounts
        } else {
            accounts.into_iter().skip(offset).collect()
        };
        let has_more = result.len() > limit;
        let mut result = result;
        result.truncate(limit);

        Ok((result, has_more, truncated, dropped_rows))
    }

    /// Query the dedicated `all_accounts` table (one row per unique account).
    /// Uses TOKEN-based cursor for stable pagination across partitions.
    ///
    /// **Known limitation:** if two distinct account IDs share a Murmur3 token
    /// and the cursor equals that token, `TOKEN(pk) > TOKEN(cursor)` will skip
    /// any other keys at the same token position. Astronomically unlikely in a
    /// 64-bit token space.
    ///
    /// Returns `(accounts, has_more, dropped_rows)`.
    pub async fn query_all_accounts(
        &self,
        limit: usize,
        after_account: Option<&str>,
    ) -> anyhow::Result<(Vec<String>, bool, usize)> {
        let mut rows_stream = match after_account {
            Some(cursor) => self
                .scylla_session
                .execute_iter(self.accounts_all_cursor.clone(), (cursor,))
                .await?
                .rows_stream::<ContractAccountRow>()?,
            None => self
                .scylla_session
                .execute_iter(self.accounts_all.clone(), &[])
                .await?
                .rows_stream::<ContractAccountRow>()?,
        };

        // Defensive guard: drop the cursor value if it reappears in results.
        // The true token-tie limitation (a *different* key sharing the same
        // token being skipped) cannot be solved at this layer — see doc above.
        let mut skipped_cursor = false;
        let page = collect_page(
            &mut rows_stream,
            limit,
            0,    // no offset — cursor handles resumption
            None, // overfetch mode
            |row: ContractAccountRow| {
                if !skipped_cursor {
                    if let Some(c) = after_account {
                        if row.predecessor_id == c {
                            skipped_cursor = true;
                            tracing::debug!(
                                target: "fastkv-server",
                                cursor = c,
                                "Dropped cursor row reappearance"
                            );
                            return None;
                        }
                    }
                }
                Some(row.predecessor_id)
            },
        )
        .await;

        Ok((page.items, page.has_more, page.dropped_rows))
    }

    /// Returns true if the global contracts scan feature is available
    /// (i.e., the SELECT DISTINCT prepared statements succeeded).
    /// Query all distinct contract IDs from the `kv_accounts` table.
    /// Uses TOKEN-based cursor for stable pagination across partitions.
    /// Deduplicates consecutive rows with the same `current_account_id`.
    ///
    /// Returns `(contracts, has_more, dropped_rows)`.
    pub async fn query_all_contracts(
        &self,
        limit: usize,
        after_contract: Option<&str>,
    ) -> anyhow::Result<(Vec<String>, bool, usize)> {
        let mut rows_stream = match after_contract {
            Some(cursor) => self
                .scylla_session
                .execute_iter(self.contracts_all_cursor.clone(), (cursor,))
                .await?
                .rows_stream::<ContractRow>()?,
            None => self
                .scylla_session
                .execute_iter(self.contracts_all.clone(), &[])
                .await?
                .rows_stream::<ContractRow>()?,
        };

        let after = after_contract.map(|s| s.to_string());
        let mut past_cursor = after.is_none();
        let mut last_contract: Option<String> = None;
        let page = collect_page(
            &mut rows_stream,
            limit,
            0,    // no offset — cursor handles resumption
            None, // overfetch mode
            |row: ContractRow| {
                // Deduplicate: rows within a partition share the same current_account_id
                if let Some(ref prev) = last_contract {
                    if row.current_account_id == *prev {
                        return None;
                    }
                }
                last_contract = Some(row.current_account_id.clone());

                if !past_cursor {
                    if let Some(ref c) = after {
                        if row.current_account_id == *c {
                            return None; // skip cursor row reappearance
                        }
                    }
                    past_cursor = true;
                }
                Some(row.current_account_id)
            },
        )
        .await;

        Ok((page.items, page.has_more, page.dropped_rows))
    }

    /// Query distinct contracts that a specific account has written to.
    /// Uses `s_kv_last` where `predecessor_id` is the partition key, so this
    /// is a cheap single-partition query.
    ///
    /// Returns `(contracts, has_more, dropped_rows)`.
    pub async fn query_contracts_by_account(
        &self,
        account_id: &str,
        limit: usize,
        after_contract: Option<&str>,
    ) -> anyhow::Result<(Vec<String>, bool, usize)> {
        let mut rows_stream = self
            .scylla_session
            .execute_iter(self.contracts_by_account.clone(), (account_id,))
            .await?
            .rows_stream::<ContractKeyRow>()?;

        let after = after_contract.map(|s| s.to_string());
        let mut past_cursor = after.is_none();
        let mut last_contract: Option<String> = None;
        let page = collect_page(
            &mut rows_stream,
            limit,
            0,
            None, // overfetch mode
            |row: ContractKeyRow| {
                // Deduplicate: rows are ordered by current_account_id ASC
                if let Some(ref prev) = last_contract {
                    if row.current_account_id == *prev {
                        return None;
                    }
                }
                last_contract = Some(row.current_account_id.clone());

                if !past_cursor {
                    if let Some(ref c) = after {
                        if row.current_account_id <= *c {
                            return None; // skip until past cursor
                        }
                    }
                    past_cursor = true;
                }
                Some(row.current_account_id)
            },
        )
        .await;

        Ok((page.items, page.has_more, page.dropped_rows))
    }

    /// Returns (entries, has_more, dropped_rows).
    pub async fn query_kv_with_pagination(
        &self,
        params: &QueryParams,
    ) -> anyhow::Result<(Vec<KvEntry>, bool, usize)> {
        let mut rows_stream = match (&params.key_prefix, &params.after_key) {
            // Prefix + cursor: key > cursor AND key < prefix_end
            (Some(prefix), Some(cursor)) => {
                let prefix_end = compute_prefix_end(prefix);
                self.scylla_session
                    .execute_iter(
                        self.prefix_cursor_query.clone(),
                        (
                            &params.predecessor_id,
                            &params.current_account_id,
                            cursor,
                            &prefix_end,
                        ),
                    )
                    .await?
                    .rows_stream::<KvRow>()?
            }
            // Prefix only: key >= prefix AND key < prefix_end
            (Some(prefix), None) => {
                let prefix_end = compute_prefix_end(prefix);
                self.scylla_session
                    .execute_iter(
                        self.prefix_query.clone(),
                        (
                            &params.predecessor_id,
                            &params.current_account_id,
                            prefix.as_str(),
                            &prefix_end,
                        ),
                    )
                    .await?
                    .rows_stream::<KvRow>()?
            }
            // No prefix + cursor: key > cursor
            (None, Some(cursor)) => self
                .scylla_session
                .execute_iter(
                    self.query_kv_cursor.clone(),
                    (&params.predecessor_id, &params.current_account_id, cursor),
                )
                .await?
                .rows_stream::<KvRow>()?,
            // No prefix, no cursor: all keys
            (None, None) => self
                .scylla_session
                .execute_iter(
                    self.query_kv_no_prefix.clone(),
                    (&params.predecessor_id, &params.current_account_id),
                )
                .await?
                .rows_stream::<KvRow>()?,
        };

        let exclude_deleted = params.exclude_deleted.unwrap_or(false);
        let offset = effective_offset(params.after_key.as_deref(), params.offset);
        let page = collect_page(
            &mut rows_stream,
            params.limit,
            offset,
            None,
            |row: KvRow| {
                let entry = KvEntry::from(row);
                if exclude_deleted && entry.value == "null" {
                    return None;
                }
                Some(entry)
            },
        )
        .await;

        Ok((page.items, page.has_more, page.dropped_rows))
    }

    pub async fn get_kv_at_block(
        &self,
        predecessor_id: &str,
        current_account_id: &str,
        key: &str,
        block_height: i64,
    ) -> anyhow::Result<Option<KvEntry>> {
        let result = self
            .scylla_session
            .execute_unpaged(
                &self.get_kv_at_block,
                (predecessor_id, current_account_id, key, block_height),
            )
            .await?
            .into_rows_result()?;

        // Multiple rows can exist per block_height (different order_id values);
        // .last() takes the highest order_id since clustering order is ASC.
        let mut last_ok: Option<KvHistoryRow> = None;
        for row_result in result.rows::<KvHistoryRow>()? {
            match row_result {
                Ok(row) => last_ok = Some(row),
                Err(e) => {
                    tracing::warn!(
                        target: "fastkv-server",
                        error = %e,
                        "Failed to deserialize row in get_kv_at_block"
                    );
                }
            }
        }
        let entry = last_ok.map(KvEntry::from);

        Ok(entry)
    }

    pub async fn get_kv_timeline(
        &self,
        params: &TimelineParams,
    ) -> anyhow::Result<(Vec<KvEntry>, bool, usize, Option<String>)> {
        let is_asc = params.order.eq_ignore_ascii_case("asc");

        let cursor = match &params.cursor {
            Some(c) if !c.is_empty() => {
                let (cb, ck) = crate::models::parse_timeline_cursor(c)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                Some((cb, ck))
            }
            _ => None,
        };

        let mut from_block = params.from_block.unwrap_or(0);
        let mut to_block = params.to_block.unwrap_or(i64::MAX);
        if let Some((cb, _)) = &cursor {
            if is_asc {
                from_block = from_block.max(*cb);
            } else {
                to_block = to_block.min(*cb);
            }
        }

        let stmt = if is_asc {
            self.timeline_asc.clone()
        } else {
            self.timeline_desc.clone()
        };

        let mut rows_stream = self
            .scylla_session
            .execute_iter(
                stmt,
                (&params.predecessor_id, &params.current_account_id, from_block, to_block),
            )
            .await?
            .rows_stream::<KvTimelineRow>()?;

        let page = collect_page(
            &mut rows_stream,
            params.limit,
            0,
            None,
            |row: KvTimelineRow| {
                if let Some((cb, ref ck)) = cursor {
                    if row.block_height == cb {
                        if is_asc {
                            if row.key.as_str() >= ck.as_str() {
                                return None;
                            }
                        } else if row.key.as_str() <= ck.as_str() {
                            return None;
                        }
                    }
                }
                let key = row.key.clone();
                Some((KvEntry::from(row), key))
            },
        )
        .await;

        let next_cursor = page
            .items
            .last()
            .map(|(e, key)| format!("{}:{key}", e.block_height));
        let entries: Vec<KvEntry> = page.items.into_iter().map(|(e, _)| e).collect();

        Ok((entries, page.has_more, page.dropped_rows, next_cursor))
    }

    /// Returns (entries, has_more, dropped_rows).
    pub async fn query_edges(
        &self,
        edge_type: &str,
        target: &str,
        limit: usize,
        offset: usize,
        after_source: Option<&str>,
    ) -> anyhow::Result<(Vec<EdgeSourceEntry>, bool, usize)> {
        let mut rows_stream = match after_source {
            Some(cursor) => self
                .scylla_session
                .execute_iter(self.edges_list_cursor.clone(), (edge_type, target, cursor))
                .await?
                .rows_stream::<EdgeRow>()?,
            None => self
                .scylla_session
                .execute_iter(self.edges_list.clone(), (edge_type, target))
                .await?
                .rows_stream::<EdgeRow>()?,
        };

        let effective_offset = if after_source.is_some() { 0 } else { offset };
        let page = collect_page(
            &mut rows_stream,
            limit,
            effective_offset,
            None,
            |row: EdgeRow| {
                Some(EdgeSourceEntry {
                    source: row.source,
                    block_height: bigint_to_u64(row.block_height),
                })
            },
        )
        .await;

        Ok((page.items, page.has_more, page.dropped_rows))
    }

    pub async fn count_edges(&self, edge_type: &str, target: &str) -> anyhow::Result<usize> {
        let result = self
            .scylla_session
            .execute_unpaged(&self.edges_count, (edge_type, target))
            .await?
            .into_rows_result()?;

        let count = result
            .rows::<(i64,)>()?
            .next()
            .transpose()?
            .map(|row| row.0.max(0) as usize)
            .unwrap_or(0);

        Ok(count)
    }

    pub async fn get_kv_history(
        &self,
        params: &HistoryParams,
    ) -> anyhow::Result<(Vec<KvEntry>, bool, usize, Option<String>)> {
        let is_asc = params.order.eq_ignore_ascii_case("asc");

        let cursor = match &params.cursor {
            Some(c) if !c.is_empty() => {
                let (cb, co) = crate::models::parse_history_cursor(c)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                Some((cb, co))
            }
            _ => None,
        };

        let mut from_block = params.from_block.unwrap_or(0);
        let mut to_block = params.to_block.unwrap_or(i64::MAX);
        if let Some((cb, _)) = cursor {
            if is_asc {
                from_block = from_block.max(cb);
            } else {
                to_block = to_block.min(cb);
            }
        }

        let stmt = if is_asc {
            self.history_asc.clone()
        } else {
            self.history_desc.clone()
        };

        let mut rows_stream = self
            .scylla_session
            .execute_iter(
                stmt,
                (
                    &params.predecessor_id,
                    &params.current_account_id,
                    &params.key,
                    from_block,
                    to_block,
                ),
            )
            .await?
            .rows_stream::<KvHistoryRow>()?;

        let page = collect_page(
            &mut rows_stream,
            params.limit,
            0,
            None,
            |row: KvHistoryRow| {
                if let Some((cb, co)) = cursor {
                    if row.block_height == cb {
                        if is_asc {
                            if row.order_id <= co {
                                return None;
                            }
                        } else if row.order_id >= co {
                            return None;
                        }
                    }
                }
                let oid = row.order_id;
                Some((KvEntry::from(row), oid))
            },
        )
        .await;

        let next_cursor = page
            .items
            .last()
            .map(|(e, oid)| format!("{}:{oid}", e.block_height));
        let entries: Vec<KvEntry> = page.items.into_iter().map(|(e, _)| e).collect();

        Ok((entries, page.has_more, page.dropped_rows, next_cursor))
    }

    pub async fn get_indexer_block_height(&self) -> anyhow::Result<Option<u64>> {
        let result = self
            .scylla_session
            .execute_unpaged(&self.meta_query, ("kv-1",))
            .await?
            .into_rows_result()?;

        let block_height = result
            .rows::<(i64,)>()?
            .next()
            .transpose()?
            .map(|row| row.0.max(0) as u64);

        Ok(block_height)
    }
}

fn compute_prefix_end(prefix: &str) -> String {
    format!("{prefix}\u{10ffff}")
}

fn effective_offset(cursor: Option<&str>, offset: usize) -> usize {
    if cursor.is_some() { 0 } else { offset }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_identifier_accepts_valid() {
        assert!(validate_identifier("fastdata_mainnet", "TEST").is_ok());
        assert!(validate_identifier("s_kv_last", "TEST").is_ok());
        assert!(validate_identifier("mv_kv_by_contract", "TEST").is_ok());
        assert!(validate_identifier("A1_b2", "TEST").is_ok());
    }

    #[test]
    fn test_validate_identifier_rejects_injection() {
        assert!(validate_identifier("; DROP TABLE users", "TEST").is_err());
        assert!(validate_identifier("table; --", "TEST").is_err());
        assert!(validate_identifier("name with spaces", "TEST").is_err());
        assert!(validate_identifier("", "TEST").is_err());
        assert!(validate_identifier("table.name", "TEST").is_err());
        assert!(validate_identifier("name-with-dashes", "TEST").is_err());
    }

    fn make_err() -> NextRowError {
        NextRowError::from(scylla::deserialize::DeserializationError::new(
            std::io::Error::other("test deser error"),
        ))
    }

    #[tokio::test]
    async fn test_collect_page_overfetch() {
        let items: Vec<Result<i32, NextRowError>> = (1..=6).map(Ok).collect();
        let mut s = futures::stream::iter(items);
        let page = collect_page(&mut s, 5, 0, None, Some).await;
        assert_eq!(page.items.len(), 5);
        assert!(page.has_more);
        assert!(!page.truncated);
        assert_eq!(page.dropped_rows, 0);
    }

    #[tokio::test]
    async fn test_collect_page_under_limit() {
        let items: Vec<Result<i32, NextRowError>> = (1..=3).map(Ok).collect();
        let mut s = futures::stream::iter(items);
        let page = collect_page(&mut s, 5, 0, None, Some).await;
        assert_eq!(page.items.len(), 3);
        assert!(!page.has_more);
    }

    #[tokio::test]
    async fn test_collect_page_with_offset() {
        let items: Vec<Result<i32, NextRowError>> = (1..=10).map(Ok).collect();
        let mut s = futures::stream::iter(items);
        let page = collect_page(&mut s, 5, 3, None, Some).await;
        assert_eq!(page.items, vec![4, 5, 6, 7, 8]);
        assert!(page.has_more); // item 9 caused overfetch
    }

    #[tokio::test]
    async fn test_collect_page_dropped_rows() {
        let items: Vec<Result<i32, NextRowError>> =
            vec![Ok(1), Err(make_err()), Ok(2), Err(make_err()), Ok(3)];
        let mut s = futures::stream::iter(items);
        let page = collect_page(&mut s, 10, 0, None, Some).await;
        assert_eq!(page.items, vec![1, 2, 3]);
        assert_eq!(page.dropped_rows, 2);
        assert!(!page.has_more);
    }

    #[tokio::test]
    async fn test_collect_page_scan_cap() {
        let items: Vec<Result<i32, NextRowError>> = (1..=20).map(Ok).collect();
        let mut s = futures::stream::iter(items);
        let page = collect_page(&mut s, 100, 0, Some(10), Some).await;
        assert!(page.truncated);
        assert_eq!(page.items.len(), 10);
        assert!(!page.has_more); // caller computes in scan-cap mode
    }

    #[tokio::test]
    async fn test_collect_page_filter_via_transform() {
        // Only keep odd numbers; offset should count filtered items
        let items: Vec<Result<i32, NextRowError>> = (1..=10).map(Ok).collect();
        let mut s = futures::stream::iter(items);
        let page = collect_page(
            &mut s,
            3,
            1,
            None,
            |n| {
                if n % 2 == 1 {
                    Some(n)
                } else {
                    None
                }
            },
        )
        .await;
        // Odd items: 1, 3, 5, 7, 9. Skip 1 (offset), take 3+1: [3,5,7,9]
        assert_eq!(page.items, vec![3, 5, 7]);
        assert!(page.has_more);
    }

    #[tokio::test]
    async fn test_collect_page_empty_stream() {
        let items: Vec<Result<i32, NextRowError>> = vec![];
        let mut s = futures::stream::iter(items);
        let page = collect_page(&mut s, 10, 0, None, Some).await;
        assert!(page.items.is_empty());
        assert!(!page.has_more);
        assert!(!page.truncated);
        assert_eq!(page.dropped_rows, 0);
    }

    #[tokio::test]
    async fn test_collect_page_all_filtered() {
        // Every item filtered out by transform — result is empty, no has_more
        let items: Vec<Result<i32, NextRowError>> = (1..=10).map(Ok).collect();
        let mut s = futures::stream::iter(items);
        let page: PageResult<i32> = collect_page(&mut s, 5, 0, None, |_| None).await;
        assert!(page.items.is_empty());
        assert!(!page.has_more);
        assert!(!page.truncated);
        assert_eq!(page.dropped_rows, 0);
    }

    #[test]
    fn test_compute_prefix_end() {
        assert_eq!(compute_prefix_end("graph/follow/"), "graph/follow/\u{10ffff}");
        assert_eq!(compute_prefix_end("test"), "test\u{10ffff}");
        assert_eq!(compute_prefix_end(""), "\u{10ffff}");
    }
}
