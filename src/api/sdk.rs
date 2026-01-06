//! Name resolution API handlers
//!
//! Endpoints for external clients (DApps, wallets):
//! - GET /v1/names/{name} - Forward resolution (name -> address)
//! - GET /v1/addresses/{address}/names - Reverse resolution (address -> names)
//!
//! These endpoints proxy requests to the Ord backend server.

use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        // V1 API endpoints
        .route("/v1/names/{name}", get(get_name))
        .route("/v1/addresses/{address}/names", get(get_address_names))
        // Health check (no version prefix for Cloud Run compatibility)
        .route("/health", get(health))
}

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

/// Ord backend /bns/rune/{rune} response
#[derive(Debug, Deserialize)]
struct OrdResolveRuneResponse {
    pub result: OrdRuneResult,
}

#[derive(Debug, Deserialize)]
struct OrdRuneResult {
    pub address: String,
    pub inscription_id: String,
    pub rune_id: String,
    pub inscription_number: u64,
    pub confirmations: u64,
}

/// Name entry in address names response
#[derive(Debug, Serialize, Deserialize)]
pub struct NameEntry {
    pub name: String,
    pub id: String,
    pub is_primary: bool,
    pub confirmations: u64,
}

/// GET /v1/addresses/{address}/names response
#[derive(Debug, Serialize, Deserialize)]
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

/// Backend response from Ord for /bns/address/{address}
#[derive(Debug, Deserialize)]
struct OrdAddressNamesResponse {
    pub runes: Vec<OrdRuneEntry>,
}

#[derive(Debug, Deserialize)]
struct OrdRuneEntry {
    pub rune_id: String,
    pub rune_name: String,
    pub confirmations: u64,
}

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
/// Proxies request to Ord backend and returns the Bitcoin address,
/// inscription details, and metadata for a given name.
pub async fn get_name(State(state): State<AppState>, Path(name): Path<String>) -> Response {
    let Some(ord_url) = &state.config.ord_url else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "Ord backend not configured".to_string(),
            }),
        )
            .into_response();
    };

    // Ord backend uses /bns/rune/{rune}
    let url = format!("{}/bns/rune/{}", ord_url, name);

    match state.http_client.get(&url).send().await {
        Ok(resp) => {
            let status = resp.status();
            if !status.is_success() {
                return (
                    StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
                    Json(ErrorResponse {
                        error: format!("Backend returned status: {}", status),
                    }),
                )
                    .into_response();
            }

            match resp.json::<OrdResolveRuneResponse>().await {
                Ok(ord_data) => {
                    // Fetch metadata from database
                    let mut metadata = HashMap::new();
                    if let Ok(Some(db_metadata)) = state.postgres.get_name_metadata(&name).await {
                        if let Some(desc) = db_metadata.description {
                            metadata.insert("description".to_string(), desc);
                        }
                        if let Some(url) = db_metadata.url {
                            metadata.insert("url".to_string(), url);
                        }
                        if let Some(twitter) = db_metadata.twitter {
                            metadata.insert("twitter".to_string(), twitter);
                        }
                        if let Some(email) = db_metadata.email {
                            metadata.insert("email".to_string(), email);
                        }
                    }

                    let response = GetNameResponse {
                        result: NameResult {
                            name: name.clone(),
                            address: ord_data.result.address,
                            id: ord_data.result.rune_id,
                            inscription_id: ord_data.result.inscription_id,
                            inscription_number: ord_data.result.inscription_number,
                            confirmations: ord_data.result.confirmations,
                            metadata,
                        },
                    };
                    Json(response).into_response()
                }
                Err(e) => (
                    StatusCode::BAD_GATEWAY,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                )
                    .into_response(),
            }
        }
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
/// Proxies request to Ord backend and returns all names
/// owned by a given Bitcoin address with pagination.
pub async fn get_address_names(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(pagination): Query<PaginationQuery>,
) -> Response {
    let Some(ord_url) = &state.config.ord_url else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "Ord backend not configured".to_string(),
            }),
        )
            .into_response();
    };

    // Ord backend uses /bns/address/{address}
    let url = format!("{}/bns/address/{}", ord_url, address);

    match state.http_client.get(&url).send().await {
        Ok(resp) => {
            let status = resp.status();
            if !status.is_success() {
                return (
                    StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
                    Json(ErrorResponse {
                        error: format!("Backend returned status: {}", status),
                    }),
                )
                    .into_response();
            }

            match resp.json::<OrdAddressNamesResponse>().await {
                Ok(ord_data) => {
                    let total = ord_data.runes.len() as u32;
                    let page = pagination.page.max(1);
                    let page_size = pagination.page_size.clamp(1, 100);

                    // Get user's primary name from database
                    let primary_name = state
                        .postgres
                        .get_user(&address)
                        .await
                        .ok()
                        .flatten()
                        .and_then(|u| u.primary_name);

                    // Calculate pagination bounds
                    let start = ((page - 1) * page_size) as usize;
                    let end = (start + page_size as usize).min(ord_data.runes.len());

                    // Map to NameEntry with rune_id, is_primary, and confirmations
                    let names: Vec<NameEntry> = if start < ord_data.runes.len() {
                        ord_data.runes[start..end]
                            .iter()
                            .map(|rune| NameEntry {
                                name: rune.rune_name.clone(),
                                id: rune.rune_id.clone(),
                                is_primary: primary_name
                                    .as_ref()
                                    .is_some_and(|p| p == &rune.rune_name),
                                confirmations: rune.confirmations,
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };

                    let response = GetAddressNamesResponse {
                        address: address.clone(),
                        names,
                        page,
                        page_size,
                        total,
                    };

                    Json(response).into_response()
                }
                Err(e) => (
                    StatusCode::BAD_GATEWAY,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Health check endpoint
///
/// GET /health
pub async fn health() -> &'static str {
    "OK"
}
