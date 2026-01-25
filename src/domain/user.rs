//! User domain model
//!
//! Represents users authenticated via BIP-322 (Sign-In With Bitcoin).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// User entity
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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
    /// User's Bitcoin address
    pub address: String,
    /// Names currently listed for sale
    pub listed: Vec<String>,
    /// Names owned but not listed
    pub unlisted: Vec<String>,
    /// Count of listed names
    pub listed_count: usize,
    /// Count of unlisted names
    pub unlisted_count: usize,
    /// Total value of all listings in satoshis
    pub total_listed_value_sats: u64,
    /// Global rank (currently always 0)
    pub global_rank: u64,
}
