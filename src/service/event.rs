//! Event service
//!
//! Handles BNS canister event polling and transaction status updates.
//! Polls the canister for ReeActionStatusChanged events and updates
//! listings accordingly.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use uuid::Uuid;

use crate::domain::{Listing, ListingStatus};
use crate::infra::bns_canister::{BnsCanisterEvent, ReeActionStatus};
use crate::infra::{DynPostgresClient, DynRedisClient, IcAgent, ListingMeta};

/// Event service for canister event polling
pub struct EventService {
    ic_agent: Arc<IcAgent>,
    redis: DynRedisClient,
    postgres: DynPostgresClient,
    poll_interval: Duration,
}

impl EventService {
    pub fn new(ic_agent: Arc<IcAgent>, redis: DynRedisClient, postgres: DynPostgresClient) -> Self {
        Self {
            ic_agent,
            redis,
            postgres,
            poll_interval: Duration::from_secs(60),
        }
    }

    /// Start the background polling task
    ///
    /// This spawns an async task that polls the BNS canister for events
    /// and processes them. The task runs indefinitely.
    pub fn start_polling(self: Arc<Self>) {
        tokio::spawn(async move {
            self.polling_loop().await;
        });
    }

    /// Main polling loop
    ///
    /// Polls BNS canister get_events every minute and processes:
    /// - Pending: Save listing to PostgreSQL and Redis
    /// - Finalized: Update status to Active, remove from tracking
    /// - Confirmed: Informational logging only
    /// - Rejected: Remove from tracking
    async fn polling_loop(&self) {
        // Load last event offset from database (persisted across restarts)
        let mut last_event_offset = match self.postgres.get_event_offset().await {
            Ok(offset) => {
                tracing::info!("Loaded event offset from database: {}", offset);
                offset
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to load event offset from database, starting from 0: {:?}",
                    e
                );
                0
            }
        };

        tracing::info!(
            "Starting event polling (interval: {:?}, offset: {})",
            self.poll_interval,
            last_event_offset
        );

