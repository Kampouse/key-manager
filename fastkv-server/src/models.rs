use actix_web::{error::ResponseError, http::StatusCode, HttpResponse};
use scylla::DeserializeRow;
use serde::{Deserialize, Serialize};
use std::fmt;

// Shared validation constants
pub const MAX_OFFSET: usize = 100_000;
pub const MAX_PREFIX_LENGTH: usize = 1000;
pub const MAX_ACCOUNT_ID_LENGTH: usize = 256;
pub const MAX_KEY_LENGTH: usize = 10000;
pub const MAX_BATCH_KEYS: usize = 100;
pub const MAX_BATCH_KEY_LENGTH: usize = 1024;
pub const MAX_SOCIAL_RESULTS: usize = 1000;
pub const MAX_SOCIAL_KEYS: usize = 100;
pub const MAX_STREAM_ERRORS: usize = 10;
pub const MAX_DEDUP_SCAN: usize = 100_000;
pub const MAX_EDGE_TYPE_LENGTH: usize = 256;
pub const MAX_SCAN_LIMIT: usize = 1000;
pub const MAX_CURSOR_LENGTH: usize = 1024;
pub const PROJECT_ID: &str = "near-garden";

// Raw row from ScyllaDB s_kv_last (matches table schema exactly)
#[derive(DeserializeRow, Debug, Clone)]
pub struct KvRow {
    pub predecessor_id: String,
    pub current_account_id: String,
    pub key: String,
    pub value: String,
    pub block_height: i64,
    pub block_timestamp: i64,
    pub receipt_id: String,
    pub tx_hash: String,
}

// Raw row from ScyllaDB s_kv (history table with additional fields)
#[derive(DeserializeRow, Debug, Clone)]
pub struct KvHistoryRow {
    pub predecessor_id: String,
    pub current_account_id: String,
    pub key: String,
    pub block_height: i64,
    pub order_id: i64,
    pub value: String,
    pub block_timestamp: i64,
    pub receipt_id: String,
    pub tx_hash: String,
    pub signer_id: String,
    pub shard_id: i32,
    pub receipt_index: i32,
    pub action_index: i32,
}

// Row from s_kv_by_block table (9 columns, no signer/shard/receipt_index/action_index)
#[derive(DeserializeRow, Debug, Clone)]
pub struct KvTimelineRow {
    pub predecessor_id: String,
    pub current_account_id: String,
    pub block_height: i64,
    pub key: String,
    pub order_id: i64,
    pub value: String,
    pub block_timestamp: i64,
    pub receipt_id: String,
    pub tx_hash: String,
}

// Lightweight row for contract-based account queries (predecessor_id only)
#[derive(DeserializeRow, Debug, Clone)]
pub struct ContractAccountRow {
    pub predecessor_id: String,
}

// Row for contract listing from kv_accounts (deduplicated in app code)
#[derive(DeserializeRow, Debug, Clone)]
pub struct ContractRow {
    pub current_account_id: String,
}

// Row for contract listing from s_kv_last (includes key clustering column)
#[derive(DeserializeRow, Debug, Clone)]
pub struct ContractKeyRow {
    pub current_account_id: String,
    #[allow(dead_code)]
    pub key: String,
}

// API response
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KvEntry {
    #[serde(rename = "accountId")]
    pub predecessor_id: String,
    #[serde(rename = "contractId")]
    pub current_account_id: String,
    pub key: String,
    pub value: String,
    pub block_height: u64,
    pub block_timestamp: u64,
    pub receipt_id: String,
    pub tx_hash: String,
    /// True when the entry represents a deletion (value is the literal string "null").
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_deleted: bool,
}

