//! User service
//!
//! Handles user operations:
//! - Primary name management
//! - Name metadata updates
//! - Ownership verification
//! - User inventory

use chrono::Utc;
use std::collections::HashSet;
use std::sync::Arc;

use crate::constants::FINALIZE_THRESHOLD;
use crate::domain::{NameMetadata, UpdateNameMetadataRequest, UserInventory};
use crate::error::{AppError, Result};
use crate::infra::{DynBlockchainClient, DynPostgresClient};

// ============================================================================
// User service
// ============================================================================

/// User service
pub struct UserService {
    blockchain: DynBlockchainClient,
    postgres: DynPostgresClient,
}

impl UserService {
    pub fn new(blockchain: DynBlockchainClient, postgres: DynPostgresClient) -> Self {
        Self {
            blockchain,
            postgres,
        }
    }

    /// Verify that a name belongs to the given address with sufficient confirmations
    pub async fn verify_name_ownership(&self, name: &str, address: &str) -> Result<()> {
        let ord_data = self
            .blockchain
            .ord_bns_rune(name)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Name '{}' not found", name)))?;

        // Verify ownership
        if ord_data.address != address {
            return Err(AppError::Forbidden(format!(
                "Name '{}' does not belong to address {}",
                name, address
            )));
        }

        // Verify confirmations
        if ord_data.confirmations < FINALIZE_THRESHOLD {
            return Err(AppError::BadRequest(format!(
                "Name '{}' has {} confirmations, but requires at least {}",
                name, ord_data.confirmations, FINALIZE_THRESHOLD
            )));
        }

        Ok(())
    }

    /// Set primary name for user
    pub async fn set_primary_name(&self, address: &str, name: &str) -> Result<()> {
        // Verify ownership first
        self.verify_name_ownership(name, address).await?;

        // Set primary name in database
        self.postgres.set_primary_name(address, name).await
    }

    /// Clear primary name for user
    pub async fn clear_primary_name(&self, address: &str) -> Result<()> {
        self.postgres.clear_primary_name(address).await
    }

    /// Update name metadata
    pub async fn update_name_metadata(
        &self,
        name: &str,
        owner_address: &str,
        request: UpdateNameMetadataRequest,
    ) -> Result<NameMetadata> {
        // Verify ownership first
        self.verify_name_ownership(name, owner_address).await?;

        // Get existing metadata or create new
        let now = Utc::now();
        let existing = self.postgres.get_name_metadata(name).await?;

        let metadata = NameMetadata {
            name: name.to_string(),
            owner_address: owner_address.to_string(),
            description: request.description,
            url: request.url,
            twitter: request.twitter,
            email: request.email,
            created_at: existing.map(|m| m.created_at).unwrap_or(now),
            updated_at: now,
        };

        self.postgres.upsert_name_metadata(&metadata).await?;

        Ok(metadata)
    }

    /// Get user inventory: listed and unlisted names
    pub async fn get_inventory(&self, user_address: &str) -> Result<UserInventory> {
        // Get listed names and total value from database
        let (listed_names, total_listed_value_sats) = self
            .postgres
            .get_listed_names_for_seller(user_address)
            .await?;
        let listed_set: HashSet<&str> = listed_names.iter().map(|s| s.as_str()).collect();

        // Get all names owned by address from blockchain
        let all_names_result = self.blockchain.ord_bns_address(user_address).await?;
        let all_names: HashSet<String> = all_names_result
            .runes
            .into_iter()
            .map(|r| r.rune_name)
            .collect();

        // Get pending delist names (will be returned to user)
        let pending_delist_names = self.postgres.get_pending_delist_names(user_address).await?;

        // Get pending buy_and_delist names (user is buying and delisting)
        let pending_buy_and_delist_names = self
            .postgres
            .get_pending_buy_and_delist_names(user_address)
            .await?;

        // Calculate unlisted:
        // (all on-chain names) - (listed names) + (pending delist) + (pending buy_and_delist)
        let mut unlisted: Vec<String> = all_names
            .iter()
            .filter(|name| !listed_set.contains(name.as_str()))
            .cloned()
            .collect();

        // Add pending delist names (they're being returned to user)
        for name in pending_delist_names {
            if !unlisted.contains(&name) {
                unlisted.push(name);
            }
        }

        // Add pending buy_and_delist names (user is acquiring them)
        for name in pending_buy_and_delist_names {
            if !unlisted.contains(&name) {
                unlisted.push(name);
            }
        }

        let listed_count = listed_names.len();
        let unlisted_count = unlisted.len();

        Ok(UserInventory {
            address: user_address.to_string(),
            listed: listed_names,
            unlisted,
            listed_count,
            unlisted_count,
            total_listed_value_sats,
            global_rank: 0,
        })
    }
}

pub type DynUserService = Arc<UserService>;