        loop {
            tokio::time::sleep(self.poll_interval).await;

            if let Some(new_offset) = self.poll_once(last_event_offset).await {
                if new_offset > last_event_offset {
                    last_event_offset = new_offset;
                    if let Err(e) = self.postgres.set_event_offset(last_event_offset).await {
                        tracing::error!("Failed to persist event offset to database: {:?}", e);
                    } else {
                        tracing::debug!("Persisted event offset: {}", last_event_offset);
                    }
                }
            }
        }
    }

    /// Poll once and process events
    ///
    /// Returns the new offset if events were processed, None on error.
    async fn poll_once(&self, last_event_offset: u64) -> Option<u64> {
        // Get pending tx_ids from database
        let pending_txs = match self.postgres.get_pending_txs().await {
            Ok(txs) => txs,
            Err(e) => {
                tracing::error!("Failed to get pending txs from database: {:?}", e);
                return None;
            }
        };

        if pending_txs.is_empty() {
            tracing::debug!("No pending transactions to track");
            return None;
        }

        tracing::debug!("Tracking {} pending transactions", pending_txs.len());

        // Poll events from BNS canister
        let events = match self.ic_agent.get_events(last_event_offset, 100).await {
            Ok(events) => events,
            Err(e) => {
                tracing::error!("Failed to poll get_events: {:?}", e);
                return None;
            }
        };

        if events.is_empty() {
            return None;
        }

        tracing::debug!("Got {} events from BNS canister", events.len());

        // Build a map of pending tx_ids for quick lookup
        let pending_map: HashMap<String, serde_json::Value> = pending_txs
            .into_iter()
            .filter_map(|(tx_id, data)| serde_json::from_str(&data).ok().map(|v| (tx_id, v)))
            .collect();

        let mut new_offset = last_event_offset;

        for (event_id, event) in events {
            // Update offset for next poll
            if let Ok(id) = event_id.parse::<u64>() {
                if id >= new_offset {
                    new_offset = id + 1;
                }
            }

            // Handle ReeActionStatusChanged events
            if let BnsCanisterEvent::ReeActionStatusChanged {
                action_id,
                status,
                timestamp_nanos: _,
            } = event
            {
                // Check if this action_id matches any pending tx_id
                if let Some(tracking_data) = pending_map.get(&action_id) {
                    self.handle_status_change(&action_id, status, tracking_data)
                        .await;
                }
            }
        }

        Some(new_offset)
    }

    /// Handle a status change event for a tracked transaction
    async fn handle_status_change(
        &self,
        action_id: &str,
        status: ReeActionStatus,
        tracking_data: &serde_json::Value,
    ) {
        tracing::info!(
            "Found status change for tracked tx_id {}: {:?}",
            action_id,
            status
        );

        match status {
            ReeActionStatus::Pending => {
                self.handle_pending(action_id, tracking_data).await;
            }
            ReeActionStatus::Finalized => {
                self.handle_finalized(action_id, tracking_data).await;
            }
            ReeActionStatus::Confirmed(confirmations) => {
                tracing::debug!(
                    "Tx {} confirmed with {} confirmations",
                    action_id,
                    confirmations
                );
            }
            ReeActionStatus::Rejected(reason) => {
                tracing::warn!("Tx {} rejected: {}", action_id, reason);
                let _ = self.postgres.remove_pending_tx(action_id).await;
            }
        }
    }

    /// Handle Pending status - save listing to PostgreSQL and Redis
    async fn handle_pending(&self, action_id: &str, tracking_data: &serde_json::Value) {
        let name = tracking_data["name"].as_str().unwrap_or("");
        let price = tracking_data["price"].as_u64().unwrap_or(0);
        let seller_address = tracking_data["seller_address"].as_str().unwrap_or("");
        let pool_address = tracking_data["pool_address"].as_str().unwrap_or("");

        let now = Utc::now();
        let listing = Listing {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            seller_address: seller_address.to_string(),
            pool_address: pool_address.to_string(),
            price_sats: price,
            status: ListingStatus::Pending,
            listed_at: now,
            updated_at: now,
            previous_price_sats: None,
            tx_id: Some(action_id.to_string()),
        };

        if let Err(e) = self.postgres.create_listing(&listing).await {
            tracing::error!("Failed to save listing for {}: {:?}", name, e);
        } else {
            tracing::info!(
                "Saved listing to PostgreSQL: name={}, tx_id={}",
                name,
                action_id
            );
        }

        // ZADD to Redis for new-listings ranking
        let meta = ListingMeta {
            name: name.to_string(),
            price_sats: price,
            seller_address: seller_address.to_string(),
            listed_at: now.timestamp(),
            tx_id: Some(action_id.to_string()),
        };

        if let Err(e) = self.redis.add_new_listing(&meta).await {
            tracing::error!("Failed to add listing {} to Redis ranking: {:?}", name, e);
        } else {
            tracing::info!("Added listing {} to Redis new-listings ranking", name);
        }
    }

    /// Handle Finalized status - update listing to Active and remove from tracking
    async fn handle_finalized(&self, action_id: &str, tracking_data: &serde_json::Value) {
        let name = tracking_data["name"].as_str().unwrap_or("");

        if let Err(e) = self
            .postgres
            .update_listing_status(name, ListingStatus::Active)
            .await
        {
            tracing::error!("Failed to update listing status for {}: {:?}", name, e);
        } else {
            tracing::info!("Tx {} finalized, listing {} is now active", action_id, name);
        }

        // Remove from tracking
        if let Err(e) = self.postgres.remove_pending_tx(action_id).await {
            tracing::error!(
                "Failed to remove tx_id {} from tracking: {:?}",
                action_id,
                e
            );
        }
    }
}

pub type DynEventService = Arc<EventService>;
