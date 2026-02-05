//! PostgreSQL client for BNS Server
//!
//! Handles persistent storage:
//! - User data
//! - Listing records
//! - Name metadata

use crate::AppError;
use crate::domain::{Listing, ListingStatus, NameMetadata, NftPoints, PendingTx, PendingTxAction, PendingTxStatus, ShoutOut, ShoutOutStatus, Star, StarTargetType, User};
use crate::error::Result;
use async_trait::async_trait;
use chrono::{TimeDelta, Utc};
use rust_decimal::prelude::ToPrimitive;
use sqlx::PgPool;
use sqlx::types::BigDecimal;
use std::ops::Sub;
use std::sync::Arc;

pub type ListingCount = u64;
pub type Valuation = u64;
pub type TxCount = u64;
pub type Volume = u64;
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
    async fn get_user_history(
        &self,
        user: &str,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<Listing>, i64)>;

    async fn get_name_history(
        &self,
        name: &str,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<Listing>, u64)>;

    async fn get_listed_listing_by_name(&self, name: &str) -> Result<Option<Listing>>;
    async fn get_listing_traded_count(&self, name: &str) -> Result<u64>;
    async fn get_top_earner(&self, user_address: &str) -> Result<(i64, u32)>;
    async fn create_listing(&self, listing: &Listing) -> Result<()>;
    async fn update_listing_status_buy_txid(&self, txid: &str, status: ListingStatus)
    -> Result<()>;
    async fn update_listing_status_by_id(&self, id: &str, status: ListingStatus) -> Result<()>;
    async fn update_listing_price(&self, id: &str, price: u64) -> Result<()>;
    async fn update_listing_to_bought_and_relisted(
        &self,
        id: &str,
        buyer_address: &str,
        new_price_sats: u64,
    ) -> Result<()>;
    async fn update_listing_to_bought_and_delisted(
        &self,
        id: &str,
        buyer_address: &str,
    ) -> Result<()>;
    async fn update_listing_to_relisted(&self, id: &str, new_price_sats: u64) -> Result<()>;
    async fn update_listing_to_list(&self, id: &str) -> Result<()>;
    // System state operations (event polling)
    async fn get_event_offset(&self) -> Result<u64>;
    async fn set_event_offset(&self, offset: u64) -> Result<()>;

    // Pending transaction tracking (waiting for canister events)
    async fn add_pending_tx(&self, pending_tx: &PendingTx) -> Result<()>;
    async fn get_pending_txs(&self) -> Result<Vec<PendingTx>>;
    async fn get_pending_tx_by_id(&self, tx_id: &str) -> Result<Option<PendingTx>>;
    async fn update_pending_tx_status(
        &self,
        tx_id: &str,
        status: PendingTxStatus,
    ) -> Result<Option<PendingTx>>;
    async fn get_last_bought_price(&self, name: &str) -> Result<Option<u64>>;
    // Name pool address cache
    async fn get_pool_address(&self, name: &str) -> Result<Option<String>>;
    async fn save_pool_address(&self, name: &str, pool_address: &str) -> Result<()>;

    // Inventory queries
    /// Get listed names and total value for a seller address
    async fn get_listed_names_for_seller(&self, seller_address: &str)
    -> Result<(Vec<String>, u64)>;
    /// Get names with pending delist transactions for a seller
    async fn get_pending_delist_names(&self, seller_address: &str) -> Result<Vec<String>>;
    /// Get names with pending buy_and_delist transactions for a buyer
    async fn get_pending_buy_and_delist_names(&self, buyer_address: &str) -> Result<Vec<String>>;
    async fn get_listing_count_and_valuation(&self) -> Result<(ListingCount, Valuation)>;
    async fn get_user_count(&self) -> Result<u64>;
    async fn get_24h_tx_vol(&self) -> Result<(TxCount, Volume)>;
    async fn star(&self, user: &str, name: &str, target_type: StarTargetType) -> Result<()>;
    async fn unstar(&self, user: &str, name: &str) -> Result<()>;
    async fn user_stars(&self, user: &str) -> Result<Vec<Star>>;
    async fn add_nft_points(&self, nft: &str, points: i64) -> Result<()>;
    async fn get_nft_points(&self, nft: &str) -> Result<Option<NftPoints>>;
    async fn insert_shout_out(&self, shout_out: &ShoutOut) -> Result<()>;
    async fn get_last_n_shout_out(&self, n: u64) -> Result<Vec<ShoutOut>>;
    async fn confirm_shout_out(&self, tx_id: &str) -> Result<()>;
    async fn get_pending_shout_out(&self) -> Result<Vec<ShoutOut>>;
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
pub struct ListingRow {
    pub id: String,
    pub name: String,
    pub seller_address: String,
    pub price_sats: i64,
    pub status: String,
    pub listed_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub previous_price_sats: Option<i64>,
    pub tx_id: String,
    pub buyer_address: Option<String>,
    pub new_price_sats: Option<i64>,
    pub inscription_utxo_sats: i64,
}

