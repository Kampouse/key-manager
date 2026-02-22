# FastData Indexer

High-performance blockchain data pipeline for the NEAR FastData protocol. Captures `__fastdata_*` transactions from NEAR blockchain and processes them into queryable ScyllaDB tables.

## Architecture

```
NEAR Blockchain
      ↓
fastnear neardata API (HTTP)
      ↓
┌─────────────────────────┐
│    main-indexer         │ → blobs table (all FastData transactions)
│  (8 concurrent threads) │ → meta table (block height tracking)
└─────────────────────────┘
      ↓
┌─────────────────────────┐
│   suffix-fetcher        │ ← Data distribution library
│ (stream-based polling)  │
└─────────────────────────┘
      ↓                ↓
┌──────────────┐  ┌────────────────┐
│kv-sub-indexer│  │fastfs-sub-      │
│              │  │indexer          │
└──────────────┘  └────────────────┘
      ↓                ↓
s_kv, s_kv_last    s_fastfs_v2
mv_kv_key          (file chunks)
mv_kv_cur_key
kv_accounts
kv_reverse
kv_edges
```

## Quick Start

Get FastData Indexer running in 5 minutes.

### Prerequisites

- **Rust:** Install with `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **ScyllaDB:** Access to a ScyllaDB cluster (local or cloud)
- **Fastnear API:** Optional auth bearer token for neardata API

### Setup

**1. Clone and configure:**

```bash
git clone https://github.com/fastnear/fastdata-indexer.git
cd fastdata-indexer
cp .env.example .env
# Edit .env with your credentials
```

**2. Create ScyllaDB keyspace:**

Before running indexers, manually create the keyspace:

```sql
CREATE KEYSPACE IF NOT EXISTS fastdata_mainnet
WITH REPLICATION = {
    'class': 'NetworkTopologyStrategy',
    'dc1': 3
} AND TABLETS = {'enabled': true};
```

Replace `mainnet` with your `CHAIN_ID` value.

**3. Build:**

```bash
cargo build --release
```

**4. Run main-indexer:**

```bash
./target/release/main-indexer
```

**5. In separate terminals, run sub-indexers:**

```bash
./target/release/kv-sub-indexer
./target/release/fastfs-sub-indexer
```

**6. Verify it's working:**

```bash
# Check logs for "Received block" messages
# Query ScyllaDB:
cqlsh -u scylla -p <password> <host>
> SELECT * FROM fastdata_mainnet.meta;
```

See [Development](#development) section for detailed setup instructions.

---

## Components

### Core Infrastructure

#### scylladb (Shared Database Layer)

Core database abstraction providing connection management and schema definitions.

**Features:**

- TLS/mTLS authentication support
- Schema creation and migration
- Prepared statement management
- Keyspace: `fastdata_{CHAIN_ID}` (separate per chain)
- Replication: `NetworkTopologyStrategy` with 3x replication in dc1
- Tablets: Enabled for horizontal scalability

**Consistency Levels:**

- Writes: `LocalQuorum` (durable)
- Reads: `LocalOne` for blobs, `LocalQuorum` for checkpoints

**Key Types:**

- `FastData` - Core domain model for all indexed data
- `FastDataRow` - Database row representation
- `ScyllaDb` - Connection and session manager

#### suffix-fetcher (Data Distribution Library)

Enables sub-indexers to fetch data by suffix from the main blobs table without re-fetching from NEAR.

**Functionality:**

- Polls universal suffix (`*`) for latest block height
- Streams data by requested suffix in block height ranges
- Emits events: `FastData` or `EndOfRange` signals
- Async stream-based iteration

**Usage Pattern:**

```rust
// Create fetcher (connects to ScyllaDB if not provided)
let fetcher = SuffixFetcher::new(chain_id, None).await?;

// Set up channel for receiving updates
let (sender, mut receiver) = mpsc::channel(100);