impl KvEntry {
    /// Convert to JSON with only requested fields. Pass a pre-built HashSet to avoid
    /// rebuilding it per entry when called in a loop.
    pub fn to_json_with_fields(
        &self,
        fields: &Option<std::collections::HashSet<String>>,
    ) -> serde_json::Value {
        if let Some(field_set) = fields {
            let mut map = serde_json::Map::new();

            if field_set.contains("accountId") {
                map.insert(
                    "accountId".to_string(),
                    serde_json::json!(&self.predecessor_id),
                );
            }
            if field_set.contains("contractId") {
                map.insert(
                    "contractId".to_string(),
                    serde_json::json!(&self.current_account_id),
                );
            }
            if field_set.contains("key") {
                map.insert("key".to_string(), serde_json::json!(&self.key));
            }
            if field_set.contains("value") {
                map.insert("value".to_string(), serde_json::json!(&self.value));
            }
            if field_set.contains("blockHeight") {
                map.insert(
                    "blockHeight".to_string(),
                    serde_json::json!(self.block_height),
                );
            }
            if field_set.contains("blockTimestamp") {
                map.insert(
                    "blockTimestamp".to_string(),
                    serde_json::json!(self.block_timestamp),
                );
            }
            if field_set.contains("receiptId") {
                map.insert(
                    "receiptId".to_string(),
                    serde_json::json!(&self.receipt_id),
                );
            }
            if field_set.contains("txHash") {
                map.insert("txHash".to_string(), serde_json::json!(&self.tx_hash));
            }
            if field_set.contains("isDeleted") && self.is_deleted {
                map.insert("isDeleted".to_string(), serde_json::json!(true));
            }

            serde_json::Value::Object(map)
        } else {
            // No field filtering - return all fields
            serde_json::to_value(self).unwrap_or_else(|e| {
                tracing::error!(target: "fastkv-server", error = %e, "Failed to serialize KvEntry");
                serde_json::Value::Null
            })
        }
    }
}

/// Convert a ScyllaDB bigint (i64) to u64, clamping negatives to 0.
/// ScyllaDB stores block heights/timestamps as bigint (i64) but they are
/// logically unsigned. Negative values indicate upstream data issues.
pub fn bigint_to_u64(val: i64) -> u64 {
    val.max(0) as u64
}

impl From<KvRow> for KvEntry {
    fn from(row: KvRow) -> Self {
        let is_deleted = row.value == "null";
        Self {
            predecessor_id: row.predecessor_id,
            current_account_id: row.current_account_id,
            key: row.key,
            value: row.value,
            block_height: bigint_to_u64(row.block_height),
            block_timestamp: bigint_to_u64(row.block_timestamp),
            receipt_id: row.receipt_id,
            tx_hash: row.tx_hash,
            is_deleted,
        }
    }
}

impl From<KvHistoryRow> for KvEntry {
    fn from(row: KvHistoryRow) -> Self {
        let is_deleted = row.value == "null";
        Self {
            predecessor_id: row.predecessor_id,
            current_account_id: row.current_account_id,
            key: row.key,
            value: row.value,
            block_height: bigint_to_u64(row.block_height),
            block_timestamp: bigint_to_u64(row.block_timestamp),
            receipt_id: row.receipt_id,
            tx_hash: row.tx_hash,
            is_deleted,
        }
    }
}

impl From<KvTimelineRow> for KvEntry {
    fn from(row: KvTimelineRow) -> Self {
        let is_deleted = row.value == "null";
        Self {
            predecessor_id: row.predecessor_id,
            current_account_id: row.current_account_id,
            key: row.key,
            value: row.value,
            block_height: bigint_to_u64(row.block_height),
            block_timestamp: bigint_to_u64(row.block_timestamp),
            receipt_id: row.receipt_id,
            tx_hash: row.tx_hash,
            is_deleted,
        }
    }
}

// Pagination metadata returned in all paginated responses
#[derive(Serialize, utoipa::ToSchema)]
pub struct PaginationMeta {
    pub has_more: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    #[schema(default = false)]
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Number of rows skipped due to deserialization errors. Omitted when zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dropped_rows: Option<u32>,
}

// Standardized paginated response for all list endpoints
#[derive(Serialize, utoipa::ToSchema)]
pub struct PaginatedResponse<T: Serialize + utoipa::ToSchema> {
    pub data: Vec<T>,
    pub meta: PaginationMeta,
}

