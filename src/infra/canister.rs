//! Canister client for BNS Server
//!
//! Interacts with ICP Canisters:
//! - BNS Canister: create_pool, get_events
//! - Orchestrator Canister: invoke (for transactions)

use async_trait::async_trait;
use candid::{Decode, Encode, Principal};
use ic_agent::Agent;
use ic_agent::identity::Secp256k1Identity;
use std::sync::Arc;

use crate::config::IcConfig;
use crate::domain::CanisterEvent;
use crate::error::{AppError, Result};
use crate::infra::bns_canister::{
    BnsCanisterEvent, BnsResultString, GetPoolInfoArgs, PagingArgs, PoolInfo,
};
use crate::infra::orchestrator_canister::{InvokeArgs, Result3};

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

/// IC mainnet URL
const IC_MAINNET_URL: &str = "https://ic0.app";

/// IC Agent wrapper for canister interactions
pub struct IcAgent {
    agent: Agent,
    bns_canister_id: Principal,
    orchestrator_canister_id: Principal,
}

impl IcAgent {
    /// Create a new IC Agent from configuration (always connects to mainnet)
    pub async fn new(config: &IcConfig) -> Result<Self> {
        // Create identity from PEM
        let identity = Secp256k1Identity::from_pem(config.identity_pem.as_bytes())
            .map_err(|e| AppError::Internal(format!("Failed to parse IC identity PEM: {}", e)))?;

        // Build agent (always mainnet)
        let agent = Agent::builder()
            .with_url(IC_MAINNET_URL)
            .with_identity(identity)
            .build()
            .map_err(|e| AppError::Internal(format!("Failed to build IC agent: {}", e)))?;

        // Parse BNS canister ID
        let bns_canister_id = Principal::from_text(&config.bns_canister_id)
            .map_err(|e| AppError::Internal(format!("Invalid BNS canister ID: {}", e)))?;

        // Parse Orchestrator canister ID
        let orchestrator_canister_id = Principal::from_text(&config.orchestrator_canister_id)
            .map_err(|e| AppError::Internal(format!("Invalid Orchestrator canister ID: {}", e)))?;

        tracing::info!(
            "IC Agent initialized: url={}, bns_canister_id={}, orchestrator_canister_id={}, principal={}",
            IC_MAINNET_URL,
            bns_canister_id,
            orchestrator_canister_id,
            agent
                .get_principal()
                .map(|p| p.to_string())
                .unwrap_or_else(|_| "unknown".to_string())
        );

        Ok(Self {
            agent,
            bns_canister_id,
            orchestrator_canister_id,
        })
    }

    /// Get the underlying agent
    pub fn agent(&self) -> &Agent {
        &self.agent
    }

    /// Get the BNS canister ID
    pub fn bns_canister_id(&self) -> Principal {
        self.bns_canister_id
    }

    /// Get the Orchestrator canister ID
    pub fn orchestrator_canister_id(&self) -> Principal {
        self.orchestrator_canister_id
    }

    /// Call BNS canister create_pool method
    ///
    /// Creates a new pool for a name, returns the pool address.
    pub async fn create_pool(&self, name: &str) -> Result<String> {
        let args = Encode!(&name.to_string())
            .map_err(|e| AppError::Canister(format!("Failed to encode args: {}", e)))?;

        let response = self
            .agent
            .update(&self.bns_canister_id, "create_pool")
            .with_arg(args)
            .call_and_wait()
            .await
            .map_err(|e| AppError::Canister(format!("create_pool call failed: {}", e)))?;

        let result = Decode!(&response, BnsResultString)
            .map_err(|e| AppError::Canister(format!("Failed to decode response: {}", e)))?;

        match result {
            BnsResultString::Ok(pool_address) => {
                tracing::info!("Created pool for {}: {}", name, pool_address);
                Ok(pool_address)
            }
            BnsResultString::Err(err) => {
                tracing::warn!("create_pool failed for {}: {}", name, err);
                Err(AppError::Canister(err))
            }
        }
    }

