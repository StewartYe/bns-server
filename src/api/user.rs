//! User API handlers
//!
//! Endpoints for user-specific operations:
//! - GET /v1/user/inventory - Get user inventory (listed and unlisted names)
//! - PUT /v1/user/primary-name - Set primary name
//! - DELETE /v1/user/primary-name - Clear primary name
//! - PUT /v1/user/names/{name}/metadata - Update name metadata

use std::collections::HashMap;

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use serde::Serialize;

use crate::domain::{SetPrimaryNameRequest, UpdateNameMetadataRequest, UserInventory, UserSession};
use crate::error::Result;
use crate::state::AppState;

/// Response for primary name operations
#[derive(Debug, Serialize)]
pub struct PrimaryNameResponse {
    pub address: String,
    pub primary_name: Option<String>,
}

/// Response for name metadata
#[derive(Debug, Serialize)]
pub struct NameMetadataResponse {
    pub name: String,
    pub metadata: HashMap<String, String>,
}

/// Get user inventory
///
/// GET /v1/user/inventory
pub async fn get_inventory(
    State(state): State<AppState>,
    Extension(session): Extension<UserSession>,
) -> Result<Json<UserInventory>> {
    let inventory = state
        .user_service
        .get_inventory(&session.btc_address)
        .await?;

    Ok(Json(inventory))
}

/// Set primary name for authenticated user
///
/// PUT /v1/user/primary-name
pub async fn set_primary_name(
    State(state): State<AppState>,
    Extension(session): Extension<UserSession>,
    Json(req): Json<SetPrimaryNameRequest>,
) -> Result<Json<PrimaryNameResponse>> {
    // Set primary name (UserService handles ownership verification)
    state
        .user_service
        .set_primary_name(&session.btc_address, &req.name)
        .await?;

    Ok(Json(PrimaryNameResponse {
        address: session.btc_address,
        primary_name: Some(req.name),
    }))
}

/// Clear primary name for authenticated user
///
/// DELETE /v1/user/primary-name
pub async fn clear_primary_name(
    State(state): State<AppState>,
    Extension(session): Extension<UserSession>,
) -> Result<Json<PrimaryNameResponse>> {
    // Clear primary name
    state
        .user_service
        .clear_primary_name(&session.btc_address)
        .await?;

    Ok(Json(PrimaryNameResponse {
        address: session.btc_address,
        primary_name: None,
    }))
}

/// Update name metadata
///
/// PUT /v1/user/names/{name}/metadata
pub async fn update_name_metadata(
    State(state): State<AppState>,
    Extension(session): Extension<UserSession>,
    Path(name): Path<String>,
    Json(req): Json<UpdateNameMetadataRequest>,
) -> Result<Json<NameMetadataResponse>> {
    // Update metadata (UserService handles ownership verification)
    let metadata = state
        .user_service
        .update_name_metadata(&name, &session.btc_address, req)
        .await?;

    // Return updated metadata as HashMap
    let mut map: HashMap<String, String> = HashMap::new();
    if let Some(ref desc) = metadata.description {
        map.insert("description".to_string(), desc.clone());
    }
    if let Some(ref url) = metadata.url {
        map.insert("url".to_string(), url.clone());
    }
    if let Some(ref twitter) = metadata.twitter {
        map.insert("twitter".to_string(), twitter.clone());
    }
    if let Some(ref email) = metadata.email {
        map.insert("email".to_string(), email.clone());
    }

    Ok(Json(NameMetadataResponse {
        name,
        metadata: map,
    }))
}
