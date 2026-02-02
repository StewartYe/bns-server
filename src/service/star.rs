//! Star service
//!
//! Handles starring/bookmarking operations:
//! - Star a name or collector address
//! - Unstar a name or collector
//! - Get user's starred items
//! - Validate target type (name or collector address)

use crate::AppError;
use crate::config::CONFIG;
use crate::domain::{StarResponse, StarTargetType};
use crate::infra::{DynBlockchainClient, DynPostgresClient};
use std::str::FromStr;
use std::sync::Arc;

/// Star service for managing user bookmarks
pub struct StarService {
    pub postgres: DynPostgresClient,
    pub blockchain: DynBlockchainClient,
}

impl StarService {
    pub fn new(postgres: DynPostgresClient, blockchain: DynBlockchainClient) -> Self {
        Self {
            postgres,
            blockchain,
        }
    }

    pub async fn star(&self, user_address: &str, target: &str) -> crate::Result<()> {
        let target_type;
        if let Ok(addr) = bitcoin::Address::from_str(target) {
            if addr.is_valid_for_network(CONFIG.bitcoin_network()) {
                target_type = StarTargetType::Collector;
            } else {
                return Err(AppError::BadRequest(
                    "invalid collector's network".to_string(),
                ));
            }
        } else {
            self.blockchain
                .ord_bns_rune(target)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Name '{}' not found", target)))?;
            target_type = StarTargetType::Name;
        }
        self.postgres.star(user_address, target, target_type).await
    }

    pub async fn unstar(&self, user_address: &str, name: &str) -> crate::Result<()> {
        self.postgres.unstar(user_address, name).await?;
        Ok(())
    }

    pub async fn get_stars(&self, user_address: &str) -> crate::Result<Vec<StarResponse>> {
        let stars = self.postgres.user_stars(user_address).await?;
        Ok(stars.into_iter().map(StarResponse::from).collect())
    }
}

pub type DynStarService = Arc<StarService>;
