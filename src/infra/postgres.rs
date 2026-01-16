//! PostgreSQL client for BNS Server
//!
//! Handles persistent storage:
//! - User data
//! - Listing records
//! - Name metadata

use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;

use crate::domain::{Listing, ListingStatus, NameMetadata, User};
use crate::error::Result;

/// PostgreSQL client abstraction
#[async_trait]
pub trait PostgresClient: Send + Sync {
    // User operations
    async fn get_user(&self, address: &str) -> Result<Option<User>>;
    async fn set_primary_name(&self, address: &str, name: &str) -> Result<()>;
    async fn clear_primary_name(&self, address: &str) -> Result<()>;

    // Name metadata operations
    async fn get_name_metadata(&self, name: &str) -> Result<Option<NameMetadata>>;
    async fn upsert_name_metadata(&self, metadata: &NameMetadata) -> Result<()>;

    // Listing operations
    async fn get_all_listings(&self, limit: u32, offset: u32) -> Result<(Vec<Listing>, i64)>;
    async fn create_listing(&self, listing: &Listing) -> Result<()>;
    async fn update_listing_status(&self, name: &str, status: ListingStatus) -> Result<()>;

    // System state operations (event polling)
    async fn get_event_offset(&self) -> Result<u64>;
    async fn set_event_offset(&self, offset: u64) -> Result<()>;

    // Pending transaction tracking (waiting for canister events)
    async fn add_pending_tx(&self, tx_id: &str, tracking_data: &str) -> Result<()>;
    async fn get_pending_txs(&self) -> Result<Vec<(String, String)>>;
    async fn remove_pending_tx(&self, tx_id: &str) -> Result<()>;
}

/// PostgreSQL client implementation
pub struct PostgresClientImpl {
    pool: PgPool,
}

impl PostgresClientImpl {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// Database row for listings table
#[derive(Debug, sqlx::FromRow)]
struct ListingRow {
    id: String,
    name: String,
    seller_address: String,
    pool_address: String,
    price_sats: i64,
    status: String,
    listed_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    previous_price_sats: Option<i64>,
    tx_id: Option<String>,
}

impl From<ListingRow> for Listing {
    fn from(row: ListingRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            seller_address: row.seller_address,
            pool_address: row.pool_address,
            price_sats: row.price_sats as u64,
            status: str_to_listing_status(&row.status),
            listed_at: row.listed_at,
            updated_at: row.updated_at,
            previous_price_sats: row.previous_price_sats.map(|p| p as u64),
            tx_id: row.tx_id,
        }
    }
}

fn str_to_listing_status(s: &str) -> ListingStatus {
    match s {
        "active" => ListingStatus::Active,
        "pending" => ListingStatus::Pending,
        "sold" => ListingStatus::Sold,
        "delisted" => ListingStatus::Delisted,
        "cancelled" => ListingStatus::Cancelled,
        _ => ListingStatus::Pending,
    }
}

fn listing_status_to_str(status: ListingStatus) -> &'static str {
    match status {
        ListingStatus::Active => "active",
        ListingStatus::Pending => "pending",
        ListingStatus::Sold => "sold",
        ListingStatus::Delisted => "delisted",
        ListingStatus::Cancelled => "cancelled",
    }
}

/// Database row for users table
#[derive(Debug, sqlx::FromRow)]
struct UserRow {
    btc_address: String,
    primary_name: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    last_seen_at: chrono::DateTime<chrono::Utc>,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        Self {
            btc_address: row.btc_address,
            primary_name: row.primary_name,
            created_at: row.created_at,
            last_seen_at: row.last_seen_at,
        }
    }
}

