//! API layer
//!
//! HTTP endpoints:
//! - SDK API: Name resolution for external clients
//! - Auth: BIP-322 authentication
//! - Listings: Name listing and retrieval

mod auth;
mod listing;
mod sdk;

pub use auth::*;
pub use listing::*;
pub use sdk::*;

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
        .with_state(state)
}
