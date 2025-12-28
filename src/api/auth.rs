//! Authentication middleware and handlers
//!
//! Implements BIP-322 authentication.

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};

use crate::domain::{AuthResponse, Bip322AuthRequest, UserSession};
use crate::error::{AppError, Result};
use crate::state::AppState;

/// Authenticate with BIP-322
///
/// POST /v1/auth/login
pub async fn authenticate(
    State(state): State<AppState>,
    Json(request): Json<Bip322AuthRequest>,
) -> Result<Json<AuthResponse>> {
    let response = state.auth_service.authenticate(&request).await?;
    Ok(Json(response))
}

/// Logout (invalidate session)
///
/// POST /v1/auth/logout
pub async fn logout(State(state): State<AppState>, request: Request) -> Result<StatusCode> {
    let session_id = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Unauthorized("Missing authorization header".into()))?;

    state.auth_service.logout(session_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Get current session info
///
/// GET /v1/auth/me
pub async fn get_me(State(state): State<AppState>, request: Request) -> Result<Json<UserSession>> {
    let session_id = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Unauthorized("Missing authorization header".into()))?;

    let session = state
        .auth_service
        .validate_session(session_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired session".into()))?;

    Ok(Json(session))
}

/// Authentication middleware (optional auth - adds session to request if valid)
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> std::result::Result<Response, AppError> {
    let session_id = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    if let Some(session_id) = session_id {
        if let Ok(Some(session)) = state.auth_service.validate_session(session_id).await {
            request.extensions_mut().insert(session);
        }
    }

    Ok(next.run(request).await)
}

/// Require authentication middleware (fails if not authenticated)
pub async fn require_auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> std::result::Result<Response, AppError> {
    let session_id = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Unauthorized("Missing authorization header".into()))?;

    let session = state
        .auth_service
        .validate_session(session_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired session".into()))?;

    request.extensions_mut().insert(session);

    Ok(next.run(request).await)
}

/// Extract authenticated user session from request extensions
pub fn extract_user_session(request: &Request) -> Option<&UserSession> {
    request.extensions().get::<UserSession>()
}
