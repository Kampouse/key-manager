use futures::StreamExt;
mod types;
mod utils;

pub use crate::types::{FastData, UNIVERSAL_SUFFIX};
pub use crate::utils::*;
use scylla::client::session::Session;
use scylla::client::session_builder::SessionBuilder;
use scylla::errors::SingleRowError;
use scylla::statement::prepared::PreparedStatement;

use crate::types::{FastDataRow, BLOCK_HEIGHT_BUCKET_SIZE};
use fastnear_primitives::types::ChainId;
use futures::{stream, Stream};
use rustls::pki_types::pem::PemObject;
use rustls::{ClientConfig, RootCertStore};
use std::env;
use std::sync::Arc;

pub const SCYLLADB: &str = "scylladb";

pub struct ScyllaDb {
    pub insert_fastdata_query: PreparedStatement,
    pub select_fastdata_query_by_suffix_from: PreparedStatement,
    pub insert_last_processed_block_height_query: PreparedStatement,
    pub select_last_processed_block_height_query: PreparedStatement,

    pub scylla_session: Session,
}

pub fn create_rustls_client_config() -> Arc<ClientConfig> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .expect("Failed to install default provider");
    }
    let ca_cert_path =
        env::var("SCYLLA_SSL_CA").expect("SCYLLA_SSL_CA environment variable not set");
    let client_cert_path = env::var("SCYLLA_SSL_CERT").ok();
    let client_key_path = env::var("SCYLLA_SSL_KEY").ok();

    let ca_certs = rustls::pki_types::CertificateDer::from_pem_file(ca_cert_path)
        .expect("Failed to load CA certs");

    let mut root_store = RootCertStore::empty();
    root_store.add(ca_certs).expect("Failed to add CA certs");

    let config = match (client_cert_path, client_key_path) {
        (Some(cert_path), Some(key_path)) => {
            // mTLS: Load client certificate and key
            let client_certs = rustls::pki_types::CertificateDer::from_pem_file(cert_path)
                .expect("Failed to load client certs");
            let client_key = rustls::pki_types::PrivateKeyDer::from_pem_file(key_path)
                .expect("Failed to load client key");

            ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_client_auth_cert(vec![client_certs], client_key)
                .expect("Failed to create client config")
        }
        _ => {
            // Server-only TLS: No client certificate (e.g., ScyllaDB Cloud with password auth)
            ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth()
        }
    };

    Arc::new(config)
}

impl ScyllaDb {
    pub async fn new_scylla_session() -> anyhow::Result<Session> {
        let scylla_url = env::var("SCYLLA_URL").expect("SCYLLA_URL must be set");
        let scylla_username = env::var("SCYLLA_USERNAME").expect("SCYLLA_USERNAME must be set");
        let scylla_password = env::var("SCYLLA_PASSWORD").expect("SCYLLA_PASSWORD must be set");

        let tls_config = env::var("SCYLLA_SSL_CA").ok().map(|_| create_rustls_client_config());
        if tls_config.is_none() {
            tracing::warn!("SCYLLA_SSL_CA not set - ScyllaDB connection is unencrypted");
        }

        let mut builder = SessionBuilder::new();
        for node in scylla_url.split(',') {
            let node = node.trim();
            if !node.is_empty() {
                builder = builder.known_node(node);
            }
        }
        let session: Session = builder
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
        scylla_session
            .query_unpaged("SELECT now() FROM system.local", &[])
            .await?;
        Ok(())
    }

    pub async fn new(
        chain_id: ChainId,
        scylla_session: Session,
        create_tables: bool,
    ) -> anyhow::Result<Self> {
        scylla_session
            .use_keyspace(format!("fastdata_{chain_id}"), false)
            .await?;
        if create_tables {
            Self::create_tables(&scylla_session).await?;
        }

        Ok(Self {
            insert_fastdata_query: Self::prepare_query(
                &scylla_session,
                "INSERT INTO blobs (receipt_id, action_index, suffix, block_height_bucket, data, tx_hash, signer_id, predecessor_id, current_account_id, block_height, block_timestamp, shard_id, receipt_index) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                scylla::frame::types::Consistency::LocalQuorum,
            )
            .await?,
            select_fastdata_query_by_suffix_from: Self::prepare_query(
                &scylla_session,
                "SELECT * FROM blobs WHERE suffix = ? AND block_height_bucket = ? AND block_height >= ? AND block_height <= ?",
                scylla::frame::types::Consistency::LocalOne,
            ).await?,
            insert_last_processed_block_height_query: Self::prepare_query(
                &scylla_session,
                "INSERT INTO meta (suffix, last_processed_block_height) VALUES (?, ?)",
                scylla::frame::types::Consistency::LocalQuorum,
            ).await?,
            select_last_processed_block_height_query: Self::prepare_query(
                &scylla_session,
                "SELECT last_processed_block_height FROM meta WHERE suffix = ? LIMIT 1",
                scylla::frame::types::Consistency::LocalQuorum,
            )
            .await?,
            scylla_session,
        })
    }

