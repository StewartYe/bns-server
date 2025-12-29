//! Market service
//!
//! Handles rankings and market statistics.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::domain::ListingDisplay;
use crate::error::Result;
use crate::infra::{DynPostgresClient, DynRedisClient};

/// Ranking types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankingType {
    /// 24h top earners (by total sold amount)
    TopEarners24h,
    /// Newly listed names
    NewList,
    /// Recently sold names
    LastSold,
    /// Most active in 1h (by trade count)
    Active1h,
    /// Most active in 24h (by trade count)
    Active24h,
    /// Top sell prices in 24h
    TopSell24h,
    /// Best discounts
    BestDiscount,
    /// Best bargains
    BestBargain,
}

impl RankingType {
    /// Get the key suffix for this ranking type
    pub fn key_suffix(&self) -> &'static str {
        match self {
            RankingType::TopEarners24h => "rank:24h_winners",
            RankingType::NewList => "rank:new_list",
            RankingType::LastSold => "rank:last_sold",
            RankingType::Active1h => "rank:1h_active",
            RankingType::Active24h => "rank:24h_active",
            RankingType::TopSell24h => "rank:24h_top_sell",
            RankingType::BestDiscount => "rank:best_discount",
            RankingType::BestBargain => "rank:best_bargain",
        }
    }
}

/// Ranking entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankingEntry {
    pub rank: u32,
    pub name: String,
    pub score: f64,
    pub price_change_pct: Option<f64>,
}

/// Market statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStats {
    pub online_users: u64,
    pub total_users: u64,
    pub gas_level: String,
    pub listings_count: u64,
    pub total_list_value_sats: u64,
    pub tx_count_24h: u64,
    pub volume_24h_sats: u64,
}

/// Market service
pub struct MarketService {
    redis: DynRedisClient,
    postgres: DynPostgresClient,
}

impl MarketService {
    pub fn new(redis: DynRedisClient, postgres: DynPostgresClient) -> Self {
        Self { redis, postgres }
    }

    /// Get ranking list
    pub async fn get_ranking(
        &self,
        _ranking_type: RankingType,
        _limit: u32,
    ) -> Result<Vec<RankingEntry>> {
        todo!("Implement get_ranking")
    }

    /// Get market statistics
    pub async fn get_stats(&self) -> Result<MarketStats> {
        todo!("Implement get_stats")
    }

    /// Update ranking after a trade
    pub async fn update_rankings_on_trade(
        &self,
        _name: &str,
        _price_sats: u64,
        _seller: &str,
    ) -> Result<()> {
        todo!("Implement update_rankings_on_trade")
    }

    /// Update ranking after a new listing
    pub async fn update_rankings_on_list(
        &self,
        _name: &str,
        _price_sats: u64,
        _previous_price: Option<u64>,
    ) -> Result<()> {
        todo!("Implement update_rankings_on_list")
    }

    /// Increment online user count
    pub async fn increment_online_users(&self) -> Result<u64> {
        todo!("Implement increment_online_users")
    }

    /// Decrement online user count
    pub async fn decrement_online_users(&self) -> Result<u64> {
        todo!("Implement decrement_online_users")
    }

    /// Clean up expired ranking entries (called periodically)
    pub async fn cleanup_expired_rankings(&self) -> Result<()> {
        todo!("Implement cleanup_expired_rankings")
    }
}

pub type DynMarketService = Arc<MarketService>;
