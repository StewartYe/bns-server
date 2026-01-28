use crate::AppState;
use crate::domain::MarketingInfo;
use crate::error::Result;
use axum::Json;
use axum::extract::State;

pub async fn marketing_info(State(state): State<AppState>) -> Result<Json<MarketingInfo>> {
    let marketing_info = state.marketing_service.get_marking_info().await?;
    Ok(Json(marketing_info))
}
