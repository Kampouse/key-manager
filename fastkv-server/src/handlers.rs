use crate::models::*;
use crate::scylladb::ScyllaDb;
use crate::tree::build_tree;
use crate::AppState;
use actix_web::{get, post, web, HttpRequest, HttpResponse};

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

const THROTTLE_EXPIRY: Duration = Duration::from_secs(60);
const MAX_THROTTLE_ENTRIES: usize = 50_000;

pub(crate) async fn require_db(state: &AppState) -> Result<Arc<ScyllaDb>, ApiError> {
    state
        .scylladb
        .read()
        .await
        .clone()
        .ok_or(ApiError::DatabaseUnavailable)
}

/// Attempt to JSON-decode the `"value"` field in a serialized entry.
/// If the value is a JSON string, it is parsed into the decoded JSON type
/// (e.g., `"\"Alice\""` becomes `"Alice"`, `"42"` becomes `42`).
fn decode_value_in_json(json: &mut serde_json::Value) {
    if let Some(map) = json.as_object_mut() {
        if let Some(raw) = map
            .get("value")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
        {
            if let Ok(decoded) = serde_json::from_str::<serde_json::Value>(&raw) {
                map.insert("value".to_string(), decoded);
            }
        }
    }
}

fn respond_paginated(
    entries: Vec<KvEntry>,
    meta: PaginationMeta,
    fields: &Option<HashSet<String>>,
    decode: bool,
) -> HttpResponse {
    if fields.is_some() || decode {
        let filtered: Vec<_> = entries
            .into_iter()
            .map(|e| {
                let mut json = e.to_json_with_fields(fields);
                if decode {
                    decode_value_in_json(&mut json);
                }
                json
            })
            .collect();
        HttpResponse::Ok().json(serde_json::json!({ "data": filtered, "meta": meta }))
    } else {
        HttpResponse::Ok().json(PaginatedResponse {
            data: entries,
            meta,
        })
    }
}

pub(crate) fn validate_account_id(value: &str, name: &str) -> Result<(), ApiError> {
    if value.is_empty() {
        return Err(ApiError::InvalidParameter(format!(
            "{name}: cannot be empty"
        )));
    }
    if value.len() > MAX_ACCOUNT_ID_LENGTH {
        return Err(ApiError::InvalidParameter(format!(
            "{name}: cannot exceed {MAX_ACCOUNT_ID_LENGTH} characters"
        )));
    }
    Ok(())
}

pub(crate) fn validate_key(value: &str, name: &str, max_len: usize) -> Result<(), ApiError> {
    if value.is_empty() {
        return Err(ApiError::InvalidParameter(format!(
            "{name}: cannot be empty"
        )));
    }
    if value.len() > max_len {
        return Err(ApiError::InvalidParameter(format!(
            "{name}: cannot exceed {max_len} characters"
        )));
    }
    Ok(())
}

pub(crate) fn validate_offset(offset: usize) -> Result<(), ApiError> {
    if offset > MAX_OFFSET {
        return Err(ApiError::InvalidParameter(format!(
            "offset: cannot exceed {MAX_OFFSET}"
        )));
    }
    Ok(())
}

pub(crate) fn validate_cursor_or_offset(
    cursor: Option<&str>,
    cursor_name: &str,
    offset: usize,
    validate_cursor_fn: impl FnOnce(&str, &str) -> Result<(), ApiError>,
) -> Result<(), ApiError> {
    if let Some(c) = cursor {
        validate_cursor_fn(c, cursor_name)?;
        if offset > 0 {
            return Err(ApiError::InvalidParameter(format!(
                "{cursor_name}: cannot combine with offset"
            )));
        }
    } else {
        validate_offset(offset)?;
    }
    Ok(())
}

pub(crate) fn validate_order(order: &str) -> Result<(), ApiError> {
    if !order.eq_ignore_ascii_case("asc") && !order.eq_ignore_ascii_case("desc") {
        return Err(ApiError::InvalidParameter(
            "order: must be 'asc' or 'desc'".to_string(),
        ));
    }
    Ok(())
}