impl From<ListingRow> for Listing {
    fn from(row: ListingRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            seller_address: row.seller_address,
            price_sats: row.price_sats as u64,
            status: ListingStatus::from(row.status),
            listed_at: row.listed_at,
            updated_at: row.updated_at,
            previous_price_sats: row.previous_price_sats.map(|p| p as u64).unwrap_or(0),
            tx_id: row.tx_id,
            buyer_address: row.buyer_address,
            new_price_sats: row.new_price_sats.map(|p| p as u64),
            inscription_utxo_sats: row.inscription_utxo_sats as u64,
        }
    }
}

/// Database row for pending_txs table
#[derive(Debug, sqlx::FromRow)]
struct PendingTxRow {
    tx_id: String,
    created_at: chrono::DateTime<chrono::Utc>,
    name: String,
    action: String,
    status: String,
    previous_price_sats: Option<i64>,
    price_sats: Option<i64>,
    seller_address: Option<String>,
    buyer_address: Option<String>,
    inscription_utxo_sats: i64,
    platform_fee: Option<i64>,
}

impl From<PendingTxRow> for PendingTx {
    fn from(row: PendingTxRow) -> Self {
        Self {
            tx_id: row.tx_id,
            created_at: row.created_at,
            name: row.name,
            action: PendingTxAction::from(row.action),
            status: PendingTxStatus::from(row.status),
            previous_price_sats: row.previous_price_sats.map(|p| p as u64),
            price_sats: row.price_sats.map(|p| p as u64),
            seller_address: row.seller_address,
            buyer_address: row.buyer_address,
            inscription_utxo_sats: row.inscription_utxo_sats as u64,
            platform_fee: row.platform_fee.map(|p| p as u64),
        }
    }
}