// Standardized single-item response wrapper
#[derive(Serialize, utoipa::ToSchema)]
pub struct DataResponse<T: Serialize + utoipa::ToSchema> {
    pub data: T,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct TreeResponse {
    pub tree: serde_json::Value,
    /// True when results were capped by the limit parameter.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub has_more: bool,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct HealthResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
}

// Query parameter structs
#[derive(Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct GetParams {
    #[serde(rename = "accountId")]
    pub predecessor_id: String,
    #[serde(rename = "contractId")]
    pub current_account_id: String,
    pub key: String,
    #[serde(default)]
    pub fields: Option<String>, // Comma-separated field names
    /// Value format: "raw" (default) or "json" (decoded).
    #[serde(default)]
    pub value_format: Option<String>,
}

const VALID_FIELDS: &[&str] = &[
    "accountId",
    "contractId",
    "key",
    "value",
    "blockHeight",
    "blockTimestamp",
    "receiptId",
    "txHash",
    "isDeleted",
];

/// Parse a comma-separated fields string into a set of field names.
/// Returns 400 if any field name is not in the valid set.
pub fn parse_field_set(
    fields: &Option<String>,
) -> Result<Option<std::collections::HashSet<String>>, ApiError> {
    match fields.as_ref() {
        None => Ok(None),
        Some(f) => {
            let set: std::collections::HashSet<String> = f
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let invalid: Vec<&str> = set
                .iter()
                .filter(|s| !VALID_FIELDS.contains(&s.as_str()))
                .map(|s| s.as_str())
                .collect();
            if !invalid.is_empty() {
                return Err(ApiError::InvalidParameter(format!(
                    "fields: unknown field(s): {}. Valid: {}",
                    invalid.join(", "),
                    VALID_FIELDS.join(", ")
                )));
            }
            Ok(if set.is_empty() { None } else { Some(set) })
        }
    }
}

/// Convert a dropped-row count to `Option<u32>`, returning `None` for zero.
pub(crate) fn dropped_to_option(n: usize) -> Option<u32> {
    if n > 0 {
        Some(n.min(u32::MAX as usize) as u32)
    } else {
        None
    }
}

/// Resolve whether to decode values based on `value_format`.
pub fn should_decode(value_format: &Option<String>) -> Result<bool, ApiError> {
    match value_format.as_deref() {
        Some("json") => Ok(true),
        Some("raw") | None => Ok(false),
        Some(other) => Err(ApiError::InvalidParameter(format!(
            "value_format: must be 'json' or 'raw' (got '{other}')"
        ))),
    }
}

pub fn parse_history_cursor(cursor: &str) -> Result<(i64, i64), ApiError> {
    let (bh_str, oid_str) = cursor.split_once(':').ok_or_else(|| {
        ApiError::InvalidParameter("cursor: expected format block_height:order_id".to_string())
    })?;
    let block_height: i64 = bh_str.parse().map_err(|_| {
        ApiError::InvalidParameter("cursor: block_height must be a non-negative integer".to_string())
    })?;
    let order_id: i64 = oid_str.parse().map_err(|_| {
        ApiError::InvalidParameter("cursor: order_id must be an integer".to_string())
    })?;
    if block_height < 0 {
        return Err(ApiError::InvalidParameter(
            "cursor: block_height must be non-negative".to_string(),
        ));
    }
    Ok((block_height, order_id))
}

pub fn parse_timeline_cursor(cursor: &str) -> Result<(i64, String), ApiError> {
    let (bh_str, key) = cursor.split_once(':').ok_or_else(|| {
        ApiError::InvalidParameter("cursor: expected format block_height:key".to_string())
    })?;
    let block_height: i64 = bh_str.parse().map_err(|_| {
        ApiError::InvalidParameter("cursor: block_height must be a non-negative integer".to_string())
    })?;
    if block_height < 0 {
        return Err(ApiError::InvalidParameter(
            "cursor: block_height must be non-negative".to_string(),
        ));
    }
    Ok((block_height, key.to_string()))
}