fn validate_block_range(from_block: Option<i64>, to_block: Option<i64>) -> Result<(), ApiError> {
    if from_block.is_some_and(|v| v < 0) || to_block.is_some_and(|v| v < 0) {
        return Err(ApiError::InvalidParameter(
            "from_block/to_block: cannot be negative".to_string(),
        ));
    }
    if let (Some(from), Some(to)) = (from_block, to_block) {
        if from > to {
            return Err(ApiError::InvalidParameter(
                "from_block: must be <= to_block".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_prefix(prefix: &Option<String>) -> Result<(), ApiError> {
    if let Some(ref p) = prefix {
        if p.is_empty() {
            return Err(ApiError::InvalidParameter(
                "key_prefix: cannot be empty (omit to skip filtering)".to_string(),
            ));
        }
        if p.len() > MAX_PREFIX_LENGTH {
            return Err(ApiError::InvalidParameter(format!(
                "key_prefix: cannot exceed {MAX_PREFIX_LENGTH} characters"
            )));
        }
    }
    Ok(())
}

/// Extract client IP from X-Forwarded-For (rightmost entry = added by Railway's proxy).
/// Correct for a single trusted proxy hop. If a CDN is added in front, this would
/// need to skip additional hops from the right.
fn extract_client_ip(req: &HttpRequest) -> String {
    req.headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.rsplit(',').next())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != "unknown")
        .map(|s| s.to_string())
        .or_else(|| {
            req.connection_info()
                .realip_remote_addr()
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| {
            req.connection_info()
                .peer_addr()
                .unwrap_or("unknown")
                .to_string()
        })
}

/// Prevents accidental repeated scan requests from a single client (courtesy limit, not a security boundary).
fn check_scan_throttle(app_state: &AppState, ip: &str) -> Result<(), ApiError> {
    let mut throttle = app_state
        .scan_throttle
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let now = std::time::Instant::now();
    let cutoff = now - THROTTLE_EXPIRY;
    throttle.retain(|_, ts| *ts > cutoff);
    if let Some(last) = throttle.get(ip) {
        if now.duration_since(*last) < std::time::Duration::from_secs(1) {
            return Err(ApiError::TooManyRequests(
                "Too many scan requests. Try again shortly.".to_string(),
            ));
        }
    }
    if throttle.len() >= MAX_THROTTLE_ENTRIES {
        return Err(ApiError::TooManyRequests(
            "Too many scan requests. Try again shortly.".to_string(),
        ));
    }
    throttle.insert(ip.to_string(), now);
    Ok(())
}

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse),
        (status = 503, description = "Database unavailable", body = HealthResponse)
    ),
    tag = "health"
)]
#[get("/health")]
pub async fn health_check(app_state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let db = app_state.scylladb.read().await.clone();
    match db.as_ref() {
        Some(db) => match db.health_check().await {
            Ok(_) => Ok(HttpResponse::Ok().json(HealthResponse {
                status: "ok".to_string(),
                database: None,
            })),
            Err(e) => {
                tracing::warn!(target: PROJECT_ID, error = %e, "Health check failed");
                Ok(HttpResponse::ServiceUnavailable().json(HealthResponse {
                    status: "degraded".to_string(),
                    database: Some("unavailable".to_string()),
                }))
            }
        },
        None => Ok(HttpResponse::ServiceUnavailable().json(HealthResponse {
            status: "degraded".to_string(),
            database: Some("unavailable".to_string()),
        })),
    }
}

/// Get a single KV entry by exact key
#[utoipa::path(
    get,
    path = "/v1/kv/get",
    params(GetParams),
    responses(
        (status = 200, description = "Entry found or null if not found", body = inline(DataResponse<Option<KvEntry>>)),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "kv"
)]
#[get("/v1/kv/get")]
pub async fn get_kv_handler(
    query: web::Query<GetParams>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    validate_account_id(&query.predecessor_id, "accountId")?;
    validate_account_id(&query.current_account_id, "contractId")?;
    validate_key(&query.key, "key", MAX_KEY_LENGTH)?;

    tracing::info!(
        target: PROJECT_ID,
        accountId = %query.predecessor_id,
        contractId = %query.current_account_id,
        key = %query.key,
        "GET /v1/kv/get"
    );

    let db = require_db(&app_state).await?;
    let entry = db
        .get_kv(&query.predecessor_id, &query.current_account_id, &query.key)
        .await?;

    // Apply field selection and optional value decoding
    let fields = parse_field_set(&query.fields)?;
    let decode = should_decode(&query.value_format)?;
    match entry {
        Some(entry) => {
            if fields.is_some() || decode {
                let mut json = entry.to_json_with_fields(&fields);
                if decode {
                    decode_value_in_json(&mut json);
                }
                Ok(HttpResponse::Ok().json(serde_json::json!({ "data": json })))
            } else {
                Ok(HttpResponse::Ok().json(DataResponse { data: Some(entry) }))
            }
        }
        None => Ok(HttpResponse::Ok().json(DataResponse {
            data: Option::<KvEntry>::None,
        })),
    }
}

