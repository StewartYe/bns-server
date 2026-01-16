//! Trading domain model
//!
//! Represents marketplace trading entities for Rune names.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
}

// ============================================================================
// Pool API types
// ============================================================================

/// Request to get or create a pool for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPoolRequest {
    /// The rune name to get/create pool for
    pub name: String,
}

/// Response from get/create pool
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPoolResponse {
    /// The rune name
    pub name: String,
    /// The pool address (Bitcoin address)
    pub pool_address: String,
}

// ============================================================================
// List name API types
// ============================================================================

/// Request to list a name via orchestrator canister invoke
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListNameRequest {
    /// The intention set containing the listing intention
    pub intention_set: crate::infra::orchestrator_canister::IntentionSet,
    /// PSBT hex string (signed by user)
    pub psbt_hex: String,
    /// Initiator UTXO proof (base64 encoded blob from frontend)
    pub initiator_utxo_proof: String,
}

/// Parameters for listing a name (extracted from action_params JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListNameParams {
    pub name: String,
    pub seller_address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seller_token_address: Option<String>,
    pub price: u64,
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
}

// ============================================================================
// Get listings API types
// ============================================================================

/// Response for get listings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingsResponse {
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
}

// ============================================================================
// Future: Delist and Buy types
// ============================================================================

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
