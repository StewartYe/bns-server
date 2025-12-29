//! Listing domain model
//!
//! Represents market listings for Rune names.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Confirmation threshold for finalizing listings
pub const FINALIZE_THRESHOLD: i32 = 3;

/// Listing status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListingStatus {
    /// Listed and available for purchase
    Active,
    /// Temporarily pending (during transaction)
    Pending,
    /// Sold
    Sold,
    /// Delisted by owner
    Delisted,
    /// Cancelled due to error
    Cancelled,
}

/// Market listing entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listing {
    /// Unique listing ID
    pub id: String,
    /// The Rune name being listed
    pub name: String,
    /// Seller's Bitcoin address
    pub seller_address: String,
    /// Pool address for the listing (from Canister)
    pub pool_address: String,
    /// Price in satoshis
    pub price_sats: u64,
    /// Current status
    pub status: ListingStatus,
    /// Listing creation time
    pub listed_at: DateTime<Utc>,
    /// Last update time
    pub updated_at: DateTime<Utc>,
    /// Previous price (for discount calculation)
    pub previous_price_sats: Option<u64>,
    /// Bitcoin transaction ID
    pub tx_id: Option<String>,
    /// Number of confirmations
    pub confirmations: i32,
}

/// Request to create a new listing (deprecated, use ListNameRequest)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateListingRequest {
    pub name: String,
    pub price_sats: u64,
}

/// Request to list a name via PSBT
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListNameRequest {
    /// The name to list
    pub name: String,
    /// Price in satoshis
    pub price_sats: u64,
    /// Seller's Bitcoin address (initiator_address)
    pub seller_address: String,
    /// Signed PSBT (base64 encoded) to broadcast
    pub psbt: String,
}

/// Response from listing a name
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListNameResponse {
    /// Unique listing ID
    pub id: String,
    /// Bitcoin transaction ID
    pub tx_id: String,
    /// The name that was listed
    pub name: String,
    /// Price in satoshis
    pub price_sats: u64,
    /// Seller's Bitcoin address
    pub seller_address: String,
    /// Current confirmation count
    pub confirmations: i32,
}

/// Response for get_listed_names
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListedNamesResponse {
    pub listings: Vec<ListingInfo>,
    pub total: i64,
}

/// Info about a listed name
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingInfo {
    pub id: String,
    pub name: String,
    pub seller_address: String,
    pub pool_address: String,
    pub price_sats: u64,
    pub status: ListingStatus,
    pub listed_at: DateTime<Utc>,
    pub tx_id: Option<String>,
    pub confirmations: i32,
}

/// Request to update listing price
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateListingRequest {
    pub name: String,
    pub new_price_sats: u64,
}

/// Request to delist a name
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelistRequest {
    pub name: String,
}

/// Buy action type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuyAction {
    /// Buy and relist at a new price
    BuyAndRelist,
    /// Buy and withdraw to own wallet
    BuyAndWithdraw,
}

/// Request to buy a listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuyRequest {
    pub name: String,
    pub action: BuyAction,
    /// New price if action is BuyAndRelist
    pub relist_price_sats: Option<u64>,
}

/// Listing with computed fields for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingDisplay {
    pub listing: Listing,
    /// Price change percentage from previous listing
    pub price_change_pct: Option<f64>,
}
