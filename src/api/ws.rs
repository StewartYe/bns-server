//! WebSocket API for real-time updates
//!
//! Provides real-time delta updates via in-memory broadcast:
//! - Connect to /v1/ws/connect
//! - Send: {"type": "subscribe", "channel": "new-listings"}
//! - Receive delta updates when new listings are added
//!
//! For initial data snapshot, use GET /v1/rankings/{type}

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::state::{AppState, BroadcastEvent};

/// Subscription message from client
#[derive(Debug, serde::Deserialize)]
struct SubscriptionMessage {
    #[serde(rename = "type")]
    msg_type: String,
    channel: String,
}

/// Valid channel names for subscription
pub const VALID_CHANNELS: &[&str] = &[
    "new-listings",
    "recent-sales",
    "top-earners",
    "most-traded",
    "top-sales",
    "best-deals",
];

fn is_valid_channel(channel: &str) -> bool {
    VALID_CHANNELS.contains(&channel)
}

/// WebSocket handler - unified endpoint for all subscriptions
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

/// Handle WebSocket connection with broadcast channel
async fn handle_ws(socket: WebSocket, state: AppState) {
    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(Mutex::new(sender));
    let subscriptions: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    // Active broadcast listener tasks
    let listener_tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>> =
        Arc::new(Mutex::new(Vec::new()));

    // Handle incoming messages
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                // Parse subscription message
                if let Ok(sub_msg) = serde_json::from_str::<SubscriptionMessage>(&text) {
                    match sub_msg.msg_type.as_str() {
                        "subscribe" => {
                            let mut subs = subscriptions.lock().await;

                            // Check if already subscribed
                            if subs.contains(&sub_msg.channel) {
                                let ack = serde_json::json!({
                                    "type": "error",
                                    "message": format!("Already subscribed to {}", sub_msg.channel)
                                });
                                let mut sender_guard = sender.lock().await;
                                let _ = sender_guard
                                    .send(Message::Text(ack.to_string().into()))
                                    .await;
                                continue;
                            }

                            // Validate channel name
                            if !is_valid_channel(&sub_msg.channel) {
                                let ack = serde_json::json!({
                                    "type": "error",
                                    "message": format!("Unknown channel: {}", sub_msg.channel)
                                });
                                let mut sender_guard = sender.lock().await;
                                let _ = sender_guard
                                    .send(Message::Text(ack.to_string().into()))
                                    .await;
                                continue;
                            }

                            tracing::info!("Client subscribing to: {}", sub_msg.channel);
                            subs.insert(sub_msg.channel.clone());
                            drop(subs);

                            // Start broadcast listener for this channel
                            let sender_clone = sender.clone();
                            let channel_name = sub_msg.channel.clone();
                            let broadcast_rx = state.broadcast_tx.subscribe();

                            let listener_task = tokio::spawn(async move {
                                run_broadcast_listener(
                                    broadcast_rx,
                                    channel_name.clone(),
                                    sender_clone,
                                )
                                .await;
                                tracing::debug!("Broadcast listener for {} ended", channel_name);
                            });

                            let mut tasks = listener_tasks.lock().await;
                            tasks.push(listener_task);

                            // Send subscription confirmation
                            let ack = serde_json::json!({
                                "type": "subscribed",
                                "channel": sub_msg.channel
                            });
                            let mut sender_guard = sender.lock().await;
                            let _ = sender_guard
                                .send(Message::Text(ack.to_string().into()))
                                .await;
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
                            let mut sender_guard = sender.lock().await;
                            let _ = sender_guard
                                .send(Message::Text(ack.to_string().into()))
                                .await;
                        }
                        _ => {
                            tracing::debug!("Unknown message type: {}", sub_msg.msg_type);
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => {
                tracing::debug!("Client closed WebSocket connection");
                break;
            }
            Err(e) => {
                tracing::debug!("WebSocket disconnected: {:?}", e);
                break;
            }
            _ => {}
        }
    }

    // Clean up - abort all listener tasks
    let tasks = listener_tasks.lock().await;
    for task in tasks.iter() {
        task.abort();
    }
}

/// Listen to broadcast channel and forward matching events to WebSocket
async fn run_broadcast_listener(
    mut broadcast_rx: tokio::sync::broadcast::Receiver<BroadcastEvent>,
    ws_channel: String,
    sender: Arc<Mutex<futures_util::stream::SplitSink<WebSocket, Message>>>,
) {
    loop {
        match broadcast_rx.recv().await {
            Ok(event) => {
                // Filter events based on subscribed channel
                let ws_msg = match (&event, ws_channel.as_str()) {
                    // NewListing event goes to new-listings channel
                    (BroadcastEvent::NewListing(item), "new-listings") => Some(serde_json::json!({
                        "type": "delta",
                        "channel": "new-listings",
                        "data": {
                            "type": "new_listing",
                            "data": item
                        }
                    })),
                    // TopSale event goes to top-sales channel
                    (BroadcastEvent::TopSale(item), "top-sales") => Some(serde_json::json!({
                        "type": "delta",
                        "channel": "top-sales",
                        "data": {
                            "type": "top_sale",
                            "data": item
                        }
                    })),
                    // BestDeal event goes to best-deals channel
                    (BroadcastEvent::BestDeal(item), "best-deals") => Some(serde_json::json!({
                        "type": "delta",
                        "channel": "best-deals",
                        "data": {
                            "type": "best_deal",
                            "data": item
                        }
                    })),
                    // RecentSale event goes to recent-sales channel
                    (BroadcastEvent::RecentSale(item), "recent-sales") => Some(serde_json::json!({
                        "type": "delta",
                        "channel": "recent-sales",
                        "data": {
                            "type": "recent_sale",
                            "data": item
                        }
                    })),
                    // MostTraded event goes to most-traded channel
                    (BroadcastEvent::MostTraded(item), "most-traded") => Some(serde_json::json!({
                        "type": "delta",
                        "channel": "most-traded",
                        "data": {
                            "type": "most_traded",
                            "data": item
                        }
                    })),
                    // TopEarner event goes to top-earners channel
                    (BroadcastEvent::TopEarner(item), "top-earners") => Some(serde_json::json!({
                        "type": "delta",
                        "channel": "top-earners",
                        "data": {
                            "type": "top_earner",
                            "data": item
                        }
                    })),
                    // RemoveNewListing event goes to new-listings channel
                    (BroadcastEvent::RemoveNewListing(name), "new-listings") => {
                        Some(serde_json::json!({
                            "type": "delta",
                            "channel": "new-listings",
                            "data": {
                                "type": "remove",
                                "name": name
                            }
                        }))
                    }
                    // RemoveTopSale event goes to top-sales channel
                    (BroadcastEvent::RemoveTopSale(name), "top-sales") => Some(serde_json::json!({
                        "type": "delta",
                        "channel": "top-sales",
                        "data": {
                            "type": "remove",
                            "name": name
                        }
                    })),
                    // RemoveBestDeal event goes to best-deals channel
                    (BroadcastEvent::RemoveBestDeal(name), "best-deals") => {
                        Some(serde_json::json!({
                            "type": "delta",
                            "channel": "best-deals",
                            "data": {
                                "type": "remove",
                                "name": name
                            }
                        }))
                    }
                    _ => None,
                };

                if let Some(msg) = ws_msg {
                    let mut sender_guard = sender.lock().await;
                    if sender_guard
                        .send(Message::Text(msg.to_string().into()))
                        .await
                        .is_err()
                    {
                        tracing::debug!("WebSocket send failed, stopping broadcast listener");
                        break;
                    }
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!("Broadcast listener lagged by {} messages", n);
                // Continue receiving, some messages were dropped
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                tracing::debug!("Broadcast channel closed");
                break;
            }
        }
    }
}
