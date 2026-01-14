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

    // Pending confirmations queue (names waiting for confirmation threshold)
    pub fn pending_confirmations(&self) -> String {
        self.key("pending_confirmations")
    }

    // Pub/Sub channels
    pub fn channel_events(&self) -> String {
        self.key("events")
    }

    pub fn channel_new_list(&self) -> String {
        self.key("new_list")
    }

    // Sessions
    pub fn session(&self, session_id: &str) -> String {
        self.key(&format!("session:{}", session_id))
    }

    pub fn user_sessions(&self, btc_address: &str) -> String {
        self.key(&format!("user_sessions:{}", btc_address))
    }

    // Pending transactions (tx_ids waiting for canister events)
    pub fn pending_txs(&self) -> String {
        self.key("pending_txs")
    }

    // Last processed event offset (for get_events polling)
    pub fn event_offset(&self) -> String {
        self.key("event_offset")
    }
}

/// Listing info stored in Redis
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ListingMeta {
    pub name: String,
    pub price_sats: u64,
    pub seller_address: String,
    pub listed_at: i64, // Unix timestamp
    #[serde(default)]
    pub tx_id: Option<String>,
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

    /// Get top N newest listings
    async fn get_new_listings(&self, count: usize) -> Result<Vec<ListingMeta>>;

    // Pending confirmations queue operations

    /// Add a listing to the pending confirmations queue
    async fn add_pending_confirmation(&self, meta: &ListingMeta) -> Result<()>;

    /// Get all pending confirmations (names waiting for threshold)
    async fn get_pending_confirmations(&self) -> Result<Vec<ListingMeta>>;

    /// Remove a listing from the pending confirmations queue
    async fn remove_pending_confirmation(&self, name: &str) -> Result<()>;

    // Session operations

    /// Store a session with TTL
    async fn set_session(&self, session_id: &str, session_json: &str, ttl_secs: u64) -> Result<()>;

    /// Get a session by ID
    async fn get_session(&self, session_id: &str) -> Result<Option<String>>;

    /// Delete a session
    async fn delete_session(&self, session_id: &str) -> Result<()>;

    /// Add session ID to user's session set (for invalidating all user sessions)
    async fn add_user_session(&self, btc_address: &str, session_id: &str, ttl_secs: u64) -> Result<()>;

    /// Get all session IDs for a user
    async fn get_user_sessions(&self, btc_address: &str) -> Result<Vec<String>>;

    /// Delete all sessions for a user
    async fn delete_user_sessions(&self, btc_address: &str) -> Result<u64>;

    // Pending transaction tracking (for get_events polling)

    /// Add a pending transaction to track
    async fn add_pending_tx(&self, tx_id: &str, tracking_data: &str) -> Result<()>;

    /// Get all pending transactions
    async fn get_pending_txs(&self) -> Result<Vec<(String, String)>>;

    /// Remove a pending transaction
    async fn remove_pending_tx(&self, tx_id: &str) -> Result<()>;

    // Event offset tracking (for get_events polling persistence)

    /// Get the last processed event offset
    async fn get_event_offset(&self) -> Result<u64>;

    /// Set the last processed event offset
    async fn set_event_offset(&self, offset: u64) -> Result<()>;
}

/// Redis client implementation with TLS support and automatic token refresh
///
/// Uses a fresh connection for each operation when IAM auth is enabled to avoid
/// issues with MultiplexedConnection internal reconnects losing auth state.
pub struct RedisClientImpl {
    config: RedisConfig,
    keys: KeyBuilder,
    /// Cached token and its fetch time
    token_cache: Arc<Mutex<Option<(String, std::time::Instant)>>>,
}

// Token refresh interval (50 minutes, before 1-hour expiry)
const TOKEN_REFRESH_SECS: u64 = 50 * 60;

impl RedisClientImpl {
    pub async fn new(config: &RedisConfig, network: Network) -> Result<Self> {
        tracing::info!(
            "Connecting to Redis at {}:{} (TLS: {}, IAM: {})",
            config.host,
            config.port,
            config.tls,
            config.use_iam
        );

        // Test connection on startup
        let client = Self {
            config: config.clone(),
            keys: KeyBuilder::new(network),
            token_cache: Arc::new(Mutex::new(None)),
        };

        // Verify we can connect
        let mut conn = client.get_connection().await?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        tracing::info!("Connected to Redis successfully (PING OK)");

        Ok(client)
    }

