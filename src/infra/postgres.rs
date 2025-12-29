//! PostgreSQL client for BNS Server
//!
//! Handles persistent storage:
//! - Transaction history
//! - User data
//! - Event logs
//! - Listing records

use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;

use crate::domain::{
    CanisterEvent, Listing, ListingStatus, ShoutOut, TransactionStatus, TransactionType, User,
    UserTransaction,
};
use crate::error::Result;

/// PostgreSQL client abstraction
#[async_trait]
pub trait PostgresClient: Send + Sync {
    // User operations
    async fn get_user(&self, address: &str) -> Result<Option<User>>;
    async fn upsert_user(&self, user: &User) -> Result<()>;

    // Listing operations
    async fn get_listing(&self, name: &str) -> Result<Option<Listing>>;
    async fn get_listing_by_id(&self, id: &str) -> Result<Option<Listing>>;
    async fn get_listings_by_seller(&self, address: &str) -> Result<Vec<Listing>>;
    async fn get_active_listings(&self, limit: u32, offset: u32) -> Result<Vec<Listing>>;
    async fn get_all_listings(&self, limit: u32, offset: u32) -> Result<(Vec<Listing>, i64)>;
    async fn create_listing(&self, listing: &Listing) -> Result<()>;
    async fn update_listing_status(&self, name: &str, status: ListingStatus) -> Result<()>;
    async fn update_listing_price(&self, name: &str, price_sats: u64) -> Result<()>;
    async fn update_listing_confirmations(&self, id: &str, confirmations: i32) -> Result<()>;
    async fn get_pending_confirmations(&self, threshold: i32) -> Result<Vec<Listing>>;

    // Transaction history
    async fn get_user_transactions(
        &self,
        address: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<UserTransaction>>;
    async fn create_transaction(&self, tx: &UserTransaction, address: &str) -> Result<()>;
    async fn update_transaction_status(
        &self,
        tx_id: &str,
        status: TransactionStatus,
    ) -> Result<()>;

    // Event log
    async fn save_event(&self, event: &CanisterEvent) -> Result<()>;
    async fn get_last_processed_event_id(&self) -> Result<Option<String>>;

    // ShoutOut operations
    async fn create_shoutout(&self, shoutout: &ShoutOut) -> Result<()>;
    async fn get_active_shoutouts(&self) -> Result<Vec<ShoutOut>>;
    async fn expire_shoutouts(&self) -> Result<u64>;
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
    confirmations: i32,
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
            confirmations: row.confirmations,
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

#[async_trait]
impl PostgresClient for PostgresClientImpl {
    async fn get_user(&self, _address: &str) -> Result<Option<User>> {
        todo!("Implement get_user")
    }

    async fn upsert_user(&self, _user: &User) -> Result<()> {
        todo!("Implement upsert_user")
    }

    async fn get_listing(&self, name: &str) -> Result<Option<Listing>> {
        let row = sqlx::query_as::<_, ListingRow>(
            "SELECT id, name, seller_address, pool_address, price_sats, status, listed_at, updated_at, previous_price_sats, tx_id, confirmations
             FROM listings WHERE name = $1"
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn get_listing_by_id(&self, id: &str) -> Result<Option<Listing>> {
        let row = sqlx::query_as::<_, ListingRow>(
            "SELECT id, name, seller_address, pool_address, price_sats, status, listed_at, updated_at, previous_price_sats, tx_id, confirmations
             FROM listings WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn get_listings_by_seller(&self, address: &str) -> Result<Vec<Listing>> {
        let rows = sqlx::query_as::<_, ListingRow>(
            "SELECT id, name, seller_address, pool_address, price_sats, status, listed_at, updated_at, previous_price_sats, tx_id, confirmations
             FROM listings WHERE seller_address = $1 ORDER BY listed_at DESC"
        )
        .bind(address)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn get_active_listings(&self, limit: u32, offset: u32) -> Result<Vec<Listing>> {
        let rows = sqlx::query_as::<_, ListingRow>(
            "SELECT id, name, seller_address, pool_address, price_sats, status, listed_at, updated_at, previous_price_sats, tx_id, confirmations
             FROM listings WHERE status = 'active' ORDER BY listed_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn get_all_listings(&self, limit: u32, offset: u32) -> Result<(Vec<Listing>, i64)> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM listings")
            .fetch_one(&self.pool)
            .await?;

        let rows = sqlx::query_as::<_, ListingRow>(
            "SELECT id, name, seller_address, pool_address, price_sats, status, listed_at, updated_at, previous_price_sats, tx_id, confirmations
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
            "INSERT INTO listings (id, name, seller_address, pool_address, price_sats, status, listed_at, updated_at, previous_price_sats, tx_id, confirmations)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"
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
        .bind(listing.confirmations)
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

    async fn update_listing_price(&self, name: &str, price_sats: u64) -> Result<()> {
        sqlx::query(
            "UPDATE listings SET previous_price_sats = price_sats, price_sats = $1, updated_at = NOW() WHERE name = $2"
        )
        .bind(price_sats as i64)
        .bind(name)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_listing_confirmations(&self, id: &str, confirmations: i32) -> Result<()> {
        sqlx::query("UPDATE listings SET confirmations = $1, updated_at = NOW() WHERE id = $2")
            .bind(confirmations)
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_pending_confirmations(&self, threshold: i32) -> Result<Vec<Listing>> {
        let rows = sqlx::query_as::<_, ListingRow>(
            "SELECT id, name, seller_address, pool_address, price_sats, status, listed_at, updated_at, previous_price_sats, tx_id, confirmations
             FROM listings WHERE confirmations < $1 AND tx_id IS NOT NULL ORDER BY listed_at ASC"
        )
        .bind(threshold)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn get_user_transactions(
        &self,
        _address: &str,
        _limit: u32,
        _offset: u32,
    ) -> Result<Vec<UserTransaction>> {
        todo!("Implement get_user_transactions")
    }

    async fn create_transaction(&self, _tx: &UserTransaction, _address: &str) -> Result<()> {
        todo!("Implement create_transaction")
    }

    async fn update_transaction_status(
        &self,
        _tx_id: &str,
        _status: TransactionStatus,
    ) -> Result<()> {
        todo!("Implement update_transaction_status")
    }

    async fn save_event(&self, _event: &CanisterEvent) -> Result<()> {
        todo!("Implement save_event")
    }

    async fn get_last_processed_event_id(&self) -> Result<Option<String>> {
        todo!("Implement get_last_processed_event_id")
    }

    async fn create_shoutout(&self, _shoutout: &ShoutOut) -> Result<()> {
        todo!("Implement create_shoutout")
    }

    async fn get_active_shoutouts(&self) -> Result<Vec<ShoutOut>> {
        todo!("Implement get_active_shoutouts")
    }

    async fn expire_shoutouts(&self) -> Result<u64> {
        todo!("Implement expire_shoutouts")
    }
}

pub type DynPostgresClient = Arc<dyn PostgresClient>;
