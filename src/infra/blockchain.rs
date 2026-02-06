//! Blockchain client for BNS Server
//!
//! Interacts with:
//! - Ord indexer: BNS name resolution and output queries
//! - Bitcoin fullnode: Transaction info queries

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::{AppError, Result};

// ============================================================================
// Ord backend response types
// ============================================================================

/// Result from /bns/rune/{rune}
#[derive(Debug, Clone, Deserialize)]
pub struct OrdBnsRuneResult {
    pub address: String,
    pub inscription_id: String,
    pub rune_id: String,
    #[serde(default)]
    pub etching: Option<String>,
    pub inscription_number: u64,
    pub confirmations: u64,
}

/// Transaction info from getrawtransaction
#[derive(Debug, Deserialize)]
struct RawTransactionInfo {
    confirmations: Option<u32>,
}

/// Internal response wrapper for /bns/rune/{rune}
#[derive(Debug, Deserialize)]
struct OrdBnsRuneResponse {
    pub result: Option<OrdBnsRuneResult>,
}

/// Result from /bns/address/{address}
#[derive(Debug, Default, Clone, Deserialize)]
pub struct OrdBnsAddressResult {
    pub runes: Vec<OrdBnsRuneEntry>,
}

/// Entry in /bns/address response
#[derive(Debug, Clone, Deserialize)]
pub struct OrdBnsRuneEntry {
    pub rune_id: String,
    pub rune_name: String,
    pub confirmations: u64,
}

/// Result from /output/{outpoint}
#[derive(Debug, Clone, Deserialize)]
pub struct OrdOutputResult {
    pub address: Option<String>,
    pub confirmations: u32,
    pub indexed: bool,
    pub inscriptions: Option<Vec<String>>,
    pub outpoint: String,
    pub spent: bool,
    pub value: u64,
}

// ============================================================================
// Bitcoin RPC types
// ============================================================================

/// Bitcoin RPC JSON-RPC request
#[derive(Debug, Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'static str,
    id: &'static str,
    method: &'a str,
    params: Vec<serde_json::Value>,
}

/// Bitcoin RPC JSON-RPC response
#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    result: Option<T>,
    error: Option<RpcError>,
}

/// Bitcoin RPC error
#[derive(Debug, Deserialize)]
struct RpcError {
    code: i32,
    message: String,
}

/// Transaction info from getrawtransaction (verbose=true)
#[derive(Debug, Clone, Deserialize)]
pub struct BitcoinTxInfo {
    pub txid: String,
    #[serde(default)]
    pub confirmations: Option<u32>,
    /// Virtual size in vbytes
    #[serde(default)]
    pub vsize: Option<u64>,
    /// Transaction fee in BTC (only available if wallet has inputs)
    #[serde(default)]
    pub fee: Option<f64>,
    pub vin: Vec<TxInput>,
    pub vout: Vec<TxOutput>,
}

/// Transaction input
#[derive(Debug, Clone, Deserialize)]
pub struct TxInput {
    pub txid: Option<String>,
    pub vout: Option<u32>,
    /// Previous output value (from Bitcoin Core 25.0+, needs previous tx lookup otherwise)
    #[serde(default)]
    pub prevout: Option<PrevOut>,
}

/// Previous output info (Bitcoin Core 25.0+)
#[derive(Debug, Clone, Deserialize)]
pub struct PrevOut {
    pub value: f64,
}

/// Transaction output
#[derive(Debug, Clone, Deserialize)]
pub struct TxOutput {
    pub value: f64,
    pub n: u32,
}

// ============================================================================
// Blockchain client trait
// ============================================================================

/// Blockchain client abstraction for Ord backend and Bitcoin fullnode requests
#[async_trait]
pub trait BlockchainClient: Send + Sync {
    /// Get BNS rune info by name (forward resolution)
    /// Calls: GET /bns/rune/{rune}
    async fn ord_bns_rune(&self, rune: &str) -> Result<Option<OrdBnsRuneResult>>;

    /// Get all BNS runes owned by an address (reverse resolution)
    /// Calls: GET /bns/address/{address}
    async fn ord_bns_address(&self, address: &str) -> Result<OrdBnsAddressResult>;

    /// Get output info including inscriptions
    /// Calls: GET /output/{outpoint}
    async fn ord_output(&self, outpoint: &str) -> Result<Option<OrdOutputResult>>;

    /// Get transaction info from Bitcoin fullnode
    /// Calls: getrawtransaction RPC
    async fn bitcoind_tx(&self, txid: &str) -> Result<Option<BitcoinTxInfo>>;

