//! WebSocket API for real-time updates
//!
//! Provides real-time delta updates via Redis Pub/Sub:
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

use crate::state::AppState;

/// Subscription message from client
#[derive(Debug, serde::Deserialize)]
struct SubscriptionMessage {
    #[serde(rename = "type")]
    msg_type: String,
    channel: String,
}

/// Map channel names to Redis Pub/Sub channel keys
fn channel_to_redis_key(channel: &str, state: &AppState) -> Option<String> {
    let keys = state.redis_client.keys();
    match channel {
        "new-listings" => Some(keys.channel_new_listings()),
        "recent-sales" => Some(keys.channel_recent_sales()),
        "top-earners" => Some(keys.channel_top_earners()),
        "most-traded" => Some(keys.channel_most_traded()),
        "top-sales" => Some(keys.channel_top_sales()),
        "best-deals" => Some(keys.channel_best_deals()),
        _ => None,
    }
}

/// WebSocket handler - unified endpoint for all subscriptions
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

/// Handle WebSocket connection with Redis Pub/Sub
async fn handle_ws(socket: WebSocket, state: AppState) {
    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(Mutex::new(sender));
    let subscriptions: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    // Active pub/sub tasks (one per subscribed channel)
    let pubsub_tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>> =
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

                            // Get Redis channel key
                            let Some(redis_channel) =
                                channel_to_redis_key(&sub_msg.channel, &state)
                            else {
                                let ack = serde_json::json!({
                                    "type": "error",
                                    "message": format!("Unknown channel: {}", sub_msg.channel)
                                });
                                let mut sender_guard = sender.lock().await;
                                let _ = sender_guard
                                    .send(Message::Text(ack.to_string().into()))
                                    .await;
                                continue;
                            };

                            tracing::info!("Client subscribing to: {}", sub_msg.channel);
                            subs.insert(sub_msg.channel.clone());
                            drop(subs);

                            // Start Redis Pub/Sub listener for this channel
                            let sender_clone = sender.clone();
                            let channel_name = sub_msg.channel.clone();
                            let state_clone = state.clone();

                            let pubsub_task = tokio::spawn(async move {
                                if let Err(e) = run_pubsub_listener(
                                    state_clone,
                                    redis_channel,
                                    channel_name.clone(),
                                    sender_clone,
                                )
                                .await
                                {
                                    tracing::warn!(
                                        "Pub/Sub listener for {} ended: {:?}",
                                        channel_name,
                                        e
                                    );
                                }
                            });

                            let mut tasks = pubsub_tasks.lock().await;
                            tasks.push(pubsub_task);

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

    // Clean up - abort all pub/sub listener tasks
    let tasks = pubsub_tasks.lock().await;
    for task in tasks.iter() {
        task.abort();
    }
}

/// Run a Redis Pub/Sub listener and forward messages to WebSocket
async fn run_pubsub_listener(
    state: AppState,
    redis_channel: String,
    ws_channel: String,
    sender: Arc<Mutex<futures_util::stream::SplitSink<WebSocket, Message>>>,
) -> crate::error::Result<()> {
    use futures_util::StreamExt;
    use redis::aio::PubSub;

    let mut pubsub: PubSub = state.redis_client.get_pubsub().await?;

    // Subscribe to the Redis channel
    pubsub.subscribe(&redis_channel).await?;
    tracing::info!("Subscribed to Redis channel: {}", redis_channel);

    // Get the message stream
    let mut stream = pubsub.on_message();

    // Forward messages to WebSocket
    while let Some(msg) = stream.next().await {
        let payload: String = msg.get_payload().map_err(crate::error::AppError::Redis)?;

        // Forward to WebSocket with channel info
        let ws_msg = serde_json::json!({
            "type": "delta",
            "channel": ws_channel,
            "data": serde_json::from_str::<serde_json::Value>(&payload).unwrap_or(serde_json::Value::String(payload))
        });

        let mut sender_guard = sender.lock().await;
        if sender_guard
            .send(Message::Text(ws_msg.to_string().into()))
            .await
            .is_err()
        {
            tracing::debug!("WebSocket send failed, stopping pub/sub listener");
            break;
        }
    }

    Ok(())
}
