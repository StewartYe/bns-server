//! API layer
//!
//! HTTP endpoints:
//! - SDK API: Name resolution for external clients
//! - Auth: BIP-322 authentication
//! - Listings: Name listing and retrieval
//! - WebSocket: Real-time updates

mod auth;
mod listing;
mod sdk;
mod ws;

pub use auth::*;
pub use listing::*;
pub use sdk::*;
pub use ws::*;

use axum::{routing::{get, post}, Router};

use crate::state::AppState;

/// Build the API router
pub fn build_router(state: AppState) -> Router {
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
        // Real-time endpoints
        .route("/v1/listings/new", get(ws::get_new_listings))
        .route("/v1/ws/new-listings", get(ws::ws_new_listings))
        .with_state(state)
}
