//! WebSocket API for real-time updates
//!
//! Provides real-time updates via subscription model:
//! - Connect to /v1/ws
//! - Send: {"type": "subscribe", "channel": "new-listings"}
//! - Receive updates for subscribed channels

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::interval;

use crate::state::AppState;

/// Subscription message from client
#[derive(Debug, serde::Deserialize)]
struct SubscriptionMessage {
    #[serde(rename = "type")]
    msg_type: String,
    channel: String,
}

/// WebSocket handler - unified endpoint for all subscriptions
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

/// Handle WebSocket connection with subscription model
async fn handle_ws(socket: WebSocket, state: AppState) {
    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(Mutex::new(sender));
    let subscriptions: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    // Spawn task to send periodic updates for subscribed channels
    let sender_clone = sender.clone();
    let subscriptions_clone = subscriptions.clone();
    let state_clone = state.clone();
    let send_task = tokio::spawn(async move {
        let mut update_interval = interval(Duration::from_secs(5));

        loop {
            update_interval.tick().await;

            let subs = subscriptions_clone.lock().await;
            if subs.contains("new-listings") {
                drop(subs); // Release lock before async operation

                match state_clone.listing_service.get_new_listings(20).await {
                    Ok(listings) => {
                        let msg = serde_json::json!({
                            "type": "update",
                            "channel": "new-listings",
                            "data": listings
                        });
                        let mut sender = sender_clone.lock().await;
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
        }
    });

    // Handle incoming messages
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                // Parse subscription message
                if let Ok(sub_msg) = serde_json::from_str::<SubscriptionMessage>(&text) {
                    match sub_msg.msg_type.as_str() {
                        "subscribe" => {
                            tracing::info!("Client subscribed to: {}", sub_msg.channel);
                            let mut subs = subscriptions.lock().await;
                            subs.insert(sub_msg.channel.clone());
                            drop(subs);

                            // Send initial data for the subscription
                            if sub_msg.channel == "new-listings" {
                                if let Ok(listings) = state.listing_service.get_new_listings(20).await {
                                    let msg = serde_json::json!({
                                        "type": "snapshot",
                                        "channel": "new-listings",
                                        "data": listings
                                    });
                                    let mut sender = sender.lock().await;
                                    let _ = sender.send(Message::Text(msg.to_string().into())).await;
                                }
                            }

                            // Send subscription confirmation
                            let ack = serde_json::json!({
                                "type": "subscribed",
                                "channel": sub_msg.channel
                            });
                            let mut sender = sender.lock().await;
                            let _ = sender.send(Message::Text(ack.to_string().into())).await;
                        }
                        "unsubscribe" => {
                            tracing::info!("Client unsubscribed from: {}", sub_msg.channel);
                            let mut subs = subscriptions.lock().await;
                            subs.remove(&sub_msg.channel);

                            // Send unsubscribe confirmation
                            let ack = serde_json::json!({
                                "type": "unsubscribed",
                                "channel": sub_msg.channel
                            });
                            drop(subs);
                            let mut sender = sender.lock().await;
                            let _ = sender.send(Message::Text(ack.to_string().into())).await;
                        }
                        _ => {
                            tracing::debug!("Unknown message type: {}", sub_msg.msg_type);
                        }
                    }
                }
            }
            Ok(Message::Ping(data)) => {
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
    tracing::info!("GET /v1/listings/new - fetching new listings from Redis");
    match state.listing_service.get_new_listings(20).await {
        Ok(listings) => {
            tracing::info!("Found {} listings in Redis", listings.len());
            axum::Json(serde_json::json!({
                "listings": listings
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get new listings: {:?}", e);
            axum::Json(serde_json::json!({
                "error": format!("{:?}", e),
                "listings": []
            }))
            .into_response()
        }
    }
}
