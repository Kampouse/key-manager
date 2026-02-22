mod handlers;
mod models;
mod scylladb;
mod social_handlers;
mod tree;

use crate::handlers::{
    accounts_handler, batch_kv_handler, contracts_handler, diff_kv_handler, edges_count_handler,
    edges_handler, get_kv_handler, health_check, history_kv_handler, query_kv_handler,
    status_handler, timeline_kv_handler, watch_kv_handler, writers_handler,
};
use crate::scylladb::ScyllaDb;
use crate::social_handlers::{
    social_account_feed_handler, social_followers_handler, social_following_handler,
    social_get_handler, social_index_handler, social_keys_handler, social_profile_handler,
};
use actix_cors::Cors;
use actix_files::Files;
use actix_web::http::header;
use actix_web::{dev::Service, middleware, web, App, HttpServer};
use dotenvy::dotenv;
use fastnear_primitives::types::ChainId;
use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::models::PROJECT_ID;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::health_check,
        handlers::status_handler,
        handlers::get_kv_handler,
        handlers::query_kv_handler,
        handlers::history_kv_handler,
        handlers::writers_handler,
        handlers::diff_kv_handler,
        handlers::timeline_kv_handler,
        handlers::batch_kv_handler,
        handlers::accounts_handler,
        handlers::contracts_handler,
        handlers::edges_handler,
        handlers::edges_count_handler,
        handlers::watch_kv_handler,
        social_handlers::social_get_handler,
        social_handlers::social_keys_handler,
        social_handlers::social_index_handler,
        social_handlers::social_profile_handler,
        social_handlers::social_followers_handler,
        social_handlers::social_following_handler,
        social_handlers::social_account_feed_handler,
    ),
    components(schemas(
        models::KvEntry,
        models::HealthResponse,
        models::StatusResponse,
        models::GetParams,
        models::QueryParams,
        models::HistoryParams,
        models::WritersParams,
        models::ApiError,
        models::ErrorCode,
        models::ErrorResponse,
        models::BatchQuery,
        models::BatchResultItem,
        models::TreeResponse,
        models::DiffParams,
        models::DiffResponse,
        models::TimelineParams,
        models::AccountsQueryParams,
        models::ContractsQueryParams,
        models::EdgesParams,
        models::EdgesCountParams,
        models::EdgeSourceEntry,
        models::EdgesCountResponse,
        models::SocialGetBody,
        models::SocialGetOptions,
        models::SocialKeysBody,
        models::SocialKeysOptions,
        models::SocialIndexParams,
        models::SocialProfileParams,
        models::SocialFollowParams,
        models::SocialAccountFeedParams,
        models::IndexEntry,
        models::SocialFollowResponse,
        models::PaginationMeta,
        models::WatchParams,
        models::WatchEvent,
    )),
    info(
        title = "FastKV API",
        version = "1.0.0",
        description = "Query FastData KV entries from ScyllaDB. This API provides access to NEAR Protocol data storage."
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "kv", description = "Key-Value storage operations"),
        (name = "social", description = "SocialDB-compatible convenience API")
    )
)]
struct ApiDoc;

