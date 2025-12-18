use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::get,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{env, sync::Arc};

#[derive(Clone)]
struct AppState {
    client: Client,
    backend_url: String,
}

#[derive(Serialize, Deserialize)]
struct ResolveRuneResult {
    address: String,
    inscription_id: String,
}

#[derive(Serialize, Deserialize)]
struct ResolveRuneResponse {
    result: ResolveRuneResult,
}

#[derive(Serialize, Deserialize)]
struct ResolveAddressResponse {
    rune_names: Vec<String>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

async fn resolve_rune(
    State(state): State<Arc<AppState>>,
    Path(rune): Path<String>,
) -> Result<Json<ResolveRuneResponse>, (StatusCode, Json<ErrorResponse>)> {
    let url = format!("{}/resolve_rune/{}", state.backend_url, rune);

    let resp = state.client.get(&url).send().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;

    if !resp.status().is_success() {
        return Err((
            StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
            Json(ErrorResponse {
                error: format!("Backend returned status: {}", resp.status()),
            }),
        ));
    }

    let data: ResolveRuneResponse = resp.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;

    Ok(Json(data))
}

async fn resolve_address(
    State(state): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> Result<Json<ResolveAddressResponse>, (StatusCode, Json<ErrorResponse>)> {
    let url = format!("{}/resolve_address/{}", state.backend_url, address);

    let resp = state.client.get(&url).send().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;

    if !resp.status().is_success() {
        return Err((
            StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
            Json(ErrorResponse {
                error: format!("Backend returned status: {}", resp.status()),
            }),
        ));
    }

    let data: ResolveAddressResponse = resp.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse { error: e.to_string() }),
        )
    })?;

    Ok(Json(data))
}

async fn health() -> &'static str {
    "OK"
}

#[tokio::main]
async fn main() {
    // ORD backend URL via env var (GKE Internal LB IP)
    // e.g., ORD_BACKEND_URL=http://10.128.15.243
    let ord_backend_url =
        env::var("ORD_BACKEND_URL").expect("ORD_BACKEND_URL environment variable is required");

    // Cloud Run sets PORT automatically
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());

    let state = Arc::new(AppState {
        client: Client::new(),
        backend_url: ord_backend_url,
    });

    let app = Router::new()
        .route("/resolve_rune/{rune}", get(resolve_rune))
        .route("/resolve_address/{address}", get(resolve_address))
        .route("/health", get(health))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    println!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
