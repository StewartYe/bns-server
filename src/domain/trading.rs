//! Trading domain model
//!
//! Represents marketplace trading entities for Rune names.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// Market listing entity (only currently listed names)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listing {
    /// The Rune name being listed (primary key)
    pub name: String,
    /// Seller's Bitcoin address
    pub seller_address: String,
    /// Price in satoshis
    pub price_sats: u64,
    /// Listing creation time
    pub listed_at: DateTime<Utc>,
    /// Last update time
    pub updated_at: DateTime<Utc>,
    /// Bitcoin transaction ID
    pub tx_id: String,
    pub inscription_utxo_sats: u64,
}

/// Trade action type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeAction {
    List,
    Delist,
    Relist,
    BuyAndRelist,
    BuyAndDelist,
}

impl From<String> for TradeAction {
    fn from(value: String) -> Self {
        match value.as_str() {
            "list" => TradeAction::List,
            "delist" => TradeAction::Delist,
            "relist" => TradeAction::Relist,
            "buy_and_relist" => TradeAction::BuyAndRelist,
            "buy_and_delist" => TradeAction::BuyAndDelist,
            _ => TradeAction::List,
        }
    }
}

impl Display for TradeAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TradeAction::List => write!(f, "list"),
            TradeAction::Delist => write!(f, "delist"),
            TradeAction::Relist => write!(f, "relist"),
            TradeAction::BuyAndRelist => write!(f, "buy_and_relist"),
            TradeAction::BuyAndDelist => write!(f, "buy_and_delist"),
        }
    }
}

/// Trade history status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeStatus {
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

impl From<String> for TradeStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "submitted" => TradeStatus::Submitted,
            "pending" => TradeStatus::Pending,
            "finalized" => TradeStatus::Finalized,
            "confirmed" => TradeStatus::Confirmed,
            "rejected" => TradeStatus::Rejected,
            _ => TradeStatus::Submitted,
        }
    }
}

impl Display for TradeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TradeStatus::Submitted => write!(f, "submitted"),
            TradeStatus::Pending => write!(f, "pending"),
            TradeStatus::Finalized => write!(f, "finalized"),
            TradeStatus::Confirmed => write!(f, "confirmed"),
            TradeStatus::Rejected => write!(f, "rejected"),
        }
    }
}

/// Trade history record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub id: String,
    pub name: String,
    pub who: String,
    pub action: TradeAction,
    pub tx_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: TradeStatus,
    pub seller_address: Option<String>,
    pub previous_price_sats: Option<u64>,
    pub price_sats: Option<u64>,
    pub inscription_utxo_sats: u64,
    pub buyer_address: Option<String>,
    pub platform_fee: Option<u64>,
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
    pub fee_sats: u64,
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
    pub fee_sats: u64,
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
    /// Bitcoin transaction ID
    pub tx_id: String,
    /// The name that was delisted
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

/// Response for get listings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetListingResponse {
    pub listing: Option<ListingInfo>,
    pub last_price_sat: u64,
    pub pool_address: Option<String>,
    pub fee_sats: Option<u64>,
}

/// Response for get user histories
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserHistoriesResponse {
    pub histories: Vec<TradeHistoryItem>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeHistoryItem {
    pub id: String,
    pub name: String,
    pub txid: String,
    pub action: String,
    pub price_sats: Option<u64>,
    pub status: String,
    pub time: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NameHistoriesResponse {
    pub histories: Vec<NameDealHistory>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NameDealHistory {
    pub seller_address: String,
    pub buyer_address: String,
    pub txid: String,
    pub price_sats: u64,
    pub time: DateTime<Utc>,
}

/// Info about a listed name
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListingInfo {
    pub name: String,
    pub seller_address: String,
    pub price_sats: u64,
    pub listed_at: DateTime<Utc>,
    pub tx_id: String,
    pub inscription_utxo_sats: u64,
}

impl From<Listing> for ListingInfo {
    fn from(listing: Listing) -> Self {
        ListingInfo {
            name: listing.name,
            seller_address: listing.seller_address,
            price_sats: listing.price_sats,
            listed_at: listing.listed_at,
            tx_id: listing.tx_id,
            inscription_utxo_sats: listing.inscription_utxo_sats,
        }
    }
}

/// Buy action type (kept for backwards compatibility in validators)
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
