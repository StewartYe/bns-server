//! Trading domain model
//!
//! Represents marketplace trading entities for Rune names.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// Listing status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListingStatus {
    /// Currently listed and available for purchase
    Listed,
    /// Was bought and immediately re-listed (historical)
    BoughtAndRelisted,
    /// Was bought and taken off market (historical)
    BoughtAndDelisted,
    /// Price was changed by seller (historical)
    Relisted,
    /// Was removed from sale by owner (historical)
    Delisted,
}

impl From<String> for ListingStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "listed" => ListingStatus::Listed,
            "bought_and_relisted" => ListingStatus::BoughtAndRelisted,
            "bought_and_delisted" => ListingStatus::BoughtAndDelisted,
            "relisted" => ListingStatus::Relisted,
            "delisted" => ListingStatus::Delisted,
            _ => ListingStatus::Listed,
        }
    }
}

impl Display for ListingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListingStatus::Listed => write!(f, "listed"),
            ListingStatus::BoughtAndRelisted => write!(f, "bought_and_relisted"),
            ListingStatus::BoughtAndDelisted => write!(f, "bought_and_delisted"),
            ListingStatus::Relisted => write!(f, "relisted"),
            ListingStatus::Delisted => write!(f, "delisted"),
        }
    }
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
    /// Price in satoshis
    pub price_sats: u64,
    /// Current status
    pub status: ListingStatus,
    /// Listing creation time
    pub listed_at: DateTime<Utc>,
    /// Last update time
    pub updated_at: DateTime<Utc>,
    /// Previous price (for discount calculation)
    pub previous_price_sats: u64,
    /// Bitcoin transaction ID
    pub tx_id: Option<String>,
    /// Buyer's Bitcoin address (for bought_and_relisted, bought_and_delisted)
    pub buyer_address: Option<String>,
    /// New price in satoshis (for bought_and_relisted, relisted)
    pub new_price_sats: Option<u64>,
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
pub struct ListRequest {
    /// The intention set containing the listing intention
    pub intention_set: crate::infra::orchestrator_canister::IntentionSet,
    /// PSBT hex string (signed by user)
    pub psbt_hex: String,
    /// Initiator UTXO proof
    pub initiator_utxo_proof: Vec<u8>,
}

pub type BuyAndRelistRequest = ListRequest;

pub type BuyAndDelistRequest = ListRequest;

pub type DelistRequest = ListRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuyAndRelistParams {
    pub name: String,
    pub payment_sats: u64,
    pub buyer_address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buyer_token_address: Option<String>,
    pub new_price: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelistRequest {
    pub name: String,
    pub new_price: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuyAndDelistParams {
    pub name: String,
    pub payment_sats: u64,
    pub buyer_address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buyer_token_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelistParams {
    pub name: String,
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
pub struct ListResponse {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelistResponse {
    /// Unique listing ID
    pub id: String,
    /// Bitcoin transaction ID
    pub tx_id: String,
    /// The name that was listed
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelistResponse {
    pub name: String,
    pub new_price: u64,
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

/// Response for get listing price range
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingPriceRangeResponse {
    pub min: u64,
    pub max: u64,
}

/// Info about a listed name
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingInfo {
    pub id: String,
    pub name: String,
    pub seller_address: String,
    pub price_sats: u64,
    pub status: ListingStatus,
    pub listed_at: DateTime<Utc>,
    pub tx_id: Option<String>,
}

/// Buy action type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradingAction {
    /// Buy and relist at a new price
    BuyAndRelist,
    /// Buy and withdraw to own wallet
    BuyAndDelist,
    Relist,
    List,
    Delist,
}

/// Pending transaction status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingTxStatus {
    /// Initial state before canister processes
    Submitted,
    /// Canister has started processing
    Pending,
    /// Transaction finalized in mempool
    Finalized,
    /// Transaction confirmed on chain
    Confirmed,
    /// Transaction rejected
    Rejected,
}

impl From<String> for PendingTxStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "submitted" => PendingTxStatus::Submitted,
            "pending" => PendingTxStatus::Pending,
            "finalized" => PendingTxStatus::Finalized,
            "confirmed" => PendingTxStatus::Confirmed,
            "rejected" => PendingTxStatus::Rejected,
            _ => PendingTxStatus::Submitted,
        }
    }
}

impl std::fmt::Display for PendingTxStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PendingTxStatus::Submitted => write!(f, "submitted"),
            PendingTxStatus::Pending => write!(f, "pending"),
            PendingTxStatus::Finalized => write!(f, "finalized"),
            PendingTxStatus::Confirmed => write!(f, "confirmed"),
            PendingTxStatus::Rejected => write!(f, "rejected"),
        }
    }
}

/// Pending transaction action type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingTxAction {
    List,
    BuyAndRelist,
    BuyAndDelist,
    Delist,
}

impl From<String> for PendingTxAction {
    fn from(value: String) -> Self {
        match value.as_str() {
            "list" => PendingTxAction::List,
            "buy_and_relist" => PendingTxAction::BuyAndRelist,
            "buy_and_delist" => PendingTxAction::BuyAndDelist,
            "delist" => PendingTxAction::Delist,
            _ => PendingTxAction::List,
        }
    }
}

impl std::fmt::Display for PendingTxAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PendingTxAction::List => write!(f, "list"),
            PendingTxAction::BuyAndRelist => write!(f, "buy_and_relist"),
            PendingTxAction::BuyAndDelist => write!(f, "buy_and_delist"),
            PendingTxAction::Delist => write!(f, "delist"),
        }
    }
}

/// Pending transaction entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTx {
    pub tx_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub name: String,
    pub action: PendingTxAction,
    pub status: PendingTxStatus,
    pub previous_price_sats: Option<u64>,
    pub price_sats: Option<u64>,
    pub seller_address: Option<String>,
    pub buyer_address: Option<String>,
}