pub fn validate_limit(limit: usize) -> Result<(), ApiError> {
    if limit == 0 || limit > 1000 {
        return Err(ApiError::InvalidParameter(
            "limit: must be between 1 and 1000".to_string(),
        ));
    }
    Ok(())
}

#[derive(Deserialize, Clone, utoipa::ToSchema, utoipa::IntoParams)]
pub struct QueryParams {
    #[serde(rename = "accountId")]
    pub predecessor_id: String,
    #[serde(rename = "contractId")]
    pub current_account_id: String,
    #[serde(default)]
    pub key_prefix: Option<String>,
    #[serde(default)]
    pub exclude_deleted: Option<bool>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    pub fields: Option<String>, // Comma-separated field names
    /// Response format. Use `"tree"` for nested JSON; omit for paginated list.
    #[serde(default)]
    pub format: Option<String>,
    /// Value format: "raw" (default) or "json" (decoded).
    #[serde(default)]
    pub value_format: Option<String>,
    /// Cursor: return entries with key alphabetically after this value (exclusive).
    /// Cannot be combined with offset > 0.
    #[serde(default)]
    pub after_key: Option<String>,
}

// GET /v1/kv/writers — replaces /v1/kv/reverse and /v1/kv/by-key
#[derive(Deserialize, Clone, utoipa::ToSchema, utoipa::IntoParams)]
pub struct WritersParams {
    #[serde(rename = "contractId")]
    pub current_account_id: String,
    pub key: String,
    /// Optional: filter to a specific writer account
    #[serde(rename = "accountId")]
    #[serde(default)]
    pub predecessor_id: Option<String>,
    #[serde(default)]
    pub exclude_deleted: Option<bool>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    pub fields: Option<String>,
    /// Value format: "raw" (default) or "json" (decoded).
    #[serde(default)]
    pub value_format: Option<String>,
    /// Cursor: return writers with account ID alphabetically after this value (exclusive).
    /// Cannot be combined with offset > 0.
    #[serde(default)]
    pub after_account: Option<String>,
}

fn default_limit() -> usize {
    100
}

#[derive(Deserialize, Clone, utoipa::ToSchema, utoipa::IntoParams)]
pub struct HistoryParams {
    #[serde(rename = "accountId")]
    pub predecessor_id: String,
    #[serde(rename = "contractId")]
    pub current_account_id: String,
    pub key: String,
    #[serde(default = "default_history_limit")]
    pub limit: usize,
    #[serde(default = "default_order_desc")]
    pub order: String,
    #[serde(default)]
    pub from_block: Option<i64>,
    #[serde(default)]
    pub to_block: Option<i64>,
    #[serde(default)]
    pub fields: Option<String>,
    #[serde(default)]
    pub value_format: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
}

fn default_history_limit() -> usize {
    100
}

fn default_order_desc() -> String {
    "desc".to_string()
}

// Internal accounts query parameters (used by social handlers, not exposed in API)
#[derive(Deserialize, Clone)]
pub struct AccountsParams {
    #[serde(rename = "contractId")]
    pub current_account_id: String,
    pub key: String,
    #[serde(default)]
    pub exclude_deleted: Option<bool>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    /// Cursor: return accounts alphabetically after this value (exclusive).
    #[serde(default)]
    pub after_account: Option<String>,
}

// Accounts-by-contract query parameters
#[derive(Deserialize, Clone, utoipa::ToSchema, utoipa::IntoParams)]
pub struct AccountsQueryParams {
    /// Contract account. When omitted, performs a full table scan (throttled).
    #[serde(rename = "contractId", default)]
    pub contract_id: Option<String>,
    /// Optional key filter. Recommended for large contracts to avoid expensive full-partition scans.
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    /// Cursor: return accounts after this value (exclusive).
    /// Token-ordered (Murmur3) in scan mode; lexicographic with contractId.
    /// Cannot be combined with offset > 0.
    /// Note: responses always emit next_cursor (even when has_more is false);
    /// use it for resumption, especially when truncated=true.
    #[serde(default)]
    pub after_account: Option<String>,
}