/// Query KV entries with optional prefix filtering and pagination
#[utoipa::path(
    get,
    path = "/v1/kv/query",
    params(QueryParams),
    responses(
        (status = 200, description = "List of matching entries", body = inline(PaginatedResponse<KvEntry>)),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "kv"
)]
#[get("/v1/kv/query")]
pub async fn query_kv_handler(
    query: web::Query<QueryParams>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    validate_account_id(&query.predecessor_id, "accountId")?;
    validate_account_id(&query.current_account_id, "contractId")?;
    validate_limit(query.limit)?;
    validate_prefix(&query.key_prefix)?;

    validate_cursor_or_offset(
        query.after_key.as_deref(),
        "after_key",
        query.offset,
        |c, n| validate_key(c, n, MAX_KEY_LENGTH),
    )?;

    if let Some(ref fmt) = query.format {
        if fmt != "tree" {
            return Err(ApiError::InvalidParameter(
                "format: must be 'tree' or omitted".to_string(),
            ));
        }
    }

    tracing::info!(
        target: PROJECT_ID,
        accountId = %query.predecessor_id,
        contractId = %query.current_account_id,
        key_prefix = ?query.key_prefix,
        limit = query.limit,
        offset = query.offset,
        after_key = ?query.after_key,
        "GET /v1/kv/query"
    );

    let db = require_db(&app_state).await?;
    let (entries, has_more, dropped) = db.query_kv_with_pagination(&query).await?;

    if query.format.as_deref() == Some("tree") {
        let items: Vec<(String, String)> = entries.into_iter().map(|e| (e.key, e.value)).collect();
        let tree = build_tree(&items);
        return Ok(HttpResponse::Ok().json(TreeResponse { tree, has_more }));
    }

    let next_cursor = entries.last().map(|e| e.key.clone());
    let meta = PaginationMeta {
        has_more,
        truncated: false,
        next_cursor,
        dropped_rows: dropped_to_option(dropped),
    };
    let fields = parse_field_set(&query.fields)?;
    let decode = should_decode(&query.value_format)?;
    Ok(respond_paginated(entries, meta, &fields, decode))
}

#[utoipa::path(
    get,
    path = "/v1/kv/history",
    params(HistoryParams),
    responses(
        (status = 200, description = "List of historical entries", body = inline(PaginatedResponse<KvEntry>)),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "kv"
)]
#[get("/v1/kv/history")]
pub async fn history_kv_handler(
    query: web::Query<HistoryParams>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    validate_account_id(&query.predecessor_id, "accountId")?;
    validate_account_id(&query.current_account_id, "contractId")?;
    validate_key(&query.key, "key", MAX_KEY_LENGTH)?;
    validate_limit(query.limit)?;
    validate_order(&query.order)?;
    validate_block_range(query.from_block, query.to_block)?;
    if let Some(ref c) = query.cursor {
        if c.len() > MAX_CURSOR_LENGTH {
            return Err(ApiError::InvalidParameter(
                "cursor: exceeds max length".to_string(),
            ));
        }
        if !c.is_empty() {
            parse_history_cursor(c)?;
        }
    }

    tracing::info!(
        target: PROJECT_ID,
        accountId = %query.predecessor_id,
        contractId = %query.current_account_id,
        key = %query.key,
        limit = query.limit,
        cursor = ?query.cursor,
        order = %query.order,
        from_block = ?query.from_block,
        to_block = ?query.to_block,
        "GET /v1/kv/history"
    );

    let db = require_db(&app_state).await?;
    let (entries, has_more, dropped, next_cursor) = db.get_kv_history(&query).await?;

    let meta = PaginationMeta {
        has_more,
        truncated: false,
        next_cursor,
        dropped_rows: dropped_to_option(dropped),
    };
    let fields = parse_field_set(&query.fields)?;
    let decode = should_decode(&query.value_format)?;
    Ok(respond_paginated(entries, meta, &fields, decode))
}

