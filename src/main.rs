//! BNS Server - Bitcoin Name Service
//!
//! Main entry point.

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use bns_server::{
    api,
    config::Config,
    infra::{
        BlockchainClientImpl, IcAgent, ListingMeta, PostgresClientImpl, RedisClient,
        RedisClientImpl,
    },
    service::{AuthConfig, AuthService, EventService, NameService, TradingService, UserService},
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

    // Initialize blockchain client for Ord backend
    let ord_url = config.ord_url.clone().unwrap_or_default();
    let blockchain_client = Arc::new(BlockchainClientImpl::new(&ord_url, ""));

    // Initialize name service
    let name_service = Arc::new(NameService::new(
        blockchain_client.clone(),
        postgres_client.clone(),
    ));

    // Initialize user service
    let user_service = Arc::new(UserService::new(
        blockchain_client.clone(),
        postgres_client.clone(),
    ));

    // Initialize trading service
    let trading_service = Arc::new(TradingService::new(
        ic_agent.clone(),
        postgres_client.clone(),
        redis_client.clone(),
        blockchain_client.clone(),
    ));

    // Initialize and start event service for canister event polling
    let event_service = Arc::new(EventService::new(
        ic_agent.clone(),
        redis_client.clone(),
        postgres_client.clone(),
    ));
    event_service.start_polling();

    // Create postgres client for AppState (reuse the pool)
    let postgres_state = Arc::new(PostgresClientImpl::new(&config.database_url).await?);

    // Create application state
    let state = AppState::new(
        config.clone(),
        auth_service,
        name_service,
        user_service,
        trading_service,
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
