//! User service
//!
//! Handles user operations:
//! - Primary name management
//! - Name metadata updates
//! - Ownership verification

use std::sync::Arc;

use chrono::Utc;

use crate::domain::{NameMetadata, UpdateNameMetadataRequest};
use crate::error::{AppError, Result};
use crate::infra::{DynBlockchainClient, DynPostgresClient};

/// Minimum confirmations required for name operations
const FINALIZE_THRESHOLD: u64 = 3;

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
}

pub type DynUserService = Arc<UserService>;
