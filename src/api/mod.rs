//! API layer
//!
//! HTTP endpoints:
//! - SDK API: Name resolution for external clients
//! - Auth: BIP-322 authentication
//! - Listings: Name listing and retrieval
//! - User: User settings and name metadata
//! - Pool: Pool creation for listing names
//! - WebSocket: Real-time updates

mod auth;
mod listing;
mod pool;
mod rankings;
mod sdk;
mod user;
mod ws;

pub use auth::*;
pub use listing::*;
pub use pool::*;
pub use rankings::*;
pub use sdk::*;
pub use user::*;
pub use ws::*;

use axum::{
    Router, middleware,
    routing::{delete, get, post, put},
};

use crate::state::AppState;

/// Build the API router
pub fn build_router(state: AppState) -> Router {
    // Routes that require authentication
    let authenticated_routes = Router::new()
        // Pool endpoints (requires auth)
        .route("/v1/pool", post(pool::get_pool))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth_middleware,
        ));

    Router::new()
        // SDK endpoints (name resolution)
        .merge(sdk::router())
        // Auth endpoints
        .route("/v1/auth/login", post(auth::authenticate))
        .route("/v1/auth/logout", post(auth::logout))
        .route("/v1/auth/me", get(auth::get_me))
        // Listing endpoints
        .route("/v1/listings", post(listing::list_name))
        .route("/v1/listings", get(listing::get_listed_names))
        // User endpoints
        .route("/v1/user/primary-name", put(user::set_primary_name))
        .route("/v1/user/primary-name", delete(user::clear_primary_name))
        // Name metadata endpoints
        .route("/v1/names/{name}/metadata", put(user::update_name_metadata))
        // Rankings endpoints
        .route("/v1/rankings/{type}", get(rankings::get_ranking))
        // WebSocket endpoint
        .route("/v1/ws/connect", get(ws::ws_handler))
        // Merge authenticated routes
        .merge(authenticated_routes)
        .with_state(state)
}
