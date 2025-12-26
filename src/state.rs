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

    /// Auth service (optional - only available when database is configured)
    pub auth_service: Option<DynAuthService>,

    /// Database pool (optional)
    pub db_pool: Option<sqlx::PgPool>,
}

impl AppState {
    /// Create application state with all services
    pub fn new(
        config: Config,
        http_client: reqwest::Client,
        auth_service: Option<DynAuthService>,
        db_pool: Option<sqlx::PgPool>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            http_client,
            auth_service,
            db_pool,
        }
    }

    /// Create minimal state (for proxy-only mode without database)
    pub fn new_minimal(config: Config, http_client: reqwest::Client) -> Self {
        Self {
            config: Arc::new(config),
            http_client,
            auth_service: None,
            db_pool: None,
        }
    }
}