/// Find all writers for a key under a contract, with optional account filter
#[utoipa::path(
    get,
    path = "/v1/kv/writers",
    params(WritersParams),
    responses(
        (status = 200, description = "List of entries from writers", body = inline(PaginatedResponse<KvEntry>)),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "kv"
)]
#[get("/v1/kv/writers")]
pub async fn writers_handler(
    query: web::Query<WritersParams>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    validate_account_id(&query.current_account_id, "contractId")?;
    validate_key(&query.key, "key", MAX_KEY_LENGTH)?;
    validate_limit(query.limit)?;
    if let Some(ref pred) = query.predecessor_id {
        validate_account_id(pred, "accountId")?;
    }

    validate_cursor_or_offset(
        query.after_account.as_deref(),
        "after_account",
        query.offset,
        validate_account_id,
    )?;

    tracing::info!(
        target: PROJECT_ID,
        contractId = %query.current_account_id,
        key = %query.key,
        accountId = ?query.predecessor_id,
        limit = query.limit,
        offset = query.offset,
        after_account = ?query.after_account,
        "GET /v1/kv/writers"
    );

    let db = require_db(&app_state).await?;
    let (entries, has_more, truncated, dropped) = db.query_writers(&query).await?;

    let next_cursor = entries.last().map(|e| e.predecessor_id.clone());
    let meta = PaginationMeta {
        has_more,
        truncated,
        next_cursor,
        dropped_rows: dropped_to_option(dropped),
    };
    let fields = parse_field_set(&query.fields)?;
    let decode = should_decode(&query.value_format)?;
    Ok(respond_paginated(entries, meta, &fields, decode))
}

/// List unique writer accounts for a contract (or across all contracts).
///
/// Returns deduplicated predecessor accounts that have written to the given contract.
/// Providing a `key` filter is recommended — omitting it scans the entire contract partition
/// which can be expensive for large contracts. Results are capped at 100,000 unique accounts;
/// if this limit is reached the response will include `meta.truncated: true`.
///
/// When `contractId` is omitted, performs a full table scan throttled to 1 req/sec per IP.
/// Limit is clamped to 1,000.
#[utoipa::path(
    get,
    path = "/v1/kv/accounts",
    params(AccountsQueryParams),
    responses(
        (status = 200, description = "List of writer accounts", body = inline(PaginatedResponse<String>)),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 429, description = "Too many scan requests", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "kv"
)]
#[get("/v1/kv/accounts")]
pub async fn accounts_handler(
    req: HttpRequest,
    query: web::Query<AccountsQueryParams>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let contract_id = query.contract_id.as_deref();
    let is_scan = contract_id.is_none();

    if is_scan {
        if query.key.is_some() {
            return Err(ApiError::InvalidParameter(
                "key: requires contractId".to_string(),
            ));
        }
        if query.offset > 0 {
            return Err(ApiError::InvalidParameter(
                "offset: requires contractId (use after_account cursor instead)".to_string(),
            ));
        }
    } else if let Some(cid) = contract_id {
        validate_account_id(cid, "contractId")?;
    }

    let limit = if is_scan {
        query.limit.min(MAX_SCAN_LIMIT)
    } else {
        query.limit
    };
    validate_limit(limit)?;

    if let Some(ref key) = query.key {
        validate_key(key, "key", MAX_KEY_LENGTH)?;
    }

    validate_cursor_or_offset(
        query.after_account.as_deref(),
        "after_account",
        query.offset,
        validate_account_id,
    )?;

    if is_scan {
        check_scan_throttle(&app_state, &extract_client_ip(&req))?;
    }

    tracing::info!(
        target: PROJECT_ID,
        contractId = ?query.contract_id,
        scan = is_scan,
        key = ?query.key,
        limit = limit,
        offset = query.offset,
        after_account = ?query.after_account,
        "GET /v1/kv/accounts"
    );

    let db = require_db(&app_state).await?;

    let (accounts, has_more, truncated, dropped) = if let Some(cid) = contract_id {
        db.query_accounts_by_contract(
            cid,
            query.key.as_deref(),
            limit,
            query.offset,
            query.after_account.as_deref(),
        )
        .await?
    } else {
        let (accounts, has_more, dropped) = db
            .query_all_accounts(limit, query.after_account.as_deref())
            .await?;
        (accounts, has_more, false, dropped)
    };

    let next_cursor = accounts.last().cloned();
    let meta = PaginationMeta {
        has_more,
        truncated,
        next_cursor,
        dropped_rows: dropped_to_option(dropped),
    };

    Ok(HttpResponse::Ok().json(PaginatedResponse {
        data: accounts,
        meta,
    }))
}

