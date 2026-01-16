//! API layer
//!
//! HTTP endpoints:
//! - Name: Name resolution (forward and reverse)
//! - Auth: BIP-322 authentication
//! - Trading: Marketplace trading (pools, listings)
//! - User: User settings and name metadata
//! - WebSocket: Real-time updates

mod auth;
mod name;
mod rankings;
mod trading;
mod user;
mod ws;

pub use auth::*;
pub use name::*;
pub use rankings::*;
pub use trading::*;
pub use user::*;
pub use ws::*;

use axum::{
    Router, middleware,
    routing::{delete, get, post, put},
};

use crate::state::AppState;

/// Health check endpoint
///
/// GET /health
pub async fn health() -> &'static str {
    "OK"
}

/// Build the API router
pub fn build_router(state: AppState) -> Router {
    // Routes that require authentication (get pool)
    let authenticated_routes = Router::new()
        // Get pool address (first step before listing)
        .route("/v1/trading/pool", post(trading::get_pool))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth_middleware,
        ));

    Router::new()
        // Health check (no version prefix for Cloud Run compatibility)
        .route("/health", get(health))
        // Name resolution endpoints
        .route("/v1/names/{name}", get(name::get_name))
        .route(
            "/v1/addresses/{address}/names",
            get(name::get_address_names),
        )
        // Auth endpoints
        .route("/v1/auth/login", post(auth::authenticate))
        .route("/v1/auth/logout", post(auth::logout))
        .route("/v1/auth/me", get(auth::get_me))
        // Trading endpoints
        .route("/v1/trading/list", post(trading::list_name))
        .route("/v1/trading/listings", get(trading::get_listings))
        // User endpoints
        .route("/v1/user/primary-name", put(user::set_primary_name))
        .route("/v1/user/primary-name", delete(user::clear_primary_name))
        .route(
            "/v1/user/names/{name}/metadata",
            put(user::update_name_metadata),
        )
        // Rankings endpoints
        .route("/v1/rankings/{type}", get(rankings::get_ranking))
        // WebSocket endpoint
        .route("/v1/ws/connect", get(ws::ws_handler))
        // Merge authenticated routes
        .merge(authenticated_routes)
        .with_state(state)
}