    /// Calculate transaction fee in satoshis
    /// Returns None if fee cannot be determined
    async fn get_tx_fee_sats(&self, txid: &str) -> Result<Option<u64>>;

    // Bitcoin fullnode operations

    /// Broadcast a signed transaction (hex format)
    async fn broadcast_transaction(&self, tx_hex: &str) -> Result<String>;

    /// Broadcast a signed PSBT (base64 format)
    async fn broadcast_psbt(&self, psbt_base64: &str) -> Result<String>;

    /// Get transaction confirmations
    async fn get_transaction_confirmations(&self, tx_id: &str) -> Result<Option<u32>>;
}

// ============================================================================
// Implementation
// ============================================================================

/// Blockchain client implementation
pub struct BlockchainClientImpl {
    ord_url: String,
    bitcoin_rpc_url: String,
    client: reqwest::Client,
}

impl BlockchainClientImpl {
    pub fn new(ord_url: &str, bitcoin_rpc_url: &str) -> Self {
        Self {
            ord_url: ord_url.to_string(),
            bitcoin_rpc_url: bitcoin_rpc_url.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Make a Bitcoin RPC call
    async fn bitcoin_rpc<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<T> {
        let request = RpcRequest {
            jsonrpc: "1.0",
            id: "bns-server",
            method,
            params,
        };

        let response = self
            .client
            .post(&self.bitcoin_rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Bitcoin RPC request failed: {}", e)))?;

        let response_text = response
            .text()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read RPC response: {}", e)))?;

        let rpc_response: RpcResponse<T> = serde_json::from_str(&response_text).map_err(|e| {
            tracing::error!(
                "Failed to parse RPC response for {}: {}. Response: {}",
                method,
                e,
                &response_text[..response_text.len().min(500)]
            );
            AppError::Internal(format!("Failed to parse RPC response: {}", e))
        })?;

        if let Some(error) = rpc_response.error {
            return Err(AppError::Internal(format!(
                "Bitcoin RPC error {}: {}",
                error.code, error.message
            )));
        }

        rpc_response
            .result
            .ok_or_else(|| AppError::Internal("Empty RPC result".into()))
    }
}

#[async_trait]
impl BlockchainClient for BlockchainClientImpl {
    async fn ord_bns_rune(&self, rune: &str) -> Result<Option<OrdBnsRuneResult>> {
        let url = format!("{}/bns/rune/{}", self.ord_url, rune);

        let resp = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to call Ord backend: {}", e)))?;

        let status = resp.status();
        if status.as_u16() == 404 {
            return Ok(None);
        }

        if !status.is_success() {
            return Err(AppError::Internal(format!(
                "Ord backend returned status: {}",
                status
            )));
        }

        let data: OrdBnsRuneResponse = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse Ord response: {}", e)))?;

        Ok(data.result)
    }

    async fn ord_bns_address(&self, address: &str) -> Result<OrdBnsAddressResult> {
        let url = format!("{}/bns/address/{}", self.ord_url, address);

        let resp = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to call Ord backend: {}", e)))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(AppError::Internal(format!(
                "Ord backend returned status: {}",
                status
            )));
        }

        let data: OrdBnsAddressResult = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse Ord response: {}", e)))?;