/// List all distinct contract IDs
#[utoipa::path(
    get,
    path = "/v1/kv/contracts",
    params(ContractsQueryParams),
    responses(
        (status = 200, description = "List of contract IDs", body = inline(PaginatedResponse<String>)),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 429, description = "Too many requests", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "kv"
)]
#[get("/v1/kv/contracts")]
pub async fn contracts_handler(
    req: HttpRequest,
    query: web::Query<ContractsQueryParams>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let limit = query.limit.min(MAX_SCAN_LIMIT);
    validate_limit(limit)?;

    if let Some(ref cursor) = query.after_contract {
        validate_account_id(cursor, "after_contract")?;
    }

    let db = require_db(&app_state).await?;

    let (contracts, has_more, dropped) = if let Some(ref account_id) = query.predecessor_id {
        // Per-account query: cheap single-partition lookup, no throttle needed
        validate_account_id(account_id, "accountId")?;

        tracing::info!(
            target: PROJECT_ID,
            account_id = account_id,
            limit = limit,
            after_contract = ?query.after_contract,
            "GET /v1/kv/contracts (by account)"
        );

        db.query_contracts_by_account(account_id, limit, query.after_contract.as_deref())
            .await?
    } else {
        check_scan_throttle(&app_state, &extract_client_ip(&req))?;

        tracing::info!(
            target: PROJECT_ID,
            limit = limit,
            after_contract = ?query.after_contract,
            "GET /v1/kv/contracts (scan)"
        );

        db.query_all_contracts(limit, query.after_contract.as_deref())
            .await?
    };

    let next_cursor = contracts.last().cloned();
    let meta = PaginationMeta {
        has_more,
        truncated: false,
        next_cursor,
        dropped_rows: dropped_to_option(dropped),
    };

    Ok(HttpResponse::Ok().json(PaginatedResponse {
        data: contracts,
        meta,
    }))
}

/// Compare a key's value at two different block heights
#[utoipa::path(
    get,
    path = "/v1/kv/diff",
    params(DiffParams),
    responses(
        (status = 200, description = "Values at both block heights", body = inline(DataResponse<DiffResponse>)),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "kv"
)]
#[get("/v1/kv/diff")]
pub async fn diff_kv_handler(
    query: web::Query<DiffParams>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    validate_account_id(&query.predecessor_id, "accountId")?;
    validate_account_id(&query.current_account_id, "contractId")?;
    validate_key(&query.key, "key", MAX_KEY_LENGTH)?;
    if query.block_height_a < 0 || query.block_height_b < 0 {
        return Err(ApiError::InvalidParameter(
            "block_height_a/block_height_b: must be non-negative".to_string(),
        ));
    }

    tracing::info!(
        target: PROJECT_ID,
        accountId = %query.predecessor_id,
        contractId = %query.current_account_id,
        key = %query.key,
        block_height_a = query.block_height_a,
        block_height_b = query.block_height_b,
        "GET /v1/kv/diff"
    );

    let db = require_db(&app_state).await?;
    let (a, b) = futures::future::try_join(
        db.get_kv_at_block(
            &query.predecessor_id,
            &query.current_account_id,
            &query.key,
            query.block_height_a,
        ),
        db.get_kv_at_block(
            &query.predecessor_id,
            &query.current_account_id,
            &query.key,
            query.block_height_b,
        ),
    )
    .await?;

    let fields = parse_field_set(&query.fields)?;
    let decode = should_decode(&query.value_format)?;
    if fields.is_some() || decode {
        let mut a_json = a.as_ref().map(|e| e.to_json_with_fields(&fields));
        let mut b_json = b.as_ref().map(|e| e.to_json_with_fields(&fields));
        if decode {
            if let Some(ref mut v) = a_json {
                decode_value_in_json(v);
            }
            if let Some(ref mut v) = b_json {
                decode_value_in_json(v);
            }
        }
        Ok(HttpResponse::Ok().json(serde_json::json!({ "data": { "a": a_json, "b": b_json } })))
    } else {
        Ok(HttpResponse::Ok().json(DataResponse {
            data: DiffResponse { a, b },
        }))
    }
}