    pub async fn prepare_query(
        scylla_db_session: &Session,
        query_text: &str,
        consistency: scylla::frame::types::Consistency,
    ) -> anyhow::Result<PreparedStatement> {
        let mut query = scylla::statement::Statement::new(query_text);
        query.set_consistency(consistency);
        Ok(scylla_db_session.prepare(query).await?)
    }

    pub async fn create_tables(scylla_session: &Session) -> anyhow::Result<()> {
        let queries = [
            "CREATE TABLE IF NOT EXISTS blobs (
                receipt_id text,
                action_index int,
                suffix text,
                block_height_bucket bigint,
                data blob,
                tx_hash text,
                signer_id text,
                predecessor_id text,
                current_account_id text,
                block_height bigint,
                block_timestamp bigint,
                shard_id int,
                receipt_index int,
                PRIMARY KEY ((suffix, block_height_bucket), block_height, shard_id, receipt_index, action_index, receipt_id)
            ) WITH default_time_to_live = 2592000", // 30 days TTL — blobs are only needed for sub-indexer replay
            // Secondary indexes on high-cardinality columns removed (ScyllaDB anti-pattern).
            // Use dedicated lookup tables if tx_hash/receipt_id queries are needed.
            "CREATE TABLE IF NOT EXISTS meta (
                suffix text PRIMARY KEY,
                last_processed_block_height bigint
            )",
        ];
        for query in queries.iter() {
            tracing::debug!(target: SCYLLADB, "Creating table: {}", query);
            scylla_session.query_unpaged(*query, &[]).await?;
        }
        Ok(())
    }

    pub async fn add_data(&self, fastdata: FastData) -> anyhow::Result<()> {
        self.scylla_session
            .execute_unpaged(&self.insert_fastdata_query, FastDataRow::from(fastdata))
            .await?;
        Ok(())
    }

    /// Fetches all fast data for a given suffix and block height range (inclusive both ends).
    /// Queries each block_height_bucket partition separately and chains results in order.
    pub async fn get_suffix_data(
        &self,
        suffix: &str,
        from_block_height: u64,
        to_block_height: u64,
    ) -> anyhow::Result<impl Stream<Item = anyhow::Result<FastData>>> {
        let from_bucket = from_block_height / BLOCK_HEIGHT_BUCKET_SIZE;
        let to_bucket = to_block_height / BLOCK_HEIGHT_BUCKET_SIZE;

        const MAX_BUCKET_SPAN: u64 = 1_000; // 10M blocks max per query
        if to_bucket.saturating_sub(from_bucket) > MAX_BUCKET_SPAN {
            return Err(anyhow::anyhow!(
                "Block range too large: {} buckets (max {}). from={}, to={}",
                to_bucket - from_bucket, MAX_BUCKET_SPAN, from_block_height, to_block_height
            ));
        }

        let mut bucket_streams = Vec::new();
        for bucket in from_bucket..=to_bucket {
            let rows_stream = self
                .scylla_session
                .execute_iter(
                    self.select_fastdata_query_by_suffix_from.clone(),
                    (
                        suffix.to_string(),
                        i64::try_from(bucket).expect("bucket exceeds i64"),
                        i64::try_from(from_block_height).expect("from_block_height exceeds i64"),
                        i64::try_from(to_block_height).expect("to_block_height exceeds i64"),
                    ),
                )
                .await?
                .rows_stream::<FastDataRow>()?;
            bucket_streams.push(rows_stream);
        }

        // Chain bucket streams sequentially to maintain block_height ordering
        Ok(stream::iter(bucket_streams).flatten().map(|row| match row {
            Ok(row) => row.try_into(),
            Err(e) => Err(anyhow::anyhow!("Database error: {:?}", e)),
        }))
    }

    pub async fn set_last_processed_block_height(
        &self,
        suffix: &str,
        last_processed_block_height: u64,
    ) -> anyhow::Result<()> {
        self.scylla_session
            .execute_unpaged(
                &self.insert_last_processed_block_height_query,
                (suffix.to_string(), i64::try_from(last_processed_block_height).expect("block_height exceeds i64")),
            )
            .await?;
        Ok(())
    }

    pub async fn get_last_processed_block_height(
        &self,
        suffix: &str,
    ) -> anyhow::Result<Option<u64>> {
        let rows = self
            .scylla_session
            .execute_unpaged(
                &self.select_last_processed_block_height_query,
                (suffix.to_string(),),
            )
            .await?
            .into_rows_result()?;
        match rows.single_row::<(i64,)>() {
            Ok((v,)) => {
                let height = u64::try_from(v)
                    .map_err(|_| anyhow::anyhow!("Negative checkpoint in DB: {}", v))?;
                Ok(Some(height))
            }
            Err(SingleRowError::UnexpectedRowCount(0)) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Checkpoint read error: {:?}", e)),
        }
    }
}
