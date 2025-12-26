//! Event domain model
//!
//! Events from Canister event queue for transaction status updates.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Event types from Canister
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// Pool created for a name
    PoolCreated,
    /// Name listed successfully
    Listed,
    /// Name delisted
    Delisted,
    /// Name purchased
    Purchased,
    /// Transaction failed
    TransactionFailed,
    /// Transaction confirmed on Bitcoin
    TransactionConfirmed,
    /// Transaction finalized (sufficient confirmations)
    TransactionFinalized,
}

/// Transaction status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    /// Transaction pending
    Pending,
    /// Transaction broadcast, waiting for confirmation
    Broadcast,
    /// Transaction confirmed (1+ confirmations)
    Confirmed,
    /// Transaction finalized (6+ confirmations)
    Finalized,
    /// Transaction failed
    Failed,
}

/// Canister event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanisterEvent {
    /// Event ID (for deduplication)
    pub event_id: String,
    /// Event type
    pub event_type: EventType,
    /// Related name
    pub name: String,
    /// Related Bitcoin transaction ID
    pub tx_id: Option<String>,
    /// Related addresses
    pub addresses: Vec<String>,
    /// Event timestamp from Canister
    pub timestamp: DateTime<Utc>,
    /// Additional data (JSON)
    pub data: Option<serde_json::Value>,
}

/// Event processing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventProcessingResult {
    pub event_id: String,
    pub processed: bool,
    pub error: Option<String>,
}

/// WebSocket event notification (sent to clients)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketEvent {
    /// Event type for client routing
    pub event_type: String,
    /// Affected names
    pub names: Vec<String>,
    /// Affected addresses
    pub addresses: Vec<String>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}
