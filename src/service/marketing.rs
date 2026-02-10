//! Marketing service
//!
//! Handles marketing and platform statistics operations:
//! - Get total users count
//! - Get listed count and listed value
//! - Get 24-hour transaction volume and count

use crate::domain::MarketingInfo;
use crate::error::Result;
use crate::infra::{DynPostgresClient, DynRedisClient};
use crate::service::DynUserService;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Marketing service for platform statistics
pub struct MarketingService {
    postgres: DynPostgresClient,
    _redis: DynRedisClient,
    _user_service: DynUserService,
    online_users: Arc<AtomicU64>,
}

impl MarketingService {
    pub fn new(
        postgres: DynPostgresClient,
        _redis: DynRedisClient,
        _user_service: DynUserService,
        online_users: Arc<AtomicU64>,
    ) -> Self {
        Self {
            postgres,
            _redis,
            _user_service,
            online_users,
        }
    }

    pub async fn get_marketing_info(&self) -> Result<MarketingInfo> {
        let (listing_count, listed_value) = self.postgres.get_listing_count_and_valuation().await?;
        let user_count = self.postgres.get_user_count().await?;
        let (tx_count, volume) = self.postgres.get_24h_tx_vol().await?;
        Ok(MarketingInfo {
            total_users: user_count,
            total_online: self.online_users.load(Ordering::Relaxed),
            listed_count: listing_count,
            txs_24h: tx_count,
            vol_24h: volume,
            listed_value,
        })
    }
}

pub type DynMarketingService = Arc<MarketingService>;
