//! Authentication service
//!
//! Handles BIP-322 authentication and session management using Redis.
//!
//! Session Security (BFF - Backend For Frontend):
//! - Sessions are stored in Redis with network-prefixed keys
//! - Session token format: session_id:session_secret
//! - Only SHA256(session_secret) is stored in Redis
//! - Sessions are set via secure HttpOnly cookies
//! - Old sessions are invalidated on re-login (prevents session fixation)

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

use crate::domain::{AuthResponse, Bip322AuthRequest, User, UserSession};
use crate::error::Result;
use crate::infra::{bip322, DynRedisClient};

/// Auth service configuration
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// Session TTL in seconds
    pub session_ttl_secs: i64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            session_ttl_secs: 86400, // 24 hours
        }
    }
}

/// Session data stored in Redis
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionData {
    pub session_id: String,
    pub btc_address: String,
    pub secret_hash: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl SessionData {
    fn to_user_session(&self) -> UserSession {
        UserSession {
            session_id: self.session_id.clone(),
            btc_address: self.btc_address.clone(),
            created_at: self.created_at,
            expires_at: self.expires_at,
        }
    }
}

/// Auth service using Redis for session storage
pub struct AuthService {
    redis: DynRedisClient,
    pool: sqlx::PgPool,
    config: AuthConfig,
}

/// Generate a cryptographically secure session secret
fn generate_session_secret() -> String {
    Uuid::new_v4().to_string()
}

/// Hash a session secret using SHA256
fn hash_secret(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

/// Parse a session token into (session_id, session_secret)
pub fn parse_session_token(token: &str) -> Option<(&str, &str)> {
    let parts: Vec<&str> = token.splitn(2, ':').collect();
    if parts.len() == 2 {
        Some((parts[0], parts[1]))
    } else {
        None
    }
}

/// Create a session token from session_id and session_secret
fn create_session_token(session_id: &str, session_secret: &str) -> String {
    format!("{}:{}", session_id, session_secret)
}

impl AuthService {
    pub fn new(redis: DynRedisClient, pool: sqlx::PgPool, config: AuthConfig) -> Self {
        Self { redis, pool, config }
    }

    /// Authenticate with BIP-322 signature
    pub async fn authenticate(&self, request: &Bip322AuthRequest) -> Result<AuthResponse> {
        let btc_address = &request.address;

        // Verify BIP-322 signature (timestamp and nonce are parsed from message)
        bip322::verify_bip322_signature(btc_address, &request.message, &request.signature)?;

        tracing::info!("Verified BIP-322 signature for address: {}", btc_address);

        // Find or create user in PostgreSQL (users table still needed for primary_name etc.)
        let (user, is_new_user) = self.find_or_create_user(btc_address).await?;

        // Invalidate old sessions in Redis (prevent session fixation)
        self.invalidate_user_sessions(&user.btc_address).await?;

        // Create new session in Redis
        let (session, session_token) = self.create_session(&user).await?;

        Ok(AuthResponse {
            session_id: session_token,
            btc_address: user.btc_address,
            expires_at: session.expires_at,
            is_new_user,
        })
    }

    /// Find existing user or create new one (in PostgreSQL)
    async fn find_or_create_user(&self, btc_address: &str) -> Result<(User, bool)> {
        let existing: Option<UserRow> = sqlx::query_as(
            r#"
            SELECT btc_address, primary_name, created_at, last_seen_at
            FROM users
            WHERE btc_address = $1
            "#,
        )
        .bind(btc_address)
        .fetch_optional(&self.pool)
        .await?;

        match existing {
            Some(row) => {
                let mut user: User = row.into();
                let now = Utc::now();
                sqlx::query(
                    r#"
                    UPDATE users
                    SET last_seen_at = $1
                    WHERE btc_address = $2
                    "#,
                )
                .bind(now)
                .bind(btc_address)
                .execute(&self.pool)
                .await?;

                user.last_seen_at = now;
                Ok((user, false))
            }
            None => {
                let now = Utc::now();
                let user = User {
                    btc_address: btc_address.to_string(),
                    primary_name: None,
                    created_at: now,
                    last_seen_at: now,
                };

                sqlx::query(
                    r#"
                    INSERT INTO users (btc_address, primary_name, created_at, last_seen_at)
                    VALUES ($1, $2, $3, $4)
                    "#,
                )
                .bind(&user.btc_address)
                .bind(&user.primary_name)
                .bind(user.created_at)
                .bind(user.last_seen_at)
                .execute(&self.pool)
                .await?;

                tracing::info!("Created new user: {}", btc_address);
                Ok((user, true))
            }
        }
    }

    /// Invalidate all sessions for a user in Redis (prevents session fixation attacks)
    async fn invalidate_user_sessions(&self, btc_address: &str) -> Result<u64> {
        let count = self.redis.delete_user_sessions(btc_address).await?;
        if count > 0 {
            tracing::info!(
                "Invalidated {} old session(s) for user {}",
                count,
                btc_address
            );
        }
        Ok(count)
    }

    /// Create a new session for user in Redis
    async fn create_session(&self, user: &User) -> Result<(UserSession, String)> {
        let now = Utc::now();
        let session_id = Uuid::new_v4().to_string();
        let session_secret = generate_session_secret();
        let secret_hash = hash_secret(&session_secret);

        let session_data = SessionData {
            session_id: session_id.clone(),
            btc_address: user.btc_address.clone(),
            secret_hash,
            created_at: now,
            expires_at: now + Duration::seconds(self.config.session_ttl_secs),
        };

        let session_json = serde_json::to_string(&session_data)?;
        let ttl_secs = self.config.session_ttl_secs as u64;

        // Store session in Redis
        self.redis
            .set_session(&session_id, &session_json, ttl_secs)
            .await?;

        // Add to user's session set (for bulk invalidation)
        self.redis
            .add_user_session(&user.btc_address, &session_id, ttl_secs)
            .await?;

        let full_token = create_session_token(&session_id, &session_secret);
        let user_session = session_data.to_user_session();

        tracing::info!(
            "Created session {} for user {}",
            session_id,
            user.btc_address
        );
        Ok((user_session, full_token))
    }

    /// Validate a session token and return the user info
    pub async fn validate_session(&self, token: &str) -> Result<Option<UserSession>> {
        let (session_id, session_secret) = match parse_session_token(token) {
            Some(parts) => parts,
            None => {
                tracing::warn!("Invalid session token format");
                return Ok(None);
            }
        };

        let secret_hash = hash_secret(session_secret);

        // Get session from Redis
        let session_json = match self.redis.get_session(session_id).await? {
            Some(json) => json,
            None => return Ok(None),
        };

        let session_data: SessionData = match serde_json::from_str(&session_json) {
            Ok(data) => data,
            Err(e) => {
                tracing::error!("Failed to parse session data: {}", e);
                return Ok(None);
            }
        };

        // Verify secret hash
        if session_data.secret_hash != secret_hash {
            tracing::warn!("Session secret mismatch for session {}", session_id);
            return Ok(None);
        }

        // Check expiration (Redis TTL should handle this, but double-check)
        if session_data.expires_at < Utc::now() {
            tracing::warn!("Session {} has expired", session_id);
            return Ok(None);
        }

        Ok(Some(session_data.to_user_session()))
    }

    /// Logout - invalidate session by token
    pub async fn logout(&self, token: &str) -> Result<()> {
        let (session_id, session_secret) = match parse_session_token(token) {
            Some(parts) => parts,
            None => {
                tracing::warn!("Invalid session token format for logout");
                return Ok(());
            }
        };

        // Verify the secret before deleting
        let secret_hash = hash_secret(session_secret);

        if let Some(session_json) = self.redis.get_session(session_id).await? {
            if let Ok(session_data) = serde_json::from_str::<SessionData>(&session_json) {
                if session_data.secret_hash == secret_hash {
                    self.redis.delete_session(session_id).await?;
                    tracing::info!("Logged out session {}", session_id);
                }
            }
        }

        Ok(())
    }

    /// Get session TTL in seconds (for cookie max-age)
    pub fn session_ttl_secs(&self) -> i64 {
        self.config.session_ttl_secs
    }
}

/// Database row for user
#[derive(Debug, sqlx::FromRow)]
struct UserRow {
    btc_address: String,
    primary_name: Option<String>,
    created_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        User {
            btc_address: row.btc_address,
            primary_name: row.primary_name,
            created_at: row.created_at,
            last_seen_at: row.last_seen_at,
        }
    }
}

pub type DynAuthService = Arc<AuthService>;
