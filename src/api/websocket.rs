//! WebSocket handlers
//!
//! Real-time event notifications for market updates.

use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::domain::WebSocketEvent;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/ws", get(ws_handler))
}

/// WebSocket upgrade handler
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle WebSocket connection
async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to events
    let mut event_rx = state.event_service.subscribe();

    // Increment online user count
    let _ = state.market_service.increment_online_users().await;

    // Spawn task to forward events to client
    let send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            let msg = serde_json::to_string(&event).unwrap_or_default();
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages (ping/pong, subscriptions)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    // Handle subscription requests
                    handle_client_message(&text).await;
                }
                Message::Ping(data) => {
                    // Pong is handled automatically by axum
                }
                Message::Close(_) => {
                    break;
                }
                _ => {}
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }

    // Decrement online user count
    let _ = state.market_service.decrement_online_users().await;
}

/// Handle incoming client messages
async fn handle_client_message(text: &str) {
    // TODO: Implement subscription management
    // e.g., subscribe to specific names or addresses
    tracing::debug!("WebSocket message: {}", text);
}

/// Subscription request from client
#[derive(Debug, serde::Deserialize)]
pub struct SubscriptionRequest {
    pub action: String, // "subscribe" | "unsubscribe"
    pub names: Option<Vec<String>>,
    pub addresses: Option<Vec<String>>,
}
