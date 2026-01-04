//! Application state
//!
//! Shared state accessible from all handlers.

use std::sync::Arc;

use crate::config::Config;
use crate::infra::{DynPostgresClient, DynRedisClient};
use crate::service::{DynAuthService, DynListingService};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,

    /// HTTP client for Ord backend requests
    pub http_client: reqwest::Client,

    /// Auth service
    pub auth_service: DynAuthService,

    /// Listing service
    pub listing_service: DynListingService,

    /// Redis client
    pub redis_client: DynRedisClient,

    /// PostgreSQL client
    pub postgres: DynPostgresClient,

    /// Database pool (for migrations)
    pub db_pool: sqlx::PgPool,
}

impl AppState {
    /// Create application state
    pub fn new(
        config: Config,
        http_client: reqwest::Client,
        auth_service: DynAuthService,
        listing_service: DynListingService,
        redis_client: DynRedisClient,
        postgres: DynPostgresClient,
        db_pool: sqlx::PgPool,
    ) -> Self {
        Self {
            config: Arc::new(config),
            http_client,
            auth_service,
            listing_service,
            redis_client,
            postgres,
            db_pool,
        }
    }
}
