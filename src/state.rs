//! Application state
//!
//! Shared state accessible from all handlers.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use crate::api::rankings::{
    BestDealItem, MostTradedItem, NewListingItem, RecentSaleItem, TopEarnerItem, TopSaleItem,
};
use crate::config::Config;
use crate::domain::{StarResponse, TradeHistoryItem, UserInventory};
use crate::infra::{DynPostgresClient, DynRedisClient, IcAgent};
use crate::service::{
    DynAuthService, DynMarketingService, DynNameService, DynShoutOutService, DynStarService,
    DynTradingService, DynUserService,
};
use tokio::sync::broadcast;

/// Broadcast event types for real-time WebSocket updates
#[derive(Debug, Clone)]
pub enum BroadcastEvent {
    /// New listing added (new_listings ranking)
    NewListing(NewListingItem),
    /// Top sale updated (top_sales ranking)
    TopSale(TopSaleItem),
    /// Best deal updated (best_deals ranking)
    BestDeal(BestDealItem),
    /// Recent sale completed (recent_sales ranking)
    RecentSale(RecentSaleItem),
    /// Most traded updated (most_traded ranking)
    MostTraded(MostTradedItem),
    /// Top earner updated (top_earners ranking)
    TopEarner(TopEarnerItem),
    /// Remove from new_listings ranking
    RemoveNewListing(String),
    /// Remove from top_sales ranking
    RemoveTopSale(String),
    /// Remove from best_deals ranking
    RemoveBestDeal(String),
    /// Market online users changed
    MarketOnlineUpdated { total_online: u64 },
    /// Market listing count/value changed
    MarketListingsUpdated {
        listed_count: u64,
        listed_value: u64,
    },
    /// Market 24h trades/volume changed
    MarketTrades24hUpdated { txs_24h: u64, vol_24h: u64 },
    /// User inventory changed
    UserInventory {
        user_address: String,
        inventory: UserInventory,
    },
    /// User new activities
    UserActivities {
        user_address: String,
        activities: Vec<TradeHistoryItem>,
    },
    /// User stars changed
    UserStars {
        user_address: String,
        op: String,
        star: StarResponse,
    },
}

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,

    /// Auth service
    pub auth_service: DynAuthService,

    /// Name service
    pub name_service: DynNameService,

    /// User service
    pub user_service: DynUserService,

    /// Marketing service
    pub marketing_service: DynMarketingService,

    /// Trading service
    pub trading_service: DynTradingService,

    // Star_service: DynStarService
    pub star_service: DynStarService,

    //shoutOutService: DynShoutOutService
    pub shout_out_service: DynShoutOutService,

    /// Redis client
    pub redis_client: DynRedisClient,

    /// PostgreSQL client
    pub postgres: DynPostgresClient,

    /// Database pool (for migrations)
    pub db_pool: sqlx::PgPool,

    /// IC Agent for canister interactions
    pub ic_agent: Arc<IcAgent>,

    /// Broadcast channel for real-time WebSocket updates
    pub broadcast_tx: broadcast::Sender<BroadcastEvent>,

    /// Online authenticated WebSocket connections
    pub online_users: Arc<AtomicU64>,
}

impl AppState {
    /// Create application state
    pub fn new(
        config: Config,
        auth_service: DynAuthService,
        name_service: DynNameService,
        user_service: DynUserService,
        marketing_service: DynMarketingService,
        trading_service: DynTradingService,
        star_service: DynStarService,
        shout_out_service: DynShoutOutService,
        redis_client: DynRedisClient,
        postgres: DynPostgresClient,
        db_pool: sqlx::PgPool,
        ic_agent: Arc<IcAgent>,
        broadcast_tx: broadcast::Sender<BroadcastEvent>,
        online_users: Arc<AtomicU64>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            auth_service,
            name_service,
            user_service,
            marketing_service,
            trading_service,
            star_service,
            shout_out_service,
            redis_client,
            postgres,
            db_pool,
            ic_agent,
            broadcast_tx,
            online_users,
        }
    }
}