#[utoipa::path(
    get,
    path = "/v1/kv/timeline",
    params(TimelineParams),
    responses(
        (status = 200, description = "Chronological list of all writes", body = inline(PaginatedResponse<KvEntry>)),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "kv"
)]
#[get("/v1/kv/timeline")]
pub async fn timeline_kv_handler(
    query: web::Query<TimelineParams>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    validate_account_id(&query.predecessor_id, "accountId")?;
    validate_account_id(&query.current_account_id, "contractId")?;
    validate_limit(query.limit)?;
    validate_order(&query.order)?;
    validate_block_range(query.from_block, query.to_block)?;
    if let Some(ref c) = query.cursor {
        if c.len() > MAX_CURSOR_LENGTH {
            return Err(ApiError::InvalidParameter(
                "cursor: exceeds max length".to_string(),
            ));
        }
        if !c.is_empty() {
            parse_timeline_cursor(c)?;
        }
    }

    tracing::info!(
        target: PROJECT_ID,
        accountId = %query.predecessor_id,
        contractId = %query.current_account_id,
        limit = query.limit,
        cursor = ?query.cursor,
        order = %query.order,
        from_block = ?query.from_block,
        to_block = ?query.to_block,
        "GET /v1/kv/timeline"
    );

    let db = require_db(&app_state).await?;
    let (entries, has_more, dropped, next_cursor) = db.get_kv_timeline(&query).await?;

    let meta = PaginationMeta {
        has_more,
        truncated: false,
        next_cursor,
        dropped_rows: dropped_to_option(dropped),
    };
    let fields = parse_field_set(&query.fields)?;
    let decode = should_decode(&query.value_format)?;
    Ok(respond_paginated(entries, meta, &fields, decode))
}

/// Batch lookup: get values for multiple keys in a single request
#[utoipa::path(
    post,
    path = "/v1/kv/batch",
    request_body = BatchQuery,
    responses(
        (status = 200, description = "Batch results", body = inline(DataResponse<Vec<BatchResultItem>>)),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "kv"
)]
#[post("/v1/kv/batch")]
pub async fn batch_kv_handler(
    body: web::Json<BatchQuery>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    validate_account_id(&body.predecessor_id, "accountId")?;
    validate_account_id(&body.current_account_id, "contractId")?;
    if body.keys.is_empty() {
        return Err(ApiError::InvalidParameter(
            "keys: cannot be empty".to_string(),
        ));
    }
    if body.keys.len() > MAX_BATCH_KEYS {
        return Err(ApiError::InvalidParameter(format!(
            "keys: cannot exceed {MAX_BATCH_KEYS} items"
        )));
    }
    for key in &body.keys {
        if key.is_empty() {
            return Err(ApiError::InvalidParameter(
                "keys[]: cannot be empty".to_string(),
            ));
        }
        if key.len() > MAX_BATCH_KEY_LENGTH {
            return Err(ApiError::InvalidParameter(format!(
                "keys[]: cannot exceed {MAX_BATCH_KEY_LENGTH} characters"
            )));
        }
    }

    tracing::info!(
        target: PROJECT_ID,
        accountId = %body.predecessor_id,
        contractId = %body.current_account_id,
        key_count = body.keys.len(),
        "POST /v1/kv/batch"
    );

    // Verify DB is available before starting batch
    let _ = require_db(&app_state).await?;

    use futures::stream::{self, StreamExt};
    let items: Vec<BatchResultItem> = stream::iter(body.keys.iter().map(|key| {
        let scylladb = app_state.scylladb.clone();
        let predecessor_id = body.predecessor_id.clone();
        let current_account_id = body.current_account_id.clone();
        let key = key.clone();
        async move {
            let db = scylladb.read().await.clone();
            let Some(ref db) = db else {
                return BatchResultItem {
                    key,
                    found: false,
                    value: None,
                    error: Some("Database unavailable".to_string()),
                };
            };
            match db.get_kv_last(&predecessor_id, &current_account_id, &key).await {
                Ok(value) => BatchResultItem {
                    key,
                    found: value.is_some(),
                    value,
                    error: None,
                },
                Err(e) => {
                    // Log full error internally, return generic message to client
                    tracing::warn!(target: PROJECT_ID, error = %e, key = %key, "Batch key lookup failed");
                    BatchResultItem {
                        key,
                        found: false,
                        value: None,
                        error: Some("Lookup failed".to_string()),
                    }
                }
            }
        }
    }))
    .buffered(10)
    .collect()
    .await;

    Ok(HttpResponse::Ok().json(DataResponse { data: items }))
}

