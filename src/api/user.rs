//! User API handlers
//!
//! Endpoints for user-specific operations:
//! - PUT /v1/user/primary-name - Set primary name
//! - DELETE /v1/user/primary-name - Clear primary name
//! - GET /v1/names/{name}/metadata - Get name metadata
//! - PUT /v1/names/{name}/metadata - Update name metadata

use std::collections::HashMap;

use axum::{
    extract::{Path, Request, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::domain::{NameMetadata, SetPrimaryNameRequest, UpdateNameMetadataRequest};
use crate::error::AppError;
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

/// Ord backend resolve_rune response (for ownership verification)
#[derive(Debug, Deserialize)]
struct OrdResolveRuneResponse {
    pub result: OrdRuneResult,
}

#[derive(Debug, Deserialize)]
struct OrdRuneResult {
    pub address: String,
}

/// Error response
#[derive(Debug, Serialize)]
struct ErrorResponse {
    pub error: String,
}

/// Helper to extract session from request
fn extract_session_id(request: &Request) -> Result<&str, AppError> {
    request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Unauthorized("Missing authorization header".into()))
}

/// Verify that the given name belongs to the given address by querying Ord
async fn verify_name_ownership(
    state: &AppState,
    name: &str,
    address: &str,
) -> Result<OrdRuneResult, Response> {
    let Some(ord_url) = &state.config.ord_url else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "Ord backend not configured".to_string(),
            }),
        )
            .into_response());
    };

    let url = format!("{}/resolve_rune/{}", ord_url, name);

    let resp = state.http_client.get(&url).send().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response()
    })?;

    let status = resp.status();
    if !status.is_success() {
        return Err((
            StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
            Json(ErrorResponse {
                error: format!("Name not found: {}", name),
            }),
        )
            .into_response());
    }

    let ord_data: OrdResolveRuneResponse = resp.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response()
    })?;

    // Verify ownership
    if ord_data.result.address != address {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: format!("Name {} does not belong to address {}", name, address),
            }),
        )
            .into_response());
    }

    Ok(ord_data.result)
}

/// Set primary name for authenticated user
///
/// PUT /v1/user/primary-name
pub async fn set_primary_name(
    State(state): State<AppState>,
    request: Request,
) -> Result<Response, AppError> {
    let session_id = extract_session_id(&request)?;

    let session = state
        .auth_service
        .validate_session(session_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired session".into()))?;

    // Parse request body
    let body = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::BadRequest("Invalid request body".into()))?;

    let req: SetPrimaryNameRequest = serde_json::from_slice(&body)
        .map_err(|e| AppError::BadRequest(format!("Invalid JSON: {}", e)))?;

    // Verify ownership
    verify_name_ownership(&state, &req.name, &session.btc_address)
        .await
        .map_err(|resp| AppError::Internal(format!("Ownership verification failed: {:?}", resp)))?;

    // Set primary name
    state
        .postgres
        .set_primary_name(&session.btc_address, &req.name)
        .await?;

    Ok(Json(PrimaryNameResponse {
        address: session.btc_address,
        primary_name: Some(req.name),
    })
    .into_response())
}

/// Clear primary name for authenticated user
///
/// DELETE /v1/user/primary-name
pub async fn clear_primary_name(
    State(state): State<AppState>,
    request: Request,
) -> Result<Response, AppError> {
    let session_id = extract_session_id(&request)?;

    let session = state
        .auth_service
        .validate_session(session_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired session".into()))?;

    // Clear primary name
    state.postgres.clear_primary_name(&session.btc_address).await?;

    Ok(Json(PrimaryNameResponse {
        address: session.btc_address,
        primary_name: None,
    })
    .into_response())
}

/// Get name metadata
///
/// GET /v1/names/{name}/metadata
pub async fn get_name_metadata(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Response {
    match state.postgres.get_name_metadata(&name).await {
        Ok(Some(metadata)) => {
            let mut map = HashMap::new();
            if let Some(desc) = metadata.description {
                map.insert("description".to_string(), desc);
            }
            if let Some(url) = metadata.url {
                map.insert("url".to_string(), url);
            }
            if let Some(twitter) = metadata.twitter {
                map.insert("twitter".to_string(), twitter);
            }
            if let Some(email) = metadata.email {
                map.insert("email".to_string(), email);
            }

            Json(NameMetadataResponse { name, metadata: map }).into_response()
        }
        Ok(None) => {
            // Return empty metadata if not found
            Json(NameMetadataResponse {
                name,
                metadata: HashMap::new(),
            })
            .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Update name metadata
///
/// PUT /v1/names/{name}/metadata
pub async fn update_name_metadata(
    State(state): State<AppState>,
    Path(name): Path<String>,
    request: Request,
) -> Result<Response, AppError> {
    let session_id = extract_session_id(&request)?;

    let session = state
        .auth_service
        .validate_session(session_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired session".into()))?;

    // Parse request body
    let body = axum::body::to_bytes(request.into_body(), 1024 * 10)
        .await
        .map_err(|_| AppError::BadRequest("Invalid request body".into()))?;

    let req: UpdateNameMetadataRequest = serde_json::from_slice(&body)
        .map_err(|e| AppError::BadRequest(format!("Invalid JSON: {}", e)))?;

    // Verify ownership
    verify_name_ownership(&state, &name, &session.btc_address)
        .await
        .map_err(|_| AppError::Forbidden("Name does not belong to this address".into()))?;

    // Get existing metadata or create new
    let now = Utc::now();
    let existing = state.postgres.get_name_metadata(&name).await?;

    let metadata = NameMetadata {
        name: name.clone(),
        owner_address: session.btc_address.clone(),
        description: req.description,
        url: req.url,
        twitter: req.twitter,
        email: req.email,
        created_at: existing.map(|m| m.created_at).unwrap_or(now),
        updated_at: now,
    };

    state.postgres.upsert_name_metadata(&metadata).await?;

    // Return updated metadata as HashMap
    let mut map = HashMap::new();
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

    Ok(Json(NameMetadataResponse { name, metadata: map }).into_response())
}