#[derive(Clone)]
pub struct AppState {
    pub scylladb: Arc<RwLock<Option<Arc<ScyllaDb>>>>,
    pub chain_id: ChainId,
    /// Per-IP throttle for scan=1 requests on /v1/kv/accounts.
    pub scan_throttle: Arc<std::sync::Mutex<std::collections::HashMap<String, std::time::Instant>>>,
    /// Active SSE watch connection count.
    pub watch_count: Arc<std::sync::atomic::AtomicUsize>,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "scylladb=info,near-garden=info,fastkv-server=info".into()),
        )
        .init();

    tracing::info!(target: PROJECT_ID, "FastKV server starting");

    let chain_id: ChainId = env::var("CHAIN_ID")
        .expect("CHAIN_ID required")
        .try_into()
        .expect("Invalid chain id");

    tracing::info!(target: PROJECT_ID, %chain_id, "Configuration loaded");

    // Validate DB env vars early (they're required by the background connection task)
    for var in ["SCYLLA_URL", "SCYLLA_USERNAME", "SCYLLA_PASSWORD"] {
        env::var(var).unwrap_or_else(|_| panic!("{var} must be set"));
    }

    let scylladb: Arc<RwLock<Option<Arc<ScyllaDb>>>> = Arc::new(RwLock::new(None));
    tracing::info!(target: PROJECT_ID, "Database connection deferred to background task");

    // Background reconnection task with exponential backoff
    let reconnect_base_secs: u64 = env::var("DB_RECONNECT_INTERVAL_SECS")
        .unwrap_or_else(|_| "5".to_string())
        .parse()
        .unwrap_or(5)
        .max(5);
    let reconnect_max_secs: u64 = 300;
    {
        let scylladb = Arc::clone(&scylladb);
        tokio::spawn(async move {
            let mut delay_secs = reconnect_base_secs;
            let mut is_initial = true;
            loop {
                if is_initial {
                    is_initial = false;
                } else {
                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                }
                if scylladb.read().await.is_some() {
                    delay_secs = reconnect_base_secs;
                    continue;
                }
                tracing::info!(target: PROJECT_ID, delay_secs, "Attempting to connect to ScyllaDB...");
                let session = match ScyllaDb::new_scylla_session().await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(target: PROJECT_ID, error = %e, delay_secs, "ScyllaDB connection failed");
                        delay_secs = (delay_secs * 2).min(reconnect_max_secs);
                        continue;
                    }
                };
                if let Err(e) = ScyllaDb::test_connection(&session).await {
                    tracing::warn!(target: PROJECT_ID, error = %e, delay_secs, "ScyllaDB connection test failed");
                    delay_secs = (delay_secs * 2).min(reconnect_max_secs);
                    continue;
                }
                match ScyllaDb::new(chain_id, session).await {
                    Ok(db) => {
                        *scylladb.write().await = Some(Arc::new(db));
                        delay_secs = reconnect_base_secs;
                        tracing::info!(target: PROJECT_ID, "Successfully connected to ScyllaDB");
                    }
                    Err(e) => {
                        tracing::warn!(target: PROJECT_ID, error = %e, delay_secs, "ScyllaDB initialization failed");
                        delay_secs = (delay_secs * 2).min(reconnect_max_secs);
                    }
                }
            }
        });
    }

    // Background task to cache indexer block height for response headers
    let indexer_block_cache = Arc::new(AtomicU64::new(0));
    {
        let cache = Arc::clone(&indexer_block_cache);
        let scylladb = Arc::clone(&scylladb);
        tokio::spawn(async move {
            loop {
                let db = scylladb.read().await.clone();
                if let Some(ref db) = db {
                    if let Ok(Some(h)) = db.get_indexer_block_height().await {
                        cache.store(h, Ordering::Release);
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        });
    }

    let scan_throttle = Arc::new(std::sync::Mutex::new(std::collections::HashMap::<
        String,
        std::time::Instant,
    >::new()));

    let port = env::var("PORT").unwrap_or_else(|_| "3001".to_string());
    tracing::info!(target: PROJECT_ID, %port, "Binding HTTP server");

    HttpServer::new(move || {
        let block_cache = Arc::clone(&indexer_block_cache);

        // Configure CORS middleware
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(vec!["GET", "POST"])
            .allowed_headers(vec![header::CONTENT_TYPE, header::ACCEPT])
            .expose_headers(vec![
                "X-Results-Truncated",
                "X-Indexer-Block",
            ])
            .max_age(3600);

        App::new()
            .app_data(web::JsonConfig::default().limit(262_144))
            .app_data(web::Data::new(AppState {
                scylladb: Arc::clone(&scylladb),
                chain_id,
                scan_throttle: scan_throttle.clone(),
                watch_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            }))
            .wrap(cors)
            .wrap_fn({
                let cache = block_cache;
                move |req, srv| {
                    let h = cache.load(Ordering::Acquire);
                    let path = req.path().to_string();
                    let method = req.method().clone();
                    let fut = srv.call(req);
                    async move {
                        let mut res = fut.await?;
                        if h > 0 {
                            res.headers_mut().insert(
                                header::HeaderName::from_static("x-indexer-block"),
                                header::HeaderValue::from(h),
                            );
                        }
                        // Default Cache-Control for successful GET API responses.
                        // Handlers that set their own Cache-Control header take precedence.
                        if method == actix_web::http::Method::GET
                            && res.status().is_success()
                            && !res.headers().contains_key(header::CACHE_CONTROL)
                        {
                            let cc = if path == "/health" || path == "/v1/status" {
                                "no-cache"
                            } else if path.starts_with("/v1/") {
                                "public, max-age=5"
                            } else {
                                ""
                            };
                            if !cc.is_empty() {
                                res.headers_mut().insert(
                                    header::CACHE_CONTROL,
                                    header::HeaderValue::from_static(cc),
                                );
                            }
                        }
                        // Security headers on all responses.
                        res.headers_mut().insert(
                            header::HeaderName::from_static("x-content-type-options"),
                            header::HeaderValue::from_static("nosniff"),
                        );
                        res.headers_mut().insert(
                            header::HeaderName::from_static("x-frame-options"),
                            header::HeaderValue::from_static("DENY"),
                        );
                        Ok(res)
                    }
                }
            })
            .wrap(middleware::Compress::default())
            .wrap(middleware::Logger::new(
                "%{r}a \"%r\"	%s %b \"%{Referer}i\" \"%{User-Agent}i\" %T",
            ))
            .wrap(tracing_actix_web::TracingLogger::default())
            .service(Scalar::with_url("/docs", ApiDoc::openapi()))
            .service(health_check)
            .service(status_handler)
            .service(get_kv_handler)
            .service(query_kv_handler)
            .service(history_kv_handler)
            .service(writers_handler)
            .service(batch_kv_handler)
            .service(diff_kv_handler)
            .service(timeline_kv_handler)
            .service(accounts_handler)
            .service(contracts_handler)
            .service(edges_handler)
            .service(edges_count_handler)
            .service(watch_kv_handler)
            .service(social_get_handler)
            .service(social_keys_handler)
            .service(social_index_handler)
            .service(social_profile_handler)
            .service(social_followers_handler)
            .service(social_following_handler)
            .service(social_account_feed_handler)
            .service(Files::new("/", "./static").index_file("index.html"))
    })
    .bind(format!("0.0.0.0:{}", port))?
    .run()
    .await?;

    Ok(())
}
