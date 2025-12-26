//! ShoutOut domain model
//!
//! Promotional/announcement messages for names.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// ShoutOut entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShoutOut {
    /// Unique ID
    pub id: String,
    /// The name being promoted
    pub name: String,
    /// Promoter's address
    pub promoter_address: String,
    /// Message content
    pub message: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Expiration timestamp
    pub expires_at: DateTime<Utc>,
    /// Whether still active
    pub is_active: bool,
}

/// Request to create a shoutout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateShoutOutRequest {
    pub name: String,
    pub message: String,
    /// Duration in seconds
    pub duration_secs: u64,
}

/// Response for shoutout list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShoutOutList {
    pub shoutouts: Vec<ShoutOut>,
}
