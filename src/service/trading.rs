//! Trading service
//!
//! Handles listing, delisting, and buying operations.

use std::sync::Arc;

use crate::domain::{BuyAction, BuyRequest, CreateListingRequest, DelistRequest, Listing};
use crate::error::Result;
use crate::infra::{DynBlockchainClient, DynCanisterClient, DynPostgresClient, DynRedisClient};

/// Result of a list operation
#[derive(Debug)]
pub struct ListResult {
    pub listing: Listing,
    pub pool_address: String,
    pub is_new_pool: bool,
}

/// Result of a buy operation
#[derive(Debug)]
pub struct BuyResult {
    pub name: String,
    pub buyer_address: String,
    pub price_sats: u64,
    pub tx_id: String,
    /// New pool address if relisted
    pub new_pool_address: Option<String>,
}

/// Trading service
pub struct TradingService {
    canister: DynCanisterClient,
    blockchain: DynBlockchainClient,
    postgres: DynPostgresClient,
    redis: DynRedisClient,
}

impl TradingService {
    pub fn new(
        canister: DynCanisterClient,
        blockchain: DynBlockchainClient,
        postgres: DynPostgresClient,
        redis: DynRedisClient,
    ) -> Self {
        Self {
            canister,
            blockchain,
            postgres,
            redis,
        }
    }

    /// Create a new listing
    ///
    /// Flow:
    /// 1. Check if pool exists for this name
    /// 2. If not, call Canister create_pool
    /// 3. Return pool address for frontend to build PSBT
    pub async fn initiate_list(
        &self,
        _request: &CreateListingRequest,
        _seller_address: &str,
    ) -> Result<String> {
        todo!("Implement initiate_list - returns pool_address")
    }

    /// Complete listing after transaction broadcast
    ///
    /// Flow:
    /// 1. Broadcast list transaction
    /// 2. Update market status
    /// 3. Call Canister list_name
    pub async fn complete_list(
        &self,
        _name: &str,
        _pool_address: &str,
        _price_sats: u64,
        _tx_hex: &str,
    ) -> Result<ListResult> {
        todo!("Implement complete_list")
    }

    /// Delist a name
    ///
    /// Flow:
    /// 1. Call Canister delist_name
    /// 2. Update market status on success
    pub async fn delist(
        &self,
        _request: &DelistRequest,
        _seller_address: &str,
    ) -> Result<()> {
        todo!("Implement delist")
    }

    /// Buy a listed name
    ///
    /// Flow:
    /// 1. Validate listing exists and is active
    /// 2. Call Canister buy_and_relist or buy_and_withdraw
    /// 3. Update market status on success
    pub async fn buy(
        &self,
        _request: &BuyRequest,
        _buyer_address: &str,
        _tx_hex: &str,
    ) -> Result<BuyResult> {
        todo!("Implement buy")
    }

    /// Get listing by name
    pub async fn get_listing(&self, name: &str) -> Result<Option<Listing>> {
        self.postgres.get_listing(name).await
    }
}

pub type DynTradingService = Arc<TradingService>;
