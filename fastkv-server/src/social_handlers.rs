use actix_web::{get, post, web, HttpResponse};

use crate::handlers::validate_account_id;
use crate::models::*;
use crate::AppState;

use std::sync::LazyLock;

static SOCIAL_CONTRACT: LazyLock<String> = LazyLock::new(|| {
    std::env::var("SOCIAL_CONTRACT").unwrap_or_else(|_| "social.near".to_string())
});

fn resolve_contract(contract_id: &Option<String>) -> Result<&str, ApiError> {
    match contract_id {
        Some(id) => {
            validate_account_id(id, "contract_id")?;
            Ok(id.as_str())
        }
        None => Ok(&SOCIAL_CONTRACT),
    }
}

// POST /v1/social/get - get values for multiple keys
#[utoipa::path(
    post,
    path = "/v1/social/get",
    request_body = SocialGetBody,
    responses(
        (status = 200, description = "Key-value data as nested tree", body = serde_json::Value),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "social"
)]
#[post("/v1/social/get")]
pub async fn social_get_handler(
    _body: web::Json<SocialGetBody>,
    _app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    // TODO: Implement with Redis
    Ok(HttpResponse::Ok().json(serde_json::json!({})))
}

// POST /v1/social/keys - list keys under a prefix
#[utoipa::path(
    post,
    path = "/v1/social/keys",
    request_body = SocialKeysBody,
    responses(
        (status = 200, description = "Keys under the given prefixes", body = serde_json::Value),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "social"
)]
#[post("/v1/social/keys")]
pub async fn social_keys_handler(
    _body: web::Json<SocialKeysBody>,
    _app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    // TODO: Implement with Redis
    Ok(HttpResponse::Ok().json(serde_json::json!({})))
}

// GET /v1/social/index - query by index
#[utoipa::path(
    get,
    path = "/v1/social/index",
    params(SocialIndexParams),
    responses(
        (status = 200, description = "Index entries", body = inline(PaginatedResponse<IndexEntry>)),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "social"
)]
#[get("/v1/social/index")]
pub async fn social_index_handler(
    _query: web::Query<SocialIndexParams>,
    _app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    // TODO: Implement with Redis
    Ok(HttpResponse::Ok().json(PaginatedResponse::<IndexEntry> {
        data: Vec::new(),
        meta: PaginationMeta {
            has_more: false,
            truncated: false,
            next_cursor: None,
            dropped_rows: None,
        },
    }))
}

// GET /v1/social/profile - get account profile
#[utoipa::path(
    get,
    path = "/v1/social/profile",
    params(SocialProfileParams),
    responses(
        (status = 200, description = "Profile data", body = serde_json::Value),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "social"
)]
#[get("/v1/social/profile")]
pub async fn social_profile_handler(
    query: web::Query<SocialProfileParams>,
    _app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    validate_account_id(&query.account_id, "accountId")?;
    let _contract = resolve_contract(&query.contract_id)?;
    
    // TODO: Implement with Redis
    Ok(HttpResponse::Ok().json(serde_json::json!({})))
}

// GET /v1/social/followers - get followers list
#[utoipa::path(
    get,
    path = "/v1/social/followers",
    params(SocialFollowParams),
    responses(
        (status = 200, description = "Followers list", body = SocialFollowResponse),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "social"
)]
#[get("/v1/social/followers")]
pub async fn social_followers_handler(
    query: web::Query<SocialFollowParams>,
    _app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    validate_account_id(&query.account_id, "accountId")?;
    let _contract = resolve_contract(&query.contract_id)?;
    
    // TODO: Implement with Redis
    Ok(HttpResponse::Ok().json(SocialFollowResponse {
        data: Vec::new(),
        count: 0,
        meta: PaginationMeta {
            has_more: false,
            truncated: false,
            next_cursor: None,
            dropped_rows: None,
        },
    }))
}

// GET /v1/social/following - get following list
#[utoipa::path(
    get,
    path = "/v1/social/following",
    params(SocialFollowParams),
    responses(
        (status = 200, description = "Following list", body = SocialFollowResponse),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "social"
)]
#[get("/v1/social/following")]
pub async fn social_following_handler(
    query: web::Query<SocialFollowParams>,
    _app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    validate_account_id(&query.account_id, "accountId")?;
    let _contract = resolve_contract(&query.contract_id)?;
    
    // TODO: Implement with Redis
    Ok(HttpResponse::Ok().json(SocialFollowResponse {
        data: Vec::new(),
        count: 0,
        meta: PaginationMeta {
            has_more: false,
            truncated: false,
            next_cursor: None,
            dropped_rows: None,
        },
    }))
}

// GET /v1/social/feed/account - get account feed
#[utoipa::path(
    get,
    path = "/v1/social/feed/account",
    params(SocialAccountFeedParams),
    responses(
        (status = 200, description = "Account feed", body = inline(PaginatedResponse<IndexEntry>)),
        (status = 400, description = "Invalid parameters", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse),
    ),
    tag = "social"
)]
#[get("/v1/social/feed/account")]
pub async fn social_account_feed_handler(
    _query: web::Query<SocialAccountFeedParams>,
    _app_state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    // TODO: Implement with Redis
    Ok(HttpResponse::Ok().json(PaginatedResponse::<IndexEntry> {
        data: Vec::new(),
        meta: PaginationMeta {
            has_more: false,
            truncated: false,
            next_cursor: None,
            dropped_rows: None,
        },
    }))
}
