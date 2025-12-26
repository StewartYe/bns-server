//! Name domain model
//!
//! Represents a Rune name entity with inscription_id, owner address, and metadata.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Rune name entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Name {
    /// The Rune name (e.g., "SATOSHI•NAKAMOTO")
    pub name: String,
    /// Inscription ID on Bitcoin
    pub inscription_id: String,
    /// Current owner's Bitcoin address
    pub owner_address: String,
    /// Optional logo URL
    pub logo: Option<String>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

/// Name resolution result (forward: name -> address)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameResolution {
    pub name: String,
    pub address: String,
    pub inscription_id: String,
}

/// Reverse resolution result (address -> names)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressResolution {
    pub address: String,
    pub names: Vec<String>,
}

/// Name detail with extended information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameDetail {
    pub name: String,
    pub inscription_id: String,
    pub owner_address: String,
    pub logo: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Current listing info if listed
    pub listing: Option<NameListingInfo>,
}

/// Brief listing info embedded in name detail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameListingInfo {
    pub price_sats: u64,
    pub listed_at: DateTime<Utc>,
}

/// Search result for name queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameSearchResult {
    pub names: Vec<NameSummary>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
}

/// Summary info for search results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameSummary {
    pub name: String,
    pub owner_address: String,
    pub logo: Option<String>,
    pub is_listed: bool,
    pub price_sats: Option<u64>,
}