// Start fetcher in background
tokio::spawn(fetcher.start(
    SuffixFetcherConfig {
        suffix: "kv".to_string(),
        start_block_height: Some(start_height),
        sleep_duration: Duration::from_millis(500),
    },
    sender,
    is_running,
));

// Process updates
while let Some(update) = receiver.recv().await {
    match update {
        SuffixFetcherUpdate::FastData(data) => { /* process */ }
        SuffixFetcherUpdate::EndOfRange(height) => { /* checkpoint */ }
    }
}
```

### Indexers

#### main-indexer

Reads blocks from the fastnear neardata API and stores all `__fastdata_*` transactions in the `blobs` table.

**Supported Protocols:**

- `__fastdata_kv` - Key-value storage
- `__fastdata_fastfs` - File storage
- `__fastdata_raw` - Raw data
- `__fastdata_*` - Any custom suffix

**Processing:**

1. Fetches blocks from the fastnear neardata API using `fastnear-neardata-fetcher`
2. Scans all function calls for methods starting with `__fastdata_`
3. Extracts suffix (method name without prefix) and arguments as blob
4. Stores in `blobs` table with the actual suffix (kv, fastfs, raw, etc.)
5. Tracks its own progress using universal suffix (`*`) in `meta` table

**Configuration:**

- `NUM_THREADS`: Thread pool size (default: 8)
- `BLOCK_UPDATE_INTERVAL_MS`: Checkpoint interval (default: 5000ms)
- `START_BLOCK_HEIGHT`: Initial block to start syncing

#### kv-sub-indexer

Processes KV entries from `blobs` table into dedicated KV tables with optimized queries.

**Functionality:**

- Fetches data with suffix "kv" via `SuffixFetcher`
- Parses JSON blobs as key-value objects
- Validates keys (max 1024 chars) and limits to 256 keys per blob
- Deduplicates by (predecessor, account, key) within each flush batch
- Stores each key-value pair as separate rows

**Output Tables:**

- `s_kv` - All KV entries with full history
- `s_kv_last` - Latest value per key (for fast lookups)
- `kv_accounts` - Lookup table for accounts by contract
- `kv_reverse` - Latest KV repartitioned by (contract, key) for cursor pagination over predecessors
- `kv_edges` - Auto-detected graph edges from KV keys with `/`-separated segments
- `mv_kv_key` - Materialized view indexed by key
- `mv_kv_cur_key` - Materialized view for reverse lookups by account

**Indexer ID:** `kv-1`

**Validation:**

- Max 256 keys per blob
- Max 1024 characters per key
- JSON format required

#### fastfs-sub-indexer

Processes FastFS file uploads into file storage tables supporting chunked uploads.

**Functionality:**

- Fetches data with suffix "fastfs" via `SuffixFetcher`
- Deserializes Borsh-encoded data into `SimpleFastfs` or `PartialFastfs`
- Supports complete files and chunked uploads with 1MB alignment
- Validates paths, MIME types, offsets, and nonces

**Data Variants:**

- `SimpleFastfs` - Complete file upload with optional MIME type and content
- `PartialFastfs` - Chunked upload with offset, nonce, and validation

**Output Table:**

- `s_fastfs_v2` - File chunks with metadata for reassembly

**File Constraints:**

- Max relative path: 1024 characters (ASCII only)
- Max MIME type: 256 characters (ASCII only)
- Chunk size: up to 1MB
- Total file size: up to 32MB
- Offset alignment: 1MB (required for partial uploads)

**Indexer ID:** `fastfs_v2`

## Database Schema

### Keyspace Configuration

```sql
CREATE KEYSPACE IF NOT EXISTS fastdata_{CHAIN_ID}
WITH REPLICATION = {
    'class': 'NetworkTopologyStrategy',
    'dc1': 3
}
AND TABLETS = {
    'enabled': true
};
```

### blobs (main-indexer output)

Stores all raw FastData transactions from NEAR blockchain.

```sql
CREATE TABLE blobs (
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
);
```

**Note:** main-indexer stores data with the actual suffix extracted from the method name (kv, fastfs, raw, etc.). The universal suffix (`*`) is only used in the `meta` table to track main-indexer's overall progress. Sub-indexers query `blobs` by specific suffixes.

### meta (main-indexer metadata)

Tracks last processed block height per suffix for recovery and checkpointing.

```sql
CREATE TABLE meta (
    suffix text PRIMARY KEY,
    last_processed_block_height bigint
);
```

### s_kv (kv-sub-indexer historical data)

Complete history of all KV entries.

```sql
CREATE TABLE s_kv (
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
    PRIMARY KEY ((predecessor_id), current_account_id, key, block_height, order_id)
);
```

### s_kv_last (kv-sub-indexer latest values)

Latest value per key for fast lookups (most commonly used table).

```sql
CREATE TABLE s_kv_last (
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
    PRIMARY KEY ((predecessor_id), current_account_id, key)
);
```

### Materialized Views (kv-sub-indexer)

**mv_kv_key** - Query by key across all accounts:

```sql
CREATE MATERIALIZED VIEW mv_kv_key AS
    SELECT * FROM s_kv
    WHERE key IS NOT NULL
    AND block_height IS NOT NULL
    AND order_id IS NOT NULL
    AND predecessor_id IS NOT NULL
    AND current_account_id IS NOT NULL
    PRIMARY KEY ((key), block_height, order_id, predecessor_id, current_account_id);
