//! Pool API handlers
//!
//! Endpoints for pool management:
//! - POST /v1/pool - Get or create a pool for a name

use axum::{
    Json,
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::constants::FINALIZE_THRESHOLD;
use crate::domain::UserSession;
use crate::error::AppError;
use crate::state::AppState;

/// Request to get or create a pool
#[derive(Debug, Deserialize)]
pub struct GetPoolRequest {
    /// The rune name to get/create pool for
    pub name: String,
}

/// Response from get/create pool
#[derive(Debug, Serialize)]
pub struct GetPoolResponse {
    /// The rune name
    pub name: String,
    /// The pool address (Bitcoin address)
    pub pool_address: String,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct PoolErrorResponse {
    pub error: String,
    pub code: String,
}

/// Get or create a pool for a name
///
/// POST /v1/pool
///
/// Requires authentication. Verifies that the name belongs to the authenticated
/// user's address with sufficient confirmations, then calls the BNS canister
/// to create a pool.
pub async fn get_pool(State(state): State<AppState>, request: Request) -> Response {
    // Extract session from request extensions (set by auth middleware)
    let session = match request.extensions().get::<UserSession>() {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(PoolErrorResponse {
                    error: "Authentication required".to_string(),
                    code: "UNAUTHORIZED".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Parse request body
    let body = match axum::body::to_bytes(request.into_body(), 1024 * 16).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(PoolErrorResponse {
                    error: "Invalid request body".to_string(),
                    code: "BAD_REQUEST".to_string(),
                }),
            )
                .into_response();
        }
    };

    let req: GetPoolRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(PoolErrorResponse {
                    error: format!("Invalid JSON: {}", e),
                    code: "BAD_REQUEST".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Verify ownership: check if name belongs to user's address
    let Some(ord_url) = &state.config.ord_url else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(PoolErrorResponse {
                error: "Ord backend not configured".to_string(),
                code: "SERVICE_UNAVAILABLE".to_string(),
            }),
        )
            .into_response();
    };

    // Call Ord backend to get names for this address
    let url = format!("{}/bns/address/{}", ord_url, session.btc_address);
    let ord_response = match state.http_client.get(&url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(PoolErrorResponse {
                    error: format!("Failed to verify ownership: {}", e),
                    code: "BACKEND_ERROR".to_string(),
                }),
            )
                .into_response();
        }
    };

    if !ord_response.status().is_success() {
        return (
            StatusCode::BAD_GATEWAY,
            Json(PoolErrorResponse {
                error: format!("Ord backend returned: {}", ord_response.status()),
                code: "BACKEND_ERROR".to_string(),
            }),
        )
            .into_response();
    }

    #[derive(Deserialize)]
    struct OrdAddressResponse {
        runes: Vec<OrdRuneEntry>,
    }

    #[derive(Deserialize)]
    struct OrdRuneEntry {
        rune_name: String,
        confirmations: u64,
    }

    let ord_data: OrdAddressResponse = match ord_response.json().await {
        Ok(data) => data,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(PoolErrorResponse {
                    error: format!("Failed to parse Ord response: {}", e),
                    code: "BACKEND_ERROR".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Find the name in user's names
    let owned_name = ord_data.runes.iter().find(|r| r.rune_name == req.name);

    let Some(name_entry) = owned_name else {
        return (
            StatusCode::FORBIDDEN,
            Json(PoolErrorResponse {
                error: format!(
                    "Name '{}' is not owned by address {}",
                    req.name, session.btc_address
                ),
                code: "NAME_NOT_OWNED".to_string(),
            }),
        )
            .into_response();
    };

    // Check confirmations
    if name_entry.confirmations < FINALIZE_THRESHOLD {
        return (
            StatusCode::FORBIDDEN,
            Json(PoolErrorResponse {
                error: format!(
                    "Insufficient confirmations: {} < {} required",
                    name_entry.confirmations, FINALIZE_THRESHOLD
                ),
                code: "INSUFFICIENT_CONFIRMATIONS".to_string(),
            }),
        )
            .into_response();
    }

    // Call canister to create pool
    match state.ic_agent.create_pool(&req.name).await {
        Ok(pool_address) => Json(GetPoolResponse {
            name: req.name,
            pool_address,
        })
        .into_response(),
        Err(AppError::Canister(err)) => {
            // Check if pool already exists (not an error)
            if err.contains("already exists") || err.contains("Pool exists") {
                // Try to get existing pool info
                // For now, return the error - in the future we could query the pool
                (
                    StatusCode::CONFLICT,
                    Json(PoolErrorResponse {
                        error: err,
                        code: "POOL_ALREADY_EXISTS".to_string(),
                    }),
                )
                    .into_response()
            } else {
                (
                    StatusCode::BAD_GATEWAY,
                    Json(PoolErrorResponse {
                        error: err,
                        code: "CANISTER_ERROR".to_string(),
                    }),
                )
                    .into_response()
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(PoolErrorResponse {
                error: e.to_string(),
                code: "INTERNAL_ERROR".to_string(),
            }),
        )
            .into_response(),
    }
}
