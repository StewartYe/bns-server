//! Name service
//!
//! Handles name resolution, search, and detail queries.

use std::sync::Arc;

use crate::domain::{AddressResolution, NameDetail, NameResolution, NameSearchResult};
use crate::error::Result;
use crate::infra::{DynBlockchainClient, DynPostgresClient, DynRedisClient};

/// Name service
pub struct NameService {
    blockchain: DynBlockchainClient,
    postgres: DynPostgresClient,
    redis: DynRedisClient,
}

impl NameService {
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

    /// Forward resolution: name -> address (calls Ord)
    pub async fn resolve_name(&self, name: &str) -> Result<Option<NameResolution>> {
        self.blockchain.resolve_name(name).await
    }

    /// Reverse resolution: address -> names (calls Ord)
    pub async fn resolve_address(&self, address: &str) -> Result<AddressResolution> {
        self.blockchain.resolve_address(address).await
    }

    /// Search names by keyword
    pub async fn search(
        &self,
        _keyword: &str,
        _page: u32,
        _page_size: u32,
    ) -> Result<NameSearchResult> {
        todo!("Implement name search")
    }

    /// Get name detail with listing info
    pub async fn get_detail(&self, _name: &str) -> Result<Option<NameDetail>> {
        todo!("Implement get_detail")
    }
}

pub type DynNameService = Arc<NameService>;
