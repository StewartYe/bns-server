//! Event service
//!
//! Handles BNS canister event polling and transaction status updates.
//! Polls the canister for ReeActionStatusChanged events and updates
//! listings accordingly.

use std::sync::Arc;
use std::time::Duration;

use crate::api::rankings::{
    BestDealItem, MostTradedItem, NewListingItem, RecentSaleItem, TopEarnerItem, TopSaleItem,
};
use crate::domain::{Listing, ListingStatus, PendingTx, PendingTxAction, PendingTxStatus};
use crate::infra::bns_canister::{BnsCanisterEvent, ReeActionStatus};
use crate::infra::{DynBlockchainClient, DynPostgresClient, DynRedisClient, IcAgent};
use crate::state::BroadcastEvent;
use crate::{GLOBAL_MIN_PRICE, INIT_MAX_PRICE};
use chrono::Utc;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Event service for canister event polling
#[derive(Clone)]
pub struct EventService {
    ic_agent: Arc<IcAgent>,
    redis: DynRedisClient,
    postgres: DynPostgresClient,
    blockchain: DynBlockchainClient,
    poll_interval: Duration,
    broadcast_tx: broadcast::Sender<BroadcastEvent>,
}

impl EventService {
    pub fn new(
        ic_agent: Arc<IcAgent>,
        redis: DynRedisClient,
        postgres: DynPostgresClient,
        blockchain: DynBlockchainClient,
        broadcast_tx: broadcast::Sender<BroadcastEvent>,
    ) -> Self {
        Self {
            ic_agent,
            redis,
            postgres,
            blockchain,
            poll_interval: Duration::from_secs(60),
            broadcast_tx,
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
        tracing::debug!(
            "Starting poll_once, last_event_offset: {}",
            last_event_offset
        );

        // Poll events from BNS canister
        let events = match self.ic_agent.get_events(last_event_offset, 10).await {
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

        let mut new_offset = last_event_offset;

        for (_timestamp, event) in events {
            // Update offset for next poll
            new_offset += 1;

            // Handle ReeActionStatusChanged events
            if let BnsCanisterEvent::ReeActionStatusChanged {
                action_id,
                status,
                timestamp_nanos: _,
            } = event
            {
                self.handle_status_change(&action_id, status).await;
            }
        }

        Some(new_offset)
    }

    /// Handle a status change event for a tracked transaction
    async fn handle_status_change(&self, action_id: &str, status: ReeActionStatus) {
        tracing::info!(
            "Processing status change for tx_id {}: {:?}",
            action_id,
            status
        );

        match status {
            ReeActionStatus::Pending => {
                // Update status to pending and get the pending_tx data
                match self
                    .postgres
                    .update_pending_tx_status(action_id, PendingTxStatus::Pending)
                    .await
                {
                    Ok(Some(pending_tx)) => {
                        self.handle_pending(&pending_tx).await;
                    }
                    Ok(None) => {
                        tracing::debug!(
                            "No pending tx found for action_id {}, skipping",
                            action_id
                        );
                    }
                    Err(e) => {
                        tracing::error!("Failed to update pending tx status: {:?}", e);
                    }
                }
            }
            ReeActionStatus::Finalized => {
                match self
                    .postgres
                    .update_pending_tx_status(action_id, PendingTxStatus::Finalized)
                    .await
                {
                    Ok(Some(pending_tx)) => {
                        tracing::info!(
                            "Tx {} finalized for listing {}",
                            action_id,
                            pending_tx.name
                        );
                    }
                    Ok(None) => {
                        tracing::debug!("No pending tx found for action_id {}", action_id);
                    }
                    Err(e) => {
                        tracing::error!("Failed to update pending tx status to finalized: {:?}", e);
                    }
                }
            }
            ReeActionStatus::Confirmed(confirmations) => {
                tracing::debug!(
                    "Tx {} confirmed with {} confirmations",
                    action_id,
                    confirmations
                );
                if let Err(e) = self
                    .postgres
                    .update_pending_tx_status(action_id, PendingTxStatus::Confirmed)
                    .await
                {
                    tracing::error!("Failed to update pending tx status to confirmed: {:?}", e);
                }
            }
            ReeActionStatus::Rejected(reason) => {
                tracing::warn!("Tx {} rejected: {}", action_id, reason);
                if let Err(e) = self
                    .postgres
                    .update_pending_tx_status(action_id, PendingTxStatus::Rejected)
                    .await
                {
                    tracing::error!("Failed to update pending tx status to rejected: {:?}", e);
                }
            }
        }
    }

    /// Handle Pending status - save listing to PostgreSQL and Redis
    async fn handle_pending(&self, pending_tx: &PendingTx) {
        match pending_tx.action {
            PendingTxAction::List => {
                self.handle_list_pending(pending_tx).await;
            }
            PendingTxAction::BuyAndRelist => {
                self.handle_buy_and_relist_pending(pending_tx).await;
            }
            PendingTxAction::BuyAndDelist => {
                self.handle_buy_and_delist_pending(pending_tx).await;
            }
            PendingTxAction::Delist => {
                self.handle_delist_pending(pending_tx).await;
            }
        }
    }

    async fn handle_buy_and_relist_pending(&self, pending_tx: &PendingTx) {
        let action_id = &pending_tx.tx_id;
        let Some(buyer_address) = pending_tx.buyer_address.clone() else {
            tracing::error!(
                "Missing buyer_address for BuyAndRelist action: action_id={}, name={}",
                action_id,
                pending_tx.name
            );
            return;
        };
        let Some(new_price_sats) = pending_tx.price_sats else {
            tracing::error!(
                "Missing price_sats for BuyAndRelist action: action_id={}, name={}",
                action_id,
                pending_tx.name
            );
            return;
        };

        let name = &pending_tx.name;
        let mut previous_price_sats: u64 = 0;

        // Update old listing to bought_and_relisted
        if let Ok(Some(prev_listing)) = self.postgres.get_listed_listing_by_name(name).await {
            previous_price_sats = prev_listing.price_sats;

            match self
                .postgres
                .update_listing_to_bought_and_relisted(
                    &prev_listing.id,
                    &buyer_address,
                    new_price_sats,
                )
                .await
            {
                Ok(_) => {
                    tracing::info!("Updated listing {} to bought_and_relisted", name);
                }
                Err(e) => {
                    tracing::error!("Failed to update listing status: {:?}", e);
                }
            }

            // Update sale rankings: recent_sales, most_traded, top_earners
            self.update_sale_rankings(&prev_listing, &buyer_address)
                .await;

            // Remove from listing rankings (will be re-added with new price)
            self.remove_listing_rankings(name).await;
        }

        // Create new listed record
        let now = Utc::now();
        let listing = Listing {
            id: Uuid::new_v4().to_string(),
            name: name.clone(),
            seller_address: buyer_address,
            price_sats: new_price_sats,
            status: ListingStatus::Listed,
            listed_at: now,
            updated_at: now,
            previous_price_sats,
            tx_id: action_id.to_string(),
            buyer_address: None,
            new_price_sats: None,
            inscription_utxo_sats: pending_tx.inscription_utxo_sats,
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

        // Update listing rankings for the new listing
        self.update_listing_rankings(&listing).await;
    }

    /// Update sale rankings: recent_sales, most_traded, top_earners
    async fn update_sale_rankings(&self, prev_listing: &Listing, buyer_address: &str) {
        let name = &prev_listing.name;
        let now = Utc::now();

        // Get trade count for this name
        let trade_count = self
            .postgres
            .get_listing_traded_count(name)
            .await
            .unwrap_or(0) as u32;

        // 1. recent_sales: score = sold_at
        let recent_sale_item = RecentSaleItem {
            name: name.clone(),
            price_sats: prev_listing.price_sats,
            seller_address: prev_listing.seller_address.clone(),
            buyer_address: buyer_address.to_string(),
            sold_at: now.timestamp(),
        };
        if let Err(e) = self.redis.add_recent_sale(&recent_sale_item).await {
            tracing::error!("Failed to add {} to recent-sales ranking: {:?}", name, e);
        } else {
            tracing::info!("Added {} to recent-sales ranking", name);
        }
        // Broadcast recent sale event
        self.broadcast(BroadcastEvent::RecentSale(recent_sale_item));

        // 2. most_traded: score = trade_count
        let most_traded_item = MostTradedItem {
            name: name.clone(),
            price_sats: prev_listing.price_sats,
            seller_address: prev_listing.seller_address.clone(),
            buyer_address: buyer_address.to_string(),
            trade_count,
            sold_at: now.timestamp(),
        };
        if let Err(e) = self.redis.add_most_traded(&most_traded_item).await {
            tracing::error!("Failed to add {} to most-traded ranking: {:?}", name, e);
        } else {
            tracing::info!("Added {} to most-traded ranking", name);
        }
        // Broadcast most traded event
        self.broadcast(BroadcastEvent::MostTraded(most_traded_item));

        // 3. top_earners: score = total_profit_sats
        match self
            .postgres
            .get_top_earner(&prev_listing.seller_address)
            .await
        {
            Ok((total_earn, total_traded)) => {
                let top_earner_item = TopEarnerItem {
                    address: prev_listing.seller_address.clone(),
                    total_profit_sats: total_earn,
                    trade_count: total_traded,
                };

                if let Err(e) = self.redis.add_top_earner(&top_earner_item).await {
                    tracing::error!(
                        "Failed to add {} to top-earners ranking: {:?}",
                        prev_listing.seller_address,
                        e
                    );
                } else {
                    tracing::info!(
                        "Added {} to top-earners ranking",
                        prev_listing.seller_address
                    );
                }
                self.broadcast(BroadcastEvent::TopEarner(top_earner_item));
            }
            Err(e) => {
                tracing::error!("Failed to query top earner: {}", e);
            }
        }
    }

    /// Remove a name from listing rankings: new_listings, top_sales, best_deals
    async fn remove_listing_rankings(&self, name: &str) {
        // 1. Remove from new_listings
        if let Err(e) = self.redis.rem_new_listing(name).await {
            tracing::error!(
                "Failed to remove {} from new-listings ranking: {:?}",
                name,
                e
            );
        }
        // Broadcast removal from new-listings
        self.broadcast(BroadcastEvent::RemoveNewListing(name.to_string()));

        // 2. Remove from top_sales
        if let Err(e) = self.redis.rem_top_sale(name).await {
            tracing::error!("Failed to remove {} from top-sales ranking: {:?}", name, e);
        }
        // Broadcast removal from top-sales
        self.broadcast(BroadcastEvent::RemoveTopSale(name.to_string()));

        // 3. Remove from best_deals
        if let Err(e) = self.redis.rem_best_deal(name).await {
            tracing::error!("Failed to remove {} from best-deals ranking: {:?}", name, e);
        }
        // Broadcast removal from best-deals
        self.broadcast(BroadcastEvent::RemoveBestDeal(name.to_string()));

        tracing::info!("Removed {} from listing rankings", name);
    }

    async fn handle_delist_pending(&self, pending_tx: &PendingTx) {
        let name = &pending_tx.name;

        if let Ok(Some(prev_listing)) = self.postgres.get_listed_listing_by_name(name).await {
            // Update to delisted
            match self
                .postgres
                .update_listing_to_delisted(&prev_listing.id)
                .await
            {
                Ok(_) => {
                    tracing::info!("Updated listing {} to delisted", name);
                }
                Err(e) => {
                    tracing::error!("Failed to update listing status: {:?}", e);
                }
            }

            // Remove from listing rankings: new_listings, top_sales, best_deals
            self.remove_listing_rankings(name).await;
        }
    }

    async fn handle_buy_and_delist_pending(&self, pending_tx: &PendingTx) {
        let action_id = &pending_tx.tx_id;
        let Some(buyer_address) = pending_tx.buyer_address.clone() else {
            tracing::error!(
                "Missing buyer_address for BuyAndDelist action: action_id={}, name={}",
                action_id,
                pending_tx.name
            );
            return;
        };

        let name = &pending_tx.name;

        if let Ok(Some(prev_listing)) = self.postgres.get_listed_listing_by_name(name).await {
            // Update to bought_and_delisted
            match self
                .postgres
                .update_listing_to_bought_and_delisted(&prev_listing.id, &buyer_address)
                .await
            {
                Ok(_) => {
                    tracing::info!("Updated listing {} to bought_and_delisted", name);
                }
                Err(e) => {
                    tracing::error!("Failed to update listing status: {:?}", e);
                }
            }

            // Update sale rankings: recent_sales, most_traded, top_earners
            self.update_sale_rankings(&prev_listing, &buyer_address)
                .await;

            // Remove from listing rankings: new_listings, top_sales, best_deals
            self.remove_listing_rankings(name).await;
        }
    }

    fn broadcast(&self, event: BroadcastEvent) {
        // Broadcast to WebSocket subscribers
        match self.broadcast_tx.send(event.clone()) {
            Ok(receiver_count) => {
                tracing::info!(
                    "Broadcast {:?} to {} WebSocket subscribers",
                    event,
                    receiver_count
                );
            }
            Err(_) => {
                tracing::debug!("No WebSocket subscribers for event {:?}", event);
            }
        }
    }

    async fn handle_list_pending(&self, pending_tx: &PendingTx) {
        let action_id = &pending_tx.tx_id;
        let name = pending_tx.name.as_str();
        let Some(price) = pending_tx.price_sats else {
            tracing::error!(
                "Missing price_sats for List action: action_id={}, name={}",
                action_id,
                name
            );
            return;
        };
        let Some(seller_address) = pending_tx.seller_address.clone() else {
            tracing::error!(
                "Missing seller_address for List action: action_id={}, name={}",
                action_id,
                name
            );
            return;
        };
        // Use previous_price_sats from pending_tx (already calculated during list() call)
        let previous_price_sats = pending_tx.previous_price_sats.unwrap_or(0);

        let now = Utc::now();
        let listing = Listing {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            seller_address: seller_address.to_string(),
            price_sats: price,
            status: ListingStatus::Listed,
            listed_at: now,
            updated_at: now,
            previous_price_sats,
            tx_id: action_id.to_string(),
            buyer_address: None,
            new_price_sats: None,
            inscription_utxo_sats: pending_tx.inscription_utxo_sats,
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

        // Update rankings for new listing
        self.update_listing_rankings(&listing).await;
    }

    /// Update rankings for a new/relisted listing: new_listings, top_sales, best_deals
    pub async fn update_listing_rankings(&self, listing: &Listing) {
        let name = listing.name.as_str();
        let price = listing.price_sats;
        let seller_address = &listing.seller_address;
        let previous_price_sats = listing.previous_price_sats;
        let listed_at = listing.listed_at.timestamp();

        // Calculate discount using shared utility
        let discount = crate::utils::calculate_discount(price, previous_price_sats);

        // 1. new_listings: score = listed_at
        let new_listing_item = NewListingItem {
            name: name.to_string(),
            price_sats: price,
            listed_at,
            discount,
            seller_address: seller_address.clone(),
        };
        if let Err(e) = self.redis.add_new_listing(&new_listing_item).await {
            tracing::error!("Failed to add {} to new-listings ranking: {:?}", name, e);
        } else {
            tracing::info!("Added {} to new-listings ranking", name);
        }
        // Broadcast new listing event
        self.broadcast(BroadcastEvent::NewListing(new_listing_item));

        // 2. top_sales: score = price_sats
        let top_sale_item = TopSaleItem {
            name: name.to_string(),
            price_sats: price,
            listed_at,
            discount,
            seller_address: seller_address.clone(),
        };
        if let Err(e) = self.redis.add_top_sale(&top_sale_item).await {
            tracing::error!("Failed to add {} to top-sales ranking: {:?}", name, e);
        } else {
            tracing::info!("Added {} to top-sales ranking", name);
        }
        // Broadcast top sale event
        self.broadcast(BroadcastEvent::TopSale(top_sale_item));

        // 3. best_deals: score = discount
        let best_deal_item = BestDealItem {
            name: name.to_string(),
            price_sats: price,
            listed_at,
            discount,
            seller_address: seller_address.clone(),
        };
        if let Err(e) = self.redis.add_best_deal(&best_deal_item).await {
            tracing::error!("Failed to add {} to best-deals ranking: {:?}", name, e);
        } else {
            tracing::info!("Added {} to best-deals ranking", name);
        }
        // Broadcast best deal event
        self.broadcast(BroadcastEvent::BestDeal(best_deal_item));
    }
    /// Calculate initial previous_price_sats from etching tx fee
    ///
    /// Looks up the rune info, gets the etching tx, calculates fee, and returns fee * 10.
    /// Used for first-time listings only.
    /// Returns None if any step fails.
    pub async fn calculate_etching_fee_price(&self, name: &str) -> Option<u64> {
        // Get rune info from Ord backend
        let rune_info = match self.blockchain.ord_bns_rune(name).await {
            Ok(Some(info)) => info,
            Ok(None) => {
                tracing::debug!("Rune {} not found in Ord backend", name);
                return None;
            }
            Err(e) => {
                tracing::warn!("Failed to get rune info for {}: {:?}", name, e);
                return None;
            }
        };

        // Get etching tx id
        let etching_txid = match &rune_info.etching {
            Some(txid) => txid,
            None => {
                tracing::debug!("No etching tx for rune {}", name);
                return None;
            }
        };

        // Get fee from etching tx
        let fee_sats = match self.blockchain.get_tx_fee_sats(etching_txid).await {
            Ok(Some(fee)) => fee,
            Ok(None) => {
                tracing::warn!(
                    "Could not determine fee for etching tx {} of rune {}",
                    etching_txid,
                    name
                );
                return None;
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to get fee for etching tx {} of rune {}: {:?}",
                    etching_txid,
                    name,
                    e
                );
                return None;
            }
        };

        let previous_price = (fee_sats * 10).max(GLOBAL_MIN_PRICE).min(INIT_MAX_PRICE);
        tracing::info!(
            "Calculated previous_price_sats for {}: {} (etching fee {} * 10, capped at {})",
            name,
            previous_price,
            fee_sats,
            INIT_MAX_PRICE
        );

        Some(previous_price)
    }
}

pub type DynEventService = Arc<EventService>;
