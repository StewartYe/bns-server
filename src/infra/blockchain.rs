//! Blockchain client for BNS Server
//!
//! Interacts with:
//! - Ord indexer: BNS name resolution and output queries
//! - Bitcoin fullnode: Reserved for future transaction operations

use async_trait::async_trait;
use serde::Deserialize;
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

/// Internal response wrapper for /bns/rune/{rune}
#[derive(Debug, Deserialize)]
struct OrdBnsRuneResponse {
    pub result: Option<OrdBnsRuneResult>,
}

/// Result from /bns/address/{address}
#[derive(Debug, Clone, Deserialize)]
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
// Blockchain client trait
// ============================================================================

/// Blockchain client abstraction for Ord backend requests
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
}

// ============================================================================
// Implementation
// ============================================================================

/// Blockchain client implementation
pub struct BlockchainClientImpl {
    ord_url: String,
    #[allow(dead_code)]
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
}

pub type DynBlockchainClient = Arc<dyn BlockchainClient>;
