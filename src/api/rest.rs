//! REST API handlers
//!
//! Endpoints for:
//! - Market queries (rankings, stats, listings)
//! - User operations (inventory, history)
//! - Trading operations (list, delist, buy)
//! - ShoutOut management

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::domain::*;
use crate::error::Result;
use crate::service::{RankingEntry, RankingType, MarketStats};
use crate::state::AppState;

// ============================================================================
// Router
// ============================================================================

pub fn router() -> Router<AppState> {
    Router::new()
        // Auth endpoints
        .route("/api/auth/siwb", post(super::auth::authenticate))
        .route("/api/auth/logout", post(super::auth::logout))
        .route("/api/auth/me", get(super::auth::get_me))
        // Market endpoints
        .route("/api/market/rankings/{ranking_type}", get(get_rankings))
        .route("/api/market/stats", get(get_market_stats))
        .route("/api/market/listings", get(get_listings))
        // Name endpoints
        .route("/api/names/{name}", get(get_name_detail))
        .route("/api/names/search", get(search_names))
        // User endpoints
        .route("/api/users/{address}/inventory", get(get_user_inventory))
        .route("/api/users/{address}/history", get(get_user_history))
        // Trading endpoints
        .route("/api/trading/list/initiate", post(initiate_list))
        .route("/api/trading/list/complete", post(complete_list))
        .route("/api/trading/delist", post(delist))
        .route("/api/trading/buy", post(buy))
        // ShoutOut endpoints
        .route("/api/shoutouts", get(get_shoutouts))
        .route("/api/shoutouts", post(create_shoutout))
}

// ============================================================================
// Market handlers
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct RankingsQuery {
    pub limit: Option<u32>,
}

pub async fn get_rankings(
    State(state): State<AppState>,
    Path(ranking_type): Path<String>,
    Query(query): Query<RankingsQuery>,
) -> Result<Json<Vec<RankingEntry>>> {
    let ranking_type = parse_ranking_type(&ranking_type)?;
    let limit = query.limit.unwrap_or(20);
    let rankings = state.market_service.get_ranking(ranking_type, limit).await?;
    Ok(Json(rankings))
}

pub async fn get_market_stats(
    State(state): State<AppState>,
) -> Result<Json<MarketStats>> {
    let stats = state.market_service.get_stats().await?;
    Ok(Json(stats))
}

#[derive(Debug, Deserialize)]
pub struct ListingsQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ListingsResponse {
    pub listings: Vec<Listing>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
}

pub async fn get_listings(
    State(_state): State<AppState>,
    Query(_query): Query<ListingsQuery>,
) -> Result<Json<ListingsResponse>> {
    todo!("Implement get_listings")
}

// ============================================================================
// Name handlers
// ============================================================================

pub async fn get_name_detail(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<Option<NameDetail>>> {
    let detail = state.name_service.get_detail(&name).await?;
    Ok(Json(detail))
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub keyword: String,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

pub async fn search_names(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<NameSearchResult>> {
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(20);
    let result = state.name_service.search(&query.keyword, page, page_size).await?;
    Ok(Json(result))
}

// ============================================================================
// User handlers
// ============================================================================

pub async fn get_user_inventory(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Result<Json<UserInventory>> {
    let inventory = state.user_service.get_inventory(&address).await?;
    Ok(Json(inventory))
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub async fn get_user_history(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<UserHistory>> {
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);
    let history = state.user_service.get_history(&address, limit, offset).await?;
    Ok(Json(history))
}

// ============================================================================
// Trading handlers
// ============================================================================

#[derive(Debug, Serialize)]
pub struct InitiateListResponse {
    pub pool_address: String,
}

pub async fn initiate_list(
    State(_state): State<AppState>,
    Json(_request): Json<CreateListingRequest>,
) -> Result<Json<InitiateListResponse>> {
    // TODO: Extract user from auth middleware
    todo!("Implement initiate_list")
}

#[derive(Debug, Deserialize)]
pub struct CompleteListRequest {
    pub name: String,
    pub pool_address: String,
    pub price_sats: u64,
    pub tx_hex: String,
}

pub async fn complete_list(
    State(_state): State<AppState>,
    Json(_request): Json<CompleteListRequest>,
) -> Result<Json<Listing>> {
    todo!("Implement complete_list")
}

pub async fn delist(
    State(_state): State<AppState>,
    Json(_request): Json<DelistRequest>,
) -> Result<Json<()>> {
    todo!("Implement delist")
}

#[derive(Debug, Deserialize)]
pub struct BuyRequestWithTx {
    pub name: String,
    pub action: BuyAction,
    pub relist_price_sats: Option<u64>,
    pub tx_hex: String,
}

#[derive(Debug, Serialize)]
pub struct BuyResponse {
    pub name: String,
    pub tx_id: String,
    pub new_pool_address: Option<String>,
}

pub async fn buy(
    State(_state): State<AppState>,
    Json(_request): Json<BuyRequestWithTx>,
) -> Result<Json<BuyResponse>> {
    todo!("Implement buy")
}

// ============================================================================
// ShoutOut handlers
// ============================================================================

pub async fn get_shoutouts(
    State(state): State<AppState>,
) -> Result<Json<ShoutOutList>> {
    let list = state.shoutout_service.get_active().await?;
    Ok(Json(list))
}

pub async fn create_shoutout(
    State(_state): State<AppState>,
    Json(_request): Json<CreateShoutOutRequest>,
) -> Result<Json<ShoutOut>> {
    // TODO: Extract user from auth middleware
    todo!("Implement create_shoutout")
}

// ============================================================================
// Helpers
// ============================================================================

fn parse_ranking_type(s: &str) -> Result<RankingType> {
    match s {
        "top_earners_24h" | "24h_winners" => Ok(RankingType::TopEarners24h),
        "new_list" => Ok(RankingType::NewList),
        "last_sold" => Ok(RankingType::LastSold),
        "active_1h" | "1h_active" => Ok(RankingType::Active1h),
        "active_24h" | "24h_active" => Ok(RankingType::Active24h),
        "top_sell_24h" | "24h_top_sell" => Ok(RankingType::TopSell24h),
        "best_discount" => Ok(RankingType::BestDiscount),
        "best_bargain" => Ok(RankingType::BestBargain),
        _ => Err(crate::error::AppError::BadRequest(format!(
            "Unknown ranking type: {}",
            s
        ))),
    }
}
