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
    infra::{BlockchainClientImpl, PostgresClientImpl, RedisClientImpl},
    service::{AuthConfig, AuthService, DynListingService, ListingService},
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

    // Initialize auth service (using Redis for sessions)
    let auth_config = AuthConfig {
        session_ttl_secs: config.session_ttl_secs,
    };
    let auth_service = Arc::new(AuthService::new(redis_client.clone(), pool.clone(), auth_config));

    // Initialize blockchain client
    let ord_url = config.ord_url.as_deref().unwrap_or("http://localhost:80");
    let blockchain_client = Arc::new(BlockchainClientImpl::new(ord_url, &config.bitcoind_url));
    tracing::info!("Bitcoin RPC URL: {}", config.bitcoind_url);

    // Initialize postgres client wrapper
    let postgres_client = Arc::new(PostgresClientImpl::new(&config.database_url).await?);

    // Initialize listing service
    let listing_service = Arc::new(ListingService::new(
        blockchain_client,
        postgres_client,
        redis_client.clone(),
    ));

    // Sync pending listings from PostgreSQL to Redis queue on startup
    match listing_service.init_pending_queue().await {
        Ok(count) => {
            if count > 0 {
                tracing::info!("Initialized pending queue with {} listings", count);
            }
        }
        Err(e) => {
            tracing::error!("Failed to initialize pending queue: {:?}", e);
        }
    }

    // Start background task for confirmation updates
    let listing_service_bg = listing_service.clone();
    tokio::spawn(async move {
        confirmation_update_task(listing_service_bg).await;
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

/// Background task to update listing confirmations every minute
async fn confirmation_update_task(listing_service: DynListingService) {
    let interval = Duration::from_secs(60);
    tracing::info!("Starting confirmation update task (interval: {:?})", interval);

    loop {
        tokio::time::sleep(interval).await;

        match listing_service.update_confirmations().await {
            Ok(count) => {
                if count > 0 {
                    tracing::info!("Updated confirmations for {} listings", count);
                }
            }
            Err(e) => {
                tracing::error!("Failed to update confirmations: {:?}", e);
            }
        }
    }
}
