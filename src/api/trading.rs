//! Trading API handlers
//!
//! Endpoints for marketplace trading:
//! - POST /v1/trading/pool - Get pool address for listing a name
//! - POST /v1/trading/list - List a name for sale
//! - GET /v1/trading/listings - Get all listed names

use axum::{
    Json,
    extract::{Query, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::domain::{
    GetPoolRequest, ListNameRequest, ListNameResponse, ListingsResponse, UserSession,
};
use crate::error::{AppError, Result};
use crate::state::AppState;

// ============================================================================
// Error response
// ============================================================================

/// Error response for trading endpoints
#[derive(Debug, Serialize)]
pub struct TradingErrorResponse {
    pub error: String,
    pub code: String,
}

// ============================================================================
// Get Pool Address
// ============================================================================

/// Get or create a pool for listing a name
///
/// POST /v1/trading/pool
///
/// Requires authentication. Verifies that the name belongs to the authenticated
/// user's address with sufficient confirmations, then calls the BNS canister
/// to create a pool.
///
/// This is the first step before listing a name. The returned pool_address
/// is where the inscription will be transferred to.
pub async fn get_pool(State(state): State<AppState>, request: Request) -> Response {
    // Extract session from request extensions (set by auth middleware)
    let session = match request.extensions().get::<UserSession>() {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(TradingErrorResponse {
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
                Json(TradingErrorResponse {
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
                Json(TradingErrorResponse {
                    error: format!("Invalid JSON: {}", e),
                    code: "BAD_REQUEST".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Call trading service
    match state
        .trading_service
        .get_pool(&req, &session.btc_address)
        .await
    {
        Ok(response) => Json(response).into_response(),
        Err(AppError::Forbidden(msg)) => (
            StatusCode::FORBIDDEN,
            Json(TradingErrorResponse {
                error: msg,
                code: "FORBIDDEN".to_string(),
            }),
        )
            .into_response(),
        Err(AppError::Canister(err)) => {
            // Check if pool already exists (not an error)
            if err.contains("already exists") || err.contains("Pool exists") {
                (
                    StatusCode::CONFLICT,
                    Json(TradingErrorResponse {
                        error: err,
                        code: "POOL_ALREADY_EXISTS".to_string(),
                    }),
                )
                    .into_response()
            } else {
                (
                    StatusCode::BAD_GATEWAY,
                    Json(TradingErrorResponse {
                        error: err,
                        code: "CANISTER_ERROR".to_string(),
                    }),
                )
                    .into_response()
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(TradingErrorResponse {
                error: e.to_string(),
                code: "INTERNAL_ERROR".to_string(),
            }),
        )
            .into_response(),
    }
}

// ============================================================================
// List Name
// ============================================================================

/// List a name for sale
///
/// POST /v1/trading/list
///
/// Broadcasts the signed PSBT to the orchestrator canister and stores
/// the listing for tracking.
pub async fn list_name(
    State(state): State<AppState>,
    Json(request): Json<ListNameRequest>,
) -> Result<Json<ListNameResponse>> {
    let response = state.trading_service.list_name(&request).await?;
    Ok(Json(response))
}

// ============================================================================
// Get Listings
// ============================================================================

/// Query parameters for get_listings
#[derive(Debug, Deserialize)]
pub struct ListingsQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Get all listed names with pagination
///
/// GET /v1/trading/listings
pub async fn get_listings(
    State(state): State<AppState>,
    Query(query): Query<ListingsQuery>,
) -> Result<Json<ListingsResponse>> {
    let response = state
        .trading_service
        .get_listings(query.limit, query.offset)
        .await?;
    Ok(Json(response))
}
