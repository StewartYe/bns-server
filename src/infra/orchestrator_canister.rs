//! Orchestrator Canister types
//!
//! Types for interacting with the Orchestrator canister.
//! Used for invoking transactions and managing pools.

#![allow(dead_code)]

use candid::{CandidType, Deserialize, Nat, Principal};
use serde::Serialize;

#[derive(CandidType, Deserialize, Debug, Clone)]
pub enum Result_ {
    Ok,
    Err(String),
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub enum TxOutputType {
    #[serde(rename = "P2WPKH")]
    P2Wpkh,
    OpReturn(u64),
    #[serde(rename = "P2SH")]
    P2Sh,
    #[serde(rename = "P2TR")]
    P2Tr,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub enum Result1 {
    Ok(u64),
    Err(String),
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub enum Result3 {
    Ok(String),
    Err(String),
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub enum UtxoStatus {
    Error,
    AvailableUnconfirmed,
    InvalidTxid,
    InvalidVout,
    SpentInMempool,
    SpentOnChain,
    AvailableConfirmed,
}

#[derive(CandidType, Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoinBalance {
    pub id: String,
    pub value: Nat,
}

#[derive(CandidType, Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Utxo {
    pub coins: Vec<CoinBalance>,
    pub sats: u64,
    pub txid: String,
    pub vout: u32,
}

#[derive(CandidType, Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputCoin {
    pub coin: CoinBalance,
    pub from: String,
}

#[derive(CandidType, Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputCoin {
    pub to: String,
    pub coin: CoinBalance,
}

#[derive(CandidType, Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(CandidType, Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentionSet {
    pub tx_fee_in_sats: u64,
    pub initiator_address: String,
    pub intentions: Vec<Intention>,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct InvokeArgs {
    pub client_info: Option<String>,
    pub intention_set: IntentionSet,
    pub initiator_utxo_proof: serde_bytes::ByteBuf,
    pub psbt_hex: String,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct UtxoWithStatus {
    pub status: UtxoStatus,
    pub value: Option<String>,
    pub txid: String,
    pub vout: u32,
    pub ancestors: Option<u32>,
    pub address: Option<String>,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct CoinsInInputView {
    pub status: Option<UtxoWithStatus>,
    pub coins: Vec<CoinBalance>,
    pub utxo: Utxo,
    pub is_signed: bool,
    pub owner_pubkey: Option<String>,
    pub owner_address: String,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct CoinsInInputsView {
    pub coins_in_inputs: Vec<CoinsInInputView>,
    pub txid: String,
    pub signed_address_coins: Vec<(String, Vec<CoinBalance>)>,
    pub address_coins: Vec<(String, Vec<CoinBalance>)>,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct CoinsInOutputView {
    pub is_op_return: bool,
    pub utxo: Utxo,
    pub owner_address: Option<String>,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct CoinsInOutputsView {
    pub burned_coins: Vec<CoinBalance>,
    pub coins_in_outputs: Vec<CoinsInOutputView>,
    pub address_coins: Vec<(String, Vec<CoinBalance>)>,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct ExecutionContextView {
    pub pool_addresses: Vec<String>,
    pub coins_in_inputs: CoinsInInputsView,
    pub txid: String,
    pub estimated_min_tx_fee: u64,
    pub actual_tx_fee: u64,
    pub coins_in_outputs: CoinsInOutputsView,
    pub tx_vsize: u64,
    pub intention_set: IntentionSet,
    pub standard_fee_rate: u64,
    pub user_coins_in_inputs: Vec<CoinBalance>,
    pub initially_signed_addresses: Vec<String>,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct InvokeResultView {
    pub rollback_results: Vec<String>,
    pub coins_in_inputs: Vec<CoinsInInputView>,
    pub invoke_time: String,
    pub coins_in_outputs: Vec<CoinsInOutputView>,
    pub pools_in_intention_set: Vec<(String, String)>,
    pub processing_result: Result3,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct BlockBasic {
    pub block_hash: String,
    pub block_height: u32,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub enum TxStatus {
    Confirmed(u32),
    Rejected(String),
    Pending,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct ExecutionStepLogView {
    pub result: Result3,
    pub exchange_id: String,
    pub maybe_return_time: Option<String>,
    pub calling_method: String,
    pub calling_args: String,
    pub pool_address: String,
    pub calling_time: String,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct InvokeLogView {
    pub rollback_results: Vec<String>,
    pub invoke_args: String,
    pub invoke_time: String,
    pub finalized_time: Option<String>,
    pub confirmed_time: Option<String>,
    pub execution_steps: Vec<ExecutionStepLogView>,
    pub processing_result: Result3,
    pub broadcasted_time: Option<String>,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct TxDetailView {
    pub status: Option<TxStatus>,
    pub invoke_log: InvokeLogView,
    pub included_block: Option<BlockBasic>,
    pub sent_tx_hex: String,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct ExchangePool {
    pub exchange_id: String,
    pub pool_address: String,
    pub pool_key: String,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub enum ExchangeStatus {
    Abandoned { reason: String },
    Active,
    Halted { reason: String },
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct ExchangeView {
    pub status: ExchangeStatus,
    pub exchange_id: String,
    pub canister_id: Principal,
    pub utxo_proof_enabled: bool,
    pub client_canisters: Vec<Principal>,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub enum Network {
    #[serde(rename = "mainnet")]
    Mainnet,
    #[serde(rename = "regtest")]
    Regtest,
    #[serde(rename = "testnet")]
    Testnet,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct MempoolTxFeeRateView {
    pub low: u64,
    pub high: u64,
    pub update_time: String,
    pub medium: u64,
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct OrchestratorStatus {
    pub last_block: Option<BlockBasic>,
    pub pending_tx_count: u64,
    pub mempool_tx_fee_rate: MempoolTxFeeRateView,
    pub processing_pool_with_nonce: Vec<(String, u64)>,
    pub invoke_paused: bool,
    pub last_block_time: Option<(u64, String)>,
    pub failed_new_block_height: Option<(u32, String)>,
}
