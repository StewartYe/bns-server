//! Name resolution API handlers
//!
//! Endpoints for external clients (DApps, wallets):
//! - GET /v1/names/{name} - Forward resolution (name -> address)
//! - GET /v1/addresses/{address}/names - Reverse resolution (address -> names)
//!
//! These endpoints proxy requests to the Ord backend server.

use axum::{
    extract::{Path, State},
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

/// Name resolution result
#[derive(Debug, Serialize, Deserialize)]
pub struct NameResult {
    pub address: String,
    pub inscription_id: String,
}

/// GET /v1/names/{name} response
#[derive(Debug, Serialize, Deserialize)]
pub struct GetNameResponse {
    pub result: NameResult,
}

/// GET /v1/addresses/{address}/names response
#[derive(Debug, Serialize, Deserialize)]
pub struct GetAddressNamesResponse {
    pub rune_names: Vec<String>,
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
/// Proxies request to Ord backend and returns the Bitcoin address
/// and inscription ID for a given name.
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

    // Ord backend uses /resolve_rune/{rune}
    let url = format!("{}/resolve_rune/{}", ord_url, name);

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

            match resp.json::<GetNameResponse>().await {
                Ok(data) => Json(data).into_response(),
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
/// GET /v1/addresses/{address}/names
///
/// Proxies request to Ord backend and returns all names
/// owned by a given Bitcoin address.
pub async fn get_address_names(
    State(state): State<AppState>,
    Path(address): Path<String>,
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

    // Ord backend uses /resolve_address/{address}
    let url = format!("{}/resolve_address/{}", ord_url, address);

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

            match resp.json::<GetAddressNamesResponse>().await {
                Ok(data) => Json(data).into_response(),
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
