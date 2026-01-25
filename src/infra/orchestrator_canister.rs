// This is an experimental feature to generate Rust binding from Candid.
// You may want to manually adjust some of the types.
#![allow(dead_code, unused_imports)]
use candid::{self, CandidType, Decode, Deserialize, Encode, Principal};

#[derive(CandidType, Deserialize)]
pub struct AbandonExchangeArgs {
    pub exchange_id: String,
    pub reason: String,
}

#[derive(CandidType, Deserialize)]
pub enum Result_ {
    Ok,
    Err(String),
}

#[derive(CandidType, Deserialize)]
pub enum TxOutputType {
    #[serde(rename = "P2WPKH")]
    P2Wpkh,
    OpReturn(u64),
    #[serde(rename = "P2SH")]
    P2Sh,
    #[serde(rename = "P2TR")]
    P2Tr,
}

#[derive(CandidType, Deserialize)]
pub struct EstimateMinTxFeeArgs {
    pub input_types: Vec<TxOutputType>,
    pub pool_address: Vec<String>,
    pub output_types: Vec<TxOutputType>,
}

#[derive(CandidType, Deserialize)]
pub enum Result1 {
    Ok(u64),
    Err(String),
}

#[derive(CandidType, Deserialize)]
pub struct FromUserRecord {
    pub user_id: Principal,
}

#[derive(CandidType, Deserialize)]
pub struct FromCanisterRecord {
    pub canister_version: Option<u64>,
    pub canister_id: Principal,
}

#[derive(CandidType, Deserialize)]
pub enum ChangeOrigin {
    #[serde(rename = "from_user")]
    FromUser(FromUserRecord),
    #[serde(rename = "from_canister")]
    FromCanister(FromCanisterRecord),
}

#[derive(CandidType, Deserialize)]
pub struct CreationRecord {
    pub controllers: Vec<Principal>,
}

#[derive(CandidType, Deserialize)]
pub enum CodeDeploymentMode {
    #[serde(rename = "reinstall")]
    Reinstall,
    #[serde(rename = "upgrade")]
    Upgrade,
    #[serde(rename = "install")]
    Install,
}

#[derive(CandidType, Deserialize)]
pub struct CodeDeploymentRecord {
    pub mode: CodeDeploymentMode,
    pub module_hash: serde_bytes::ByteBuf,
}

#[derive(CandidType, Deserialize)]
pub struct LoadSnapshotRecord {
    pub canister_version: u64,
    pub taken_at_timestamp: u64,
    pub snapshot_id: serde_bytes::ByteBuf,
}

#[derive(CandidType, Deserialize)]
pub enum ChangeDetails {
    #[serde(rename = "creation")]
    Creation(CreationRecord),
    #[serde(rename = "code_deployment")]
    CodeDeployment(CodeDeploymentRecord),
    #[serde(rename = "load_snapshot")]
    LoadSnapshot(LoadSnapshotRecord),
    #[serde(rename = "controllers_change")]
    ControllersChange(CreationRecord),
    #[serde(rename = "code_uninstall")]
    CodeUninstall,
}

#[derive(CandidType, Deserialize)]
pub struct Change {
    pub timestamp_nanos: u64,
    pub canister_version: u64,
    pub origin: ChangeOrigin,
    pub details: ChangeDetails,
}

#[derive(CandidType, Deserialize)]
pub struct CanisterInfoResult {
    pub controllers: Vec<Principal>,
    pub module_hash: Option<serde_bytes::ByteBuf>,
    pub recent_changes: Vec<Change>,
    pub total_num_changes: u64,
}

#[derive(CandidType, Deserialize)]
pub enum Result2 {
    Ok(CanisterInfoResult),
    Err(String),
}

#[derive(CandidType, Deserialize)]
pub struct ExchangePool {
    pub exchange_id: String,
    pub pool_address: String,
    pub pool_key: String,
}

#[derive(CandidType, Deserialize)]
pub enum FailedInvokeFilter {
    All { offset: u64 },
    BySecondsPassed { period_seconds: u64, offset: u64 },
}

#[derive(CandidType, Deserialize)]
pub enum UtxoStatus {
    Error,
    AvailableUnconfirmed,
    InvalidTxid,
    InvalidVout,
    SpentInMempool,
    SpentOnChain,
    AvailableConfirmed,
}

