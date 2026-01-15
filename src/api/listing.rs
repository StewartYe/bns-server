//! Listing API handlers
//!
//! Endpoints:
//! - POST /v1/listings - List a name for sale
//! - GET /v1/listings - Get all listed names

use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;

use crate::domain::{ListNameRequest, ListNameResponse, ListedNamesResponse};
use crate::error::Result;
use crate::state::AppState;

/// Query parameters for get_listed_names
#[derive(Debug, Deserialize)]
pub struct ListingsQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// List a name for sale
///
/// POST /v1/listings
///
/// Requires authentication.
/// Broadcasts the PSBT and stores the listing.
pub async fn list_name(
    State(state): State<AppState>,
    Json(request): Json<ListNameRequest>,
) -> Result<Json<ListNameResponse>> {
    let response = state.listing_service.list_name(&request).await?;
    Ok(Json(response))
}

/// Get all listed names
///
/// GET /v1/listings
pub async fn get_listed_names(
    State(state): State<AppState>,
    Query(query): Query<ListingsQuery>,
) -> Result<Json<ListedNamesResponse>> {
    let response = state
        .listing_service
        .get_listed_names(query.limit, query.offset)
        .await?;
    Ok(Json(response))
}
