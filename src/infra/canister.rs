//! Canister client for BNS Server
//!
//! Interacts with the ICP BNS Canister for:
//! - create_pool: Create a new pool for first-time listing
//! - list_name: Complete listing after broadcast
//! - delist_name: Remove listing
//! - buy_and_relist: Purchase and relist
//! - buy_and_withdraw: Purchase and withdraw
//! - poll_events: Poll event queue for updates

use async_trait::async_trait;
use std::sync::Arc;

use crate::domain::CanisterEvent;
use crate::error::Result;

/// Pool creation result
#[derive(Debug, Clone)]
pub struct CreatePoolResult {
    pub pool_address: String,
    pub name: String,
}

/// Buy result
#[derive(Debug, Clone)]
pub struct BuyResult {
    pub success: bool,
    pub tx_id: Option<String>,
    pub new_pool_address: Option<String>,
}

/// Canister client abstraction
#[async_trait]
pub trait CanisterClient: Send + Sync {
    /// Create a new pool for first-time listing
    async fn create_pool(&self, name: &str, seller_address: &str) -> Result<CreatePoolResult>;

    /// Complete listing after transaction broadcast
    async fn list_name(
        &self,
        name: &str,
        pool_address: &str,
        price_sats: u64,
        tx_id: &str,
    ) -> Result<()>;

    /// Delist a name
    async fn delist_name(&self, name: &str, pool_address: &str) -> Result<()>;

    /// Buy and relist at new price
    async fn buy_and_relist(
        &self,
        name: &str,
        buyer_address: &str,
        new_price_sats: u64,
        tx_id: &str,
    ) -> Result<BuyResult>;

    /// Buy and withdraw to buyer's wallet
    async fn buy_and_withdraw(
        &self,
        name: &str,
        buyer_address: &str,
        tx_id: &str,
    ) -> Result<BuyResult>;

    /// Poll event queue for updates
    async fn poll_events(&self, last_event_id: Option<&str>) -> Result<Vec<CanisterEvent>>;
}

/// Canister client implementation
pub struct CanisterClientImpl {
    canister_url: String,
    client: reqwest::Client,
}

impl CanisterClientImpl {
    pub fn new(canister_url: &str) -> Self {
        Self {
            canister_url: canister_url.to_string(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl CanisterClient for CanisterClientImpl {
    async fn create_pool(&self, _name: &str, _seller_address: &str) -> Result<CreatePoolResult> {
        todo!("Implement create_pool")
    }

    async fn list_name(
        &self,
        _name: &str,
        _pool_address: &str,
        _price_sats: u64,
        _tx_id: &str,
    ) -> Result<()> {
        todo!("Implement list_name")
    }

    async fn delist_name(&self, _name: &str, _pool_address: &str) -> Result<()> {
        todo!("Implement delist_name")
    }

    async fn buy_and_relist(
        &self,
        _name: &str,
        _buyer_address: &str,
        _new_price_sats: u64,
        _tx_id: &str,
    ) -> Result<BuyResult> {
        todo!("Implement buy_and_relist")
    }

    async fn buy_and_withdraw(
        &self,
        _name: &str,
        _buyer_address: &str,
        _tx_id: &str,
    ) -> Result<BuyResult> {
        todo!("Implement buy_and_withdraw")
    }

    async fn poll_events(&self, _last_event_id: Option<&str>) -> Result<Vec<CanisterEvent>> {
        todo!("Implement poll_events")
    }
}

pub type DynCanisterClient = Arc<dyn CanisterClient>;
