//! Redis client for BNS Server
//!
//! Handles:
//! - Rankings (ZSet operations)
//! - Caching
//! - Session management
//! - Pub/Sub for real-time updates
//! - Market statistics

use async_trait::async_trait;
use redis::aio::MultiplexedConnection;
use redis::{AsyncCommands, Client};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::{Network, RedisConfig};
use crate::error::Result;

/// Redis key builder with network prefix
pub struct KeyBuilder {
    network: Network,
}

impl KeyBuilder {
    pub fn new(network: Network) -> Self {
        Self { network }
    }

    /// Build a key with network prefix
    pub fn key(&self, suffix: &str) -> String {
        format!("{}:{}", self.network.key_prefix(), suffix)
    }

    // Rankings
    pub fn rank_new_list(&self) -> String {
        self.key("rank:new_list")
    }

    pub fn rank_last_sold(&self) -> String {
        self.key("rank:last_sold")
    }

    pub fn rank_24h_winners(&self) -> String {
        self.key("rank:24h_winners")
    }

    // Metadata for rankings
    pub fn rank_meta(&self, name: &str) -> String {
        self.key(&format!("rank:meta:{}", name))
    }

    // Pub/Sub channels
    pub fn channel_events(&self) -> String {
        self.key("events")
    }

    pub fn channel_new_list(&self) -> String {
        self.key("new_list")
    }
}

/// Listing info stored in Redis
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ListingMeta {
    pub name: String,
    pub price_sats: u64,
    pub seller_address: String,
    pub confirmations: i32,
    pub listed_at: i64, // Unix timestamp
}

/// Redis client abstraction
#[async_trait]
pub trait RedisClient: Send + Sync {
    /// Get key builder for network-prefixed keys
    fn keys(&self) -> &KeyBuilder;

    // ZSet operations for rankings
    async fn zadd(&self, key: &str, score: f64, member: &str) -> Result<()>;
    async fn zrem(&self, key: &str, member: &str) -> Result<()>;
    async fn zrange_with_scores(&self, key: &str, start: isize, stop: isize) -> Result<Vec<(String, f64)>>;
    async fn zrevrange_with_scores(&self, key: &str, start: isize, stop: isize) -> Result<Vec<(String, f64)>>;
    async fn zincrby(&self, key: &str, increment: f64, member: &str) -> Result<f64>;
    async fn zcard(&self, key: &str) -> Result<u64>;

    // Hash operations for metadata
    async fn hset(&self, key: &str, field: &str, value: &str) -> Result<()>;
    async fn hget(&self, key: &str, field: &str) -> Result<Option<String>>;
    async fn hgetall(&self, key: &str) -> Result<Vec<(String, String)>>;
    async fn hdel(&self, key: &str, field: &str) -> Result<()>;

    // String operations for stats/sessions
    async fn set(&self, key: &str, value: &str) -> Result<()>;
    async fn set_ex(&self, key: &str, value: &str, seconds: u64) -> Result<()>;
    async fn get(&self, key: &str) -> Result<Option<String>>;
    async fn del(&self, key: &str) -> Result<()>;
    async fn incr(&self, key: &str) -> Result<i64>;
    async fn decr(&self, key: &str) -> Result<i64>;

    // Pub/Sub
    async fn publish(&self, channel: &str, message: &str) -> Result<u64>;

    // High-level operations

    /// Add a new listing to the new_list ranking
    async fn add_new_listing(&self, meta: &ListingMeta) -> Result<()>;

    /// Update listing confirmations in new_list
    async fn update_listing_confirmations(&self, name: &str, confirmations: i32) -> Result<()>;

    /// Get top N newest listings
    async fn get_new_listings(&self, count: usize) -> Result<Vec<ListingMeta>>;
}

/// Redis client implementation with TLS support
pub struct RedisClientImpl {
    conn: Arc<Mutex<MultiplexedConnection>>,
    keys: KeyBuilder,
}

impl RedisClientImpl {
    pub async fn new(config: &RedisConfig, network: Network) -> Result<Self> {
        let url = config.connection_url();
        tracing::info!("Connecting to Redis at {} (TLS: {})", url, config.tls);

        // Note: For GCP Memorystore with IAM auth, TLS is enabled via rediss:// scheme
        // IAM token is handled by the GCP sidecar/proxy, not in the connection string
        let client = Client::open(url.as_str())?;
        let conn = client.get_multiplexed_async_connection().await?;
        tracing::info!("Connected to Redis successfully");

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            keys: KeyBuilder::new(network),
        })
    }
}

#[async_trait]
impl RedisClient for RedisClientImpl {
    fn keys(&self) -> &KeyBuilder {
        &self.keys
    }

    async fn zadd(&self, key: &str, score: f64, member: &str) -> Result<()> {
        let mut conn = self.conn.lock().await;
        let _: () = conn.zadd(key, member, score).await?;
        Ok(())
    }

    async fn zrem(&self, key: &str, member: &str) -> Result<()> {
        let mut conn = self.conn.lock().await;
        let _: () = conn.zrem(key, member).await?;
        Ok(())
    }

    async fn zrange_with_scores(&self, key: &str, start: isize, stop: isize) -> Result<Vec<(String, f64)>> {
        let mut conn = self.conn.lock().await;
        let result: Vec<(String, f64)> = conn.zrange_withscores(key, start, stop).await?;
        Ok(result)
    }