// Contracts listing query parameters
#[derive(Deserialize, Clone, utoipa::ToSchema, utoipa::IntoParams)]
pub struct ContractsQueryParams {
    /// Optional: list contracts this account has written to (single-partition, cheap).
    /// When omitted, lists all contracts globally (full scan, throttled).
    #[serde(rename = "accountId", default)]
    pub predecessor_id: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Cursor: return contracts after this value (TOKEN-ordered when global, lexicographic when per-account).
    #[serde(default)]
    pub after_contract: Option<String>,
}

// Diff query parameters
#[derive(Deserialize, Clone, utoipa::ToSchema, utoipa::IntoParams)]
pub struct DiffParams {
    #[serde(rename = "accountId")]
    pub predecessor_id: String,
    #[serde(rename = "contractId")]
    pub current_account_id: String,
    pub key: String,
    pub block_height_a: i64,
    pub block_height_b: i64,
    #[serde(default)]
    pub fields: Option<String>,
    /// Value format: "raw" (default) or "json" (decoded).
    #[serde(default)]
    pub value_format: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct DiffResponse {
    pub a: Option<KvEntry>,
    pub b: Option<KvEntry>,
}

#[derive(Deserialize, Clone, utoipa::ToSchema, utoipa::IntoParams)]
pub struct TimelineParams {
    #[serde(rename = "accountId")]
    pub predecessor_id: String,
    #[serde(rename = "contractId")]
    pub current_account_id: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_order_desc")]
    pub order: String,
    #[serde(default)]
    pub from_block: Option<i64>,
    #[serde(default)]
    pub to_block: Option<i64>,
    #[serde(default)]
    pub fields: Option<String>,
    #[serde(default)]
    pub value_format: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
}

// Batch query structs
#[derive(Deserialize, utoipa::ToSchema)]
pub struct BatchQuery {
    #[serde(rename = "accountId")]
    pub predecessor_id: String,
    #[serde(rename = "contractId")]
    pub current_account_id: String,
    pub keys: Vec<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct BatchResultItem {
    pub key: String,
    pub value: Option<String>,
    pub found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ===== Social API types =====

// POST /v1/social/get request body
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SocialGetBody {
    pub keys: Vec<String>,
    #[serde(default)]
    #[serde(alias = "contractId")]
    pub contract_id: Option<String>,
    #[serde(default)]
    pub options: Option<SocialGetOptions>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct SocialGetOptions {
    #[serde(default)]
    pub with_block_height: Option<bool>,
    #[serde(default)]
    pub return_deleted: Option<bool>,
}

// POST /v1/social/keys request body
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SocialKeysBody {
    pub keys: Vec<String>,
    #[serde(default)]
    #[serde(alias = "contractId")]
    pub contract_id: Option<String>,
    #[serde(default)]
    pub options: Option<SocialKeysOptions>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct SocialKeysOptions {
    #[serde(default)]
    pub return_type: Option<String>, // "True" | "BlockHeight"
    #[serde(default)]
    pub return_deleted: Option<bool>,
    #[serde(default)]
    pub values_only: Option<bool>,
}

// GET /v1/social/index query params
#[derive(Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct SocialIndexParams {
    pub action: String,
    pub key: String,
    #[serde(default = "default_order_desc")]
    pub order: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub from: Option<u64>, // block_height cursor
    #[serde(default)]
    #[serde(alias = "accountId")]
    pub account_id: Option<String>,
    #[serde(default)]
    #[serde(alias = "contractId")]
    pub contract_id: Option<String>,
}

// GET /v1/social/profile query params
#[derive(Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct SocialProfileParams {
    #[serde(alias = "accountId")]
    pub account_id: String,
    #[serde(default)]
    #[serde(alias = "contractId")]
    pub contract_id: Option<String>,
}

// GET /v1/social/followers and /v1/social/following query params
#[derive(Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct SocialFollowParams {
    #[serde(alias = "accountId")]
    pub account_id: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    #[serde(alias = "contractId")]
    pub contract_id: Option<String>,
    /// Cursor: return accounts alphabetically after this value (exclusive).
    /// Cannot be combined with offset > 0.
    #[serde(default)]
    pub after_account: Option<String>,
}

// GET /v1/social/feed/account query params
#[derive(Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct SocialAccountFeedParams {
    #[serde(alias = "accountId")]
    pub account_id: String,
    #[serde(default = "default_order_desc")]
    pub order: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub from: Option<u64>,
    #[serde(default)]
    pub include_replies: Option<bool>,
    #[serde(default)]
    #[serde(alias = "contractId")]
    pub contract_id: Option<String>,
}

// Social API response types
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct IndexEntry {
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "blockHeight")]
    pub block_height: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SocialFollowResponse {
    pub data: Vec<String>,
    pub count: usize,
    pub meta: PaginationMeta,
}

