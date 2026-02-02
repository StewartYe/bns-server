//! Star API handlers
//!
//! Endpoints for starring/bookmarking names and collectors:
//! - PUT /v1/star/{target} - Star a name or collector
//! - DELETE /v1/star/{target} - Unstar a name or collector
//! - GET /v1/user/stars - Get user's starred items

use crate::AppState;
use crate::domain::{StarResponse, UserSession};
use axum::extract::{Path, State};
use axum::{Extension, Json};

/// Star (bookmark) a name or collector address
///
/// PUT /v1/star/{target}
///
/// Requires authentication. The target can be either:
/// - A BNS name (e.g., "alice")
/// - A Bitcoin collector address
///
/// The system will automatically detect the type and validate accordingly.
pub async fn star(
    State(state): State<AppState>,
    Extension(session): Extension<UserSession>,
    Path(target): Path<String>,
) -> crate::Result<()> {
    state
        .star_service
        .star(session.btc_address.as_str(), target.as_str())
        .await?;
    Ok(())
}

/// Unstar (remove bookmark) a name or collector address
///
/// DELETE /v1/star/{target}
///
/// Requires authentication. Removes the star from the authenticated user's
/// starred items.
pub async fn unstar(
    State(state): State<AppState>,
    Extension(session): Extension<UserSession>,
    Path(target): Path<String>,
) -> crate::Result<()> {
    state
        .star_service
        .unstar(session.btc_address.as_str(), target.as_str())
        .await?;
    Ok(())
}

/// Get all starred items for the authenticated user
///
/// GET /v1/user/stars
///
/// Requires authentication. Returns a list of all names and collector
/// addresses that the user has starred.
pub async fn get_stars(
    State(state): State<AppState>,
    Extension(session): Extension<UserSession>,
) -> crate::Result<Json<Vec<StarResponse>>> {
    let v = state
        .star_service
        .get_stars(session.btc_address.as_str())
        .await?;
    Ok(Json(v))
}