```

**mv_kv_cur_key** - Query by account and key:

```sql
CREATE MATERIALIZED VIEW mv_kv_cur_key AS
    SELECT * FROM s_kv
    WHERE current_account_id IS NOT NULL
    AND key IS NOT NULL
    AND block_height IS NOT NULL
    AND order_id IS NOT NULL
    AND predecessor_id IS NOT NULL
    PRIMARY KEY ((current_account_id), key, block_height, order_id, predecessor_id);
```

**kv_accounts** - Lookup table for accounts that wrote to a contract:

```sql
CREATE TABLE IF NOT EXISTS kv_accounts (
    current_account_id text,
    key text,
    predecessor_id text,
    PRIMARY KEY ((current_account_id), key, predecessor_id)
);
```

Example query to find all accounts that wrote to a contract:
```sql
SELECT * FROM kv_accounts WHERE current_account_id = 'mycontract.near';
```

**kv_reverse** - Latest KV values repartitioned by (contract, key) for cursor pagination over predecessors:

```sql
CREATE TABLE IF NOT EXISTS kv_reverse (
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
);
```

Example query to find all predecessors that set a key on a contract:
```sql
SELECT * FROM kv_reverse
WHERE current_account_id = 'app.near' AND key = 'settings';
```

**kv_edges** - Auto-detected graph edges from KV keys with `/`-separated segments:

```sql
CREATE TABLE IF NOT EXISTS kv_edges (
    edge_type text,
    target text,
    source text,
    current_account_id text,
    block_height bigint,
    block_timestamp bigint,
    order_id bigint,
    value text,
    PRIMARY KEY ((edge_type, target), source)
);
```

Edge detection: keys with 2+ `/`-separated segments produce an edge where `edge_type` = all segments except last, `target` = last segment, and `source` = `predecessor_id`. Uses `USING TIMESTAMP` for deterministic last-write-wins.

Example: key `"graph/follow/bob.near"` from predecessor `"alice.near"` produces edge_type=`"graph/follow"`, source=`"alice.near"`, target=`"bob.near"`.

```sql
-- Find all followers of bob.near
SELECT source FROM kv_edges
WHERE edge_type = 'graph/follow' AND target = 'bob.near';
```

### s_fastfs_v2 (fastfs-sub-indexer output)

Stores file chunks with metadata for FastFS file system.

```sql
CREATE TABLE s_fastfs_v2 (
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
);
```

### FastData Core Fields

All indexed data includes these provenance fields:

```rust
receipt_id: CryptoHash          // Receipt identifier
action_index: u32               // Action index within receipt
tx_hash: Option<CryptoHash>     // Transaction hash
signer_id: AccountId            // Transaction signer
predecessor_id: AccountId       // Calling account
current_account_id: AccountId   // Receiving account
block_height: u64               // Block height
block_timestamp: u64            // Block timestamp (nanoseconds)
shard_id: u32                   // Shard identifier
receipt_index: u32              // Receipt index within shard
suffix: String                  // Data type identifier
data: Vec<u8>                   // Raw payload
```

**order_id** (computed field for ordering):

```
order_id = (shard_id * 100000 + receipt_index) * 1000 + action_index
```

Ensures unique ordering across shards, receipts, and actions within a block.

## Configuration

### Environment Variables

Create a `.env` file in the project root:

```bash
# Blockchain Configuration
CHAIN_ID=mainnet                      # Chain: mainnet, testnet, etc.
START_BLOCK_HEIGHT=183140000          # Initial block to start syncing

