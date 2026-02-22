// Quick script to create missing ScyllaDB tables
use scylla::client::session_builder::SessionBuilder;
use scylla::authentication::PlainTextAuthenticator;
use std::sync::Arc;
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let host = env::var("SCYLLA_URL").unwrap_or_else(|_| "127.0.0.1:9042".to_string());
    
    let username = env::var("SCYLLA_USERNAME").unwrap_or_else(|_| "cassandra".to_string());
    let password = env::var("SCYLLA_PASSWORD").unwrap_or_else(|_| "cassandra".to_string());
    
    let chain_id = env::var("CHAIN_ID").unwrap_or_else(|_| "mainnet".to_string());
    let keyspace = env::var("KEYSPACE").unwrap_or_else(|_| format!("fastdata_{}", chain_id));
    
    println!("Connecting to ScyllaDB at {}", host);
    println!("Keyspace: {}", keyspace);
    
    // Install crypto provider (needed even without TLS for scylla driver)
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .expect("Failed to install default provider");
    }
    
    // No TLS - Railway TCP proxy doesn't support TLS for CQL protocol
    let session = SessionBuilder::new()
        .known_node(&host)
        .connection_timeout(std::time::Duration::from_secs(10))
        .authenticator_provider(Arc::new(PlainTextAuthenticator::new(username, password)))
        .build()
        .await?;
    
    println!("Connected!");
    
    // Create keyspace if not exists
    let ks_query = format!(
        "CREATE KEYSPACE IF NOT EXISTS {} WITH replication = {{'class': 'SimpleStrategy', 'replication_factor': 1}}",
        keyspace
    );
    println!("Creating keyspace: {}", keyspace);
    session.query_unpaged(ks_query.clone(), &[]).await?;
    session.use_keyspace(&keyspace, false).await?;
    println!("Using keyspace: {}", keyspace);
    
    let queries = [
        ("s_kv", "CREATE TABLE IF NOT EXISTS s_kv (
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
        )"),
        ("s_kv_last", "CREATE TABLE IF NOT EXISTS s_kv_last (
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
        )"),
        ("mv_kv_key", "CREATE MATERIALIZED VIEW IF NOT EXISTS mv_kv_key AS
            SELECT * FROM s_kv
            WHERE key IS NOT NULL AND block_height IS NOT NULL AND order_id IS NOT NULL AND predecessor_id IS NOT NULL AND current_account_id IS NOT NULL
            PRIMARY KEY((key), block_height, order_id, predecessor_id, current_account_id)
        "),
        ("mv_kv_cur_key", "CREATE MATERIALIZED VIEW IF NOT EXISTS mv_kv_cur_key AS
            SELECT * FROM s_kv
            WHERE current_account_id IS NOT NULL AND key IS NOT NULL AND block_height IS NOT NULL AND order_id IS NOT NULL AND predecessor_id IS NOT NULL
            PRIMARY KEY((current_account_id), key, block_height, order_id, predecessor_id)
        "),
        ("kv_accounts", "CREATE TABLE IF NOT EXISTS kv_accounts (
            current_account_id text,
            key text,
            predecessor_id text,
            PRIMARY KEY ((current_account_id), key, predecessor_id)
        )"),
        ("kv_edges", "CREATE TABLE IF NOT EXISTS kv_edges (
            edge_type text,
            target text,
            source text,
            current_account_id text,
            block_height bigint,
            block_timestamp bigint,
            order_id bigint,
            value text,
            PRIMARY KEY ((edge_type, target), source)
        )"),
        ("kv_reverse", "CREATE TABLE IF NOT EXISTS kv_reverse (
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
        )"),
        ("all_accounts", "CREATE TABLE IF NOT EXISTS all_accounts (
            predecessor_id text PRIMARY KEY,
            last_block_height bigint,
            last_block_timestamp bigint
        )"),
        ("s_kv_by_block", "CREATE TABLE IF NOT EXISTS s_kv_by_block (
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
        ) WITH CLUSTERING ORDER BY (block_height DESC, key ASC)"),
    ];
    
    for (name, query) in queries.iter() {
        println!("Creating {}...", name);
        match session.query_unpaged(*query, &[]).await {
            Ok(_) => println!("  ✓ OK"),
            Err(e) => println!("  ✗ ERROR: {}", e),
        }
    }
    
    println!("Done!");
    
    // Fix replication factor for single-node
    println!("\nFixing replication factor for single-node...");
    let alter = format!(
        "ALTER KEYSPACE {} WITH replication = {{'class': 'SimpleStrategy', 'replication_factor': 1}}",
        keyspace
    );
    session.query_unpaged(alter, &[]).await?;
    println!("Done! Keyspace now uses replication_factor=1");
    
    Ok(())
}
