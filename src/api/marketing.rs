//! Marketing API handlers
//!
//! Endpoints for marketing and statistics:
//! - GET /v1/marketing - Get platform marketing information and statistics

use crate::AppState;
use crate::domain::MarketingInfo;
use crate::error::Result;
use axum::Json;
use axum::extract::State;

/// Get platform marketing information and statistics
///
/// GET /v1/marketing
///
/// Returns aggregated platform statistics including:
/// - Total users count
/// - Total online users (currently 0)
/// - Total active listings count
/// - 24-hour transaction count
/// - 24-hour trading volume
/// - Total market valuation
pub async fn marketing_info(State(state): State<AppState>) -> Result<Json<MarketingInfo>> {
    let marketing_info = state.marketing_service.get_marketing_info().await?;
    Ok(Json(marketing_info))
}