# ScyllaDB Configuration
SCYLLA_URL=<host>:9042               # ScyllaDB endpoint
SCYLLA_USERNAME=scylla               # Database username
SCYLLA_PASSWORD=<password>           # Database password

# Optional: TLS/mTLS Configuration
# SCYLLA_SSL_CA=<path/to/ca.crt>       # TLS CA certificate
# SCYLLA_SSL_CERT=<path/to/client.crt> # mTLS client certificate
# SCYLLA_SSL_KEY=<path/to/client.key>  # mTLS client private key

# Optional: Fastnear neardata API
# FASTNEAR_AUTH_BEARER_TOKEN=<token>   # Auth for neardata API

# Optional: Runtime Configuration
# NUM_THREADS=8                         # Thread pool size for main-indexer (default: 8)
# BLOCK_UPDATE_INTERVAL_MS=5000        # Checkpoint interval in ms (default: 5000)
```

**Security Note:** Never commit `.env` to version control. It's already in `.gitignore`.

### TLS/mTLS Setup

**Server TLS (verify server identity):**

```bash
SCYLLA_SSL_CA=/path/to/ca-cert.pem
```

**Mutual TLS (client authentication):**

```bash
SCYLLA_SSL_CA=/path/to/ca-cert.pem
SCYLLA_SSL_CERT=/path/to/client-cert.pem
SCYLLA_SSL_KEY=/path/to/client-key.pem
```

## Development

### Prerequisites

- **Rust:** 1.86.0 or higher (nightly recommended)
- **ScyllaDB:** 5.0+ cluster with network access
- **Fastnear API:** Optional auth bearer token for neardata API
- **Git:** For version control

### Build

Build all components:

```bash
cargo build --release
```

Build specific components:

```bash
cargo build --release --bin main-indexer
cargo build --release --bin kv-sub-indexer
cargo build --release --bin fastfs-sub-indexer
```

Binaries will be located in `./target/release/`

### Running Locally

**1. Set up environment:**

```bash
cp .env.example .env
# Edit .env with your configuration
```

**2. Run main-indexer (must run first):**

```bash
./target/release/main-indexer
```

**3. Run sub-indexers (in separate terminals):**

```bash
./target/release/kv-sub-indexer
./target/release/fastfs-sub-indexer
```

**4. Monitor logs:**

- Each component outputs structured logs via `tracing`
- Logs include block heights, processing stats, and errors
- Use `RUST_LOG=debug` for verbose logging

### Development Workflow

1. Make code changes
2. Run `cargo check` for fast compile checks
3. Run `cargo test` for unit tests
4. Run `cargo build --release` for optimized builds
5. Test against local ScyllaDB instance

## Deployment

### Docker

**Build images:**

```bash
docker build -f Dockerfile.main-indexer -t fastdata-main-indexer:latest .
docker build -f Dockerfile.kv-sub-indexer -t fastdata-kv-sub-indexer:latest .
docker build -f Dockerfile.fastfs-sub-indexer -t fastdata-fastfs-sub-indexer:latest .
```

**Run containers:**

```bash
docker run --env-file .env -d --name main-indexer fastdata-main-indexer:latest
docker run --env-file .env -d --name kv-sub-indexer fastdata-kv-sub-indexer:latest
docker run --env-file .env -d --name fastfs-sub-indexer fastdata-fastfs-sub-indexer:latest
```

**Docker Compose (recommended):**

```yaml
services:
  main-indexer:
    image: fastdata-main-indexer:latest
    env_file: .env
    restart: unless-stopped

  kv-sub-indexer:
    image: fastdata-kv-sub-indexer:latest
    env_file: .env
    restart: unless-stopped
    depends_on:
      - main-indexer

  fastfs-sub-indexer:
    image: fastdata-fastfs-sub-indexer:latest
    env_file: .env
    restart: unless-stopped
    depends_on:
      - main-indexer
