//! Marketing service
//!
//! Handles marketing and platform statistics operations:
//! - Get total users count
//! - Get listing count and valuation
//! - Get 24-hour transaction volume and count

use crate::domain::MarketingInfo;
use crate::error::Result;
use crate::infra::{DynPostgresClient, DynRedisClient};
use crate::service::DynUserService;
use std::sync::Arc;

/// Marketing service for platform statistics
pub struct MarketingService {
    postgres: DynPostgresClient,
    _redis: DynRedisClient,
    _user_service: DynUserService,
}

impl MarketingService {
    pub fn new(
        postgres: DynPostgresClient,
        _redis: DynRedisClient,
        _user_service: DynUserService,
    ) -> Self {
        Self {
            postgres,
            _redis,
            _user_service,
        }
    }

    pub async fn get_marketing_info(&self) -> Result<MarketingInfo> {
        let (listing_count, valuation) = self.postgres.get_listing_count_and_valuation().await?;
        let user_count = self.postgres.get_user_count().await?;
        let (tx_count, volume) = self.postgres.get_24h_tx_vol().await?;
        Ok(MarketingInfo {
            total_users: user_count,
            total_online: 0,
            total_listings: listing_count,
            txs_24h: tx_count,
            vol_24h: volume,
            valuation,
        })
    }
}

pub type DynMarketingService = Arc<MarketingService>;
