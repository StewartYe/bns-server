//! Listing service
//!
//! Handles list_name operations with PSBT broadcasting and confirmation tracking.

use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use crate::domain::{
    Listing, ListingInfo, ListingStatus, ListNameRequest, ListNameResponse, ListedNamesResponse,
    FINALIZE_THRESHOLD,
};
use crate::error::{AppError, Result};
use crate::infra::{DynBlockchainClient, DynPostgresClient};

/// Listing service
pub struct ListingService {
    blockchain: DynBlockchainClient,
    postgres: DynPostgresClient,
}

impl ListingService {
    pub fn new(blockchain: DynBlockchainClient, postgres: DynPostgresClient) -> Self {
        Self {
            blockchain,
            postgres,
        }
    }

    /// Check if a string looks like a Bitcoin txid (64 hex characters)
    fn is_txid(s: &str) -> bool {
        s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
    }

    /// List a name for sale
    ///
    /// Flow:
    /// 1. Broadcast the signed PSBT (or use txid if already broadcast)
    /// 2. Store listing in database with confirmation=0
    /// 3. Return listing info with tx_id
    pub async fn list_name(&self, request: &ListNameRequest) -> Result<ListNameResponse> {
        // Check if psbt is actually a txid (64 hex chars = already broadcast by wallet)
        let tx_id = if Self::is_txid(&request.psbt) {
            tracing::info!("Received txid instead of PSBT, using directly: {}", request.psbt);
            request.psbt.clone()
        } else {
            // Broadcast the PSBT
            self.blockchain.broadcast_psbt(&request.psbt).await?
        };

        let now = Utc::now();
        let listing_id = Uuid::new_v4().to_string();

        // Create listing record
        let listing = Listing {
            id: listing_id.clone(),
            name: request.name.clone(),
            seller_address: request.seller_address.clone(),
            pool_address: String::new(), // Will be set by canister later
            price_sats: request.price_sats,
            status: ListingStatus::Pending,
            listed_at: now,
            updated_at: now,
            previous_price_sats: None,
            tx_id: Some(tx_id.clone()),
            confirmations: 0,
        };

        self.postgres.create_listing(&listing).await?;

        tracing::info!(
            "Created listing {} for name '{}' with tx_id {}",
            listing_id,
            request.name,
            tx_id
        );

        Ok(ListNameResponse {
            id: listing_id,
            tx_id,
            name: request.name.clone(),
            price_sats: request.price_sats,
            seller_address: request.seller_address.clone(),
            confirmations: 0,
        })
    }

    /// Get all listed names
    pub async fn get_listed_names(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<ListedNamesResponse> {
        let limit = limit.unwrap_or(50).min(100);
        let offset = offset.unwrap_or(0);

        let (listings, total) = self.postgres.get_all_listings(limit, offset).await?;

        let listing_infos: Vec<ListingInfo> = listings
            .into_iter()
            .map(|l| ListingInfo {
                id: l.id,
                name: l.name,
                seller_address: l.seller_address,
                pool_address: l.pool_address,
                price_sats: l.price_sats,
                status: l.status,
                listed_at: l.listed_at,
                tx_id: l.tx_id,
                confirmations: l.confirmations,
            })
            .collect();

        Ok(ListedNamesResponse {
            listings: listing_infos,
            total,
        })
    }

    /// Update confirmations for pending listings
    ///
    /// Called by background task every minute.
    /// When confirmations >= FINALIZE_THRESHOLD, call bns_canister.list_name (placeholder).
    pub async fn update_confirmations(&self) -> Result<u32> {
        let pending = self
            .postgres
            .get_pending_confirmations(FINALIZE_THRESHOLD)
            .await?;

        let mut updated_count = 0u32;

        for listing in pending {
            if let Some(tx_id) = &listing.tx_id {
                match self.blockchain.get_transaction_confirmations(tx_id).await {
                    Ok(Some(confirmations)) => {
                        let confirmations = confirmations as i32;

                        // Update confirmations in database
                        self.postgres
                            .update_listing_confirmations(&listing.id, confirmations)
                            .await?;

                        updated_count += 1;

                        // Check if we've reached the threshold
                        if confirmations >= FINALIZE_THRESHOLD
                            && listing.confirmations < FINALIZE_THRESHOLD
                        {
                            tracing::info!(
                                "Listing {} reached {} confirmations, calling bns_canister.list_name",
                                listing.id,
                                confirmations
                            );

                            // TODO: Call bns_canister.list_name here
                            // For now, just update status to active
                            self.postgres
                                .update_listing_status(&listing.name, ListingStatus::Active)
                                .await?;
                        }
                    }
                    Ok(None) => {
                        tracing::debug!(
                            "Transaction {} not found for listing {}",
                            tx_id,
                            listing.id
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to get confirmations for tx {}: {:?}",
                            tx_id,
                            e
                        );
                    }
                }
            }
        }

        Ok(updated_count)
    }
}

pub type DynListingService = Arc<ListingService>;
