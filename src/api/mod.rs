//! API layer
//!
//! HTTP endpoints:
//! - SDK API: Name resolution for external clients (always available)
//! - Auth: BIP-322 authentication (only when database is configured)

mod auth;
mod sdk;

pub use auth::*;
pub use sdk::*;

use axum::{routing::{get, post}, Router};

use crate::state::AppState;

/// Build the API router
pub fn build_router(state: AppState) -> Router {
    let mut router = Router::new()
        // SDK endpoints (name resolution) - always available
        .merge(sdk::router());

    // Auth endpoints - only if auth service is available
    if state.auth_service.is_some() {
        router = router
            .route("/v1/auth/login", post(auth::authenticate))
            .route("/v1/auth/logout", post(auth::logout))
            .route("/v1/auth/me", get(auth::get_me));
    }

    router.with_state(state)
}
