//! ShoutOut service
//!
//! Handles promotional message management.

use std::sync::Arc;

use crate::domain::{CreateShoutOutRequest, ShoutOut, ShoutOutList};
use crate::error::Result;
use crate::infra::DynPostgresClient;

/// ShoutOut service
pub struct ShoutOutService {
    postgres: DynPostgresClient,
}

impl ShoutOutService {
    pub fn new(postgres: DynPostgresClient) -> Self {
        Self { postgres }
    }

    /// Create a new shoutout
    pub async fn create(
        &self,
        _request: &CreateShoutOutRequest,
        _promoter_address: &str,
    ) -> Result<ShoutOut> {
        todo!("Implement create shoutout")
    }

    /// Get active shoutouts
    pub async fn get_active(&self) -> Result<ShoutOutList> {
        let shoutouts = self.postgres.get_active_shoutouts().await?;
        Ok(ShoutOutList { shoutouts })
    }

    /// Clean up expired shoutouts (called periodically)
    pub async fn cleanup_expired(&self) -> Result<u64> {
        self.postgres.expire_shoutouts().await
    }
}

pub type DynShoutOutService = Arc<ShoutOutService>;
