//! User domain model
//!
//! Represents users authenticated via BIP-322 (Sign-In With Bitcoin).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::TransactionStatus;

/// User entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Bitcoin address (primary identifier)
    pub btc_address: String,
    /// User's primary name (optional)
    pub primary_name: Option<String>,
    /// First seen timestamp
    pub created_at: DateTime<Utc>,
    /// Last active timestamp
    pub last_seen_at: DateTime<Utc>,
}

/// User session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    pub session_id: String,
    pub btc_address: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// BIP-322 authentication request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bip322AuthRequest {
    /// Bitcoin address
    pub address: String,
    /// Message that was signed: "Sign in to bns.zone at {timestamp} with nonce {nonce}"
    pub message: String,
    /// BIP-322 signature (base64 encoded)
    pub signature: String,
}

/// Authentication response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub session_id: String,
    pub btc_address: String,
    pub expires_at: DateTime<Utc>,
    pub is_new_user: bool,
}

/// User inventory response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInventory {
    pub btc_address: String,
    /// Names currently listed for sale
    pub listed_names: Vec<ListedNameInfo>,
    /// Names owned but not listed
    pub owned_names: Vec<OwnedNameInfo>,
    /// Total value of all listings in satoshis
    pub total_listed_value_sats: u64,
}

/// Info about a listed name
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListedNameInfo {
    pub name: String,
    pub price_sats: u64,
    pub listed_at: DateTime<Utc>,
}

/// Info about an owned (non-listed) name
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnedNameInfo {
    pub name: String,
    pub inscription_id: String,
}

/// User transaction history response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserHistory {
    pub btc_address: String,
    pub transactions: Vec<UserTransaction>,
}

/// Transaction types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    List,
    Delist,
    Buy,
    Sell,
}

/// A user's transaction record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTransaction {
    pub tx_type: TransactionType,
    pub name: String,
    pub price_sats: Option<u64>,
    pub counterparty: Option<String>,
    pub status: TransactionStatus,
    pub tx_id: Option<String>,
    pub timestamp: DateTime<Utc>,
}