#[async_trait]
impl PostgresClient for PostgresClientImpl {
    async fn get_user(&self, address: &str) -> Result<Option<User>> {
        let row = sqlx::query_as!(User,
            "SELECT btc_address, primary_name, created_at, last_seen_at FROM users WHERE btc_address = $1",
            address
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    async fn set_primary_name(&self, address: &str, name: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE users SET primary_name = $1, last_seen_at = NOW() WHERE btc_address = $2",
            name,
            address
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn clear_primary_name(&self, address: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE users SET primary_name = NULL, last_seen_at = NOW() WHERE btc_address = $1",
            address
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_name_metadata(&self, name: &str) -> Result<Option<NameMetadata>> {
        let row =
            sqlx::query_as!(NameMetadata,
            "SELECT name, owner_address, description, url, twitter, email, created_at, updated_at
             FROM name_metadata WHERE name = $1",
            name
        )
            .fetch_optional(&self.pool)
            .await?;

        Ok(row)
    }

    async fn upsert_name_metadata(&self, metadata: &NameMetadata) -> Result<()> {
        sqlx::query!(
            "INSERT INTO name_metadata (name, owner_address, description, url, twitter, email, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (name) DO UPDATE SET
                owner_address = EXCLUDED.owner_address,
                description = EXCLUDED.description,
                url = EXCLUDED.url,
                twitter = EXCLUDED.twitter,
                email = EXCLUDED.email,
                updated_at = EXCLUDED.updated_at",
            metadata.name,
            metadata.owner_address,
            metadata.description,
            metadata.url,
            metadata.twitter,
            metadata.email,
            metadata.created_at,
            metadata.updated_at
        ).execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_all_listings(&self, limit: u32, offset: u32) -> Result<(Vec<Listing>, i64)> {
        let count: i64 =
            sqlx::query_scalar!("SELECT COUNT(*) FROM listings WHERE status = 'listed'")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| AppError::Database(e))?
                .unwrap_or_default();

        let rows = sqlx::query_as!(ListingRow,
            "SELECT id, name, seller_address, price_sats, status, listed_at, updated_at, previous_price_sats, tx_id, buyer_address, new_price_sats, inscription_utxo_sats
             FROM listings WHERE status = 'listed' ORDER BY listed_at DESC LIMIT $1 OFFSET $2",
            limit as i64, offset as i64
        )
        .fetch_all(&self.pool)
        .await?;

        Ok((rows.into_iter().map(Into::into).collect(), count))
    }

    async fn get_user_history(
        &self,
        user: &str,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<Listing>, i64)> {
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM listings WHERE seller_address=$1 or buyer_address=$1",
            user
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::Database(e))?
        .unwrap_or_default();

        let rows = sqlx::query_as!(ListingRow,
            "SELECT id, name, seller_address, price_sats, status, listed_at, updated_at, previous_price_sats, tx_id, buyer_address, new_price_sats, inscription_utxo_sats
             FROM listings WHERE seller_address=$1 or buyer_address=$2 ORDER BY listed_at DESC LIMIT $3 OFFSET $4",
            user, user, limit as i64, offset as i64
        )
            .fetch_all(&self.pool)
            .await?;

        Ok((rows.into_iter().map(Into::into).collect(), count))
    }

    async fn get_name_history(
        &self,
        name: &str,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<Listing>, u64)> {
        let count = self.get_listing_traded_count(name).await?;

        let rows = sqlx::query_as!(ListingRow,
            "SELECT id, name, seller_address, price_sats, status, listed_at, updated_at, previous_price_sats, tx_id, buyer_address, new_price_sats, inscription_utxo_sats
             FROM listings WHERE name = $1 AND status IN ('bought_and_relisted', 'bought_and_delisted') ORDER BY listed_at DESC LIMIT $2 OFFSET $3",
            name, limit as i64, offset as i64
        )
            .fetch_all(&self.pool)
            .await?;

        Ok((rows.into_iter().map(Into::into).collect(), count))
    }

    async fn get_listed_listing_by_name(&self, name: &str) -> Result<Option<Listing>> {
        let row = sqlx::query_as!(ListingRow,
            "SELECT id, name, seller_address, price_sats, status, listed_at, updated_at, previous_price_sats, tx_id, buyer_address, new_price_sats, inscription_utxo_sats
             FROM listings WHERE name = $1 AND status = 'listed' ORDER BY listed_at DESC LIMIT 1",
            name
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn get_listing_traded_count(&self, name: &str) -> Result<u64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM listings WHERE name = $1 AND status IN ('bought_and_relisted', 'bought_and_delisted')",
        )
        .bind(name)
        .fetch_one(&self.pool)
        .await?;
        Ok(count as u64)
    }

    async fn get_top_earner(&self, user_address: &str) -> Result<(i64, u32)> {
        let lists = sqlx::query_as!(ListingRow,
            "SELECT id, name, seller_address, price_sats, status, listed_at, updated_at, previous_price_sats, tx_id, buyer_address, new_price_sats, inscription_utxo_sats
             FROM listings WHERE seller_address = $1 AND status IN ('bought_and_relisted', 'bought_and_delisted')",
                                                    user_address
        )
        .fetch_all(&self.pool)
        .await?;
        let mut total_earn = 0;
        let total_traded = lists.len() as u32;
        for l in lists {
            total_earn += l.price_sats - l.previous_price_sats.unwrap_or_default();
        }
        Ok((total_earn, total_traded))
    }

    async fn create_listing(&self, listing: &Listing) -> Result<()> {
        sqlx::query!(
            "INSERT INTO listings (id, name, seller_address, price_sats, status, listed_at, updated_at, previous_price_sats, tx_id, buyer_address, new_price_sats, inscription_utxo_sats)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
            listing.id,
            listing.name,
            listing.seller_address,
            listing.price_sats as i64,
            listing.status.to_string(),
            listing.listed_at,
            listing.updated_at,
            listing.previous_price_sats as i64,
            listing.tx_id,
            listing.buyer_address,
            listing.new_price_sats.map(|p| p as i64),
            listing.inscription_utxo_sats as i64,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_listing_status_buy_txid(
        &self,
        tx_id: &str,
        status: ListingStatus,
    ) -> Result<()> {
        sqlx::query!(
            "UPDATE listings SET status = $1, updated_at = NOW() WHERE tx_id = $2",
            status.to_string(),
            tx_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_listing_status_by_id(&self, id: &str, status: ListingStatus) -> Result<()> {
        sqlx::query!(
            "UPDATE listings SET status = $1, updated_at = NOW() WHERE id = $2",
            status.to_string(),
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_listing_price(&self, id: &str, price: u64) -> Result<()> {
        sqlx::query!(
            "UPDATE listings SET price_sats = $1, updated_at = NOW() WHERE id = $2",
            price as i64,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_listing_to_bought_and_relisted(
        &self,
        id: &str,
        buyer_address: &str,
        new_price_sats: u64,
    ) -> Result<()> {
        sqlx::query!(
            "UPDATE listings SET status = 'bought_and_relisted', buyer_address = $1, new_price_sats = $2, updated_at = NOW() WHERE id = $3",
            buyer_address,
            new_price_sats as i64,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_listing_to_bought_and_delisted(
        &self,
        id: &str,
        buyer_address: &str,
    ) -> Result<()> {
        sqlx::query!(
            "UPDATE listings SET status = 'bought_and_delisted', buyer_address = $1, updated_at = NOW() WHERE id = $2",
            buyer_address,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_listing_to_relisted(&self, id: &str, new_price_sats: u64) -> Result<()> {
        sqlx::query!(
            "UPDATE listings SET status = 'relisted', new_price_sats = $1, updated_at = NOW() WHERE id = $2",
            new_price_sats as i64,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_listing_to_list(&self, id: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE listings SET status = 'list', updated_at = NOW() WHERE id = $1",
            id
        )
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
        sqlx::query!(
            r#"
            INSERT INTO system_state (key, value_int, updated_at)
            VALUES ('event_offset', $1, NOW())
            ON CONFLICT (key) DO UPDATE SET value_int = $1, updated_at = NOW()
            "#,
            offset as i64
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Pending transaction tracking

    async fn add_pending_tx(&self, pending_tx: &PendingTx) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO pending_txs (tx_id, created_at, name, action, status, previous_price_sats, price_sats, seller_address, buyer_address, inscription_utxo_sats, platform_fee)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (tx_id) DO UPDATE SET
                name = EXCLUDED.name,
                action = EXCLUDED.action,
                status = EXCLUDED.status,
                previous_price_sats = EXCLUDED.previous_price_sats,
                price_sats = EXCLUDED.price_sats,
                seller_address = EXCLUDED.seller_address,
                buyer_address = EXCLUDED.buyer_address,
                inscription_utxo_sats = EXCLUDED.inscription_utxo_sats,
                platform_fee = EXCLUDED.platform_fee
            "#,
            pending_tx.tx_id,
            pending_tx.created_at,
            pending_tx.name,
            pending_tx.action.to_string(),
            pending_tx.status.to_string(),
            pending_tx.previous_price_sats.map(|p| p as i64),
            pending_tx.price_sats.map(|p| p as i64),
            pending_tx.seller_address,
            pending_tx.buyer_address,
            pending_tx.inscription_utxo_sats as i64,
            pending_tx.platform_fee.map(|p| p as i64)
        )
        .execute(&self.pool)
        .await?;

        tracing::info!("Added tx_id {} to pending tx tracking", pending_tx.tx_id);
        Ok(())
    }

    async fn get_pending_txs(&self) -> Result<Vec<PendingTx>> {
        let rows = sqlx::query_as!(PendingTxRow,
            "SELECT tx_id, created_at, name, action, status, previous_price_sats, price_sats, seller_address, buyer_address, inscription_utxo_sats, platform_fee FROM  pending_txs WHERE status IN ('submitted', 'pending') ORDER BY created_at"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn get_pending_tx_by_id(&self, tx_id: &str) -> Result<Option<PendingTx>> {
        let row = sqlx::query_as!(PendingTxRow,
            "SELECT tx_id, created_at, name, action, status, previous_price_sats, price_sats, seller_address, buyer_address, inscription_utxo_sats, platform_fee FROM pending_txs WHERE tx_id = $1",
            tx_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    async fn update_pending_tx_status(
        &self,
        tx_id: &str,
        status: PendingTxStatus,
    ) -> Result<Option<PendingTx>> {
        let row = sqlx::query_as!(crate::infra::postgres::PendingTxRow,
            r#"
            UPDATE pending_txs SET status = $1 WHERE tx_id = $2
            RETURNING tx_id, created_at, name, action, status, previous_price_sats, price_sats, seller_address, buyer_address, inscription_utxo_sats,platform_fee
            "#,
            status.to_string(), tx_id
        )
        .fetch_optional(&self.pool)
        .await?;

        if row.is_some() {
            tracing::info!("Updated pending tx {} status to {}", tx_id, status);
        }
        Ok(row.map(Into::into))
    }

    async fn get_last_bought_price(&self, name: &str) -> Result<Option<u64>> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT price_sats FROM listings WHERE name = $1 AND status IN ('bought_and_relisted', 'bought_and_delisted') ORDER BY updated_at DESC LIMIT 1"
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(p,)| p as u64))
    }

    // Name pool address cache

    async fn get_pool_address(&self, name: &str) -> Result<Option<String>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT pool_address FROM name_pools WHERE name = $1")
                .bind(name)
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.map(|(addr,)| addr))
    }

    async fn save_pool_address(&self, name: &str, pool_address: &str) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO name_pools (name, pool_address, created_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (name) DO UPDATE SET pool_address = $2
            "#,
            name,
            pool_address
        )
        .execute(&self.pool)
        .await?;

        tracing::debug!("Saved pool address {} for name {}", pool_address, name);
        Ok(())
    }

    // Inventory queries

    async fn get_listed_names_for_seller(
        &self,
        seller_address: &str,
    ) -> Result<(Vec<String>, u64)> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT name, price_sats FROM listings WHERE seller_address = $1 AND status = 'listed'",
        )
        .bind(seller_address)
        .fetch_all(&self.pool)
        .await?;

        let names: Vec<String> = rows.iter().map(|(name, _)| name.clone()).collect();
        let total_value: u64 = rows.iter().map(|(_, price)| *price as u64).sum();

        Ok((names, total_value))
    }

    async fn get_pending_delist_names(&self, seller_address: &str) -> Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pending_txs WHERE seller_address = $1 AND status = 'pending' AND action = 'delist'",
        )
        .bind(seller_address)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|(name,)| name).collect())
    }

    async fn get_pending_buy_and_delist_names(&self, buyer_address: &str) -> Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pending_txs WHERE buyer_address = $1 AND status = 'pending' AND action = 'buy_and_delist'",
        )
        .bind(buyer_address)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|(name,)| name).collect())
    }

    async fn get_listing_count_and_valuation(&self) -> Result<(ListingCount, Valuation)> {
        #[derive(sqlx::FromRow)]
        struct Cv {
            pub listing_count: Option<i64>,
            pub valuation: Option<BigDecimal>,
        }
        let temp = sqlx::query_as!(Cv,"SELECT count(*) as listing_count, sum(price_sats) as valuation FROM listings WHERE status = 'listed'").fetch_one(&self.pool).await?;
        Ok((
            temp.listing_count.unwrap_or_default() as u64,
            temp.valuation
                .unwrap_or_default()
                .to_u64()
                .unwrap_or_default(),
        ))
    }

    async fn get_user_count(&self) -> Result<u64> {
        let count = sqlx::query_scalar!("SELECT count(*) from users")
            .fetch_one(&self.pool)
            .await?;
        Ok(count.map(|c| c as u64).unwrap_or_default())
    }

    async fn get_24h_tx_vol(&self) -> Result<(TxCount, Volume)> {
        #[derive(sqlx::FromRow)]
        struct Tcv {
            pub tx_count: Option<i64>,
            pub volume: Option<BigDecimal>,
        }
        let now = Utc::now();
        let past_24h = now.sub(TimeDelta::days(1));
        let temp = sqlx::query_as!(Tcv,"SELECT count(*) as tx_count, sum(price_sats) as volume FROM listings WHERE status in (  'bought_and_relisted', 'bought_and_delisted') and updated_at >=$1", past_24h).fetch_one(&self.pool).await?;
        Ok((
            temp.tx_count.unwrap_or_default() as u64,
            temp.volume.unwrap_or_default().to_u64().unwrap_or_default(),
        ))
    }

    async fn star(&self, user: &str, target: &str, target_type: StarTargetType) -> Result<()> {
        sqlx::query!(
            "INSERT INTO stars (user_address, target, target_type)
             VALUES ($1, $2, $3)
             ON CONFLICT (user_address, target) DO UPDATE SET
                user_address = EXCLUDED.user_address,
                 target = EXCLUDED.target,
                 target_type = EXCLUDED.target_type",
            user,
            target,
            target_type.to_string(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn unstar(&self, user: &str, target: &str) -> Result<()> {
        sqlx::query!(
            "DELETE FROM stars WHERE user_address = $1 AND target = $2",
            user,
            target
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    async fn user_stars(&self, user: &str) -> Result<Vec<Star>> {
        let stars = sqlx::query_as!(Star, r#"SELECT id, user_address, target, target_type as "target_type:StarTargetType", created_at from stars where user_address = $1"#, user ).fetch_all(&self.pool).await?;
        Ok(stars)
    }

    async fn add_nft_points(&self, nft: &str, points: i64) -> Result<()> {
        sqlx::query!("INSERT INTO nft_points(name, points) VALUES ($1, $2) ON CONFLICT(name) DO UPDATE SET points = nft_points.points + $2",
            nft.to_string(), points
        ).execute(&self.pool).await?;
        Ok(())
    }
    async fn get_nft_points(&self, nft: &str) -> Result<Option<NftPoints>> {
        sqlx::query_as!(NftPoints, "select * from nft_points where name=$1", nft)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AppError::Database(e))
    }

    async fn insert_shout_out(&self, shout_out: &ShoutOut) -> Result<()> {
        sqlx::query!(
            "insert into shout_outs(tx_id, listing_name, user_address, ad_words, status, price) values ($1, $2, $3, $4, $5, $6)",
            shout_out.tx_id,
            shout_out.listing_name,
            shout_out.user_address,
            shout_out.ad_words,
            shout_out.status.to_string(),
            shout_out.price,
        ).execute(&self.pool).await?;
        Ok(())
    }

    async fn get_last_n_shout_out(&self, n: u64) -> Result<Vec<ShoutOut>> {
        let shout_outs = sqlx::query_as!(ShoutOut, "select * from shout_outs where status = $1 ORDER BY created_at desc  limit $2",
            ShoutOutStatus::Confirmed.to_string(),
            n as i64
        ).fetch_all(&self.pool).await?;
        Ok(shout_outs)
    }


    async fn confirm_shout_out(&self, tx_id: &str) -> Result<()> {
        let _ = sqlx::query!(
            "UPDATE shout_outs set status =$1 WHERE tx_id = $2",
            ShoutOutStatus::Confirmed.to_string(),
            tx_id
        ).execute(&self.pool).await?;
        Ok(())
    }

    async fn get_pending_shout_out(&self) -> Result<Vec<ShoutOut>> {
        let shout_outs = sqlx::query_as!(ShoutOut, "select * from shout_outs where status = $1 ORDER BY created_at desc  limit 100",
            ShoutOutStatus::Pending.to_string(),
        ).fetch_all(&self.pool).await?;
        Ok(shout_outs)
    }

}

pub type DynPostgresClient = Arc<dyn PostgresClient>;