```

### Production Deployment

**Recommended Setup:**

- Run main-indexer on dedicated instance (8+ cores for parallel fetching)
- Run each sub-indexer on separate instances for isolation
- Use ScyllaDB cluster with at least 3 nodes for replication
- Enable TLS/mTLS for production security
- Monitor with Prometheus/Grafana (ScyllaDB exports metrics)
- Set up log aggregation (ELK stack, CloudWatch, etc.)

**Scaling:**

- Main-indexer: Increase `NUM_THREADS` for faster block fetching
- Sub-indexers: Run multiple instances per suffix (ScyllaDB handles dedup)
- ScyllaDB: Add nodes to cluster for horizontal scaling (tablets auto-balance)

### Monitoring

**Key Metrics:**

- Block height lag (current vs. latest NEAR block)
- Processing throughput (blocks/sec, transactions/sec)
- Database write latency (p50, p99)
- Error rates and retry counts
- Memory usage and CPU utilization

**Health Checks:**

- Query `meta` table for last processed block heights
- Compare timestamps to detect stalls
- Monitor ScyllaDB cluster health via `nodetool status`

## Data Standards

### KV Standard (suffix: "kv")

JSON-based key-value storage with automatic deduplication.

**Format:**

```json
{
  "key1": "value1",
  "key2": "value2",
  "key3": "value3"
}
```

**Validation:**

- Max 256 keys per transaction
- Max 1024 characters per key
- Keys must be non-empty, valid UTF-8, with no control characters
- Deduplication: One value per key per block (if duplicate keys exist in same block, highest order_id wins)

**Use Cases:**

- User preferences and settings
- Application state storage
- Metadata and configuration
- Simple database alternative

**Query Examples:**

```sql
-- Get latest value for a key
SELECT * FROM s_kv_last
WHERE predecessor_id = 'user.near'
AND current_account_id = 'app.near'
AND key = 'settings';

-- Get all keys for an account
SELECT * FROM s_kv_last
WHERE predecessor_id = 'user.near'
AND current_account_id = 'app.near';

-- Get history of a key
SELECT * FROM s_kv
WHERE predecessor_id = 'user.near'
AND current_account_id = 'app.near'
AND key = 'settings'
ORDER BY block_height DESC;

-- Find all predecessors who set a key on a contract (via kv_reverse)
SELECT * FROM kv_reverse
WHERE current_account_id = 'app.near'
AND key = 'settings';

-- Find all followers of an account (via kv_edges)
SELECT source FROM kv_edges
WHERE edge_type = 'graph/follow'
AND target = 'bob.near';
```

### FastFS Standard (suffix: "fastfs")

Borsh-encoded file storage supporting complete and chunked uploads.

**SimpleFastfs (complete upload):**

```rust
SimpleFastfs {
    relative_path: String,                // Max 1024 chars, ASCII only
    content: Option<FastfsFileContent>,   // None = deletion
}