#[derive(CandidType, Deserialize)]
pub struct UtxoWithStatus {
    pub status: UtxoStatus,
    pub value: Option<String>,
    pub txid: String,
    pub vout: u32,
    pub ancestors: Option<u32>,
    pub address: Option<String>,
}

#[derive(CandidType, Deserialize, Debug, Clone, serde::Serialize)]
pub struct CoinBalance {
    pub id: String,
    pub value: candid::Nat,
}

#[derive(CandidType, Deserialize, Debug, Clone, serde::Serialize)]
pub struct Utxo {
    pub coins: Vec<CoinBalance>,
    pub sats: u64,
    pub txid: String,
    pub vout: u32,
}

#[derive(CandidType, Deserialize)]
pub struct CoinsInInputView {
    pub status: Option<UtxoWithStatus>,
    pub coins: Vec<CoinBalance>,
    pub utxo: Utxo,
    pub is_signed: bool,
    pub owner_pubkey: Option<String>,
    pub owner_address: String,
}

#[derive(CandidType, Deserialize)]
pub struct CoinsInOutputView {
    pub is_op_return: bool,
    pub utxo: Utxo,
    pub owner_address: Option<String>,
}

#[derive(CandidType, Deserialize)]
pub enum Result3 {
    Ok(String),
    Err(String),
}

#[derive(CandidType, Deserialize)]
pub struct InvokeResultView {
    pub rollback_results: Vec<String>,
    pub coins_in_inputs: Vec<CoinsInInputView>,
    pub invoke_time: String,
    pub txid: String,
    pub coins_in_outputs: Vec<CoinsInOutputView>,
    pub pools_in_intention_set: Vec<(String, String)>,
    pub processing_result: Result3,
}

#[derive(CandidType, Deserialize)]
pub enum LastSentTxsFilter {
    All,
    ByTxid(String),
    ByPoolAddress(String),
    ByConfirmations(u32),
    ByExchangeId(String),
    BySecondsPassed(u64),
}

#[derive(CandidType, Deserialize)]
pub struct BlockBasic {
    pub block_hash: String,
    pub block_height: u32,
}

#[derive(CandidType, Deserialize)]
pub struct ReceivedBlockView {
    pub processing_results: Vec<String>,
    pub block_basic: BlockBasic,
    pub txids: Vec<String>,
    pub block_time: String,
    pub received_time: String,
}

#[derive(CandidType, Deserialize)]
pub enum ExchangeStatus {
    Abandoned { reason: String },
    Active,
    Halted { reason: String },
}

#[derive(CandidType, Deserialize)]
pub struct ExchangeView {
    pub status: ExchangeStatus,
    pub exchange_id: String,
    pub canister_id: Principal,
    pub utxo_proof_enabled: bool,
    pub client_canisters: Vec<Principal>,
}

#[derive(CandidType, Deserialize)]
pub struct RejectedTxView {
    pub rollback_results: Vec<String>,
    pub txid: String,
    pub received_time: String,
    pub reason: String,
}

#[derive(CandidType, Deserialize)]
pub enum Network {
    #[serde(rename = "mainnet")]
    Mainnet,
    #[serde(rename = "regtest")]
    Regtest,
    #[serde(rename = "testnet")]
    Testnet,
}

#[derive(CandidType, Deserialize)]
pub struct OrchestratorSettings {
    pub min_seconds_of_unconfirmed_tx_in_pool_for_raising_fee_rate: u32,
    pub exchange_registry_principal: Principal,
    pub max_input_count_of_psbt: u32,
    pub min_tx_confirmations: u32,
    pub tx_history_maintenance_days: u32,
    pub mempool_connector_principal: Principal,
    pub max_unconfirmed_tx_count_in_pool: u32,
    pub min_btc_amount_for_utxo: u64,
    pub rune_indexer_principal: Principal,
    pub max_intentions_per_invoke: u32,
    pub mempool_connector_public_key: String,
    pub failed_invoke_log_maintenance_days: u32,
    pub max_received_blocks_count: u32,
    pub min_unconfirmed_tx_count_in_pool_for_raising_fee_rate: u32,
    pub bitcoin_network: Network,
}

#[derive(CandidType, Deserialize)]
pub struct MempoolTxFeeRateView {
    pub low: u64,
    pub high: u64,
    pub update_time: String,
    pub medium: u64,
}

