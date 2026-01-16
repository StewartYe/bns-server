//! Redis/Valkey client for BNS Server
//!
//! Implements connection to Google Cloud Memorystore Valkey with:
//! - TLS encryption using custom CA certificate
//! - IAM authentication with automatic token refresh
//!
//! Based on: https://docs.cloud.google.com/memorystore/docs/cluster/client-library-connection#go

use async_trait::async_trait;
use redis::aio::{MultiplexedConnection, PubSub};
use redis::{AsyncCommands, Client, TlsCertificates};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::config::{Network, RedisConfig};
use crate::error::Result;

/// Token refresh interval (5 minutes before checking, like Go example)
const TOKEN_REFRESH_DURATION: Duration = Duration::from_secs(5 * 60);

/// Token lifetime (1 hour)
const TOKEN_LIFETIME: Duration = Duration::from_secs(60 * 60);

/// Check token expiry interval (10 seconds, like Go example)
const CHECK_TOKEN_EXPIRY_INTERVAL: Duration = Duration::from_secs(10);

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

    // ===========================================
    // Name-based Rankings (ZSet, member=name, 5 types)
    // Metadata stored in rank:meta:{name}
    // ===========================================

    /// New listings: score = listed_at (unix timestamp), descending = newest first
    pub fn rank_new_listings(&self) -> String {
        self.key("rank:new_listings")
    }

    /// Recent sales: score = sold_at (unix timestamp), descending = most recent first
    pub fn rank_recent_sales(&self) -> String {
        self.key("rank:recent_sales")
    }

    /// Most traded: score = trade_count, descending = most trades first
    pub fn rank_most_traded(&self) -> String {
        self.key("rank:most_traded")
    }

    /// Top sales: score = sale_price_sats, descending = highest price first
    pub fn rank_top_sales(&self) -> String {
        self.key("rank:top_sales")
    }

    /// Best deals: score = discount_pct, descending = best discount first
    pub fn rank_best_deals(&self) -> String {
        self.key("rank:best_deals")
    }

    // ===========================================
    // Earner-based Ranking (ZSet, member=btc_address, 1 type)
    // Metadata stored in rank:earner_meta:{btc_address}
    // ===========================================

    /// Top earners: score = total_earnings_sats, descending = highest earnings first
    pub fn rank_top_earners(&self) -> String {
        self.key("rank:top_earners")
    }

    // Metadata for name-based rankings (5 types: new_listings, recent_sales, most_traded, top_sales, best_deals)
    pub fn rank_meta(&self, name: &str) -> String {
        self.key(&format!("rank:meta:{}", name))
    }

    // Metadata for earner-based ranking (top_earners, member is btc_address)
    pub fn earner_meta(&self, btc_address: &str) -> String {
        self.key(&format!("rank:earner_meta:{}", btc_address))
    }

    // Pub/Sub channels (6 types, corresponding to rankings)
    pub fn channel_new_listings(&self) -> String {
        self.key("channel:new_listings")
    }

    pub fn channel_recent_sales(&self) -> String {
        self.key("channel:recent_sales")
    }

    pub fn channel_top_earners(&self) -> String {
        self.key("channel:top_earners")
    }

    pub fn channel_most_traded(&self) -> String {
        self.key("channel:most_traded")
    }

    pub fn channel_top_sales(&self) -> String {
        self.key("channel:top_sales")
    }

    pub fn channel_best_deals(&self) -> String {
        self.key("channel:best_deals")
    }

    // Sessions
    pub fn session(&self, session_id: &str) -> String {
        self.key(&format!("session:{}", session_id))
    }

    pub fn user_sessions(&self, btc_address: &str) -> String {
        self.key(&format!("user_sessions:{}", btc_address))
    }
}

/// Metadata for name-based rankings (stored in rank:meta:{name})
/// Used by: new_listings, recent_sales, most_traded, top_sales, best_deals
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NameRankingMeta {
    pub name: String,
    /// Current listing price (for new_listings)
    #[serde(default)]
    pub list_price_sats: Option<u64>,
    /// Last sale price (for recent_sales, top_sales, best_deals)
    #[serde(default)]
    pub sale_price_sats: Option<u64>,
    /// Seller address
    #[serde(default)]
    pub seller_address: Option<String>,
    /// Buyer address (for sales)
    #[serde(default)]
    pub buyer_address: Option<String>,
    /// Listed timestamp (for new_listings)
    #[serde(default)]
    pub listed_at: Option<i64>,
    /// Sold timestamp (for recent_sales)
    #[serde(default)]
    pub sold_at: Option<i64>,
    /// Trade count (for most_traded)
    #[serde(default)]
    pub trade_count: Option<u32>,
    /// Discount percentage (for best_deals, e.g., 15.5 means 15.5% off)
    #[serde(default)]
    pub discount_pct: Option<f64>,
    /// Transaction ID
    #[serde(default)]
    pub tx_id: Option<String>,
}