FastfsFileContent {
    mime_type: String,   // Required, max 256 chars, ASCII only
    content: Vec<u8>,    // File content (up to 32MB)
}
```

**PartialFastfs (chunked upload):**

```rust
PartialFastfs {
    relative_path: String,      // Max 1024 chars, ASCII only
    offset: u32,                // Must be 1MB aligned
    mime_type: String,          // Required, max 256 chars, ASCII only
    content_chunk: Vec<u8>,     // Chunk content (up to 1MB)
    full_size: u32,             // Total file size (up to 32MB)
    nonce: u32,                 // Upload session ID (1..=i32::MAX)
}
```

**Validation:**

- Paths must be ASCII-only and ≤1024 characters
- MIME types must be non-empty, ASCII-only, and ≤256 characters
- Chunked uploads: offsets must be 1MB (1048576 bytes) aligned
- Total file size ≤32MB (33554432 bytes)
- Chunks uploaded out of order are supported (assembled by offset)
- **SimpleFastfs**: Implied nonce = 0 (single upload)
- **PartialFastfs**: Nonce must be ≥1 and ≤2,147,483,647 (i32::MAX) to differentiate upload sessions

**Use Cases:**

- Decentralized file storage
- Static website hosting
- Image and media uploads
- Document storage

**Query Examples:**

```sql
-- Get complete file (all chunks for a specific upload)
SELECT * FROM s_fastfs_v2
WHERE predecessor_id = 'user.near'
AND current_account_id = 'storage.near'
AND relative_path = 'images/avatar.png'
AND nonce = 42
ORDER BY offset ASC;

-- List all files for an account
SELECT DISTINCT relative_path FROM s_fastfs_v2
WHERE predecessor_id = 'user.near'
AND current_account_id = 'storage.near';

-- Get all uploads of a file (all nonces)
SELECT * FROM s_fastfs_v2
WHERE predecessor_id = 'user.near'
AND current_account_id = 'storage.near'
AND relative_path = 'images/avatar.png';
```

### Custom Standards

Add new data standards by creating a sub-indexer:

**1. Define your suffix:**

```rust
const MY_SUFFIX: &str = "mydata";
```

**2. Create sub-indexer using SuffixFetcher:**

```rust
use suffix_fetcher::{SuffixFetcher, SuffixFetcherConfig, SuffixFetcherUpdate};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

// Create fetcher (connects to ScyllaDB if not provided)
let fetcher = SuffixFetcher::new(chain_id, None).await?;
let scylladb = fetcher.get_scylladb();

// Set up shutdown signal
let is_running = Arc::new(AtomicBool::new(true));

// Set up channel for receiving updates
let (sender, mut receiver) = mpsc::channel(100);

// Start fetcher in background
tokio::spawn(fetcher.start(
    SuffixFetcherConfig {
        suffix: MY_SUFFIX.to_string(),
        start_block_height: Some(start_height),
        sleep_duration: Duration::from_millis(500),
    },
    sender,
    is_running.clone(),
));

// Process updates
while let Some(update) = receiver.recv().await {
    match update {
        SuffixFetcherUpdate::FastData(data) => {
            // Parse data.data (Vec<u8>) according to your format
            // Validate and transform
            // Insert into custom table
        }
        SuffixFetcherUpdate::EndOfRange(height) => {
            // Batch complete, commit or checkpoint
            scylladb.set_last_processed_block_height(INDEXER_ID, height).await?;
        }
    }
}
```

**3. Define custom schema in scylladb:**

```rust
// In scylla_types.rs
pub async fn create_my_table(scylla_db: &ScyllaDb) -> anyhow::Result<()> {
    scylla_db.scylla_session.query_unpaged(
        "CREATE TABLE IF NOT EXISTS s_my_data (...)",
        &[]
    ).await?;
    Ok(())
}
```

**4. Deploy and run:**

```bash
cargo build --release --bin my-sub-indexer
./target/release/my-sub-indexer
```

## Architecture Patterns

### Data Flow

```
1. NEAR Blockchain generates transactions with __fastdata_* methods
   ↓
2. fastnear neardata API serves block data over HTTP
   ↓
3. main-indexer fetches blocks via fastnear-neardata-fetcher (8 parallel threads)
   ↓
4. main-indexer extracts suffix and blob data from function arguments
   ↓
