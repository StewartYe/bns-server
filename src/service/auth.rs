//! Authentication service
//!
//! Handles BIP-322 authentication and session management.
//!
//! Session Security:
//! - Session token format: session_id:session_secret
//! - Only SHA256(session_secret) is stored in database
//! - Database administrators cannot impersonate users
//! - Old sessions are invalidated on re-login (prevents session fixation)

use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use std::sync::Arc;
use uuid::Uuid;

use crate::domain::{AuthResponse, Bip322AuthRequest, User, UserSession};
use crate::error::{AppError, Result};
use crate::infra::bip322;

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

/// Database row for user
#[derive(Debug, FromRow)]
struct UserRow {
    btc_address: String,
    created_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        User {
            btc_address: row.btc_address,
            created_at: row.created_at,
            last_seen_at: row.last_seen_at,
        }
    }
}

/// Database row for session
#[derive(Debug, FromRow)]
struct SessionRow {
    session_id: String,
    btc_address: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

impl From<SessionRow> for UserSession {
    fn from(row: SessionRow) -> Self {
        UserSession {
            session_id: row.session_id,
            btc_address: row.btc_address,
            created_at: row.created_at,
            expires_at: row.expires_at,
        }
    }
}

/// Auth service
pub struct AuthService {
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
    pub fn new(pool: sqlx::PgPool, config: AuthConfig) -> Self {
        Self { pool, config }
    }

    /// Authenticate with BIP-322 signature
    pub async fn authenticate(&self, request: &Bip322AuthRequest) -> Result<AuthResponse> {
        let btc_address = &request.address;

        // Verify BIP-322 signature
        bip322::verify_bip322_signature(
            btc_address,
            &request.message,
            &request.signature,
            request.timestamp,
            &request.nonce,
        )?;

        tracing::info!("Verified BIP-322 signature for address: {}", btc_address);

        // Find or create user
        let (user, is_new_user) = self.find_or_create_user(btc_address).await?;

        // Invalidate old sessions (prevent session fixation)
        self.invalidate_user_sessions(&user.btc_address).await?;

        // Create new session
        let (session, session_token) = self.create_session(&user).await?;

        Ok(AuthResponse {
            session_id: session_token,
            btc_address: user.btc_address,
            expires_at: session.expires_at,
            is_new_user,
        })
    }

    /// Find existing user or create new one
    async fn find_or_create_user(&self, btc_address: &str) -> Result<(User, bool)> {
        let existing: Option<UserRow> = sqlx::query_as(
            r#"
            SELECT btc_address, created_at, last_seen_at
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
                    created_at: now,
                    last_seen_at: now,
                };

                sqlx::query(
                    r#"
                    INSERT INTO users (btc_address, created_at, last_seen_at)
                    VALUES ($1, $2, $3)
                    "#,
                )
                .bind(&user.btc_address)
                .bind(user.created_at)
                .bind(user.last_seen_at)
                .execute(&self.pool)
                .await?;

                tracing::info!("Created new user: {}", btc_address);
                Ok((user, true))
            }
        }
    }

    /// Invalidate all sessions for a user (prevents session fixation attacks)
    async fn invalidate_user_sessions(&self, btc_address: &str) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM sessions
            WHERE btc_address = $1
            "#,
        )
        .bind(btc_address)
        .execute(&self.pool)
        .await?;

        let count = result.rows_affected();
        if count > 0 {
            tracing::info!(
                "Invalidated {} old session(s) for user {}",
                count,
                btc_address
            );
        }
        Ok(count)
    }

    /// Create a new session for user
    async fn create_session(&self, user: &User) -> Result<(UserSession, String)> {
        let now = Utc::now();
        let session_id = Uuid::new_v4().to_string();
        let session_secret = generate_session_secret();
        let secret_hash = hash_secret(&session_secret);

        let session = UserSession {
            session_id: session_id.clone(),
            btc_address: user.btc_address.clone(),
            created_at: now,
            expires_at: now + Duration::seconds(self.config.session_ttl_secs),
        };

        sqlx::query(
            r#"
            INSERT INTO sessions (session_id, btc_address, created_at, expires_at, secret_hash)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(&session.session_id)
        .bind(&session.btc_address)
        .bind(session.created_at)
        .bind(session.expires_at)
        .bind(&secret_hash)
        .execute(&self.pool)
        .await?;

        let full_token = create_session_token(&session_id, &session_secret);

        tracing::info!(
            "Created session {} for user {}",
            session.session_id,
            user.btc_address
        );
        Ok((session, full_token))
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

        let session: Option<SessionRow> = sqlx::query_as(
            r#"
            SELECT session_id, btc_address, created_at, expires_at
            FROM sessions
            WHERE session_id = $1 AND secret_hash = $2 AND expires_at > NOW()
            "#,
        )
        .bind(session_id)
        .bind(&secret_hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(session.map(|row| row.into()))
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

        let secret_hash = hash_secret(session_secret);

        sqlx::query(
            r#"
            DELETE FROM sessions
            WHERE session_id = $1 AND secret_hash = $2
            "#,
        )
        .bind(session_id)
        .bind(&secret_hash)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM sessions
            WHERE expires_at < NOW()
            "#,
        )
        .execute(&self.pool)
        .await?;

        let count = result.rows_affected();
        if count > 0 {
            tracing::info!("Cleaned up {} expired sessions", count);
        }
        Ok(count)
    }
}

pub type DynAuthService = Arc<AuthService>;