// Error handling

/// Machine-readable error codes for API responses.
#[derive(Debug, Clone, Copy, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidParameter,
    DatabaseError,
    DatabaseUnavailable,
    TooManyRequests,
}

/// Structured error response returned by all endpoints on failure.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ErrorResponse {
    pub error: String,
    pub code: ErrorCode,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub enum ApiError {
    InvalidParameter(String),
    DatabaseError(String),
    DatabaseUnavailable,
    TooManyRequests(String),
}

impl ApiError {
    pub fn code(&self) -> ErrorCode {
        match self {
            ApiError::InvalidParameter(_) => ErrorCode::InvalidParameter,
            ApiError::DatabaseError(_) => ErrorCode::DatabaseError,
            ApiError::DatabaseUnavailable => ErrorCode::DatabaseUnavailable,
            ApiError::TooManyRequests(_) => ErrorCode::TooManyRequests,
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ApiError::InvalidParameter(msg) => write!(f, "Invalid parameter: {}", msg),
            ApiError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            ApiError::DatabaseUnavailable => write!(f, "Database unavailable"),
            ApiError::TooManyRequests(msg) => write!(f, "{}", msg),
        }
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        let status = match self {
            ApiError::InvalidParameter(_) => StatusCode::BAD_REQUEST,
            ApiError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::DatabaseUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
        };

        let mut response = HttpResponse::build(status);
        if matches!(self, ApiError::TooManyRequests(_)) {
            response.insert_header(("Retry-After", "1"));
        }
        response.json(ErrorResponse {
            error: self.to_string(),
            code: self.code(),
        })
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        // Log full error internally for debugging, but return generic message to client
        // to prevent information disclosure (paths, IPs, schema details)
        tracing::error!(
            target: "fastkv-server",
            error = %err,
            "Database error occurred"
        );
        ApiError::DatabaseError("An internal database error occurred".to_string())
    }
}

// ===== Edges API types =====

// Raw row from ScyllaDB kv_edges table
#[derive(DeserializeRow, Debug, Clone)]
pub struct EdgeRow {
    pub source: String,
    pub block_height: i64,
}

// GET /v1/kv/edges query params
#[derive(Deserialize, Clone, utoipa::ToSchema, utoipa::IntoParams)]
pub struct EdgesParams {
    pub edge_type: String,
    pub target: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    /// Cursor: return sources alphabetically after this value (exclusive).
    /// Cannot be combined with offset > 0.
    #[serde(default)]
    pub after_source: Option<String>,
}

