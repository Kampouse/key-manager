# AGENTS.md — FastKV Server

What must never break if you change this code.
Consumer-facing docs: REFERENCE.md, runtime OpenAPI at `/docs` route.

## Red Flags (Read Before Editing)

- Never change response envelopes without updating every handler that uses them
- Never add a new pagination style — one exists, use it
- Never push user filters into CQL without checking partition keys first
- Never knowingly return partial data without signaling `truncated` or logging a warning
- Never bypass `validate_*` in KV handlers

## Module Ownership

- **handlers.rs**
  Owns: HTTP layer, param validation, response envelope construction
  Must NOT: issue CQL, import scylladb internals, construct `Statement`

- **social_handlers.rs**
  Owns: SocialDB compatibility layer, key pattern parsing, tree assembly
  Must NOT: define new envelope types, add pagination styles

- **scylladb.rs**
  Owns: all DB access, prepared statements, scan caps, stream iteration, `collect_page()` helper
  Must NOT: know about HTTP, import actix_web, validate query params
  Key pattern: `collect_page()` is a free function that handles overfetch+1 and scan-cap modes. Paginated methods return `(Vec<T>, bool, usize)` (entries, has_more, dropped_rows) or `(Vec<T>, bool, usize, Option<String>)` (+ next_cursor, for history/timeline).

- **models.rs**
  Owns: all request/response structs, constants, `ApiError`, serde config
  Must NOT: contain business logic or DB access

- **queries.rs**
  Owns: `compute_prefix_end()` — computes the exclusive upper bound for prefix range scans
  Must NOT: build dynamic CQL or execute queries

- **tree.rs**
  Owns: `build_tree()` — slash-delimited keys to nested JSON
  Must NOT: access DB or HTTP types

**Dependency direction:** handlers → models + scylladb + tree. Never the reverse. Handlers never import each other.

## Standard Handler Pattern

Every KV handler follows this sequence. Do not skip steps. Do not reorder.

1. Validate all params (`validate_account_id`, `validate_key`, `validate_limit`, cursor/offset conflict)
2. Log the request (`tracing::info!` with `target: PROJECT_ID`)
3. `require_db()` → get `RwLockReadGuard`
4. Call exactly one `scylladb` method
5. Build `PaginationMeta` from results
6. Return via `respond_paginated()` or `DataResponse`

Social handlers deviate: they construct `QueryParams`/`WritersParams` directly and call `scylladb` methods in loops (one per key pattern). They validate outer params themselves.

### Canonical KV Handler Skeleton

Struct fields use internal names with `#[serde(rename)]` to API params:
`predecessor_id` → `accountId`, `current_account_id` → `contractId`.
Cursor fields vary by endpoint: `after_key` (validated as key), `after_account` / `after_source` (validated as account ID).

```rust
pub async fn my_handler(
    query: web::Query<MyParams>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    // predecessor_id is #[serde(rename = "accountId")]
    validate_account_id(&query.predecessor_id, "accountId")?;
    // current_account_id is #[serde(rename = "contractId")]
    validate_account_id(&query.current_account_id, "contractId")?;
    validate_limit(query.limit)?;
    // Cursor field varies: after_key | after_account | after_source
    if let Some(ref cursor) = query.after_key {
        validate_key(cursor, "after_key", MAX_KEY_LENGTH)?;
        if query.offset > 0 {
            return Err(ApiError::InvalidParameter(
                "cannot use both 'after_key' cursor and 'offset'".to_string(),
            ));
        }
    } else {
        validate_offset(query.offset)?;
    }

    tracing::info!(target: PROJECT_ID, /* fields */, "GET /v1/kv/my-endpoint");

    let db = require_db(&app_state).await?;
    let (entries, has_more, dropped) = db.my_query(&query).await?;

    let next_cursor = entries.last().map(|e| e.key.clone());
    let meta = PaginationMeta { has_more, truncated: false, next_cursor, dropped_rows: dropped_to_option(dropped) };
    let fields = parse_field_set(&query.fields)?;
    let decode = should_decode(&query.value_format)?;
    Ok(respond_paginated(entries, meta, &fields, decode))
}
```

## Pagination Invariants

These are load-bearing. Breaking any one breaks client pagination.

