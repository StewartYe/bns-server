//! Rankings API endpoints
//!
//! Provides REST endpoints for initial ranking data (max 20 items each):
//! - GET /rankings/new-listings - Newest listings (implemented)
//! - GET /rankings/recent-sales - Recent sales (placeholder)
//! - GET /rankings/top-earners - Top earners by address (placeholder)
//! - GET /rankings/most-traded - Most traded names (placeholder)
//! - GET /rankings/top-sales - Highest price sales (placeholder)
//! - GET /rankings/best-deals - Best discount deals (placeholder)
//!
//! Delta updates are delivered via WebSocket Pub/Sub

use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::infra::ListingMeta;
use crate::state::AppState;

/// Maximum items returned per ranking
const MAX_RANKING_ITEMS: usize = 20;

/// Supported ranking types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankingType {
    /// Newest listings - shows latest names listed for sale
    NewListings,
    /// Recent sales - shows recently completed transactions
    RecentSales,
    /// Top earners - addresses ranked by cumulative profit
    TopEarners,
    /// Most traded - names ranked by number of transactions
    MostTraded,
    /// Top sales - names ranked by highest sale price
    TopSales,
    /// Best deals - current listings with highest discount percentage
    BestDeals,
}

impl RankingType {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "new-listings" => Some(RankingType::NewListings),
            "recent-sales" => Some(RankingType::RecentSales),
            "top-earners" => Some(RankingType::TopEarners),
            "most-traded" => Some(RankingType::MostTraded),
            "top-sales" => Some(RankingType::TopSales),
            "best-deals" => Some(RankingType::BestDeals),
            _ => None,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            RankingType::NewListings => "new-listings",
            RankingType::RecentSales => "recent-sales",
            RankingType::TopEarners => "top-earners",
            RankingType::MostTraded => "most-traded",
            RankingType::TopSales => "top-sales",
            RankingType::BestDeals => "best-deals",
        }
    }
}

/// All supported ranking types for error messages
const SUPPORTED_RANKINGS: [&str; 6] = [
    "new-listings",
    "recent-sales",
    "top-earners",
    "most-traded",
    "top-sales",
    "best-deals",
];

/// Response for ranking endpoint
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankingResponse<T> {
    /// The ranking type
    pub ranking_type: String,
    /// List of items in the ranking (max 20)
    pub items: Vec<T>,
    /// Total count of items returned
    pub total: usize,
}

/// Item for new-listings ranking
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewListingItem {
    /// The name
    pub name: String,
    /// Price in satoshis
    pub price_sats: u64,
    /// Seller's Bitcoin address
    pub seller_address: String,
    /// Unix timestamp when listed
    pub listed_at: i64,
    /// Optional transaction ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_id: Option<String>,
}

impl From<ListingMeta> for NewListingItem {
    fn from(meta: ListingMeta) -> Self {
        Self {
            name: meta.name,
            price_sats: meta.price_sats,
            seller_address: meta.seller_address,
            listed_at: meta.listed_at,
            tx_id: meta.tx_id,
        }
    }
}

/// Item for recent-sales ranking
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentSaleItem {
    pub name: String,
    pub price_sats: u64,
    pub seller_address: String,
    pub buyer_address: String,
    pub sold_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_id: Option<String>,
}

/// Item for top-earners ranking
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopEarnerItem {
    pub address: String,
    pub total_profit_sats: i64,
    pub trade_count: u32,
}

/// Item for most-traded ranking
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MostTradedItem {
    pub name: String,
    pub trade_count: u32,
    pub last_price_sats: u64,
    pub last_traded_at: i64,
}

/// Item for top-sales ranking
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopSaleItem {
    pub name: String,
    pub price_sats: u64,
    pub seller_address: String,
    pub buyer_address: String,
    pub sold_at: i64,
}

/// Item for best-deals ranking
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BestDealItem {
    pub name: String,
    pub current_price_sats: u64,
    pub previous_price_sats: u64,
    pub discount_percent: f64,
    pub seller_address: String,
    pub listed_at: i64,
}

/// GET /rankings/{type}
///
/// Returns the initial snapshot of a ranking (max 20 items).
/// Use WebSocket to subscribe for real-time delta updates.
pub async fn get_ranking(
    Path(ranking_type): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let Some(ranking) = RankingType::from_str(&ranking_type) else {
        return Json(serde_json::json!({
            "error": format!("Unknown ranking type: {}", ranking_type),
            "supported": SUPPORTED_RANKINGS
        }))
        .into_response();
    };

    match ranking {
        RankingType::NewListings => get_new_listings_ranking(state).await,
        RankingType::RecentSales => get_placeholder_ranking(ranking).await,
        RankingType::TopEarners => get_placeholder_ranking(ranking).await,
        RankingType::MostTraded => get_placeholder_ranking(ranking).await,
        RankingType::TopSales => get_placeholder_ranking(ranking).await,
        RankingType::BestDeals => get_placeholder_ranking(ranking).await,
    }
}

/// Get new-listings ranking (implemented)
async fn get_new_listings_ranking(state: AppState) -> axum::response::Response {
    match state.redis_client.get_new_listings(MAX_RANKING_ITEMS).await {
        Ok(listings) => {
            let items: Vec<NewListingItem> =
                listings.into_iter().map(NewListingItem::from).collect();
            let total = items.len();
            Json(RankingResponse {
                ranking_type: "new-listings".to_string(),
                items,
                total,
            })
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get new-listings ranking: {:?}", e);
            Json(serde_json::json!({
                "error": format!("{:?}", e),
                "items": [],
                "total": 0
            }))
            .into_response()
        }
    }
}

/// Placeholder for unimplemented rankings
async fn get_placeholder_ranking(ranking: RankingType) -> axum::response::Response {
    Json(serde_json::json!({
        "rankingType": ranking.as_str(),
        "items": [],
        "total": 0,
        "message": "This ranking is not yet implemented"
    }))
    .into_response()
}