// GET /v1/kv/edges/count query params
#[derive(Deserialize, Clone, utoipa::ToSchema, utoipa::IntoParams)]
pub struct EdgesCountParams {
    pub edge_type: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EdgeSourceEntry {
    pub source: String,
    pub block_height: u64,
}

// StatusResponse for /v1/status
#[derive(Serialize, utoipa::ToSchema)]
pub struct StatusResponse {
    pub indexer_block: Option<u64>,
    pub timestamp: String,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EdgesCountResponse {
    pub edge_type: String,
    pub target: String,
    pub count: usize,
}

// ===== SSE Watch API types =====

pub const MAX_CONCURRENT_WATCHES: usize = 100;
pub const MIN_POLL_INTERVAL: u64 = 2;
pub const MAX_POLL_INTERVAL: u64 = 30;
pub const SSE_HEARTBEAT_SECS: u64 = 15;

/// Parameters for the SSE key watch endpoint.
#[derive(Deserialize, Clone, utoipa::ToSchema, utoipa::IntoParams)]
pub struct WatchParams {
    /// NEAR account that wrote the data (signer/predecessor).
    #[serde(rename = "accountId")]
    pub predecessor_id: String,
    /// Contract where the data is stored.
    #[serde(rename = "contractId")]
    pub current_account_id: String,
    /// Key to watch for changes.
    pub key: String,
    /// Poll interval in seconds (default 5, clamped to 2–30).
    #[serde(default = "default_watch_interval")]
    pub interval: u64,
}

fn default_watch_interval() -> u64 {
    5
}

/// SSE event payload emitted when a watched key changes.
#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WatchEvent {
    pub key: String,
    pub value: String,
    pub block_height: u64,
    pub block_timestamp: u64,
    #[serde(rename = "accountId")]
    pub predecessor_id: String,
    #[serde(rename = "contractId")]
    pub current_account_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kv_entry_from_row() {
        let row = KvRow {
            predecessor_id: "alice.near".to_string(),
            current_account_id: "social.near".to_string(),
            key: "profile".to_string(),
            value: "test".to_string(),
            block_height: 123456789,
            block_timestamp: 1234567890123456789,
            receipt_id: "abc123".to_string(),
            tx_hash: "def456".to_string(),
        };

        let entry: KvEntry = row.into();
        assert_eq!(entry.predecessor_id, "alice.near");
        assert_eq!(entry.current_account_id, "social.near");
        assert_eq!(entry.key, "profile");
        assert_eq!(entry.block_height, 123456789);
        assert_eq!(entry.block_timestamp, 1234567890123456789);
        assert_eq!(entry.receipt_id, "abc123");
        assert_eq!(entry.tx_hash, "def456");
        assert!(!entry.is_deleted);
    }

    #[test]
    fn test_kv_entry_is_deleted() {
        let row = KvRow {
            predecessor_id: "alice.near".to_string(),
            current_account_id: "social.near".to_string(),
            key: "profile".to_string(),
            value: "null".to_string(),
            block_height: 100,
            block_timestamp: 200,
            receipt_id: "r".to_string(),
            tx_hash: "t".to_string(),
        };

        let entry: KvEntry = row.into();
        assert!(entry.is_deleted);

        // Verify isDeleted is serialized when true (camelCase)
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["isDeleted"], true);
    }

    #[test]
    fn test_kv_entry_is_deleted_omitted_when_false() {
        let row = KvRow {
            predecessor_id: "alice.near".to_string(),
            current_account_id: "social.near".to_string(),
            key: "profile".to_string(),
            value: "\"hello\"".to_string(),
            block_height: 100,
            block_timestamp: 200,
            receipt_id: "r".to_string(),
            tx_hash: "t".to_string(),
        };

        let entry: KvEntry = row.into();
        assert!(!entry.is_deleted);

        // Verify isDeleted is omitted when false (camelCase)
        let json = serde_json::to_value(&entry).unwrap();
        assert!(json.get("isDeleted").is_none());
    }

    #[test]
    fn test_should_decode() {
        assert!(should_decode(&Some("json".to_string())).unwrap());
        assert!(!should_decode(&Some("raw".to_string())).unwrap());
        assert!(!should_decode(&None).unwrap());
        // Invalid value_format
        assert!(should_decode(&Some("invalid".to_string())).is_err());
    }

    #[test]
    fn test_default_limit() {
        assert_eq!(default_limit(), 100);
    }

