//! Name resolution API handlers
//!
//! Endpoints:
//! - GET /v1/names/{name} - Forward resolution (name -> address)
//! - GET /v1/addresses/{address}/names - Reverse resolution (address -> names)

use std::collections::HashMap;

use crate::error::AppError;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

// ============================================================================
// Response types
// ============================================================================

/// Name resolution result for GET /v1/names/{name}
#[derive(Debug, Serialize)]
pub struct NameResult {
    pub name: String,
    pub address: String,
    pub id: String,
    pub inscription_id: String,
    pub inscription_number: u64,
    pub confirmations: u64,
    pub metadata: HashMap<String, String>,
}

/// GET /v1/names/{name} response
#[derive(Debug, Serialize)]
pub struct GetNameResponse {
    pub result: NameResult,
}

/// Name entry in address names response
#[derive(Debug, Serialize)]
pub struct NameEntry {
    pub name: String,
    pub id: String,
    pub is_primary: bool,
    pub confirmations: u64,
}

/// GET /v1/addresses/{address}/names response
#[derive(Debug, Serialize)]
pub struct GetAddressNamesResponse {
    pub address: String,
    pub names: Vec<NameEntry>,
    pub page: u32,
    pub page_size: u32,
    pub total: u32,
}

/// Query parameters for pagination
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    20
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// Forward resolution: name -> address
///
/// GET /v1/names/{name}
///
/// Returns the Bitcoin address, inscription details, and metadata for a given name.
pub async fn get_name(State(state): State<AppState>, Path(name): Path<String>) -> Response {
    match state.name_service.get_name(&name).await {
        Ok(info) => {
            let response = GetNameResponse {
                result: NameResult {
                    name: info.name,
                    address: info.address,
                    id: info.id,
                    inscription_id: info.inscription_id,
                    inscription_number: info.inscription_number,
                    confirmations: info.confirmations,
                    metadata: info.metadata,
                },
            };
            Json(response).into_response()
        }
        Err(AppError::NotFound(msg)) => {
            (StatusCode::NOT_FOUND, Json(ErrorResponse { error: msg })).into_response()
        }
        Err(AppError::Internal(msg)) if msg.contains("not configured") => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse { error: msg }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Reverse resolution: address -> names
///
/// GET /v1/addresses/{address}/names?page=1&page_size=20
///
/// Returns all names owned by a given Bitcoin address with pagination.
pub async fn get_address_names(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(pagination): Query<PaginationQuery>,
) -> Response {
    match state
        .name_service
        .get_address_names(&address, pagination.page, pagination.page_size)
        .await
    {
        Ok(result) => {
            let response = GetAddressNamesResponse {
                address: result.address,
                names: result
                    .names
                    .into_iter()
                    .map(|n| NameEntry {
                        name: n.name,
                        id: n.id,
                        is_primary: n.is_primary,
                        confirmations: n.confirmations,
                    })
                    .collect(),
                page: result.page,
                page_size: result.page_size,
                total: result.total,
            };
            Json(response).into_response()
        }
        Err(AppError::Internal(msg)) if msg.contains("not configured") => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse { error: msg }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}
