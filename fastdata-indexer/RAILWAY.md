# FastData Indexer - Railway Deployment

This directory contains Railway deployment configuration for the FastData indexer components.

## Architecture

```
┌─────────────────────┐
│   main-indexer      │ → ScyllaDB (blobs table)
└─────────────────────┘
          ↓
┌─────────────────────┐     ┌─────────────────────┐
│   kv-sub-indexer    │     │  fastfs-sub-indexer │
└─────────────────────┘     └─────────────────────┘
          ↓                           ↓
    ScyllaDB (KV tables)      ScyllaDB (FastFS tables)
```

## Prerequisites

1. **ScyllaDB** - Deploy ScyllaDB first (use Railway's ScyllaDB template or external)
2. **Environment Variables** - Set up all required env vars
3. **Railway CLI** - Install: `npm i -g @railway/cli`

## Deployment Steps

### 1. Deploy ScyllaDB

Option A: Use Railway's managed ScyllaDB
```bash
railway add --plugin scylladb
```

Option B: Use external ScyllaDB (Astra, ScyllaDB Cloud, etc.)

### 2. Set Environment Variables

In Railway dashboard, set these variables:

```bash
CHAIN_ID=mainnet
START_BLOCK_HEIGHT=183140000
SCYLLA_URL=<scylla-host>:9042
SCYLLA_USERNAME=scylla
SCYLLA_PASSWORD=<password>
# Optional:
# FASTNEAR_AUTH_BEARER_TOKEN=<token>
# SCYLLA_SSL_CA=/path/to/ca.crt
# SCYLLA_SSL_CERT=/path/to/client.crt
# SCYLLA_SSL_KEY=/path/to/client.key
```

### 3. Create ScyllaDB Keyspace

Before running indexers, create the keyspace:

```sql
CREATE KEYSPACE IF NOT EXISTS fastdata_mainnet
WITH REPLICATION = {
    'class': 'NetworkTopologyStrategy',
    'dc1': 3
} AND TABLETS = {'enabled': true};
```

### 4. Deploy Services

#### Option A: Deploy each service separately

```bash
# Login to Railway
railway login

# Link to your project
railway link

# Deploy main-indexer
cd fastdata-indexer
railway up --service main-indexer

# Deploy kv-sub-indexer
railway up --service kv-sub-indexer

# Deploy fastfs-sub-indexer
railway up --service fastfs-sub-indexer
```

#### Option B: Use Railway Dashboard

1. Go to your Railway project
2. Click "New Service" → "GitHub Repo"
3. Select `Kampouse/key-manager`
4. Set root directory to `fastdata-indexer`
5. Choose "Dockerfile" builder
6. Set Dockerfile path:
   - `Dockerfile.main-indexer` for main-indexer
   - `Dockerfile.kv-sub-indexer` for kv-sub-indexer
   - `Dockerfile.fastfs-sub-indexer` for fastfs-sub-indexer

### 5. Configure Service Dependencies

In Railway, set up service dependencies:

```
main-indexer → (no dependencies, starts first)
kv-sub-indexer → depends on main-indexer
fastfs-sub-indexer → depends on main-indexer
```

## Service Configuration

### main-indexer

- **Port**: None (background worker)
- **Resources**: 2 CPU, 4GB RAM recommended
- **Environment**:
  - `NUM_THREADS=8` (adjust based on CPU)
  - `BLOCK_UPDATE_INTERVAL_MS=5000`

### kv-sub-indexer

- **Port**: None (background worker)
- **Resources**: 1 CPU, 2GB RAM recommended

### fastfs-sub-indexer

- **Port**: None (background worker)
- **Resources**: 1 CPU, 2GB RAM recommended

## Monitoring

### Health Checks

Check the `meta` table to verify each indexer is progressing:

```sql
-- Check main-indexer progress (uses universal suffix *)
SELECT * FROM fastdata_mainnet.meta WHERE suffix = '*';

-- Check kv-sub-indexer progress
SELECT * FROM fastdata_mainnet.meta WHERE suffix = 'kv-1';

-- Check fastfs-sub-indexer progress
SELECT * FROM fastdata_mainnet.meta WHERE suffix = 'fastfs_v2';
```

### Logs

View logs in Railway dashboard:
```bash
railway logs --service main-indexer
railway logs --service kv-sub-indexer
railway logs --service fastfs-sub-indexer
```

### Metrics

Monitor these key metrics:
- Block height lag (compare to latest NEAR block)
- Processing throughput (blocks/sec)
- Memory usage
- ScyllaDB write latency

## Scaling

### Horizontal Scaling

- **main-indexer**: Single instance only (single-writer pattern)
- **kv-sub-indexer**: Can run multiple instances (idempotent writes)
- **fastfs-sub-indexer**: Can run multiple instances (idempotent writes)

### Vertical Scaling

- Increase `NUM_THREADS` for main-indexer (requires more CPU)
- Increase RAM for larger batch processing

## Troubleshooting

### Service won't start

1. Check ScyllaDB connection: `SCYLLA_URL`, `SCYLLA_USERNAME`, `SCYLLA_PASSWORD`
2. Verify keyspace exists
3. Check Railway logs for errors

### Indexer lagging

1. Check main-indexer is running
2. Monitor ScyllaDB performance
3. Scale up resources if needed

### Database connection errors

1. Verify network connectivity (Railway private networking)
2. Check TLS certificates if using SSL
3. Ensure firewall allows port 9042

## Costs

Estimated Railway costs (varies by usage):

- **main-indexer**: ~$10-20/month (2 CPU, 4GB RAM)
- **kv-sub-indexer**: ~$5-10/month (1 CPU, 2GB RAM)
- **fastfs-sub-indexer**: ~$5-10/month (1 CPU, 2GB RAM)
- **ScyllaDB**: ~$20-50/month (depends on storage)

**Total**: ~$40-90/month for full setup

## Security

### Production Checklist

- [ ] Enable TLS for ScyllaDB connections
- [ ] Use strong passwords for ScyllaDB
- [ ] Enable Railway private networking
- [ ] Set up log aggregation
- [ ] Configure alerts for service failures
- [ ] Regular backups of ScyllaDB

### Secrets Management

Store sensitive data in Railway variables:
- `SCYLLA_PASSWORD` - Database password
- `FASTNEAR_AUTH_BEARER_TOKEN` - API token (optional)

## Support

- **Documentation**: See README.md for architecture details
- **Issues**: https://github.com/Kampouse/key-manager/issues
- **Railway Docs**: https://docs.railway.app
