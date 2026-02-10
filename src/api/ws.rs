//! WebSocket API for real-time updates
//!
//! Provides real-time updates via in-memory broadcast:
//! - Connect to /v1/ws/connect
//! - Send: {"type": "subscribe", "channel": "new-listings"}
//! - Receive delta/update events by channel

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, header},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

use crate::constants::SESSION_COOKIE_NAME;
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
    "market-stats",
    "user-self",
];

fn is_valid_channel(channel: &str) -> bool {
    VALID_CHANNELS.contains(&channel)
}

fn is_auth_required_channel(channel: &str) -> bool {
    channel == "user-self"
}

/// WebSocket handler - unified endpoint for all subscriptions
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_address = match extract_session_token(&headers) {
        Some(token) => match state.auth_service.validate_session(token.as_str()).await {
            Ok(Some(session)) => Some(session.btc_address),
            Ok(None) => None,
            Err(e) => {
                tracing::warn!("Failed to validate session in websocket handshake: {:?}", e);
                None
            }
        },
        None => None,
    };

    ws.on_upgrade(move |socket| handle_ws(socket, state, user_address))
}

/// Handle WebSocket connection with broadcast channel
async fn handle_ws(socket: WebSocket, state: AppState, user_address: Option<String>) {
    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(Mutex::new(sender));
    let subscriptions: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    // Active broadcast listener tasks
    let listener_tasks: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let counted_as_online = user_address.is_some();
    if counted_as_online {
        let total_online = state.online_users.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = state
            .broadcast_tx
            .send(BroadcastEvent::MarketOnlineUpdated { total_online });
    }

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

                            if is_auth_required_channel(&sub_msg.channel) && user_address.is_none()
                            {
                                let ack = serde_json::json!({
                                    "type": "error",
                                    "message": "Channel user-self requires authentication"
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
                            let user_address_clone = user_address.clone();

                            let listener_task = tokio::spawn(async move {
                                run_broadcast_listener(
                                    broadcast_rx,
                                    channel_name.clone(),
                                    sender_clone,
                                    user_address_clone,
                                )
                                .await;
                                tracing::debug!("Broadcast listener for {} ended", channel_name);
                            });

                            let mut tasks = listener_tasks.lock().await;
                            tasks.insert(sub_msg.channel.clone(), listener_task);

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

                            let mut tasks = listener_tasks.lock().await;
                            if let Some(task) = tasks.remove(&sub_msg.channel) {
                                task.abort();
                            }

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
    for task in tasks.values() {
        task.abort();
    }

    if counted_as_online {
        let total_online = decrement_online_users(&state);
        let _ = state
            .broadcast_tx
            .send(BroadcastEvent::MarketOnlineUpdated { total_online });
    }
}

/// Get current timestamp in milliseconds
fn now_ts_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Listen to broadcast channel and forward matching events to WebSocket
async fn run_broadcast_listener(
    mut broadcast_rx: tokio::sync::broadcast::Receiver<BroadcastEvent>,
    ws_channel: String,
    sender: Arc<Mutex<futures_util::stream::SplitSink<WebSocket, Message>>>,
    user_address: Option<String>,
) {
    loop {
        match broadcast_rx.recv().await {
            Ok(event) => {
                let ts = now_ts_ms();

                // Filter events based on subscribed channel
                // New flattened format: { type, channel, ts, op, key, data? }
                let ws_msg = match (&event, ws_channel.as_str()) {
                    // NewListing event goes to new-listings channel
                    (BroadcastEvent::NewListing(item), "new-listings") => Some(serde_json::json!({
                        "type": "delta",
                        "channel": "new-listings",
                        "ts": ts,
                        "op": "upsert",
                        "key": item.name,
                        "data": item
                    })),
                    // TopSale event goes to top-sales channel
                    (BroadcastEvent::TopSale(item), "top-sales") => Some(serde_json::json!({
                        "type": "delta",
                        "channel": "top-sales",
                        "ts": ts,
                        "op": "upsert",
                        "key": item.name,
                        "data": item
                    })),
                    // BestDeal event goes to best-deals channel
                    (BroadcastEvent::BestDeal(item), "best-deals") => Some(serde_json::json!({
                        "type": "delta",
                        "channel": "best-deals",
                        "ts": ts,
                        "op": "upsert",
                        "key": item.name,
                        "data": item
                    })),
                    // RecentSale event goes to recent-sales channel
                    (BroadcastEvent::RecentSale(item), "recent-sales") => Some(serde_json::json!({
                        "type": "delta",
                        "channel": "recent-sales",
                        "ts": ts,
                        "op": "upsert",
                        "key": item.name,
                        "data": item
                    })),
                    // MostTraded event goes to most-traded channel
                    (BroadcastEvent::MostTraded(item), "most-traded") => Some(serde_json::json!({
                        "type": "delta",
                        "channel": "most-traded",
                        "ts": ts,
                        "op": "upsert",
                        "key": item.name,
                        "data": item
                    })),
                    // TopEarner event goes to top-earners channel (key is address)
                    (BroadcastEvent::TopEarner(item), "top-earners") => Some(serde_json::json!({
                        "type": "delta",
                        "channel": "top-earners",
                        "ts": ts,
                        "op": "upsert",
                        "key": item.address,
                        "data": item
                    })),
                    // RemoveNewListing event goes to new-listings channel
                    (BroadcastEvent::RemoveNewListing(name), "new-listings") => {
                        Some(serde_json::json!({
                            "type": "delta",
                            "channel": "new-listings",
                            "ts": ts,
                            "op": "remove",
                            "key": name
                        }))
                    }
                    // RemoveTopSale event goes to top-sales channel
                    (BroadcastEvent::RemoveTopSale(name), "top-sales") => Some(serde_json::json!({
                        "type": "delta",
                        "channel": "top-sales",
                        "ts": ts,
                        "op": "remove",
                        "key": name
                    })),
                    // RemoveBestDeal event goes to best-deals channel
                    (BroadcastEvent::RemoveBestDeal(name), "best-deals") => {
                        Some(serde_json::json!({
                            "type": "delta",
                            "channel": "best-deals",
                            "ts": ts,
                            "op": "remove",
                            "key": name
                        }))
                    }
                    (BroadcastEvent::MarketOnlineUpdated { total_online }, "market-stats") => {
                        Some(serde_json::json!({
                            "type": "update",
                            "channel": "market-stats",
                            "event": "online",
                            "ts": ts,
                            "data": {
                                "totalOnline": total_online
                            }
                        }))
                    }
                    (
                        BroadcastEvent::MarketListingsUpdated {
                            listed_count,
                            listed_value,
                        },
                        "market-stats",
                    ) => Some(serde_json::json!({
                        "type": "update",
                        "channel": "market-stats",
                        "event": "listings",
                        "ts": ts,
                        "data": {
                            "listedCount": listed_count,
                            "listedValue": listed_value
                        }
                    })),
                    (
                        BroadcastEvent::MarketTrades24hUpdated { txs_24h, vol_24h },
                        "market-stats",
                    ) => Some(serde_json::json!({
                        "type": "update",
                        "channel": "market-stats",
                        "event": "trades24h",
                        "ts": ts,
                        "data": {
                            "txs24h": txs_24h,
                            "vol24h": vol_24h
                        }
                    })),
                    (
                        BroadcastEvent::UserInventory {
                            user_address: event_user,
                            inventory,
                        },
                        "user-self",
                    ) if Some(event_user.as_str()) == user_address.as_deref() => {
                        Some(serde_json::json!({
                            "type": "update",
                            "channel": "user-self",
                            "event": "inventory",
                            "ts": ts,
                            "data": inventory
                        }))
                    }
                    (
                        BroadcastEvent::UserActivities {
                            user_address: event_user,
                            activities,
                        },
                        "user-self",
                    ) if Some(event_user.as_str()) == user_address.as_deref() => {
                        Some(serde_json::json!({
                            "type": "update",
                            "channel": "user-self",
                            "event": "activities",
                            "ts": ts,
                            "data": activities
                        }))
                    }
                    (
                        BroadcastEvent::UserStars {
                            user_address: event_user,
                            op,
                            star,
                        },
                        "user-self",
                    ) if Some(event_user.as_str()) == user_address.as_deref() => {
                        if op == "remove" {
                            Some(serde_json::json!({
                                "type": "delta",
                                "channel": "user-self",
                                "event": "stars",
                                "ts": ts,
                                "op": "remove",
                                "key": star.target
                            }))
                        } else {
                            Some(serde_json::json!({
                                "type": "delta",
                                "channel": "user-self",
                                "event": "stars",
                                "ts": ts,
                                "op": "upsert",
                                "key": star.target,
                                "data": star
                            }))
                        }
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

fn extract_session_token(headers: &HeaderMap) -> Option<String> {
    if let Some(cookie_header) = headers.get(header::COOKIE) {
        if let Ok(cookies_str) = cookie_header.to_str() {
            for cookie in cookies_str.split(';') {
                let cookie = cookie.trim();
                if let Some(value) = cookie.strip_prefix(&format!("{}=", SESSION_COOKIE_NAME)) {
                    return Some(value.to_string());
                }
            }
        }
    }

    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|v| v.to_string())
}

fn decrement_online_users(state: &AppState) -> u64 {
    let mut current = state.online_users.load(Ordering::SeqCst);
    loop {
        let next = current.saturating_sub(1);
        match state
            .online_users
            .compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => return next,
            Err(actual) => current = actual,
        }
    }
}
