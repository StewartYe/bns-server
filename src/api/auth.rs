//! Authentication middleware and handlers
//!
//! Implements BIP-322 authentication with secure HttpOnly cookies (BFF pattern).
//!
//! Session tokens are delivered via:
//! - Secure, HttpOnly, SameSite=Strict cookies (recommended for web browsers)
//! - Bearer token in response (for mobile apps/CLI tools)
//!
//! Session validation accepts tokens from:
//! - Cookie (bns_session)
//! - Authorization: Bearer header

use axum::{
    Json,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use time::Duration;

use crate::constants::SESSION_COOKIE_NAME;
use crate::domain::{Bip322AuthRequest, UserSession};
use crate::error::{AppError, Result};
use crate::state::AppState;

/// Authenticate with BIP-322
///
/// POST /v1/auth/login
///
/// Returns session token in both:
/// - Response body (for API clients)
/// - Set-Cookie header (for browser clients, Secure + HttpOnly + SameSite=Strict)
pub async fn authenticate(
    State(state): State<AppState>,
    Json(request): Json<Bip322AuthRequest>,
) -> Result<Response> {
    let response = state.auth_service.authenticate(&request).await?;

    // Build secure cookie
    let cookie = Cookie::build((SESSION_COOKIE_NAME, response.session_id.clone()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(true) // Only sent over HTTPS
        .max_age(Duration::seconds(state.auth_service.session_ttl_secs()))
        .build();

    // Return both JSON response and Set-Cookie header
    let mut res = Json(response).into_response();
    res.headers_mut()
        .insert(header::SET_COOKIE, cookie.to_string().parse().unwrap());

    Ok(res)
}

/// Logout (invalidate session)
///
/// POST /v1/auth/logout
///
/// Clears session from both Redis and the cookie
pub async fn logout(State(state): State<AppState>, request: Request) -> Result<Response> {
    // Try to get session from cookie or header
    let session_token = extract_session_token(&request);

    if let Some(token) = session_token {
        state.auth_service.logout(token).await?;
    }

    // Clear the cookie
    let cookie = Cookie::build((SESSION_COOKIE_NAME, ""))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(true)
        .max_age(Duration::seconds(0)) // Expire immediately
        .build();

    let mut res = StatusCode::NO_CONTENT.into_response();
    res.headers_mut()
        .insert(header::SET_COOKIE, cookie.to_string().parse().unwrap());

    Ok(res)
}

/// Get current session info
///
/// GET /v1/auth/me
pub async fn get_me(State(state): State<AppState>, request: Request) -> Result<Json<UserSession>> {
    let session_token = extract_session_token(&request)
        .ok_or_else(|| AppError::Unauthorized("Missing session".into()))?;

    let session = state
        .auth_service
        .validate_session(session_token)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired session".into()))?;

    Ok(Json(session))
}

/// Extract session token from request (cookie or Bearer header)
fn extract_session_token(request: &Request) -> Option<&str> {
    // Try cookie first (preferred for browser security)
    if let Some(cookie_header) = request.headers().get(header::COOKIE) {
        if let Ok(cookies_str) = cookie_header.to_str() {
            for cookie in cookies_str.split(';') {
                let cookie = cookie.trim();
                if let Some(value) = cookie.strip_prefix(&format!("{}=", SESSION_COOKIE_NAME)) {
                    return Some(value);
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
}

/// Authentication middleware (optional auth - adds session to request if valid)
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> std::result::Result<Response, AppError> {
    if let Some(session_token) = extract_session_token(&request) {
        if let Ok(Some(session)) = state.auth_service.validate_session(session_token).await {
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
    let session_token = extract_session_token(&request)
        .ok_or_else(|| AppError::Unauthorized("Missing session".into()))?;

    let session = state
        .auth_service
        .validate_session(session_token)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired session".into()))?;

    request.extensions_mut().insert(session);

    Ok(next.run(request).await)
}

/// Extract authenticated user session from request extensions
pub fn extract_user_session(request: &Request) -> Option<&UserSession> {
    request.extensions().get::<UserSession>()
}