/// Metadata for top earners ranking (stored in rank:earner_meta:{btc_address})
/// Member is btc_address, score is total_earnings_sats
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EarnerMeta {
    pub btc_address: String,
    /// Total earnings in satoshis
    pub total_earnings_sats: u64,
    /// Number of sales
    pub sale_count: u32,
    /// Last sale timestamp
    #[serde(default)]
    pub last_sale_at: Option<i64>,
}

/// Legacy: Listing info for pending confirmations
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ListingMeta {
    pub name: String,
    pub price_sats: u64,
    pub seller_address: String,
    pub listed_at: i64,
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
    async fn zrange_with_scores(
        &self,
        key: &str,
        start: isize,
        stop: isize,
    ) -> Result<Vec<(String, f64)>>;
    async fn zrevrange_with_scores(
        &self,
        key: &str,
        start: isize,
        stop: isize,
    ) -> Result<Vec<(String, f64)>>;
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

    // Session operations

    /// Store a session with TTL
    async fn set_session(&self, session_id: &str, session_json: &str, ttl_secs: u64) -> Result<()>;

    /// Get a session by ID
    async fn get_session(&self, session_id: &str) -> Result<Option<String>>;

    /// Delete a session
    async fn delete_session(&self, session_id: &str) -> Result<()>;

    /// Add session ID to user's session set (for invalidating all user sessions)
    async fn add_user_session(
        &self,
        btc_address: &str,
        session_id: &str,
        ttl_secs: u64,
    ) -> Result<()>;

    /// Get all session IDs for a user
    async fn get_user_sessions(&self, btc_address: &str) -> Result<Vec<String>>;

    /// Delete all sessions for a user
    async fn delete_user_sessions(&self, btc_address: &str) -> Result<u64>;

    /// Get a pub/sub connection for subscribing to channels
    /// Returns a stream of messages
    async fn get_pubsub(&self) -> Result<PubSub>;
}

/// Token cache for IAM authentication
struct TokenCache {
    token: String,
    last_refresh_instant: Instant,
    last_error: Option<String>,
}

/// Redis client implementation with TLS and IAM authentication
///
/// Follows the Go example from Google Cloud documentation:
/// - Loads CA certificate for TLS
/// - Background token refresh loop
/// - Uses "default" username with access token as password
pub struct RedisClientImpl {
    /// Redis client with TLS configured
    client: Client,
    /// Key builder for network-prefixed keys
    keys: KeyBuilder,
    /// Whether IAM auth is enabled
    use_iam: bool,
    /// Token cache (shared with background refresh task)
    token_cache: Arc<RwLock<Option<TokenCache>>>,
}

impl RedisClientImpl {
    /// Create a new Redis client following the Go example pattern
    pub async fn new(config: &RedisConfig, network: Network) -> Result<Self> {
        tracing::info!(
            "Connecting to Valkey at {}:{} (TLS: {}, IAM: {})",
            config.host,
            config.port,
            config.tls,
            config.use_iam
        );

        // Build connection URL
        let scheme = if config.tls { "rediss" } else { "redis" };
        let url = format!("{}://{}:{}", scheme, config.host, config.port);

        // Create client with TLS if enabled
        let client = if config.tls {
            // Load CA certificate from file (like Go example: caCert, err := ioutil.ReadFile(caFilePath))
            let ca_cert = if let Some(ref ca_path) = config.ca_file_path {
                tracing::info!("Loading CA certificate from: {}", ca_path);
                let cert_data = tokio::fs::read(ca_path).await.map_err(|e| {
                    crate::error::AppError::Internal(format!(
                        "Failed to read CA certificate from {}: {}",
                        ca_path, e
                    ))
                })?;
                Some(cert_data)
            } else {
                tracing::warn!("No CA certificate file configured, using system root certificates");
                None
            };

            // Build client with TLS (like Go: caCertPool.AppendCertsFromPEM(caCert))
            let tls_certs = TlsCertificates {
                client_tls: None, // No client certificate (mTLS not required)
                root_cert: ca_cert,
            };

            Client::build_with_tls(url.as_str(), tls_certs).map_err(|e| {
                crate::error::AppError::Internal(format!(
                    "Failed to create TLS Redis client: {}",
                    e
                ))
            })?
        } else {
            Client::open(url.as_str())?
        };

        let token_cache = Arc::new(RwLock::new(None));

        // If using IAM, get initial token (like Go: token, err = retrieveToken())
        if config.use_iam {
            let token = Self::retrieve_token().await?;
            let cache = TokenCache {
                token,
                last_refresh_instant: Instant::now(),
                last_error: None,
            };
            *token_cache.write().await = Some(cache);
            tracing::info!("Initial IAM token retrieved");

            // Start background token refresh loop (like Go: go refreshTokenLoop())
            let cache_clone = token_cache.clone();
            tokio::spawn(async move {
                Self::refresh_token_loop(cache_clone).await;
            });
        }

        let redis_client = Self {
            client,
            keys: KeyBuilder::new(network),
            use_iam: config.use_iam,
            token_cache,
        };

        // Test connection (like Go: err = client.Set(ctx, "key", "value", 0).Err())
        let mut conn = redis_client.get_connection().await?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        tracing::info!("Connected to Valkey successfully (PING OK)");

        Ok(redis_client)
    }

