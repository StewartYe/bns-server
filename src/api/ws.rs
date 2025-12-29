//! WebSocket API for real-time updates
//!
//! Provides real-time updates for:
//! - New listings
//! - Confirmation updates

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::time::interval;

use crate::state::AppState;

/// WebSocket endpoint for new listings updates
pub async fn ws_new_listings(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_new_listings_ws(socket, state))
}

/// Handle WebSocket connection for new listings
async fn handle_new_listings_ws(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Send initial data
    if let Ok(listings) = state.listing_service.get_new_listings(20).await {
        let msg = serde_json::json!({
            "type": "initial",
            "data": listings
        });
        if sender
            .send(Message::Text(msg.to_string().into()))
            .await
            .is_err()
        {
            return;
        }
    }

    // Spawn task to send periodic updates
    let state_clone = state.clone();
    let send_task = tokio::spawn(async move {
        let mut update_interval = interval(Duration::from_secs(5));

        loop {
            update_interval.tick().await;

            // Get current listings
            match state_clone.listing_service.get_new_listings(20).await {
                Ok(listings) => {
                    let msg = serde_json::json!({
                        "type": "update",
                        "data": listings
                    });
                    if sender
                        .send(Message::Text(msg.to_string().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to get listings for WS: {:?}", e);
                }
            }
        }
    });

    // Handle incoming messages (ping/pong, close)
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Ping(data)) => {
                // Pong is handled automatically by axum
                tracing::debug!("Received ping: {:?}", data);
            }
            Ok(Message::Close(_)) => {
                tracing::debug!("Client closed WebSocket connection");
                break;
            }
            Err(e) => {
                tracing::warn!("WebSocket error: {:?}", e);
                break;
            }
            _ => {}
        }
    }

    // Clean up
    send_task.abort();
}

/// Get new listings via HTTP (alternative to WebSocket)
pub async fn get_new_listings(State(state): State<AppState>) -> impl IntoResponse {
    match state.listing_service.get_new_listings(20).await {
        Ok(listings) => axum::Json(serde_json::json!({
            "listings": listings
        }))
        .into_response(),
        Err(e) => {
            tracing::error!("Failed to get new listings: {:?}", e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