    /// Get a fresh authenticated connection
    async fn get_connection(&self) -> Result<MultiplexedConnection> {
        let url = self.config.connection_url();
        let client = Client::open(url.as_str())?;
        let mut conn = client.get_multiplexed_async_connection().await?;

        // If using IAM auth, authenticate with access token
        if self.config.use_iam {
            let token = self.get_cached_token().await?;

            let auth_result: std::result::Result<String, redis::RedisError> =
                redis::cmd("AUTH")
                    .arg("default")
                    .arg(&token)
                    .query_async(&mut conn)
                    .await;

            if let Err(e) = auth_result {
                tracing::error!("Valkey IAM authentication failed: {:?}", e);
                return Err(crate::error::AppError::Redis(e));
            }
        }

        Ok(conn)
    }

    /// Get cached token or fetch a new one if expired
    async fn get_cached_token(&self) -> Result<String> {
        let mut cache = self.token_cache.lock().await;

        // Check if we have a valid cached token
        if let Some((ref token, ref fetched_at)) = *cache {
            if fetched_at.elapsed().as_secs() < TOKEN_REFRESH_SECS {
                return Ok(token.clone());
            }
        }

        // Fetch new token
        tracing::info!("Fetching GCP access token for Valkey IAM auth...");
        let token = Self::get_gcp_access_token().await?;
        tracing::info!("Got access token (length: {})", token.len());

        *cache = Some((token.clone(), std::time::Instant::now()));
        Ok(token)
    }

    /// Get GCP access token from metadata server
    async fn get_gcp_access_token() -> Result<String> {
        let client = reqwest::Client::new();
        let response = client
            .get("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token")
            .header("Metadata-Flavor", "Google")
            .send()
            .await
            .map_err(|e| crate::error::AppError::Internal(format!("Failed to get GCP token: {}", e)))?;

        if !response.status().is_success() {
            return Err(crate::error::AppError::Internal(format!(
                "GCP metadata server returned {}",
                response.status()
            )));
        }

        #[derive(serde::Deserialize)]
        struct TokenResponse {
            access_token: String,
        }

        let token_resp: TokenResponse = response.json().await.map_err(|e| {
            crate::error::AppError::Internal(format!("Failed to parse GCP token response: {}", e))
        })?;

        Ok(token_resp.access_token)
    }
}

#[async_trait]
impl RedisClient for RedisClientImpl {
    fn keys(&self) -> &KeyBuilder {
        &self.keys
    }