1. **Overfetch by 1 (cursor endpoints).** Query `limit + 1` rows via `collect_page()` overfetch mode. Got `limit + 1` back → `has_more = true`, then truncate to `limit`. Applies to: query, writers, edges, history, timeline, accounts-scan. (`accounts-by-contract` uses manual overfetch with HashSet dedup, not `collect_page()`.) History/timeline use cursor-based overfetch with a post-filter skip at the cursor's block_height to handle tie-breaking (rows at the same block share different order_id/key).
2. **Always emit `next_cursor`.** Set from the last item in the result, even when `has_more == false`. Clients use `next_cursor` as the resume token.
3. **`truncated` only from scan caps.** `truncated: true` only when a scan/dedup cap was hit (100k unique accounts for accounts-by-contract). History, timeline, and writers never set `truncated` — they use clean cursor pagination. Never set `truncated` from normal limit+1 pagination.
4. **`has_more` truthfulness.** Authoritative for cursor+limit endpoints. Best-effort when `truncated == true`. If `truncated` is true, treat `has_more` as unreliable; use `next_cursor` to resume.
5. **Cursor/offset mutual exclusion.** Both `after_*` and `offset > 0` → reject with HTTP 400.
6. **No pagination fields outside `meta`.** Never add `has_more`, `next_cursor`, or `truncated` to the top level.
7. **`dropped_rows` from deser errors.** `meta.dropped_rows` reports rows skipped due to deserialization failures. Omitted when zero (`Option<u32>`, `skip_serializing_if`). All paginated endpoints (KV and social) use `meta.dropped_rows` in the JSON body. Never expose error details — just the count.

**Client stop rule:**
Stop paginating when `meta.has_more == false && meta.truncated != true`.
If `meta.truncated == true`, results are best-effort; clients may continue
using `next_cursor` but completion is unknown.

## Response Envelopes

| Kind            | Shape                                          | Used by                       |
| --------------- | ---------------------------------------------- | ----------------------------- |
| Paginated list  | `PaginatedResponse<T>` → `{ data: T[], meta }` | All KV list endpoints         |
| Singleton       | `DataResponse<T>` → `{ data: T }`              | get, batch, diff, edges/count |
| Infra           | Flat JSON, no envelope                         | /health, /v1/status           |
| Social get/keys | Raw nested JSON (SocialDB compat)              | /social/get, /social/keys     |

Do not invent ad-hoc `serde_json::json!({...})` shapes for new endpoints. Use `PaginatedResponse<T>` or `DataResponse<T>`.

## Safety Rules

1. **All user-supplied identifiers must be validated** before any DB call — `validate_account_id()`, `validate_key()`, `validate_prefix()`.
2. **Never interpolate user input into CQL.** Table names are interpolated but validated at startup via `validate_identifier()`. User values go through bind parameters (`?`) only.
3. **Do not panic on DB disconnect.** `require_db()` returns `Err(DatabaseUnavailable)`. Never `.unwrap()` on DB access.
4. **Do not add `deny_unknown_fields`** to request structs — breaks forward compatibility.
5. **Error sanitization.** `From<anyhow::Error> for ApiError` logs full context, returns generic message. Never expose table names, IPs, or schema in client-facing errors. All error responses include a machine-readable `code` field (`ErrorCode` enum) alongside the human-readable `error` string — see `ErrorResponse` in models.rs.
6. **New configurable table names must call `validate_identifier()`** in `ScyllaDb::new()`.
7. **DB is `Arc<RwLock<Option<ScyllaDb>>>`** — server starts and runs without a DB connection. All handlers must call `require_db()` and return `DatabaseUnavailable` if `None`. Never hold the `RwLock` guard across `.await` — this deadlocks the server.

## Prepared Statements

- All CQL must be prepared in `ScyllaDb::new()`. No exceptions. 25 statements currently.
- `queries.rs` owns only `compute_prefix_end()` (bind param computation, not dynamic CQL).
- Default consistency: `LocalOne`. Exceptions require justification (see `accounts_by_contract` for `LocalQuorum`).
- All statements get 10s request timeout via `set_request_timeout`.

## Hot Endpoints (Do Not Remove Safeguards)

