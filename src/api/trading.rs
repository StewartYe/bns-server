//! Trading API handlers
//!
//! Endpoints for marketplace trading:
//! - POST /v1/trading/pool - Get pool address for listing a name
//! - POST /v1/trading/list - List a name for sale
//! - GET /v1/trading/listings - Get all listed names

use crate::domain::{
    BuyAndDelistRequest, BuyAndRelistRequest, DelistRequest, DelistResponse, GetListingResponse,
    GetPoolRequest, GetPoolResponse, ListRequest, ListResponse, ListingsResponse,
    NameHistoriesResponse, RelistRequest, RelistResponse, UserHistoriesResponse, UserSession,
};
use crate::error::Result;
use crate::state::AppState;
use axum::extract::Path;
use axum::{
    Extension, Json,
    extract::{Query, State},
};
use serde::Deserialize;

// ============================================================================
// Get Pool Address
// ============================================================================

/// Get or create a pool for listing a name
///
/// POST /v1/trading/pool
///
/// Requires authentication. Verifies that the name belongs to the authenticated
/// user's address with sufficient confirmations, then calls the BNS canister
/// to create a pool.
///
/// This is the first step before listing a name. The returned pool_address
/// is where the inscription will be transferred to.
pub async fn get_pool(
    State(state): State<AppState>,
    Extension(session): Extension<UserSession>,
    Json(req): Json<GetPoolRequest>,
) -> Result<Json<GetPoolResponse>> {
    let response = state
        .trading_service
        .get_pool(&req, &session.btc_address)
        .await?;
    Ok(Json(response))
}

// ============================================================================
// List Name
// ============================================================================

/// List a name for sale
///
/// POST /v1/trading/list
///
/// Broadcasts the signed PSBT to the orchestrator canister and stores
/// the listing for tracking.
pub async fn list(
    State(state): State<AppState>,
    Extension(session): Extension<UserSession>,
    Json(request): Json<ListRequest>,
) -> Result<Json<ListResponse>> {
    let response = state
        .trading_service
        .list(&request, &session.btc_address)
        .await?;
    Ok(Json(response))
}

pub async fn buy_and_relist(
    State(state): State<AppState>,
    Extension(session): Extension<UserSession>,
    Json(request): Json<BuyAndRelistRequest>,
) -> Result<Json<ListResponse>> {
    let response = state
        .trading_service
        .buy_and_relist(&request, &session.btc_address)
        .await?;
    Ok(Json(response))
}

pub async fn buy_and_delist(
    State(state): State<AppState>,
    Extension(session): Extension<UserSession>,
    Json(request): Json<BuyAndDelistRequest>,
) -> Result<Json<ListResponse>> {
    let response = state
        .trading_service
        .buy_and_delist(&request, &session.btc_address)
        .await?;
    Ok(Json(response))
}

pub async fn delist(
    State(state): State<AppState>,
    Extension(session): Extension<UserSession>,
    Json(request): Json<DelistRequest>,
) -> Result<Json<DelistResponse>> {
    let response = state
        .trading_service
        .delist(&request, &session.btc_address)
        .await?;
    Ok(Json(response))
}

pub async fn relist(
    State(state): State<AppState>,
    Extension(session): Extension<UserSession>,
    Json(request): Json<RelistRequest>,
) -> Result<Json<RelistResponse>> {
    let response = state
        .trading_service
        .relist(&request, &session.btc_address)
        .await?;
    Ok(Json(response))
}

// ============================================================================
// Get Listings
// ============================================================================

/// Query parameters for get_listings
#[derive(Debug, Deserialize)]
pub struct ListingsQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Get all listed names with pagination
///
/// GET /v1/trading/listings
pub async fn get_listings(
    State(state): State<AppState>,
    Query(query): Query<ListingsQuery>,
) -> Result<Json<ListingsResponse>> {
    let response = state
        .trading_service
        .get_listings(query.limit, query.offset)
        .await?;
    Ok(Json(response))
}

/// Get all listed names with pagination
///
/// GET /v1/trading/listing/{name}
pub async fn get_listing(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<GetListingResponse>> {
    let response = state.trading_service.get_listing(name.as_str()).await?;
    Ok(Json(response))
}

pub async fn user_history(
    State(state): State<AppState>,
    Extension(session): Extension<UserSession>,
    Path(offset): Path<u32>,
) -> Result<Json<UserHistoriesResponse>> {
    let resp = state
        .trading_service
        .get_user_history(&session.btc_address, None, Some(offset))
        .await?;
    Ok(Json(resp))
}

pub async fn name_history(
    State(state): State<AppState>,
    Path((name,offset)): Path<(String, u32)>,
) -> Result<Json<NameHistoriesResponse>> {
    let resp = state
        .trading_service
        .get_name_history(name.as_str(), None, Some(offset))
        .await?;
    Ok(Json(resp))
}
