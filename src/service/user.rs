//! User service
//!
//! Handles user inventory and transaction history.

use std::sync::Arc;

use crate::domain::{UserHistory, UserInventory};
use crate::error::Result;
use crate::infra::{DynBlockchainClient, DynPostgresClient, DynRedisClient};

/// User service
pub struct UserService {
    blockchain: DynBlockchainClient,
    postgres: DynPostgresClient,
    redis: DynRedisClient,
}

impl UserService {
    pub fn new(
        blockchain: DynBlockchainClient,
        postgres: DynPostgresClient,
        redis: DynRedisClient,
    ) -> Self {
        Self {
            blockchain,
            postgres,
            redis,
        }
    }

    /// Get user inventory
    ///
    /// Returns:
    /// - Listed names (from PostgreSQL)
    /// - Owned names (from Ord via blockchain client)
    /// - Total listed value
    pub async fn get_inventory(&self, _address: &str) -> Result<UserInventory> {
        todo!("Implement get_inventory")
    }

    /// Get user transaction history
    ///
    /// Includes list/buy/sell/delist with confirmation status
    pub async fn get_history(
        &self,
        address: &str,
        limit: u32,
        offset: u32,
    ) -> Result<UserHistory> {
        let transactions = self.postgres.get_user_transactions(address, limit, offset).await?;
        Ok(UserHistory {
            btc_address: address.to_string(),
            transactions,
        })
    }

    /// Track user activity (update last_seen)
    pub async fn track_activity(&self, _address: &str) -> Result<()> {
        todo!("Implement track_activity")
    }
}

pub type DynUserService = Arc<UserService>;
