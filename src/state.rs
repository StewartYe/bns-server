//! Application state
//!
//! Shared state accessible from all handlers.

use std::sync::Arc;

use crate::config::Config;
use crate::service::DynAuthService;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,

    /// HTTP client for Ord backend requests
    pub http_client: reqwest::Client,

    /// Auth service
    pub auth_service: DynAuthService,

    /// Database pool
    pub db_pool: sqlx::PgPool,
}

impl AppState {
    /// Create application state
    pub fn new(
        config: Config,
        http_client: reqwest::Client,
        auth_service: DynAuthService,
        db_pool: sqlx::PgPool,
    ) -> Self {
        Self {
            config: Arc::new(config),
            http_client,
            auth_service,
            db_pool,
        }
    }
}
