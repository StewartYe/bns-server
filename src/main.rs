//! BNS Server - Bitcoin Name Service
//!
//! Main entry point.

use std::sync::Arc;
use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use bns_server::{
    api,
    config::Config,
    domain::{Listing, ListingStatus},
    infra::{
        DynPostgresClient, DynRedisClient, IcAgent, ListingMeta, PostgresClientImpl, RedisClient,
        RedisClientImpl,
        bns_canister::{BnsCanisterEvent, ReeActionStatus},
    },
    service::{AuthConfig, AuthService, ListingService},
    state::AppState,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,bns_server=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    dotenvy::dotenv().ok();
    let config = Config::from_env()?;

    tracing::info!(
        "Starting BNS Server on port {} (network: {})",
        config.port,
        config.network
    );

    if let Some(ord_url) = &config.ord_url {
        tracing::info!("Ord backend URL: {}", ord_url);
    } else {
        tracing::warn!("No Ord backend URL configured - name resolution will not work");
    }

    // Create HTTP client for Ord backend
    let http_client = reqwest::Client::new();

    // Connect to PostgreSQL
    tracing::info!("Connecting to PostgreSQL...");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;
    tracing::info!("Connected to PostgreSQL");

    // Run migrations
    tracing::info!("Running database migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("Migrations complete");

    // Connect to Redis
    tracing::info!("Connecting to Redis...");
    let redis_client = Arc::new(RedisClientImpl::new(&config.redis, config.network).await?);
    tracing::info!("Connected to Redis");

    // Rebuild Redis rankings from PostgreSQL (recovery after restart/crash)
    tracing::info!("Rebuilding Redis rankings from PostgreSQL...");
    rebuild_rankings_from_postgres(&pool, redis_client.clone()).await?;
    tracing::info!("Redis rankings rebuilt");

    // Initialize auth service (using Redis for sessions)
    let auth_config = AuthConfig {
        session_ttl_secs: config.session_ttl_secs,
    };
    let auth_service = Arc::new(AuthService::new(
        redis_client.clone(),
        pool.clone(),
        auth_config,
    ));

    // Initialize postgres client wrapper
    let postgres_client = Arc::new(PostgresClientImpl::new(&config.database_url).await?);

    // Initialize IC Agent
    tracing::info!("Initializing IC Agent...");
    let ic_agent = Arc::new(IcAgent::new(&config.ic).await?);
    tracing::info!("IC Agent initialized");

    // Initialize listing service (now uses IcAgent instead of blockchain client)
    let listing_service = Arc::new(ListingService::new(
        ic_agent.clone(),
        postgres_client.clone(),
        redis_client.clone(),
        http_client.clone(),
        config.ord_url.clone(),
    ));

    // Start background task for get_events polling
    let ic_agent_bg = ic_agent.clone();
    let redis_bg = redis_client.clone();
    let postgres_bg = postgres_client.clone();
    tokio::spawn(async move {
        get_events_polling_task(ic_agent_bg, redis_bg, postgres_bg).await;
    });

    // Create postgres client for AppState (reuse the pool)
    let postgres_state = Arc::new(PostgresClientImpl::new(&config.database_url).await?);

    // Create application state
    let state = AppState::new(
        config.clone(),
        http_client,
        auth_service,
        listing_service,
        redis_client,
        postgres_state,
        pool,
        ic_agent,
    );

    // Build router
    let app = api::build_router(state)
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    // Start server
    let addr = format!("0.0.0.0:{}", config.port);
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Background task to poll BNS canister get_events every minute
///
/// Checks for ReeActionStatusChanged events and matches with pending tx_ids:
/// - Pending: Save listing to PostgreSQL
/// - Finalized: Update status to Active, remove from tracking
/// - Rejected: Remove from tracking
async fn get_events_polling_task(
    ic_agent: Arc<IcAgent>,
    redis: DynRedisClient,
    postgres: DynPostgresClient,
) {
    use chrono::Utc;
    use uuid::Uuid;

    let interval = Duration::from_secs(60);

    // Load last event offset from Redis (persisted across restarts)
    let mut last_event_offset = match redis.get_event_offset().await {
        Ok(offset) => {
            tracing::info!("Loaded event offset from Redis: {}", offset);
            offset
        }
        Err(e) => {
            tracing::warn!(
                "Failed to load event offset from Redis, starting from 0: {:?}",
                e
            );
            0
        }
    };

    tracing::info!(
        "Starting get_events polling task (interval: {:?}, offset: {})",
        interval,
        last_event_offset
    );

    loop {
        tokio::time::sleep(interval).await;

        // Get pending tx_ids from Redis
        let pending_txs = match redis.get_pending_txs().await {
            Ok(txs) => txs,
            Err(e) => {
                tracing::error!("Failed to get pending txs from Redis: {:?}", e);
                continue;
            }
        };

        if pending_txs.is_empty() {
            tracing::debug!("No pending transactions to track");
            continue;
        }

        tracing::debug!("Tracking {} pending transactions", pending_txs.len());

        // Poll events from BNS canister
        let events = match ic_agent.get_events(last_event_offset, 100).await {
            Ok(events) => events,
            Err(e) => {
                tracing::error!("Failed to poll get_events: {:?}", e);
                continue;
            }
        };

        if events.is_empty() {
            continue;
        }

        tracing::debug!("Got {} events from BNS canister", events.len());

        // Build a map of pending tx_ids for quick lookup
        let pending_map: std::collections::HashMap<String, serde_json::Value> = pending_txs
            .into_iter()
            .filter_map(|(tx_id, data)| serde_json::from_str(&data).ok().map(|v| (tx_id, v)))
            .collect();

        let mut new_offset = last_event_offset;

        for (event_id, event) in events {
            // Update offset for next poll
            if let Ok(id) = event_id.parse::<u64>() {
                if id >= new_offset {
                    new_offset = id + 1;
                }
            }

            // Handle ReeActionStatusChanged events
            if let BnsCanisterEvent::ReeActionStatusChanged {
                action_id,
                status,
                timestamp_nanos: _,
            } = event
            {
                // Check if this action_id matches any pending tx_id
                if let Some(tracking_data) = pending_map.get(&action_id) {
                    tracing::info!(
                        "Found status change for tracked tx_id {}: {:?}",
                        action_id,
                        status
                    );

                    match status {
                        ReeActionStatus::Pending => {
                            // Save listing to PostgreSQL
                            let name = tracking_data["name"].as_str().unwrap_or("");
                            let price = tracking_data["price"].as_u64().unwrap_or(0);
                            let seller_address =
                                tracking_data["seller_address"].as_str().unwrap_or("");
                            let pool_address = tracking_data["pool_address"].as_str().unwrap_or("");

                            let now = Utc::now();
                            let listing = Listing {
                                id: Uuid::new_v4().to_string(),
                                name: name.to_string(),
                                seller_address: seller_address.to_string(),
                                pool_address: pool_address.to_string(),
                                price_sats: price,
                                status: ListingStatus::Pending,
                                listed_at: now,
                                updated_at: now,
                                previous_price_sats: None,
                                tx_id: Some(action_id.clone()),
                            };

                            if let Err(e) = postgres.create_listing(&listing).await {
                                tracing::error!("Failed to save listing for {}: {:?}", name, e);
                            } else {
                                tracing::info!(
                                    "Saved listing to PostgreSQL: name={}, tx_id={}",
                                    name,
                                    action_id
                                );
                            }

                            // ZADD to Redis for new-listings ranking
                            let meta = ListingMeta {
                                name: name.to_string(),
                                price_sats: price,
                                seller_address: seller_address.to_string(),
                                listed_at: now.timestamp(),
                                tx_id: Some(action_id.clone()),
                            };

                            if let Err(e) = redis.add_new_listing(&meta).await {
                                tracing::error!(
                                    "Failed to add listing {} to Redis ranking: {:?}",
                                    name,
                                    e
                                );
                            } else {
                                tracing::info!(
                                    "Added listing {} to Redis new-listings ranking",
                                    name
                                );
                            }
                        }
                        ReeActionStatus::Finalized => {
                            // Update listing status to Active
                            let name = tracking_data["name"].as_str().unwrap_or("");
                            if let Err(e) = postgres
                                .update_listing_status(name, ListingStatus::Active)
                                .await
                            {
                                tracing::error!(
                                    "Failed to update listing status for {}: {:?}",
                                    name,
                                    e
                                );
                            } else {
                                tracing::info!(
                                    "Tx {} finalized, listing {} is now active",
                                    action_id,
                                    name
                                );
                            }

                            // Remove from tracking
                            if let Err(e) = redis.remove_pending_tx(&action_id).await {
                                tracing::error!(
                                    "Failed to remove tx_id {} from tracking: {:?}",
                                    action_id,
                                    e
                                );
                            }
                        }
                        ReeActionStatus::Confirmed(_confirmations) => {
                            // Confirmation updates are informational only
                            // The listing status will be updated when Finalized
                            tracing::debug!(
                                "Tx {} confirmed with {} confirmations",
                                action_id,
                                _confirmations
                            );
                        }
                        ReeActionStatus::Rejected(reason) => {
                            tracing::warn!("Tx {} rejected: {}", action_id, reason);
                            // Remove from tracking
                            let _ = redis.remove_pending_tx(&action_id).await;
                        }
                    }
                }
            }
        }

        // Persist the new offset if it changed
        if new_offset > last_event_offset {
            last_event_offset = new_offset;
            if let Err(e) = redis.set_event_offset(last_event_offset).await {
                tracing::error!("Failed to persist event offset to Redis: {:?}", e);
            } else {
                tracing::debug!("Persisted event offset: {}", last_event_offset);
            }
        }
    }
}

/// Rebuild Redis rankings from PostgreSQL on startup
///
/// This ensures rankings are consistent after server restart or crash.
/// Loads all pending/active listings from PostgreSQL and populates Redis.
async fn rebuild_rankings_from_postgres(
    pool: &sqlx::PgPool,
    redis: Arc<RedisClientImpl>,
) -> anyhow::Result<()> {
    // Query all pending/active listings from PostgreSQL
    let rows = sqlx::query_as::<_, ListingRow>(
        "SELECT id, name, seller_address, pool_address, price_sats, status, listed_at, updated_at, previous_price_sats, tx_id
         FROM listings WHERE status IN ('pending', 'active') ORDER BY listed_at DESC LIMIT 100"
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        tracing::info!("No listings to rebuild into Redis rankings");
        return Ok(());
    }

    tracing::info!("Rebuilding {} listings into Redis rankings", rows.len());

    for row in rows {
        let meta = ListingMeta {
            name: row.name.clone(),
            price_sats: row.price_sats as u64,
            seller_address: row.seller_address.clone(),
            listed_at: row.listed_at.timestamp(),
            tx_id: row.tx_id.clone(),
        };

        if let Err(e) = redis.add_new_listing(&meta).await {
            tracing::warn!("Failed to add listing {} to Redis: {:?}", row.name, e);
        }
    }

    Ok(())
}

/// Database row for listings table (used in rebuild)
#[derive(Debug, sqlx::FromRow)]
struct ListingRow {
    #[allow(dead_code)]
    id: String,
    name: String,
    seller_address: String,
    #[allow(dead_code)]
    pool_address: String,
    price_sats: i64,
    #[allow(dead_code)]
    status: String,
    listed_at: chrono::DateTime<chrono::Utc>,
    #[allow(dead_code)]
    updated_at: chrono::DateTime<chrono::Utc>,
    #[allow(dead_code)]
    previous_price_sats: Option<i64>,
    tx_id: Option<String>,
}