5. main-indexer writes to blobs table with actual suffix (kv, fastfs, etc.)
   ↓
6. main-indexer updates meta(*) table to track its overall progress
   ↓
7. suffix-fetcher polls meta(*) for new blocks
   ↓
8. suffix-fetcher queries blobs by specific suffix and streams to sub-indexers
   ↓
9. Sub-indexers parse, validate, and transform data
   ↓
10. Sub-indexers write to specialized tables (s_kv_last, s_fastfs_v2, etc.)
    and update meta table with their indexer ID for checkpoint tracking
```

### Scalability

**Horizontal Scaling:**

- ScyllaDB tablets automatically balance data across nodes
- Add nodes to cluster without downtime
- Sub-indexers can run multiple instances (idempotent writes)

**Vertical Scaling:**

- Increase `NUM_THREADS` for main-indexer (more parallel NEAR fetching)
- Use larger ScyllaDB instances for higher throughput
- Optimize queries with appropriate indexes and materialized views

**Async I/O:**

- All components use Tokio async runtime
- Non-blocking I/O for network and disk operations
- Efficient resource utilization

### Fault Tolerance

**Recovery:**

- `meta` table tracks last processed block per suffix
- Graceful shutdown with Ctrl+C saves current block height
- Resume from last checkpoint on restart
- No data loss on clean shutdown

**Error Handling:**

- Database write failures retry with exponential backoff (1s, 2s, 4s), then shut down gracefully
- Invalid data is logged and skipped
- Malformed blobs don't affect other transactions

**Consistency:**

- LocalQuorum writes ensure durability across replicas
- Unlogged batches for multi-row transactions (idempotent writes)
- Order ID prevents duplicate processing

## Performance

**Throughput (observed):**

- main-indexer: ~100-500 blocks/sec (depends on block density)
- kv-sub-indexer: ~1000-5000 KV pairs/sec
- fastfs-sub-indexer: ~100-500 files/sec

**Latency:**

- Block ingestion to indexing: <5 seconds (typical)
- Query latency: <10ms (p99) for key lookups
- Full history queries: <100ms (p99) for reasonable ranges

**Storage:**

- Blobs table: ~1-2 GB per million transactions (varies by payload size)
- KV tables: ~500 MB per million key-value pairs
- FastFS tables: Depends on file sizes (compressed by ScyllaDB)

## Troubleshooting

**main-indexer not starting:**

- Verify `START_BLOCK_HEIGHT` is valid for the chain
- Check Fastnear API authentication (FASTNEAR_AUTH_BEARER_TOKEN if configured)
- Ensure ScyllaDB is reachable and credentials are correct

**Sub-indexer lagging:**

- Check main-indexer is running and progressing
- Verify suffix-fetcher can query blobs table
- Monitor ScyllaDB cluster health

**Database connection errors:**

- Verify `SCYLLA_URL` and credentials
- Check TLS/mTLS certificates if configured
- Ensure firewall allows connections to port 9042

**High memory usage:**

- Reduce `NUM_THREADS` for main-indexer
- Tune ScyllaDB memory settings
- Check for slow queries with `nodetool tpstats`

**Data not appearing:**

- Verify transactions use correct `__fastdata_*` method names
- Check logs for parsing/validation errors
- Query `blobs` table directly to confirm main-indexer is working

## Contributing

**Development:**

1. Fork the repository
2. Create a feature branch
3. Make changes with tests
4. Run `cargo test` and `cargo fmt`
5. Submit a pull request

**Reporting Issues:**

- Provide logs, environment details, and reproduction steps
- Check existing issues first
- Tag with appropriate labels (bug, enhancement, etc.)

## Authors

**Fastnear Inc** <hello@fastnear.com>

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Repository

https://github.com/fastnear/fastdata-indexer

## Links

- NEAR Protocol: https://near.org
- ScyllaDB: https://www.scylladb.com
- Fastnear neardata: https://github.com/fastnear/fastnear-neardata-fetcher
- Fastnear: https://fastnear.com
