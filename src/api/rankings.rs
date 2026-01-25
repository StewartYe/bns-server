//! Rankings API endpoints
//!
//! Provides REST endpoints for initial ranking data (max 20 items each):
//! - GET /rankings/new-listings - Newest listings
//! - GET /rankings/recent-sales - Recent sales
//! - GET /rankings/top-earners - Top earners by address
//! - GET /rankings/most-traded - Most traded names
//! - GET /rankings/top-sales - Highest price listings
//! - GET /rankings/best-deals - Best discount deals
//!
//! Delta updates are delivered via WebSocket Pub/Sub

use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

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

/// Item for new-listings ranking (score = listed_at)
/// Same fields as TopSaleItem and BestDealItem
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewListingItem {
    pub name: String,
    pub price_sats: u64,
    pub listed_at: i64,
    pub discount: f64,
    pub seller_address: String,
}

/// Item for top-sales ranking (score = price_sats)
/// Same fields as NewListingItem and BestDealItem
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopSaleItem {
    pub name: String,
    pub price_sats: u64,
    pub listed_at: i64,
    pub discount: f64,
    pub seller_address: String,
}

/// Item for best-deals ranking (score = discount)
/// Same fields as NewListingItem and TopSaleItem
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BestDealItem {
    pub name: String,
    pub price_sats: u64,
    pub listed_at: i64,
    pub discount: f64,
    pub seller_address: String,
}

/// Item for recent-sales ranking (score = sold_at)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentSaleItem {
    pub name: String,
    pub price_sats: u64,
    pub seller_address: String,
    pub buyer_address: String,
    pub sold_at: i64,
}

/// Item for most-traded ranking (score = trade_count)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MostTradedItem {
    pub name: String,
    pub price_sats: u64,
    pub seller_address: String,
    pub buyer_address: String,
    pub trade_count: u32,
    pub sold_at: i64,
}

/// Item for top-earners ranking (score = total_profit_sats)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopEarnerItem {
    pub address: String,
    pub total_profit_sats: i64,
    pub trade_count: u32,
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
        RankingType::RecentSales => get_recent_sales_ranking(state).await,
        RankingType::TopEarners => get_top_earners_ranking(state).await,
        RankingType::MostTraded => get_most_traded_ranking(state).await,
        RankingType::TopSales => get_top_sales_ranking(state).await,
        RankingType::BestDeals => get_best_deals_ranking(state).await,
    }
}

async fn get_new_listings_ranking(state: AppState) -> axum::response::Response {
    match state.redis_client.get_new_listings(MAX_RANKING_ITEMS).await {
        Ok(items) => {
            let total = items.len();
            Json(RankingResponse {
                ranking_type: RankingType::NewListings.as_str().to_string(),
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

async fn get_top_sales_ranking(state: AppState) -> axum::response::Response {
    match state.redis_client.get_top_sales(MAX_RANKING_ITEMS).await {
        Ok(items) => {
            let total = items.len();
            Json(RankingResponse {
                ranking_type: RankingType::TopSales.as_str().to_string(),
                items,
                total,
            })
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get top-sales ranking: {:?}", e);
            Json(serde_json::json!({
                "error": format!("{:?}", e),
                "items": [],
                "total": 0
            }))
            .into_response()
        }
    }
}

async fn get_best_deals_ranking(state: AppState) -> axum::response::Response {
    match state.redis_client.get_best_deals(MAX_RANKING_ITEMS).await {
        Ok(items) => {
            let total = items.len();
            Json(RankingResponse {
                ranking_type: RankingType::BestDeals.as_str().to_string(),
                items,
                total,
            })
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get best-deals ranking: {:?}", e);
            Json(serde_json::json!({
                "error": format!("{:?}", e),
                "items": [],
                "total": 0
            }))
            .into_response()
        }
    }
}

async fn get_recent_sales_ranking(state: AppState) -> axum::response::Response {
    match state.redis_client.get_recent_sales(MAX_RANKING_ITEMS).await {
        Ok(items) => {
            let total = items.len();
            Json(RankingResponse {
                ranking_type: RankingType::RecentSales.as_str().to_string(),
                items,
                total,
            })
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get recent-sales ranking: {:?}", e);
            Json(serde_json::json!({
                "error": format!("{:?}", e),
                "items": [],
                "total": 0
            }))
            .into_response()
        }
    }
}

async fn get_most_traded_ranking(state: AppState) -> axum::response::Response {
    match state.redis_client.get_most_traded(MAX_RANKING_ITEMS).await {
        Ok(items) => {
            let total = items.len();
            Json(RankingResponse {
                ranking_type: RankingType::MostTraded.as_str().to_string(),
                items,
                total,
            })
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get most-traded ranking: {:?}", e);
            Json(serde_json::json!({
                "error": format!("{:?}", e),
                "items": [],
                "total": 0
            }))
            .into_response()
        }
    }
}

async fn get_top_earners_ranking(state: AppState) -> axum::response::Response {
    match state.redis_client.get_top_earners(MAX_RANKING_ITEMS).await {
        Ok(items) => {
            let total = items.len();
            Json(RankingResponse {
                ranking_type: RankingType::TopEarners.as_str().to_string(),
                items,
                total,
            })
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get top-earners ranking: {:?}", e);
            Json(serde_json::json!({
                "error": format!("{:?}", e),
                "items": [],
                "total": 0
            }))
            .into_response()
        }
    }
}
