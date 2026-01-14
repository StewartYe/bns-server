//! BNS Canister types
//!
//! Types generated from BNS canister Candid interface.
//! Used for encoding/decoding canister call arguments and responses.

#![allow(dead_code)]

use candid::{CandidType, Deserialize, Principal};

#[derive(CandidType, Deserialize, Debug, Clone)]
pub enum BnsResult {
    Ok,
    Err(String),
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub enum BnsResultString {
    Ok(String),
    Err(String),
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct CoinBalance {
    pub id: String,
    pub value: candid::Nat,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct InputCoin {
    pub coin: CoinBalance,
    pub from: String,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct OutputCoin {
    pub to: String,
    pub coin: CoinBalance,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct Utxo {
    pub coins: Vec<CoinBalance>,
    pub sats: u64,
    pub txid: String,
    pub vout: u32,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct Intention {
    pub input_coins: Vec<InputCoin>,
    pub output_coins: Vec<OutputCoin>,
    pub action: String,
    pub exchange_id: String,
    pub pool_utxo_spent: Vec<String>,
    pub action_params: String,
    pub nonce: u64,
    pub pool_address: String,
    pub pool_utxo_received: Vec<Utxo>,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct IntentionSet {
    pub tx_fee_in_sats: u64,
    pub initiator_address: String,
    pub intentions: Vec<Intention>,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct ExecuteTxArgs {
    pub zero_confirmed_tx_queue_length: u32,
    pub txid: String,
    pub intention_set: IntentionSet,
    pub intention_index: u32,
    pub is_reapply: Option<bool>,
    pub psbt_hex: String,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub enum ReeActionStatus {
    Finalized,
    Confirmed(u32),
    Rejected(String),
    Pending,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub enum ReeActionKind {
    ListName {
        name: String,
        seller_token_address: Option<String>,
        seller_address: String,
        price: u64,
    },
    BuyAndDelistName {
        payment_sats: u64,
        name: String,
        buyer_address: String,
        buyer_token_address: Option<String>,
    },
    DelistName {
        name: String,
    },
    BuyAndRelistName {
        payment_sats: u64,
        name: String,
        buyer_address: String,
        new_price: u64,
        buyer_token_address: Option<String>,
    },
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct ActionRecord {
    pub status: ReeActionStatus,
    pub timestamp_nanos: u64,
    pub action_id: String,
    pub kind: ReeActionKind,
    pub txid: String,
    pub confirmed_height: Option<u32>,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct PagingArgs {
    pub offset: u64,
    pub limit: u64,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct BlacklistEntry {
    pub timestamp_nanos: u64,
    pub reason: String,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub enum BnsCanisterEvent {
    ReeActionCommitted {
        timestamp_nanos: u64,
        action_id: String,
        kind: ReeActionKind,
        txid: String,
    },
    NameBlacklisted {
        timestamp_nanos: u64,
        name: String,
        reason: String,
    },
    ReeActionStatusChanged {
        status: ReeActionStatus,
        timestamp_nanos: u64,
        action_id: String,
    },
    NameRelisted {
        timestamp_nanos: u64,
        name: String,
        new_price: u64,
    },
    NameUnblacklisted {
        timestamp_nanos: u64,
        name: String,
        reason: String,
    },
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct BnsListing {
    pub name: String,
    pub seller_token_address: Option<String>,
    pub seller_address: String,
    pub price: u64,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct GetPoolInfoArgs {
    pub pool_address: String,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct PoolInfo {
    pub key: String,
    pub name: String,
    pub btc_reserved: u64,
    pub key_derivation_path: Vec<Vec<u8>>,
    pub coin_reserved: Vec<CoinBalance>,
    pub attributes: String,
    pub address: String,
    pub nonce: u64,
    pub utxos: Vec<Utxo>,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct PoolBasic {
    pub name: String,
    pub address: String,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub enum BtcNetwork {
    #[serde(rename = "mainnet")]
    Mainnet,
    #[serde(rename = "regtest")]
    Regtest,
    #[serde(rename = "testnet")]
    Testnet,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct ExchangeSettings {
    pub bns_backend_principal: Principal,
    pub is_paused: bool,
    pub btc_network: BtcNetwork,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct ExchangeStatus {
    pub pool_count: u64,
    pub blacklist_count: u64,
    pub action_count: u64,
    pub event_count: u64,
    pub listing_count: u64,
    pub is_paused: bool,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct NewBlockInfo {
    pub block_hash: String,
    pub confirmed_txids: Vec<String>,
    pub block_timestamp: u64,
    pub block_height: u32,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct RollbackTxArgs {
    pub txid: String,
    pub reason_code: String,
}