        Ok(data)
    }

    async fn ord_output(&self, outpoint: &str) -> Result<Option<OrdOutputResult>> {
        let url = format!("{}/output/{}", self.ord_url, outpoint);

        let resp = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to call Ord backend: {}", e)))?;

        let status = resp.status();
        if status.as_u16() == 404 {
            return Ok(None);
        }

        if !status.is_success() {
            return Err(AppError::Internal(format!(
                "Ord backend returned status: {}",
                status
            )));
        }

        let data: OrdOutputResult = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse Ord response: {}", e)))?;

        Ok(Some(data))
    }

    async fn bitcoind_tx(&self, txid: &str) -> Result<Option<BitcoinTxInfo>> {
        // getrawtransaction with verbose=true returns decoded transaction
        let result: std::result::Result<BitcoinTxInfo, _> = self
            .bitcoin_rpc(
                "getrawtransaction",
                vec![serde_json::json!(txid), serde_json::json!(true)],
            )
            .await;

        match result {
            Ok(tx_info) => Ok(Some(tx_info)),
            Err(e) => {
                // Check if it's a "not found" error (code -5)
                let err_str = e.to_string();
                if err_str.contains("-5") || err_str.contains("No such mempool or blockchain") {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn get_tx_fee_sats(&self, txid: &str) -> Result<Option<u64>> {
        let tx_info = match self.bitcoind_tx(txid).await? {
            Some(info) => info,
            None => return Ok(None),
        };

        // Method 1: If fee is directly available (wallet transaction)
        if let Some(fee_btc) = tx_info.fee {
            let fee_sats = (fee_btc.abs() * 100_000_000.0).round() as u64;
            return Ok(Some(fee_sats));
        }

        // Method 2: Calculate from prevout (Bitcoin Core 25.0+)
        let mut input_total: Option<u64> = Some(0);
        for vin in &tx_info.vin {
            if let Some(prevout) = &vin.prevout {
                let sats = (prevout.value * 100_000_000.0).round() as u64;
                input_total = input_total.map(|t| t + sats);
            } else if vin.txid.is_some() {
                // Has input but no prevout info - need to look up previous tx
                input_total = None;
                break;
            }
            // coinbase inputs (no txid) don't contribute to fee calculation
        }

        // If we couldn't get input total from prevout, try fetching previous txs
        if input_total.is_none() {
            let mut total = 0u64;
            for vin in &tx_info.vin {
                if let (Some(prev_txid), Some(vout_idx)) = (&vin.txid, vin.vout) {
                    if let Some(prev_tx) = self.bitcoind_tx(prev_txid).await? {
                        if let Some(vout) = prev_tx.vout.iter().find(|v| v.n == vout_idx) {
                            total += (vout.value * 100_000_000.0).round() as u64;
                        } else {
                            return Ok(None); // Output not found
                        }
                    } else {
                        return Ok(None); // Previous tx not found
                    }
                }
                // coinbase inputs don't have txid
            }
            input_total = Some(total);
        }

        let output_total: u64 = tx_info
            .vout
            .iter()
            .map(|v| (v.value * 100_000_000.0).round() as u64)
            .sum();

        if let Some(inputs) = input_total {
            if inputs >= output_total {
                Ok(Some(inputs - output_total))
            } else {
                // This shouldn't happen for valid transactions
                tracing::warn!(
                    "Transaction {} has outputs > inputs: {} > {}",
                    txid,
                    output_total,
                    inputs
                );
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    async fn broadcast_transaction(&self, tx_hex: &str) -> Result<String> {
        // sendrawtransaction returns the txid
        let tx_id: String = self
            .bitcoin_rpc("sendrawtransaction", vec![serde_json::json!(tx_hex)])
            .await?;

        tracing::info!("Broadcast transaction: {}", tx_id);
        Ok(tx_id)
    }

    async fn get_transaction_confirmations(&self, tx_id: &str) -> Result<Option<u32>> {
        // Use getrawtransaction with verbose=true
        let result: std::result::Result<RawTransactionInfo, _> = self
            .bitcoin_rpc(
                "getrawtransaction",
                vec![serde_json::json!(tx_id), serde_json::json!(true)],
            )
            .await;

        match result {
            Ok(info) => Ok(info.confirmations),
            Err(e) => {
                // Transaction might not be found (not in mempool or blockchain)
                tracing::debug!("Failed to get transaction {}: {:?}", tx_id, e);
                Ok(None::<u32>)
            }
        }
    }

    async fn broadcast_psbt(&self, psbt_base64: &str) -> Result<String> {
        tracing::info!(
            "Broadcasting PSBT, base64 length: {}, first 50 chars: {}",
            psbt_base64.len(),
            &psbt_base64[..psbt_base64.len().min(50)]
        );

        // First finalize the PSBT
        #[derive(Deserialize)]
        struct FinalizePsbtResult {
            hex: Option<String>,
            complete: bool,
        }

        let finalize_result: FinalizePsbtResult = self
            .bitcoin_rpc("finalizepsbt", vec![serde_json::json!(psbt_base64)])
            .await?;

        tracing::info!(
            "finalizepsbt result: complete={}, hex_len={}",
            finalize_result.complete,
            finalize_result.hex.as_ref().map(|h| h.len()).unwrap_or(0)
        );

        if !finalize_result.complete {
            return Err(AppError::BadRequest("PSBT is not fully signed".into()));
        }

        let tx_hex = finalize_result
            .hex
            .ok_or_else(|| AppError::Internal("finalizepsbt did not return hex".into()))?;

        // Now broadcast the finalized transaction
        self.broadcast_transaction(&tx_hex).await
    }
}

pub type DynBlockchainClient = Arc<dyn BlockchainClient>;