#[derive(CandidType, Deserialize)]
pub struct OrchestratorStatus {
    pub failed_invoke_logs_count: u64,
    pub last_block: Option<BlockBasic>,
    pub pending_tx_count: u64,
    pub mempool_tx_fee_rate: MempoolTxFeeRateView,
    pub processing_pool_with_nonce: Vec<(String, u64)>,
    pub invoke_paused: bool,
    pub block_history_size: u64,
    pub last_block_time: Option<(u64, String)>,
    pub tx_history_size: u64,
    pub failed_new_block_height: Option<(u32, String)>,
    pub rejected_txs_count: u64,
}

#[derive(CandidType, Deserialize)]
pub struct CoinsInInputsView {
    pub coins_in_inputs: Vec<CoinsInInputView>,
    pub txid: String,
    pub signed_address_coins: Vec<(String, Vec<CoinBalance>)>,
    pub address_coins: Vec<(String, Vec<CoinBalance>)>,
}

#[derive(CandidType, Deserialize)]
pub struct CoinsInOutputsView {
    pub burned_coins: Vec<CoinBalance>,
    pub coins_in_outputs: Vec<CoinsInOutputView>,
    pub address_coins: Vec<(String, Vec<CoinBalance>)>,
}

#[derive(CandidType, Deserialize, Debug, Clone, serde::Serialize)]
pub struct InputCoin {
    pub coin: CoinBalance,
    pub from: String,
}

#[derive(CandidType, Deserialize, Debug, Clone, serde::Serialize)]
pub struct OutputCoin {
    pub to: String,
    pub coin: CoinBalance,
}

#[derive(CandidType, Deserialize, Debug, Clone, serde::Serialize)]
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

#[derive(CandidType, Deserialize, Debug, Clone, serde::Serialize)]
pub struct IntentionSet {
    pub tx_fee_in_sats: u64,
    pub initiator_address: String,
    pub intentions: Vec<Intention>,
}

#[derive(CandidType, Deserialize)]
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

#[derive(CandidType, Deserialize)]
pub enum TxStatus {
    Confirmed(u32),
    Rejected(String),
    Pending,
}

#[derive(CandidType, Deserialize)]
pub struct ExecutionStepLogView {
    pub result: Result3,
    pub exchange_id: String,
    pub maybe_return_time: Option<String>,
    pub calling_method: String,
    pub calling_args: String,
    pub pool_address: String,
    pub calling_time: String,
}

#[derive(CandidType, Deserialize)]
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

#[derive(CandidType, Deserialize)]
pub struct TxDetailView {
    pub status: Option<TxStatus>,
    pub invoke_log: InvokeLogView,
    pub included_block: Option<BlockBasic>,
    pub sent_tx_hex: String,
}

#[derive(CandidType, Deserialize)]
pub struct OutpointWithValue {
    pub maybe_rune: Option<CoinBalance>,
    pub value: u64,
    pub script_pubkey_hex: String,
    pub outpoint: String,
}

#[derive(CandidType, Deserialize)]
pub struct InvokeArgs {
    pub client_info: Option<String>,
    pub intention_set: IntentionSet,
    pub initiator_utxo_proof: serde_bytes::ByteBuf,
    pub psbt_hex: String,
}

#[derive(CandidType, Deserialize)]
pub struct NewBlockDetectedArgs {
    pub block_hash: String,
    pub block_timestamp: u64,
    pub tx_ids: Vec<String>,
    pub block_height: u32,
}

#[derive(CandidType, Deserialize)]
pub enum Result4 {
    Ok(Vec<String>),
    Err(String),
}

#[derive(CandidType, Deserialize)]
pub struct RegisterExchangeArgs {
    pub exchange_canister: Principal,
    pub exchange_id: String,
    pub utxo_proof_enabled: bool,
    pub client_canisters: Vec<Principal>,
}

#[derive(CandidType, Deserialize)]
pub struct RejectTxArgs {
    pub txid: String,
    pub reason_code: String,
    pub reason: String,
}

#[derive(CandidType, Deserialize)]
pub struct SaveIncludedBlockForTxArgs {
    pub txid: String,
    pub timestamp: u64,
    pub block: BlockBasic,
}

#[derive(CandidType, Deserialize)]
pub struct SetTxFeePerVbyteArgs {
    pub low: u64,
    pub high: u64,
    pub medium: u64,
}
