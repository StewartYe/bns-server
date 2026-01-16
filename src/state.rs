//! Application state
//!
//! Shared state accessible from all handlers.

use std::sync::Arc;

use crate::config::Config;
use crate::infra::{DynPostgresClient, DynRedisClient, IcAgent};
use crate::service::{DynAuthService, DynNameService, DynTradingService, DynUserService};

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

    /// Trading service
    pub trading_service: DynTradingService,

    /// Redis client
    pub redis_client: DynRedisClient,

    /// PostgreSQL client
    pub postgres: DynPostgresClient,

    /// Database pool (for migrations)
    pub db_pool: sqlx::PgPool,

    /// IC Agent for canister interactions
    pub ic_agent: Arc<IcAgent>,
}

impl AppState {
    /// Create application state
    pub fn new(
        config: Config,
        auth_service: DynAuthService,
        name_service: DynNameService,
        user_service: DynUserService,
        trading_service: DynTradingService,
        redis_client: DynRedisClient,
        postgres: DynPostgresClient,
        db_pool: sqlx::PgPool,
        ic_agent: Arc<IcAgent>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            auth_service,
            name_service,
            user_service,
            trading_service,
            redis_client,
            postgres,
            db_pool,
            ic_agent,
        }
    }
}