/// List edge sources for a given edge type and target
#[utoipa::path(
    get,
    path = "/v1/kv/edges",
    params(EdgesParams),
    responses(
        (status = 200, description = "List of edge sources", body = inline(PaginatedResponse<EdgeSourceEntry>)),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "kv"
)]
#[get("/v1/kv/edges")]
pub async fn edges_handler(
    query: web::Query<EdgesParams>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    validate_key(&query.edge_type, "edge_type", MAX_EDGE_TYPE_LENGTH)?;
    validate_account_id(&query.target, "target")?;
    validate_limit(query.limit)?;

    validate_cursor_or_offset(
        query.after_source.as_deref(),
        "after_source",
        query.offset,
        validate_account_id,
    )?;

    tracing::info!(
        target: PROJECT_ID,
        edge_type = %query.edge_type,
        target = %query.target,
        limit = query.limit,
        offset = query.offset,
        after_source = ?query.after_source,
        "GET /v1/kv/edges"
    );

    let db = require_db(&app_state).await?;
    let (sources, has_more, dropped) = db
        .query_edges(
            &query.edge_type,
            &query.target,
            query.limit,
            query.offset,
            query.after_source.as_deref(),
        )
        .await?;

    let next_cursor = sources.last().map(|e| e.source.clone());
    let meta = PaginationMeta {
        has_more,
        truncated: false,
        next_cursor,
        dropped_rows: dropped_to_option(dropped),
    };

    Ok(HttpResponse::Ok().json(PaginatedResponse {
        data: sources,
        meta,
    }))
}

/// Count edges for a given edge type and target
#[utoipa::path(
    get,
    path = "/v1/kv/edges/count",
    params(EdgesCountParams),
    responses(
        (status = 200, description = "Edge count", body = inline(DataResponse<EdgesCountResponse>)),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "kv"
)]
#[get("/v1/kv/edges/count")]
pub async fn edges_count_handler(
    query: web::Query<EdgesCountParams>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    validate_key(&query.edge_type, "edge_type", MAX_EDGE_TYPE_LENGTH)?;
    validate_account_id(&query.target, "target")?;

    tracing::info!(
        target: PROJECT_ID,
        edge_type = %query.edge_type,
        target = %query.target,
        "GET /v1/kv/edges/count"
    );

    let db = require_db(&app_state).await?;
    let count = db.count_edges(&query.edge_type, &query.target).await?;

    Ok(HttpResponse::Ok().json(DataResponse {
        data: EdgesCountResponse {
            edge_type: query.edge_type.clone(),
            target: query.target.clone(),
            count,
        },
    }))
}