- `/v1/kv/query` without `key_prefix` — full partition scan. **Prefer `key_prefix` to narrow.**
- `/v1/kv/timeline` — reads `s_kv_by_block` with CQL `ORDER BY` + cursor-based overfetch. **Use `from_block`/`to_block` to narrow and `cursor` for pagination.**
- `/v1/kv/accounts` without `key` — full partition scan + HashSet dedup, capped at 100k unique. **Prefer `key` filter.**
- `/v1/kv/accounts` without `contractId` — reads `all_accounts` table (no dedup needed, TOKEN-based cursor). Courtesy-limited to 1 req/sec/IP to prevent accidental repeated scans, limit clamped to 1,000.
- `/v1/kv/writers` — streams entire reverse table partition (unbounded, no scan cap). **Use cursor pagination with tight `limit`.**
- `/v1/kv/edges/count` — `COUNT(*)` scans entire partition. **No mitigation; avoid in hot loops.**
- `/v1/kv/watch` — SSE endpoint; polls `get_kv` per interval (2–30s). **Capped at 100 concurrent connections globally.** Uses `WatchGuard` RAII for cleanup.

## Hard Limits (Do Not Change Casually)

| Constant             | Value   | Why it matters                                                                          |
| -------------------- | ------- | --------------------------------------------------------------------------------------- |
| `MAX_DEDUP_SCAN`     | 100,000 | Unique-value cap for `query_accounts_by_contract` dedup HashSet. Memory-bound.          |
| `MAX_SOCIAL_RESULTS` | 1,000   | Per-pattern cap in social handlers. Controls `X-Results-Truncated`.                     |
| `MAX_OFFSET`         | 100,000 | Hard ceiling on offset pagination.                                                      |
| `MAX_BATCH_KEYS`     | 100     | Concurrent batch lookups (buffered 10 at a time).                                       |
| `MAX_SCAN_LIMIT`     | 1,000   | Max `limit` for `/v1/kv/accounts` without `contractId`.                                  |
| `MAX_CONCURRENT_WATCHES` | 100 | Global cap on SSE `/v1/kv/watch` connections. Memory + DB polling bound.                |

Full constant list in models.rs:1–20.

## Testing Contract

- `cargo test` must pass (48 unit tests)
- `cargo clippy` must pass
- No ScyllaDB required — unit tests cover serde, validation, tree building, prefix computation
- Do not add integration tests without discussion (requires live DB)
- Tests live in `#[cfg(test)] mod tests` at the bottom of each module

## Anti-Patterns (Do NOT)

- **Do not add new pagination styles.** One style: `PaginationMeta` with `has_more`/`truncated`/`next_cursor`/`dropped_rows`.
- **Do not re-add `decode` param.** It was removed. Use `value_format=json|raw` only.
- **`/kv/timeline` uses `s_kv_by_block` table with CQL block-height pushdown.** `KvTimelineRow` (9 columns) maps to this table; do not confuse with `KvHistoryRow` (13 columns) used by `/kv/history`.
- **Do not make `build_tree()` error on conflicts.** Leaf at "a/b" blocks nesting "a/b/c" — it skips silently. This is intentional.
- **Do not remove the scan cap** from accounts. It prevents unbounded partition scans in prod. History/timeline use cursor-based overfetch (no scan cap needed).
- **Do not mix `rename` and `alias` conventions.** KV params use `#[serde(rename = "accountId")]` (one canonical form). Social params use `#[serde(alias = "accountId")]` (accept both for SocialDB compat).

## Pointers

- **REFERENCE.md** — Endpoint params, TS interfaces, env vars, cost ratings, prepared statement table
- **https://near.garden/docs** — OpenAPI spec via Scalar UI (auto-generated from utoipa annotations in main.rs)
- **models.rs:1–18** — All constants
- **scylladb.rs `collect_page()`** — Reusable paginated stream helper (overfetch + scan-cap modes). 8 unit tests.
- **scylladb.rs:131–393** — ScyllaDb struct + all prepared statement initialization
- **main.rs** — `X-Indexer-Block` + `Cache-Control` header middleware (cached `AtomicU64`, refreshed 5s); don't remove
- **models.rs `ErrorCode`** — Machine-readable codes in all error responses. Keep enum in sync with `ApiError` variants.