    #[test]
    fn test_bigint_to_u64_negative() {
        assert_eq!(bigint_to_u64(-1), 0);
        assert_eq!(bigint_to_u64(i64::MIN), 0);
        assert_eq!(bigint_to_u64(0), 0);
        assert_eq!(bigint_to_u64(42), 42);
    }

    #[test]
    fn test_pagination_meta_serialization() {
        let meta = PaginationMeta {
            has_more: true,
            truncated: false,
            next_cursor: Some("abc".to_string()),
            dropped_rows: None,
        };
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["has_more"], true);
        assert!(json.get("truncated").is_none()); // skipped when false
        assert_eq!(json["next_cursor"], "abc");
        assert!(json.get("dropped_rows").is_none()); // skipped when None

        let meta_no_cursor = PaginationMeta {
            has_more: false,
            truncated: true,
            next_cursor: None,
            dropped_rows: None,
        };
        let json = serde_json::to_value(&meta_no_cursor).unwrap();
        assert_eq!(json["truncated"], true);
        assert!(json.get("next_cursor").is_none()); // skipped when None
    }

    #[test]
    fn test_pagination_meta_cursor_without_has_more() {
        let meta = PaginationMeta {
            has_more: false,
            truncated: false,
            next_cursor: Some("last_key".to_string()),
            dropped_rows: None,
        };
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["has_more"], false);
        assert!(json.get("truncated").is_none());
        assert_eq!(json["next_cursor"], "last_key");
    }

    #[test]
    fn test_pagination_meta_dropped_rows_present_when_some() {
        let meta = PaginationMeta {
            has_more: true,
            truncated: false,
            next_cursor: None,
            dropped_rows: Some(3),
        };
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["dropped_rows"], 3);
    }

    #[test]
    fn test_parse_field_set_valid() {
        let input = Some("key,value,blockHeight".to_string());
        let result = parse_field_set(&input).unwrap().unwrap();
        assert!(result.contains("key"));
        assert!(result.contains("value"));
        assert!(result.contains("blockHeight"));
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_parse_field_set_invalid() {
        let input = Some("key,bogus".to_string());
        assert!(parse_field_set(&input).is_err());
    }

    #[test]
    fn test_parse_field_set_none() {
        assert!(parse_field_set(&None).unwrap().is_none());
    }

    #[test]
    fn test_parse_field_set_empty_string() {
        let input = Some("".to_string());
        assert!(parse_field_set(&input).unwrap().is_none());
    }

    #[test]
    fn test_error_response_serialization() {
        let resp = ErrorResponse {
            error: "test".to_string(),
            code: ErrorCode::InvalidParameter,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["error"], "test");
        assert_eq!(json["code"], "INVALID_PARAMETER");
    }

    #[test]
    fn test_parse_history_cursor() {
        let (bh, oid) = parse_history_cursor("139000500:3").unwrap();
        assert_eq!(bh, 139000500);
        assert_eq!(oid, 3);
    }

    #[test]
    fn test_parse_history_cursor_invalid() {
        assert!(parse_history_cursor("abc:3").is_err());
        assert!(parse_history_cursor("123").is_err());
        assert!(parse_history_cursor("").is_err());
        assert!(parse_history_cursor("-1:3").is_err());
    }

    #[test]
    fn test_parse_timeline_cursor() {
        let (bh, key) = parse_timeline_cursor("139000500:profile/name").unwrap();
        assert_eq!(bh, 139000500);
        assert_eq!(key, "profile/name");
    }

    #[test]
    fn test_parse_timeline_cursor_colon_in_key() {
        let (bh, key) = parse_timeline_cursor("100:key:with:colons").unwrap();
        assert_eq!(bh, 100);
        assert_eq!(key, "key:with:colons");
    }

    #[test]
    fn test_parse_timeline_cursor_invalid() {
        assert!(parse_timeline_cursor("abc:key").is_err());
        assert!(parse_timeline_cursor("123").is_err());
        assert!(parse_timeline_cursor("-1:key").is_err());
    }
}