/// Watch a key for changes via Server-Sent Events (SSE).
///
/// Returns a `text/event-stream` that emits `change` events whenever the
/// watched key's block height advances.  Supports `Last-Event-ID` for
/// reconnection.  Server limits concurrent watches to `MAX_CONCURRENT_WATCHES`.
#[utoipa::path(
    get,
    path = "/v1/kv/watch",
    params(WatchParams),
    responses(
        (status = 200, description = "SSE event stream", content_type = "text/event-stream"),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 429, description = "Too many watch connections", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "kv"
)]
#[get("/v1/kv/watch")]
pub async fn watch_kv_handler(
    query: web::Query<WatchParams>,
    app_state: web::Data<AppState>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    validate_account_id(&query.predecessor_id, "accountId")?;
    validate_account_id(&query.current_account_id, "contractId")?;
    validate_key(&query.key, "key", MAX_KEY_LENGTH)?;

    let poll_secs = query.interval.clamp(MIN_POLL_INTERVAL, MAX_POLL_INTERVAL);

    // Atomically claim a watch slot; rollback if over limit
    let prev = app_state
        .watch_count
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if prev >= MAX_CONCURRENT_WATCHES {
        app_state
            .watch_count
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        return Err(ApiError::TooManyRequests(
            "Too many active watch connections".to_string(),
        ));
    }

    // RAII guard: created immediately after incrementing watch_count so that
    // early disconnects (before the stream is polled) still decrement.
    let guard = WatchGuard(app_state.watch_count.clone());

    // Verify DB is available (guard's Drop handles rollback on error)
    let _ = require_db(&app_state).await?;

    tracing::info!(
        target: PROJECT_ID,
        accountId = %query.predecessor_id,
        contractId = %query.current_account_id,
        key = %query.key,
        interval = poll_secs,
        "GET /v1/kv/watch (SSE)"
    );

    // Support Last-Event-ID for reconnection
    let last_block: Option<u64> = req
        .headers()
        .get("Last-Event-ID")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok());

    let scylladb = app_state.scylladb.clone();
    let predecessor_id = query.predecessor_id.clone();
    let current_account_id = query.current_account_id.clone();
    let key = query.key.clone();

    let stream = async_stream::stream! {
        let _guard = guard; // move RAII guard into the stream so it lives until disconnect
        let mut last_known_block = last_block.unwrap_or(0);
        let mut poll_interval = tokio::time::interval(Duration::from_secs(poll_secs));
        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(SSE_HEARTBEAT_SECS));

        loop {
            tokio::select! {
                _ = poll_interval.tick() => {
                    // Clone the Arc and drop the guard before awaiting DB call,
                    // so the RwLock is not held across .await (blocks reconnection).
                    let db = scylladb.read().await.clone();
                    if let Some(ref db) = db {
                        match db.get_kv(&predecessor_id, &current_account_id, &key).await {
                            Ok(Some(entry)) if entry.block_height > last_known_block => {
                                last_known_block = entry.block_height;
                                let event = WatchEvent {
                                    key: entry.key,
                                    value: entry.value,
                                    block_height: entry.block_height,
                                    block_timestamp: entry.block_timestamp,
                                    predecessor_id: entry.predecessor_id.clone(),
                                    current_account_id: entry.current_account_id.clone(),
                                };
                                if let Ok(data) = serde_json::to_string(&event) {
                                    let msg = format!("id: {}\nevent: change\ndata: {}\n\n", last_known_block, data);
                                    yield Ok::<actix_web::web::Bytes, actix_web::Error>(actix_web::web::Bytes::from(msg));
                                }
                            }
                            Ok(_) => {} // No change
                            Err(e) => {
                                tracing::warn!(target: PROJECT_ID, error = %e, "Watch poll error");
                                let msg = "event: error\ndata: {\"error\":\"poll_failed\"}\n\n";
                                yield Ok(actix_web::web::Bytes::from(msg));
                            }
                        }
                    } else {
                        let msg = "event: error\ndata: {\"error\":\"database_unavailable\"}\n\n";
                        yield Ok(actix_web::web::Bytes::from(msg));
                    }
                }
                _ = heartbeat_interval.tick() => {
                    yield Ok(actix_web::web::Bytes::from(": heartbeat\n\n"));
                }
            }
        }
    };

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream))
}

/// RAII guard that decrements the watch counter when the SSE stream drops.
struct WatchGuard(std::sync::Arc<std::sync::atomic::AtomicUsize>);
impl Drop for WatchGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Indexer status: block height and server time
#[utoipa::path(
    get,
    path = "/v1/status",
    responses(
        (status = 200, description = "Indexer status", body = StatusResponse),
    ),
    tag = "kv"
)]
#[get("/v1/status")]
pub async fn status_handler(app_state: web::Data<AppState>) -> HttpResponse {
    let db = app_state.scylladb.read().await.clone();
    let indexer_block = match db.as_ref() {
        Some(db) => db.get_indexer_block_height().await.ok().flatten(),
        None => None,
    };

    HttpResponse::Ok().json(StatusResponse {
        indexer_block,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}
