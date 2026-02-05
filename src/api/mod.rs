//! API layer
//!
//! HTTP endpoints:
//! - Name: Name resolution (forward and reverse)
//! - Auth: BIP-322 authentication
//! - Trading: Marketplace trading (pools, listings)
//! - User: User settings and name metadata
//! - WebSocket: Real-time updates

mod auth;
mod marketing;
mod name;
pub mod rankings;
mod shout_out;
mod star;
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
    // Routes that require authentication
    let authenticated_router = Router::new()
        // Trading endpoints (require auth)
        .route("/trading/pool", post(trading::get_pool))
        .route("/trading/list", post(trading::list))
        .route("/trading/delist", post(trading::delist))
        .route("/trading/relist", post(trading::relist))
        .route("/trading/buy-and-relist", post(trading::buy_and_relist))
        .route("/trading/buy-and-delist", post(trading::buy_and_delist))
        .route("/trading/listing/{name}", get(trading::get_listing))
        .route("/user/trading/history/{offset}", get(user_history))
        .route("/shout-out", put(shout_out::shout_out))
        // User endpoints (require auth)
        .route("/user/inventory", get(user::get_inventory))
        .route("/user/primary-name", put(user::set_primary_name))
        .route("/user/primary-name", delete(user::clear_primary_name))
        .route(
            "/user/names/{name}/metadata",
            put(user::update_name_metadata),
        )
        .route("/star/{target}", put(star::star))
        .route("/star/{target}", delete(star::unstar))
        .route("/user/stars", get(star::get_stars))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth_middleware,
        ));

    // Public routes (no auth required)
    let public_router = Router::new()
        // Name resolution
        .route("/names/{name}", get(name::get_name))
        .route("/addresses/{address}/names", get(name::get_address_names))
        // Auth endpoints
        .route("/auth/login", post(auth::authenticate))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::get_me))
        // Public trading endpoints
        .route("/trading/listings", get(trading::get_listings))
        .route("/name/trading/history/{name}/{offset}", get(name_history))
        .route("/marketing", get(marketing::marketing_info))
        // Rankings endpoints
        .route("/rankings/{type}", get(rankings::get_ranking))
        .route("/shout-outs", get(shout_out::get_shout_outs))
        // WebSocket endpoint
        .route("/ws/connect", get(ws::ws_handler));

    let v1_router = Router::new()
        .merge(authenticated_router)
        .merge(public_router);

    Router::new()
        .route("/health", get(health))
        .nest("/v1", v1_router)
        .with_state(state)
}