/// Database row for name_metadata table
#[derive(Debug, sqlx::FromRow)]
struct NameMetadataRow {
    name: String,
    owner_address: String,
    description: Option<String>,
    url: Option<String>,
    twitter: Option<String>,
    email: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<NameMetadataRow> for NameMetadata {
    fn from(row: NameMetadataRow) -> Self {
        Self {
            name: row.name,
            owner_address: row.owner_address,
            description: row.description,
            url: row.url,
            twitter: row.twitter,
            email: row.email,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[async_trait]
impl PostgresClient for PostgresClientImpl {
    async fn get_user(&self, address: &str) -> Result<Option<User>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT btc_address, primary_name, created_at, last_seen_at FROM users WHERE btc_address = $1",
        )
        .bind(address)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn set_primary_name(&self, address: &str, name: &str) -> Result<()> {
        sqlx::query(
            "UPDATE users SET primary_name = $1, last_seen_at = NOW() WHERE btc_address = $2",
        )
        .bind(name)
        .bind(address)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn clear_primary_name(&self, address: &str) -> Result<()> {
        sqlx::query(
            "UPDATE users SET primary_name = NULL, last_seen_at = NOW() WHERE btc_address = $1",
        )
        .bind(address)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_name_metadata(&self, name: &str) -> Result<Option<NameMetadata>> {
        let row = sqlx::query_as::<_, NameMetadataRow>(
            "SELECT name, owner_address, description, url, twitter, email, created_at, updated_at
             FROM name_metadata WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn upsert_name_metadata(&self, metadata: &NameMetadata) -> Result<()> {
        sqlx::query(
            "INSERT INTO name_metadata (name, owner_address, description, url, twitter, email, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (name) DO UPDATE SET
                owner_address = EXCLUDED.owner_address,
                description = EXCLUDED.description,
                url = EXCLUDED.url,
                twitter = EXCLUDED.twitter,
                email = EXCLUDED.email,
                updated_at = EXCLUDED.updated_at",
        )
        .bind(&metadata.name)
        .bind(&metadata.owner_address)
        .bind(&metadata.description)
        .bind(&metadata.url)
        .bind(&metadata.twitter)
        .bind(&metadata.email)
        .bind(metadata.created_at)
        .bind(metadata.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_all_listings(&self, limit: u32, offset: u32) -> Result<(Vec<Listing>, i64)> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM listings")
            .fetch_one(&self.pool)
            .await?;

        let rows = sqlx::query_as::<_, ListingRow>(
            "SELECT id, name, seller_address, pool_address, price_sats, status, listed_at, updated_at, previous_price_sats, tx_id
             FROM listings ORDER BY listed_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok((rows.into_iter().map(Into::into).collect(), count.0))
    }

    async fn create_listing(&self, listing: &Listing) -> Result<()> {
        sqlx::query(
            "INSERT INTO listings (id, name, seller_address, pool_address, price_sats, status, listed_at, updated_at, previous_price_sats, tx_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"
        )
        .bind(&listing.id)
        .bind(&listing.name)
        .bind(&listing.seller_address)
        .bind(&listing.pool_address)
        .bind(listing.price_sats as i64)
        .bind(listing_status_to_str(listing.status))
        .bind(listing.listed_at)
        .bind(listing.updated_at)
        .bind(listing.previous_price_sats.map(|p| p as i64))
        .bind(&listing.tx_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_listing_status(&self, name: &str, status: ListingStatus) -> Result<()> {
        sqlx::query("UPDATE listings SET status = $1, updated_at = NOW() WHERE name = $2")
            .bind(listing_status_to_str(status))
            .bind(name)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // System state operations

    async fn get_event_offset(&self) -> Result<u64> {
        let row: Option<(Option<i64>,)> =
            sqlx::query_as("SELECT value_int FROM system_state WHERE key = 'event_offset'")
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.and_then(|(v,)| v).unwrap_or(0) as u64)
    }

    async fn set_event_offset(&self, offset: u64) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO system_state (key, value_int, updated_at)
            VALUES ('event_offset', $1, NOW())
            ON CONFLICT (key) DO UPDATE SET value_int = $1, updated_at = NOW()
            "#,
        )
        .bind(offset as i64)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Pending transaction tracking

    async fn add_pending_tx(&self, tx_id: &str, tracking_data: &str) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO pending_txs (tx_id, tracking_data, created_at)
            VALUES ($1, $2::jsonb, NOW())
            ON CONFLICT (tx_id) DO UPDATE SET tracking_data = $2::jsonb
            "#,
        )
        .bind(tx_id)
        .bind(tracking_data)
        .execute(&self.pool)
        .await?;

        tracing::info!("Added tx_id {} to pending tx tracking", tx_id);
        Ok(())
    }

    async fn get_pending_txs(&self) -> Result<Vec<(String, String)>> {
        let rows: Vec<(String, serde_json::Value)> =
            sqlx::query_as("SELECT tx_id, tracking_data FROM pending_txs ORDER BY created_at")
                .fetch_all(&self.pool)
                .await?;

        Ok(rows
            .into_iter()
            .map(|(tx_id, data)| (tx_id, data.to_string()))
            .collect())
    }

    async fn remove_pending_tx(&self, tx_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM pending_txs WHERE tx_id = $1")
            .bind(tx_id)
            .execute(&self.pool)
            .await?;

        tracing::info!("Removed tx_id {} from pending tx tracking", tx_id);
        Ok(())
    }
}

pub type DynPostgresClient = Arc<dyn PostgresClient>;
