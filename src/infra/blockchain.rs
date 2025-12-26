//! Blockchain client for BNS Server
//!
//! Interacts with:
//! - Ord indexer: Name resolution (forward/reverse)
//! - Bitcoin fullnode: Transaction broadcast

use async_trait::async_trait;
use std::sync::Arc;

use crate::domain::{AddressResolution, NameResolution};
use crate::error::Result;

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

    /// Broadcast a signed transaction
    async fn broadcast_transaction(&self, tx_hex: &str) -> Result<String>;

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

    async fn broadcast_transaction(&self, _tx_hex: &str) -> Result<String> {
        todo!("Implement broadcast_transaction")
    }

    async fn get_fee_estimates(&self) -> Result<FeeEstimates> {
        todo!("Implement get_fee_estimates")
    }

    async fn get_transaction_confirmations(&self, _tx_id: &str) -> Result<Option<u32>> {
        todo!("Implement get_transaction_confirmations")
    }
}

pub type DynBlockchainClient = Arc<dyn BlockchainClient>;
