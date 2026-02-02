use crate::AppState;
use crate::domain::{StarResponse, UserSession};
use axum::extract::{Path, State};
use axum::{Extension, Json};

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