    async fn zadd(&self, key: &str, score: f64, member: &str) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let _: () = conn.zadd(key, member, score).await?;
        Ok(())
    }

    async fn zrem(&self, key: &str, member: &str) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let _: () = conn.zrem(key, member).await?;
        Ok(())
    }

    async fn zrange_with_scores(&self, key: &str, start: isize, stop: isize) -> Result<Vec<(String, f64)>> {
        let mut conn = self.get_connection().await?;
        Ok(conn.zrange_withscores(key, start, stop).await?)
    }

    async fn zrevrange_with_scores(&self, key: &str, start: isize, stop: isize) -> Result<Vec<(String, f64)>> {
        let mut conn = self.get_connection().await?;
        Ok(conn.zrevrange_withscores(key, start, stop).await?)
    }

    async fn zincrby(&self, key: &str, increment: f64, member: &str) -> Result<f64> {
        let mut conn = self.get_connection().await?;
        Ok(conn.zincr(key, member, increment).await?)
    }

    async fn zcard(&self, key: &str) -> Result<u64> {
        let mut conn = self.get_connection().await?;
        Ok(conn.zcard(key).await?)
    }

    async fn hset(&self, key: &str, field: &str, value: &str) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let _: () = conn.hset(key, field, value).await?;
        Ok(())
    }

    async fn hget(&self, key: &str, field: &str) -> Result<Option<String>> {
        let mut conn = self.get_connection().await?;
        Ok(conn.hget(key, field).await?)
    }

    async fn hgetall(&self, key: &str) -> Result<Vec<(String, String)>> {
        let mut conn = self.get_connection().await?;
        Ok(conn.hgetall(key).await?)
    }

    async fn hdel(&self, key: &str, field: &str) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let _: () = conn.hdel(key, field).await?;
        Ok(())
    }

    async fn set(&self, key: &str, value: &str) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let _: () = conn.set(key, value).await?;
        Ok(())
    }

    async fn set_ex(&self, key: &str, value: &str, seconds: u64) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let _: () = conn.set_ex(key, value, seconds).await?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.get_connection().await?;
        Ok(conn.get(key).await?)
    }

    async fn del(&self, key: &str) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let _: () = conn.del(key).await?;
        Ok(())
    }

    async fn incr(&self, key: &str) -> Result<i64> {
        let mut conn = self.get_connection().await?;
        Ok(conn.incr(key, 1).await?)
    }

    async fn decr(&self, key: &str) -> Result<i64> {
        let mut conn = self.get_connection().await?;
        Ok(conn.decr(key, 1).await?)
    }

    async fn publish(&self, channel: &str, message: &str) -> Result<u64> {
        let mut conn = self.get_connection().await?;
        Ok(conn.publish(channel, message).await?)
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
            let mut conn = self.get_connection().await?;
            let _: () = redis::cmd("ZREMRANGEBYRANK")
                .arg(&key)
                .arg(0)
                .arg((count as i64) - 21)
                .query_async(&mut conn)
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

    // Pending confirmations queue operations

    async fn add_pending_confirmation(&self, meta: &ListingMeta) -> Result<()> {
        let key = self.keys.pending_confirmations();
        let meta_json = serde_json::to_string(meta)?;

        // Use HSET with name as field, meta as value
        self.hset(&key, &meta.name, &meta_json).await?;

        tracing::info!("Added {} to pending confirmations queue", meta.name);
        Ok(())
    }

    async fn get_pending_confirmations(&self) -> Result<Vec<ListingMeta>> {
        let key = self.keys.pending_confirmations();
        let entries = self.hgetall(&key).await?;

        let mut listings = Vec::with_capacity(entries.len());
        for (_name, meta_json) in entries {
            if let Ok(meta) = serde_json::from_str::<ListingMeta>(&meta_json) {
                listings.push(meta);
            }
        }

        Ok(listings)
    }

    async fn remove_pending_confirmation(&self, name: &str) -> Result<()> {
        let key = self.keys.pending_confirmations();
        self.hdel(&key, name).await?;

        tracing::info!("Removed {} from pending confirmations queue", name);
        Ok(())
    }

    // Session operations

    async fn set_session(&self, session_id: &str, session_json: &str, ttl_secs: u64) -> Result<()> {
        let key = self.keys.session(session_id);
        self.set_ex(&key, session_json, ttl_secs).await
    }

    async fn get_session(&self, session_id: &str) -> Result<Option<String>> {
        let key = self.keys.session(session_id);
        self.get(&key).await
    }

    async fn delete_session(&self, session_id: &str) -> Result<()> {
        let key = self.keys.session(session_id);
        self.del(&key).await
    }

    async fn add_user_session(&self, btc_address: &str, session_id: &str, ttl_secs: u64) -> Result<()> {
        let key = self.keys.user_sessions(btc_address);
        let mut conn = self.get_connection().await?;

        // Add session to set
        let _: () = conn.sadd(&key, session_id).await?;

        // Set expiry on the set (refreshed each time a session is added)
        let _: () = conn.expire(&key, ttl_secs as i64).await?;

        Ok(())
    }

    async fn get_user_sessions(&self, btc_address: &str) -> Result<Vec<String>> {
        let key = self.keys.user_sessions(btc_address);
        let mut conn = self.get_connection().await?;
        Ok(conn.smembers(&key).await?)
    }

    async fn delete_user_sessions(&self, btc_address: &str) -> Result<u64> {
        // Get all session IDs for the user
        let session_ids = self.get_user_sessions(btc_address).await?;
        let count = session_ids.len() as u64;

        // Delete each session
        for session_id in &session_ids {
            let key = self.keys.session(session_id);
            let _ = self.del(&key).await;
        }

        // Delete the user sessions set
        let key = self.keys.user_sessions(btc_address);
        self.del(&key).await?;

        if count > 0 {
            tracing::info!("Deleted {} session(s) for user {}", count, btc_address);
        }

        Ok(count)
    }

    // Pending transaction tracking

    async fn add_pending_tx(&self, tx_id: &str, tracking_data: &str) -> Result<()> {
        let key = self.keys.pending_txs();
        self.hset(&key, tx_id, tracking_data).await?;
        tracing::info!("Added tx_id {} to pending tx tracking", tx_id);
        Ok(())
    }

    async fn get_pending_txs(&self) -> Result<Vec<(String, String)>> {
        let key = self.keys.pending_txs();
        self.hgetall(&key).await
    }

    async fn remove_pending_tx(&self, tx_id: &str) -> Result<()> {
        let key = self.keys.pending_txs();
        self.hdel(&key, tx_id).await?;
        tracing::info!("Removed tx_id {} from pending tx tracking", tx_id);
        Ok(())
    }

    // Event offset tracking

    async fn get_event_offset(&self) -> Result<u64> {
        let key = self.keys.event_offset();
        match self.get(&key).await? {
            Some(val) => Ok(val.parse().unwrap_or(0)),
            None => Ok(0),
        }
    }

    async fn set_event_offset(&self, offset: u64) -> Result<()> {
        let key = self.keys.event_offset();
        self.set(&key, &offset.to_string()).await
    }
}

pub type DynRedisClient = Arc<dyn RedisClient>;