    /// Background token refresh loop (following Go: func refreshTokenLoop())
    async fn refresh_token_loop(token_cache: Arc<RwLock<Option<TokenCache>>>) {
        tracing::info!("Starting token refresh loop");

        loop {
            tokio::time::sleep(CHECK_TOKEN_EXPIRY_INTERVAL).await;

            let should_refresh = {
                let cache = token_cache.read().await;
                match &*cache {
                    Some(c) => c.last_refresh_instant.elapsed() >= TOKEN_REFRESH_DURATION,
                    None => true,
                }
            };

            if should_refresh {
                tracing::debug!("Refreshing IAM access token...");
                match Self::retrieve_token().await {
                    Ok(new_token) => {
                        let mut cache = token_cache.write().await;
                        *cache = Some(TokenCache {
                            token: new_token,
                            last_refresh_instant: Instant::now(),
                            last_error: None,
                        });
                        tracing::debug!("IAM access token refreshed successfully");
                    }
                    Err(e) => {
                        let mut cache = token_cache.write().await;
                        if let Some(ref mut c) = *cache {
                            c.last_error = Some(e.to_string());
                        }
                        tracing::error!("Failed to refresh IAM access token: {}", e);
                    }
                }
            }
        }
    }

    /// Retrieve token from GCP metadata server
    /// (Go equivalent: func retrieveToken() (string, error))
    async fn retrieve_token() -> Result<String> {
        let client = reqwest::Client::new();
        let response = client
            .get("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token")
            .header("Metadata-Flavor", "Google")
            .send()
            .await
            .map_err(|e| {
                crate::error::AppError::Internal(format!("Failed to get GCP token: {}", e))
            })?;

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

    /// Get credentials (username, password) following Go: func retrieveTokenFunc() (string, string)
    async fn retrieve_credentials(&self) -> Option<(String, String)> {
        let cache = self.token_cache.read().await;
        match &*cache {
            Some(c) => {
                // Check if token is expired (like Go: time.Now().After(lastRefreshInstant.Add(*refreshDuration)))
                if c.last_refresh_instant.elapsed() >= TOKEN_LIFETIME {
                    tracing::warn!(
                        "Token is expired. Last refresh: {:?}, error: {:?}",
                        c.last_refresh_instant.elapsed(),
                        c.last_error
                    );
                    return None;
                }
                // username := "default", password := token
                Some(("default".to_string(), c.token.clone()))
            }
            None => None,
        }
    }

    /// Get authenticated connection
    async fn get_connection(&self) -> Result<MultiplexedConnection> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;

        // If using IAM, authenticate with access token
        // (Go equivalent: using CredentialsProvider which returns default + token)
        if self.use_iam {
            let (username, password) = self.retrieve_credentials().await.ok_or_else(|| {
                crate::error::AppError::Internal("IAM token not available or expired".to_string())
            })?;

            let auth_result: std::result::Result<String, redis::RedisError> = redis::cmd("AUTH")
                .arg(&username)
                .arg(&password)
                .query_async(&mut conn)
                .await;

            if let Err(e) = auth_result {
                tracing::error!("Valkey IAM authentication failed: {:?}", e);
                return Err(crate::error::AppError::Redis(e));
            }
        }

        Ok(conn)
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

    async fn zrange_with_scores(
        &self,
        key: &str,
        start: isize,
        stop: isize,
    ) -> Result<Vec<(String, f64)>> {
        let mut conn = self.get_connection().await?;
        Ok(conn.zrange_withscores(key, start, stop).await?)
    }

    async fn zrevrange_with_scores(
        &self,
        key: &str,
        start: isize,
        stop: isize,
    ) -> Result<Vec<(String, f64)>> {
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
        let key = self.keys.rank_new_listings();
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
        let channel = self.keys.channel_new_listings();
        let event = serde_json::json!({
            "type": "new_listing",
            "data": meta
        });
        self.publish(&channel, &event.to_string()).await?;

        Ok(())
    }

    async fn get_new_listings(&self, count: usize) -> Result<Vec<ListingMeta>> {
        let key = self.keys.rank_new_listings();

        // Get top N newest (highest scores = most recent)
        let entries = self
            .zrevrange_with_scores(&key, 0, (count - 1) as isize)
            .await?;

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

    async fn add_user_session(
        &self,
        btc_address: &str,
        session_id: &str,
        ttl_secs: u64,
    ) -> Result<()> {
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

    async fn get_pubsub(&self) -> Result<PubSub> {
        // Note: PubSub connections in redis-rs don't support the same AUTH flow
        // as regular connections. For Google Cloud Memorystore with IAM auth,
        // the pub/sub functionality relies on VPC-level security.
        // If IAM auth is required for pub/sub, consider using a different approach
        // such as embedding credentials in the connection URL.
        let pubsub = self.client.get_async_pubsub().await?;
        Ok(pubsub)
    }
}

pub type DynRedisClient = Arc<dyn RedisClient>;
