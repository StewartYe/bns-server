//! BNS Server - Bitcoin Name Service
//!
//! Main entry point.

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use bns_server::api::rankings::{
    BestDealItem, MostTradedItem, NewListingItem, RecentSaleItem, TopEarnerItem, TopSaleItem,
};
use bns_server::config::CONFIG;
use bns_server::infra::ListingRow;
use bns_server::service::{MarketingService, ShoutOutService, StarService};
use bns_server::{
    api,
    infra::{BlockchainClientImpl, IcAgent, PostgresClientImpl, RedisClient, RedisClientImpl},
    service::{AuthConfig, AuthService, EventService, NameService, TradingService, UserService},
    state::{AppState, BroadcastEvent},
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
    let config = CONFIG.clone();

    tracing::info!(
        "Starting BNS Server on port {} (network: {})",
        config.port,
        config.network
    );

    tracing::info!("Ord backend URL: {}", config.ord_url);

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
    let blockchain_client = Arc::new(BlockchainClientImpl::new(
        &config.ord_url,
        &config.bitcoind_url,
    ));

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

    // Initialize and start event service for canister event polling
    // Create broadcast channel for real-time WebSocket updates
    let (broadcast_tx, _) = broadcast::channel::<BroadcastEvent>(256);

    let event_service = Arc::new(EventService::new(
        ic_agent.clone(),
        redis_client.clone(),
        postgres_client.clone(),
        blockchain_client.clone(),
        broadcast_tx.clone(),
    ));

    // Initialize marketing service
    let marketing_service = Arc::new(MarketingService::new(
        postgres_client.clone(),
        redis_client.clone(),
        user_service.clone(),
    ));

    // Initialize trading service
    let trading_service = Arc::new(TradingService::new(
        event_service.clone(),
        ic_agent.clone(),
        postgres_client.clone(),
        redis_client.clone(),
        blockchain_client.clone(),
        user_service.clone(),
    ));

    let star_service = Arc::new(StarService::new(
        postgres_client.clone(),
        blockchain_client.clone(),
    ));

    let shout_out_service = Arc::new(ShoutOutService::new(
        blockchain_client.clone(),
        postgres_client.clone(),
    ));

    event_service.start_polling();

    // Create application state
    let state = AppState::new(
        config.clone(),
        auth_service,
        name_service,
        user_service,
        marketing_service,
        trading_service,
        star_service,
        shout_out_service,
        redis_client,
        postgres_client,
        pool,
        ic_agent,
        broadcast_tx,
    );

    // Build router
    let app = api::build_router(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

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
/// Clears all 6 ranking ZSets first, then rebuilds from PostgreSQL.
async fn rebuild_rankings_from_postgres(
    pool: &sqlx::PgPool,
    redis: Arc<RedisClientImpl>,
) -> anyhow::Result<()> {
    // Step 1: Clear all 6 ranking ZSets
    tracing::info!("Clearing all ranking ZSets from Redis...");
    let keys_to_clear = [
        redis.keys().rank_new_listings(),
        redis.keys().rank_top_sales(),
        redis.keys().rank_best_deals(),
        redis.keys().rank_recent_sales(),
        redis.keys().rank_most_traded(),
        redis.keys().rank_top_earners(),
    ];
    for key in &keys_to_clear {
        if let Err(e) = redis.del(key).await {
            tracing::warn!("Failed to clear {}: {:?}", key, e);
        }
    }
    tracing::info!("Cleared all ranking ZSets");

    // Step 2: Rebuild new_listings, top_sales, best_deals from listed items
    let listed_rows = sqlx::query_as!(ListingRow,
        "SELECT id, name, seller_address, price_sats, status, listed_at, updated_at, previous_price_sats, tx_id, buyer_address, new_price_sats, inscription_utxo_sats
         FROM listings WHERE status = 'listed' ORDER BY listed_at DESC",
    )
    .fetch_all(pool)
    .await?;

    tracing::info!(
        "Rebuilding new_listings, top_sales, best_deals from {} listed items",
        listed_rows.len()
    );

    for row in &listed_rows {
        let price = row.price_sats as u64;
        let listed_at = row.listed_at.timestamp();

        // Calculate discount using shared utility
        let previous_price = row.previous_price_sats.map(|p| p as u64).unwrap_or(0);
        let discount = bns_server::utils::calculate_discount(price, previous_price);

        // Add to new_listings ranking
        let new_listing_item = NewListingItem {
            name: row.name.clone(),
            price_sats: price,
            listed_at,
            discount,
            seller_address: row.seller_address.clone(),
        };
        if let Err(e) = redis.add_new_listing(&new_listing_item).await {
            tracing::warn!("Failed to add {} to new_listings: {:?}", row.name, e);
        }

        // Add to top_sales ranking
        let top_sale_item = TopSaleItem {
            name: row.name.clone(),
            price_sats: price,
            listed_at,
            discount,
            seller_address: row.seller_address.clone(),
        };
        if let Err(e) = redis.add_top_sale(&top_sale_item).await {
            tracing::warn!("Failed to add {} to top_sales: {:?}", row.name, e);
        }

        // Add to best_deals ranking
        let best_deal_item = BestDealItem {
            name: row.name.clone(),
            price_sats: price,
            listed_at,
            discount,
            seller_address: row.seller_address.clone(),
        };
        if let Err(e) = redis.add_best_deal(&best_deal_item).await {
            tracing::warn!("Failed to add {} to best_deals: {:?}", row.name, e);
        }
    }

    // Step 3: Rebuild recent_sales from sold items
    let sold_rows = sqlx::query_as::<_, SoldRow>(
        "SELECT l.name, l.price_sats, l.seller_address, l.buyer_address, l.updated_at
         FROM listings l
         WHERE l.status IN ('bought_and_relisted', 'bought_and_delisted')
         ORDER BY l.updated_at DESC
         LIMIT 20",
    )
    .fetch_all(pool)
    .await?;

    tracing::info!(
        "Rebuilding recent_sales from {} sold items",
        sold_rows.len()
    );

    for row in &sold_rows {
        let recent_sale_item = RecentSaleItem {
            name: row.name.clone(),
            price_sats: row.price_sats as u64,
            seller_address: row.seller_address.clone(),
            buyer_address: row.buyer_address.clone().unwrap_or_default(),
            sold_at: row.updated_at.timestamp(),
        };
        if let Err(e) = redis.add_recent_sale(&recent_sale_item).await {
            tracing::warn!("Failed to add {} to recent_sales: {:?}", row.name, e);
        }
    }

    // Step 4: Rebuild most_traded (count trades per name)
    let most_traded_rows = sqlx::query_as::<_, MostTradedRow>(
        "SELECT name, COUNT(*) as trade_count, MAX(price_sats) as last_price_sats,
                MAX(seller_address) as seller_address, MAX(buyer_address) as buyer_address,
                MAX(updated_at) as last_traded_at
         FROM listings
         WHERE status IN ('bought_and_relisted', 'bought_and_delisted')
         GROUP BY name
         ORDER BY trade_count DESC
         LIMIT 20",
    )
    .fetch_all(pool)
    .await?;

    tracing::info!(
        "Rebuilding most_traded from {} names",
        most_traded_rows.len()
    );

    for row in &most_traded_rows {
        let most_traded_item = MostTradedItem {
            name: row.name.clone(),
            price_sats: row.last_price_sats as u64,
            seller_address: row.seller_address.clone().unwrap_or_default(),
            buyer_address: row.buyer_address.clone().unwrap_or_default(),
            trade_count: row.trade_count as u32,
            sold_at: row.last_traded_at.timestamp(),
        };
        if let Err(e) = redis.add_most_traded(&most_traded_item).await {
            tracing::warn!("Failed to add {} to most_traded: {:?}", row.name, e);
        }
    }

    // Step 5: Rebuild top_earners (sum earnings per seller)
    let top_earner_rows = sqlx::query_as::<_, TopEarnerRow>(
        "SELECT seller_address, SUM(price_sats)::BIGINT as total_earnings, COUNT(*) as trade_count
         FROM listings
         WHERE status IN ('bought_and_relisted', 'bought_and_delisted')
         GROUP BY seller_address
         ORDER BY total_earnings DESC
         LIMIT 20",
    )
    .fetch_all(pool)
    .await?;

    tracing::info!(
        "Rebuilding top_earners from {} sellers",
        top_earner_rows.len()
    );

    for row in &top_earner_rows {
        let top_earner_item = TopEarnerItem {
            address: row.seller_address.clone(),
            total_profit_sats: row.total_earnings,
            trade_count: row.trade_count as u32,
        };
        if let Err(e) = redis.add_top_earner(&top_earner_item).await {
            tracing::warn!(
                "Failed to add {} to top_earners: {:?}",
                row.seller_address,
                e
            );
        }
    }

    tracing::info!("All rankings rebuilt successfully");
    Ok(())
}

/// Database row for sold items (recent_sales)
#[derive(Debug, sqlx::FromRow)]
struct SoldRow {
    name: String,
    price_sats: i64,
    seller_address: String,
    buyer_address: Option<String>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

/// Database row for most_traded aggregation
#[derive(Debug, sqlx::FromRow)]
struct MostTradedRow {
    name: String,
    trade_count: i64,
    last_price_sats: i64,
    seller_address: Option<String>,
    buyer_address: Option<String>,
    last_traded_at: chrono::DateTime<chrono::Utc>,
}

/// Database row for top_earners aggregation
#[derive(Debug, sqlx::FromRow)]
struct TopEarnerRow {
    seller_address: String,
    total_earnings: i64,
    trade_count: i64,
}
