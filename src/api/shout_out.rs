use crate::AppState;
use crate::domain::{ShoutOut, ShoutOutRequest, UserSession};
use crate::error::Result;
use axum::extract::State;
use axum::{Extension, Json};

pub async fn shout_out(
    State(state): State<AppState>,
    Extension(session): Extension<UserSession>,
    Json(req): Json<ShoutOutRequest>,
) -> Result<()> {
    state
        .shout_out_service
        .shout_out(session.btc_address.as_str(), req)
        .await
}

pub async fn get_shout_outs(State(s): State<AppState>) -> Result<Json<Vec<ShoutOut>>> {
    let shout_outs = s.postgres.get_last_n_shout_out(30).await?;
    Ok(Json(shout_outs))
}