    async fn zrevrange_with_scores(&self, key: &str, start: isize, stop: isize) -> Result<Vec<(String, f64)>> {
        let mut conn = self.conn.lock().await;
        let result: Vec<(String, f64)> = conn.zrevrange_withscores(key, start, stop).await?;
        Ok(result)
    }

    async fn zincrby(&self, key: &str, increment: f64, member: &str) -> Result<f64> {
        let mut conn = self.conn.lock().await;
        let result: f64 = conn.zincr(key, member, increment).await?;
        Ok(result)
    }

    async fn zcard(&self, key: &str) -> Result<u64> {
        let mut conn = self.conn.lock().await;
        let result: u64 = conn.zcard(key).await?;
        Ok(result)
    }

    async fn hset(&self, key: &str, field: &str, value: &str) -> Result<()> {
        let mut conn = self.conn.lock().await;
        let _: () = conn.hset(key, field, value).await?;
        Ok(())
    }

    async fn hget(&self, key: &str, field: &str) -> Result<Option<String>> {
        let mut conn = self.conn.lock().await;
        let result: Option<String> = conn.hget(key, field).await?;
        Ok(result)
    }

    async fn hgetall(&self, key: &str) -> Result<Vec<(String, String)>> {
        let mut conn = self.conn.lock().await;
        let result: Vec<(String, String)> = conn.hgetall(key).await?;
        Ok(result)
    }

    async fn hdel(&self, key: &str, field: &str) -> Result<()> {
        let mut conn = self.conn.lock().await;
        let _: () = conn.hdel(key, field).await?;
        Ok(())
    }

    async fn set(&self, key: &str, value: &str) -> Result<()> {
        let mut conn = self.conn.lock().await;
        let _: () = conn.set(key, value).await?;
        Ok(())
    }

    async fn set_ex(&self, key: &str, value: &str, seconds: u64) -> Result<()> {
        let mut conn = self.conn.lock().await;
        let _: () = conn.set_ex(key, value, seconds).await?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.conn.lock().await;
        let result: Option<String> = conn.get(key).await?;
        Ok(result)
    }

    async fn del(&self, key: &str) -> Result<()> {
        let mut conn = self.conn.lock().await;
        let _: () = conn.del(key).await?;
        Ok(())
    }

    async fn incr(&self, key: &str) -> Result<i64> {
        let mut conn = self.conn.lock().await;
        let result: i64 = conn.incr(key, 1).await?;
        Ok(result)
    }

    async fn decr(&self, key: &str) -> Result<i64> {
        let mut conn = self.conn.lock().await;
        let result: i64 = conn.decr(key, 1).await?;
        Ok(result)
    }

    async fn publish(&self, channel: &str, message: &str) -> Result<u64> {
        let mut conn = self.conn.lock().await;
        let result: u64 = conn.publish(channel, message).await?;
        Ok(result)
    }

    // High-level operations

    async fn add_new_listing(&self, meta: &ListingMeta) -> Result<()> {
        let key = self.keys.rank_new_list();
        let meta_key = self.keys.rank_meta(&meta.name);

        // Add to sorted set with listed_at as score (for ordering by time)
        self.zadd(&key, meta.listed_at as f64, &meta.name).await?;

        // Store metadata as JSON
        let meta_json = serde_json::to_string(meta)?;
        self.set(&meta_key, &meta_json).await?;

        // Trim to keep only top 20
        let count = self.zcard(&key).await?;
        if count > 20 {
            // Remove oldest entries (lowest scores)
            let mut conn = self.conn.lock().await;
            let _: () = redis::cmd("ZREMRANGEBYRANK")
                .arg(&key)
                .arg(0)
                .arg((count as i64) - 21)
                .query_async(&mut *conn)
                .await?;
        }

        // Publish update event
        let channel = self.keys.channel_new_list();
        let event = serde_json::json!({
            "type": "new_listing",
            "data": meta
        });
        self.publish(&channel, &event.to_string()).await?;

        Ok(())
    }

    async fn update_listing_confirmations(&self, name: &str, confirmations: i32) -> Result<()> {
        let meta_key = self.keys.rank_meta(name);

        // Get existing metadata
        if let Some(meta_json) = self.get(&meta_key).await? {
            if let Ok(mut meta) = serde_json::from_str::<ListingMeta>(&meta_json) {
                meta.confirmations = confirmations;
                let updated_json = serde_json::to_string(&meta)?;
                self.set(&meta_key, &updated_json).await?;

                // Publish update event
                let channel = self.keys.channel_new_list();
                let event = serde_json::json!({
                    "type": "confirmation_update",
                    "data": meta
                });
                self.publish(&channel, &event.to_string()).await?;
            }
        }

        Ok(())
    }

    async fn get_new_listings(&self, count: usize) -> Result<Vec<ListingMeta>> {
        let key = self.keys.rank_new_list();

        // Get top N newest (highest scores = most recent)
        let entries = self.zrevrange_with_scores(&key, 0, (count - 1) as isize).await?;

        let mut listings = Vec::with_capacity(entries.len());
        for (name, _score) in entries {
            let meta_key = self.keys.rank_meta(&name);
            if let Some(meta_json) = self.get(&meta_key).await? {
                if let Ok(meta) = serde_json::from_str::<ListingMeta>(&meta_json) {
                    listings.push(meta);
                }
            }
        }

        Ok(listings)
    }
}

pub type DynRedisClient = Arc<dyn RedisClient>;