    /// Call Orchestrator canister invoke method
    ///
    /// Submits a transaction for execution. Returns tx_id on success.
    pub async fn invoke(&self, args: InvokeArgs) -> Result<String> {
        let encoded_args = Encode!(&args)
            .map_err(|e| AppError::Canister(format!("Failed to encode invoke args: {}", e)))?;

        let response = self
            .agent
            .update(&self.orchestrator_canister_id, "invoke")
            .with_arg(encoded_args)
            .call_and_wait()
            .await
            .map_err(|e| AppError::Canister(format!("invoke call failed: {}", e)))?;

        let result = Decode!(&response, Result3)
            .map_err(|e| AppError::Canister(format!("Failed to decode invoke response: {}", e)))?;

        match result {
            Result3::Ok(tx_id) => {
                tracing::info!("Invoke succeeded, tx_id: {}", tx_id);
                Ok(tx_id)
            }
            Result3::Err(err) => {
                tracing::warn!("invoke failed: {}", err);
                Err(AppError::Canister(err))
            }
        }
    }

    /// Call BNS canister get_events method
    ///
    /// Polls events from the BNS canister for tracking transaction status.
    pub async fn get_events(
        &self,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<(String, BnsCanisterEvent)>> {
        let args = PagingArgs { offset, limit };
        let encoded_args = Encode!(&args)
            .map_err(|e| AppError::Canister(format!("Failed to encode get_events args: {}", e)))?;

        let response = self
            .agent
            .query(&self.bns_canister_id, "get_events")
            .with_arg(encoded_args)
            .call()
            .await
            .map_err(|e| AppError::Canister(format!("get_events call failed: {}", e)))?;

        let events = Decode!(&response, Vec<(String, BnsCanisterEvent)>).map_err(|e| {
            AppError::Canister(format!("Failed to decode get_events response: {}", e))
        })?;

        Ok(events)
    }

    /// Call BNS canister get_pool_info method
    ///
    /// Gets pool info by pool address to verify the pool belongs to the expected name.
    pub async fn get_pool_info(&self, pool_address: &str) -> Result<PoolInfo> {
        let args = GetPoolInfoArgs {
            pool_address: pool_address.to_string(),
        };
        let encoded_args = Encode!(&args).map_err(|e| {
            AppError::Canister(format!("Failed to encode get_pool_info args: {}", e))
        })?;

        let response = self
            .agent
            .query(&self.bns_canister_id, "get_pool_info")
            .with_arg(encoded_args)
            .call()
            .await
            .map_err(|e| AppError::Canister(format!("get_pool_info call failed: {}", e)))?;

        // Response type is Option<PoolInfo>
        let result = Decode!(&response, Option<PoolInfo>).map_err(|e| {
            AppError::Canister(format!("Failed to decode get_pool_info response: {}", e))
        })?;

        result.ok_or_else(|| AppError::BadRequest(format!("Pool not found: {}", pool_address)))
    }
}

/// Canister client implementation using IC Agent
pub struct CanisterClientImpl {
    ic_agent: Arc<IcAgent>,
}

impl CanisterClientImpl {
    pub fn new(ic_agent: Arc<IcAgent>) -> Self {
        Self { ic_agent }
    }
}

#[async_trait]
impl CanisterClient for CanisterClientImpl {
    async fn create_pool(&self, _name: &str, _seller_address: &str) -> Result<CreatePoolResult> {
        // TODO: Implement canister call using self.ic_agent
        todo!("Implement create_pool canister call")
    }

    async fn list_name(
        &self,
        _name: &str,
        _pool_address: &str,
        _price_sats: u64,
        _tx_id: &str,
    ) -> Result<()> {
        // TODO: Implement canister call using self.ic_agent
        todo!("Implement list_name canister call")
    }

    async fn delist_name(&self, _name: &str, _pool_address: &str) -> Result<()> {
        // TODO: Implement canister call using self.ic_agent
        todo!("Implement delist_name canister call")
    }

    async fn buy_and_relist(
        &self,
        _name: &str,
        _buyer_address: &str,
        _new_price_sats: u64,
        _tx_id: &str,
    ) -> Result<BuyResult> {
        // TODO: Implement canister call using self.ic_agent
        todo!("Implement buy_and_relist canister call")
    }

    async fn buy_and_withdraw(
        &self,
        _name: &str,
        _buyer_address: &str,
        _tx_id: &str,
    ) -> Result<BuyResult> {
        // TODO: Implement canister call using self.ic_agent
        todo!("Implement buy_and_withdraw canister call")
    }

    async fn poll_events(&self, _last_event_id: Option<&str>) -> Result<Vec<CanisterEvent>> {
        // TODO: Implement canister call using self.ic_agent
        todo!("Implement poll_events canister call")
    }
}

pub type DynCanisterClient = Arc<dyn CanisterClient>;
