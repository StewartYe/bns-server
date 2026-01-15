//! Blockchain client for BNS Server
//!
//! Interacts with:
//! - Ord indexer: Name resolution (forward/reverse)
//! - Bitcoin fullnode: Transaction broadcast

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::domain::{AddressResolution, NameResolution};
use crate::error::{AppError, Result};

/// Blockchain client abstraction
#[async_trait]
pub trait BlockchainClient: Send + Sync {
    // Ord indexer operations

    /// Forward resolution: name -> address
    async fn resolve_name(&self, name: &str) -> Result<Option<NameResolution>>;

    /// Reverse resolution: address -> names
    async fn resolve_address(&self, address: &str) -> Result<AddressResolution>;

    /// Get inscription details
    async fn get_inscription(&self, inscription_id: &str) -> Result<Option<InscriptionInfo>>;

    // Bitcoin fullnode operations

    /// Broadcast a signed transaction (hex format)
    async fn broadcast_transaction(&self, tx_hex: &str) -> Result<String>;

    /// Broadcast a signed PSBT (base64 format)
    async fn broadcast_psbt(&self, psbt_base64: &str) -> Result<String>;

    /// Get current fee estimates
    async fn get_fee_estimates(&self) -> Result<FeeEstimates>;

    /// Get transaction confirmations
    async fn get_transaction_confirmations(&self, tx_id: &str) -> Result<Option<u32>>;
}

/// Inscription information from Ord
#[derive(Debug, Clone)]
pub struct InscriptionInfo {
    pub inscription_id: String,
    pub owner_address: String,
    pub content_type: Option<String>,
}

/// Fee estimates in sat/vB
#[derive(Debug, Clone)]
pub struct FeeEstimates {
    /// Fast confirmation (1-2 blocks)
    pub fast: u64,
    /// Medium confirmation (3-6 blocks)
    pub medium: u64,
    /// Slow confirmation (6+ blocks)
    pub slow: u64,
}

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

/// Transaction info from getrawtransaction
#[derive(Debug, Deserialize)]
struct RawTransactionInfo {
    confirmations: Option<u32>,
}

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

        tracing::debug!(
            "Bitcoin RPC {} response: {}",
            method,
            &response_text[..response_text.len().min(500)]
        );

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
    async fn resolve_name(&self, _name: &str) -> Result<Option<NameResolution>> {
        todo!("Implement resolve_name")
    }

    async fn resolve_address(&self, _address: &str) -> Result<AddressResolution> {
        todo!("Implement resolve_address")
    }

    async fn get_inscription(&self, _inscription_id: &str) -> Result<Option<InscriptionInfo>> {
        todo!("Implement get_inscription")
    }

    async fn broadcast_transaction(&self, tx_hex: &str) -> Result<String> {
        // sendrawtransaction returns the txid
        let tx_id: String = self
            .bitcoin_rpc("sendrawtransaction", vec![serde_json::json!(tx_hex)])
            .await?;

        tracing::info!("Broadcast transaction: {}", tx_id);
        Ok(tx_id)
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

    async fn get_fee_estimates(&self) -> Result<FeeEstimates> {
        // Use estimatesmartfee for different targets
        #[derive(Deserialize)]
        struct EstimateResult {
            feerate: Option<f64>,
        }

        let fast: EstimateResult = self
            .bitcoin_rpc("estimatesmartfee", vec![serde_json::json!(1)])
            .await
            .unwrap_or(EstimateResult { feerate: None });

        let medium: EstimateResult = self
            .bitcoin_rpc("estimatesmartfee", vec![serde_json::json!(6)])
            .await
            .unwrap_or(EstimateResult { feerate: None });

        let slow: EstimateResult = self
            .bitcoin_rpc("estimatesmartfee", vec![serde_json::json!(12)])
            .await
            .unwrap_or(EstimateResult { feerate: None });

        // Convert BTC/kB to sat/vB
        fn btc_kb_to_sat_vb(rate: Option<f64>) -> u64 {
            rate.map(|r| (r * 100_000.0) as u64).unwrap_or(1)
        }

        Ok(FeeEstimates {
            fast: btc_kb_to_sat_vb(fast.feerate),
            medium: btc_kb_to_sat_vb(medium.feerate),
            slow: btc_kb_to_sat_vb(slow.feerate),
        })
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
                Ok(None)
            }
        }
    }
}

pub type DynBlockchainClient = Arc<dyn BlockchainClient>;
