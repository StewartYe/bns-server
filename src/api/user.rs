//! User API handlers
//!
//! Endpoints for user-specific operations:
//! - PUT /v1/user/primary-name - Set primary name
//! - DELETE /v1/user/primary-name - Clear primary name
//! - PUT /v1/user/names/{name}/metadata - Update name metadata

use std::collections::HashMap;

use axum::{
    Json,
    extract::{Path, Request, State},
    http::header,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::constants::SESSION_COOKIE_NAME;
use crate::domain::{SetPrimaryNameRequest, UpdateNameMetadataRequest};
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

/// Helper to extract session from request (supports both cookie and Bearer token)
fn extract_session_token(request: &Request) -> Result<&str, AppError> {
    // Try cookie first (preferred for browser security)
    if let Some(cookie_header) = request.headers().get(header::COOKIE) {
        if let Ok(cookies_str) = cookie_header.to_str() {
            for cookie in cookies_str.split(';') {
                let cookie = cookie.trim();
                if let Some(value) = cookie.strip_prefix(&format!("{}=", SESSION_COOKIE_NAME)) {
                    return Ok(value);
                }
            }
        }
    }

    // Fall back to Bearer token (for API clients)
    request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Unauthorized("Missing session".into()))
}

/// Set primary name for authenticated user
///
/// PUT /v1/user/primary-name
pub async fn set_primary_name(
    State(state): State<AppState>,
    request: Request,
) -> Result<Response, AppError> {
    let session_id = extract_session_token(&request)?;

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

    // Set primary name (UserService handles ownership verification)
    state
        .user_service
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
    let session_id = extract_session_token(&request)?;

    let session = state
        .auth_service
        .validate_session(session_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired session".into()))?;

    // Clear primary name
    state
        .user_service
        .clear_primary_name(&session.btc_address)
        .await?;

    Ok(Json(PrimaryNameResponse {
        address: session.btc_address,
        primary_name: None,
    })
    .into_response())
}

/// Update name metadata
///
/// PUT /v1/user/names/{name}/metadata
pub async fn update_name_metadata(
    State(state): State<AppState>,
    Path(name): Path<String>,
    request: Request,
) -> Result<Response, AppError> {
    let session_id = extract_session_token(&request)?;

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
    })
    .into_response())
}
