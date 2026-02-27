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
use crate::domain::{Listing, TradeAction, TradeHistoryItem, TradeRecord, TradeStatus};
use crate::infra::bns_canister::{BnsCanisterEvent, ReeActionStatus};
use crate::infra::{DynBlockchainClient, DynPostgresClient, DynRedisClient, IcAgent};
use crate::service::DynUserService;
use crate::state::BroadcastEvent;
use crate::{GLOBAL_MIN_PRICE, INIT_MAX_PRICE};
use chrono::Utc;
use tokio::sync::broadcast;

/// Event service for canister event polling
#[derive(Clone)]
pub struct EventService {
    ic_agent: Arc<IcAgent>,
    redis: DynRedisClient,
    postgres: DynPostgresClient,
    blockchain: DynBlockchainClient,
    poll_interval: Duration,
    broadcast_tx: broadcast::Sender<BroadcastEvent>,
    user_service: DynUserService,
}

impl EventService {
    pub fn new(
        ic_agent: Arc<IcAgent>,
        redis: DynRedisClient,
        postgres: DynPostgresClient,
        blockchain: DynBlockchainClient,
        broadcast_tx: broadcast::Sender<BroadcastEvent>,
        user_service: DynUserService,
    ) -> Self {
        Self {
            ic_agent,
            redis,
            postgres,
            blockchain,
            poll_interval: Duration::from_secs(60),
            broadcast_tx,
            user_service,
        }
    }

    pub fn start_polling(self: Arc<Self>) {
        tokio::spawn(async move {
            self.polling_loop().await;
        });
    }

    async fn polling_loop(&self) {
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

    async fn poll_once(&self, last_event_offset: u64) -> Option<u64> {
        tracing::debug!(
            "Starting poll_once, last_event_offset: {}",
            last_event_offset
        );

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
            new_offset += 1;

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

    async fn handle_status_change(&self, action_id: &str, status: ReeActionStatus) {
        tracing::info!(
            "Processing status change for tx_id {}: {:?}",
            action_id,
            status
        );

        match status {
            ReeActionStatus::Pending => {
                match self
                    .postgres
                    .update_trade_status(action_id, TradeStatus::Pending)
                    .await
                {
                    Ok(Some(trade_record)) => {
                        self.handle_pending(&trade_record).await;
                    }
                    Ok(None) => {
                        tracing::debug!(
                            "No trade record found for action_id {}, skipping",
                            action_id
                        );
                    }
                    Err(e) => {
                        tracing::error!("Failed to update trade status: {:?}", e);
                    }
                }
            }
            ReeActionStatus::Finalized => {
                match self
                    .postgres
                    .update_trade_status(action_id, TradeStatus::Finalized)
                    .await
                {
                    Ok(Some(trade_record)) => {
                        tracing::info!("Tx {} finalized for name {}", action_id, trade_record.name);
                    }
                    Ok(None) => {
                        tracing::debug!("No trade record found for action_id {}", action_id);
                    }
                    Err(e) => {
                        tracing::error!("Failed to update trade status to finalized: {:?}", e);
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
                    .update_trade_status(action_id, TradeStatus::Confirmed)
                    .await
                {
                    tracing::error!("Failed to update trade status to confirmed: {:?}", e);
                }
            }
            ReeActionStatus::Rejected(reason) => {
                tracing::warn!("Tx {} rejected: {}", action_id, reason);
                match self
                    .postgres
                    .update_trade_status(action_id, TradeStatus::Rejected)
                    .await
                {
                    Ok(Some(trade_record)) => {
                        self.handle_rejected(&trade_record).await;
                    }
                    Ok(None) => {
                        tracing::debug!(
                            "No trade record found for rejected action_id {}",
                            action_id
                        );
                    }
                    Err(e) => {
                        tracing::error!("Failed to update trade status to rejected: {:?}", e);
                    }
                }
            }
        }
    }

    // ========================================================================
    // Handle Pending status - modify listings table
    // ========================================================================

    async fn handle_pending(&self, trade_record: &TradeRecord) {
        match trade_record.action {
            TradeAction::List => {
                self.handle_list_pending(trade_record).await;
            }
            TradeAction::Delist => {
                self.handle_delist_pending(trade_record).await;
            }
            TradeAction::BuyAndRelist => {
                self.handle_buy_and_relist_pending(trade_record).await;
            }
            TradeAction::BuyAndDelist => {
                self.handle_buy_and_delist_pending(trade_record).await;
            }
            TradeAction::Relist => {
                // Relist is handled synchronously, should never reach here
                tracing::warn!("Unexpected pending event for relist action");
            }
        }
    }

    async fn handle_list_pending(&self, trade_record: &TradeRecord) {
        let name = &trade_record.name;
        let Some(price) = trade_record.price_sats else {
            tracing::error!("Missing price_sats for List action: name={}", name);
            return;
        };
        let Some(seller_address) = trade_record.seller_address.clone() else {
            tracing::error!("Missing seller_address for List action: name={}", name);
            return;
        };
        let tx_id = trade_record.tx_id.clone().unwrap_or_default();
        let previous_price_sats = trade_record.previous_price_sats.unwrap_or(0);

        let now = Utc::now();
        let listing = Listing {
            name: name.clone(),
            seller_address,
            price_sats: price,
            listed_at: now,
            updated_at: now,
            tx_id,
            inscription_utxo_sats: trade_record.inscription_utxo_sats,
        };

        if let Err(e) = self.postgres.create_listing(&listing).await {
            tracing::error!("Failed to save listing for {}: {:?}", name, e);
        } else {
            tracing::info!("Saved listing to PostgreSQL: name={}", name);
        }

        self.update_listing_rankings(&listing, previous_price_sats)
            .await;

        self.broadcast_trade_updates(trade_record, false).await;
    }

    async fn handle_delist_pending(&self, trade_record: &TradeRecord) {
        let name = &trade_record.name;

        if let Err(e) = self.postgres.delete_listing_by_name(name).await {
            tracing::error!("Failed to delete listing for delist {}: {:?}", name, e);
        } else {
            tracing::info!("Deleted listing {} from PostgreSQL (delist)", name);
        }

        self.remove_listing_rankings(name).await;

        self.broadcast_trade_updates(trade_record, false).await;
    }

    async fn handle_buy_and_relist_pending(&self, trade_record: &TradeRecord) {
        let name = &trade_record.name;
        let Some(buyer_address) = trade_record.buyer_address.clone() else {
            tracing::error!("Missing buyer_address for BuyAndRelist: name={}", name);
            return;
        };
        let Some(new_price_sats) = trade_record.price_sats else {
            tracing::error!("Missing price_sats for BuyAndRelist: name={}", name);
            return;
        };
        let tx_id = trade_record.tx_id.clone().unwrap_or_default();
        let previous_price_sats = trade_record.previous_price_sats.unwrap_or(0);

        // Get old listing for sale rankings before updating
        let old_listing = self.postgres.get_listed_listing_by_name(name).await;

        // Update listing: seller→buyer, price→new price, tx_id→new tx_id
        if let Err(e) = self
            .postgres
            .update_listing_seller_price_tx(name, &buyer_address, new_price_sats, &tx_id)
            .await
        {
            tracing::error!(
                "Failed to update listing for buy_and_relist {}: {:?}",
                name,
                e
            );
        } else {
            tracing::info!("Updated listing {} for buy_and_relist", name);
        }

        // Update sale rankings using old listing data
        if let Ok(Some(prev_listing)) = old_listing {
            self.update_sale_rankings(&prev_listing, &buyer_address)
                .await;
        }

        // Remove old listing rankings and add new ones
        self.remove_listing_rankings(name).await;

        // Re-read the updated listing for new ranking data
        if let Ok(Some(updated_listing)) = self.postgres.get_listed_listing_by_name(name).await {
            self.update_listing_rankings(&updated_listing, previous_price_sats)
                .await;
        }

        // Add seller points
        self.add_seller_points(trade_record).await;

        self.broadcast_trade_updates(trade_record, true).await;
    }

    async fn handle_buy_and_delist_pending(&self, trade_record: &TradeRecord) {
        let name = &trade_record.name;
        let Some(buyer_address) = trade_record.buyer_address.clone() else {
            tracing::error!("Missing buyer_address for BuyAndDelist: name={}", name);
            return;
        };

        // Get old listing for sale rankings before deleting
        if let Ok(Some(prev_listing)) = self.postgres.get_listed_listing_by_name(name).await {
            // Update sale rankings
            self.update_sale_rankings(&prev_listing, &buyer_address)
                .await;
        }

        // Delete from listings
        if let Err(e) = self.postgres.delete_listing_by_name(name).await {
            tracing::error!(
                "Failed to delete listing for buy_and_delist {}: {:?}",
                name,
                e
            );
        } else {
            tracing::info!("Deleted listing {} from PostgreSQL (buy_and_delist)", name);
        }

        // Remove from listing rankings
        self.remove_listing_rankings(name).await;

        // Add seller points
        self.add_seller_points(trade_record).await;

        self.broadcast_trade_updates(trade_record, true).await;
    }

    pub async fn broadcast_relist_updates(&self, trade_record: &TradeRecord) {
        self.broadcast_market_listings().await;
        self.broadcast_user_inventories(trade_record).await;
        self.broadcast_user_activities(trade_record).await;
    }

    async fn broadcast_trade_updates(&self, trade_record: &TradeRecord, include_24h_trade: bool) {
        self.broadcast_market_listings().await;
        if include_24h_trade {
            self.broadcast_market_trades_24h().await;
        }
        self.broadcast_user_inventories(trade_record).await;
        self.broadcast_user_activities(trade_record).await;
    }

    async fn broadcast_market_listings(&self) {
        match self.postgres.get_listing_count_and_valuation().await {
            Ok((listed_count, listed_value)) => {
                self.broadcast(BroadcastEvent::MarketListingsUpdated {
                    listed_count,
                    listed_value,
                });
            }
            Err(e) => {
                tracing::error!("Failed to query listing stats: {:?}", e);
            }
        }
    }

    async fn broadcast_market_trades_24h(&self) {
        match self.postgres.get_24h_tx_vol().await {
            Ok((txs_24h, vol_24h)) => {
                self.broadcast(BroadcastEvent::MarketTrades24hUpdated { txs_24h, vol_24h });
            }
            Err(e) => {
                tracing::error!("Failed to query 24h market stats: {:?}", e);
            }
        }
    }

    async fn broadcast_user_inventories(&self, trade_record: &TradeRecord) {
        let mut users = std::collections::HashSet::new();
        users.insert(trade_record.who.clone());
        if let Some(seller_address) = &trade_record.seller_address {
            users.insert(seller_address.clone());
        }
        if let Some(buyer_address) = &trade_record.buyer_address {
            users.insert(buyer_address.clone());
        }

        for user in users {
            match self.user_service.get_inventory(&user).await {
                Ok(inventory) => {
                    self.broadcast(BroadcastEvent::UserInventory {
                        user_address: user.clone(),
                        inventory,
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to get inventory for {}: {:?}", user, e);
                }
            }
        }
    }

    async fn broadcast_user_activities(&self, trade_record: &TradeRecord) {
        let activity = TradeHistoryItem {
            id: trade_record.id.clone(),
            name: trade_record.name.clone(),
            txid: trade_record.tx_id.clone().unwrap_or_default(),
            action: trade_record.action.to_string(),
            price_sats: trade_record.price_sats,
            status: trade_record.status.to_string(),
            time: trade_record.created_at,
        };
        self.broadcast(BroadcastEvent::UserActivities {
            user_address: trade_record.who.clone(),
            activities: vec![activity],
        });
    }

    // ========================================================================
    // Handle Rejected status - rollback listings changes
    // ========================================================================

    async fn handle_rejected(&self, trade_record: &TradeRecord) {
        match trade_record.action {
            TradeAction::List => {
                self.handle_list_rejected(trade_record).await;
            }
            TradeAction::Delist => {
                self.handle_delist_rejected(trade_record).await;
            }
            TradeAction::BuyAndRelist => {
                self.handle_buy_and_relist_rejected(trade_record).await;
            }
            TradeAction::BuyAndDelist => {
                self.handle_buy_and_delist_rejected(trade_record).await;
            }
            TradeAction::Relist => {
                tracing::warn!("Unexpected rejected event for relist action");
                return;
            }
        }

        // Broadcast market-stat and user-self updates after rollback
        self.broadcast_trade_updates(trade_record, false).await;
    }

    /// list rejected: delete the listing that was added during pending
    async fn handle_list_rejected(&self, trade_record: &TradeRecord) {
        let name = &trade_record.name;

        if let Err(e) = self.postgres.delete_listing_by_name(name).await {
            tracing::error!("Failed to rollback list for {}: {:?}", name, e);
        } else {
            tracing::info!("Rolled back list for {} (rejected)", name);
        }

        self.remove_listing_rankings(name).await;
    }

    /// delist rejected: re-insert listing from trade_record data
    async fn handle_delist_rejected(&self, trade_record: &TradeRecord) {
        let name = &trade_record.name;
        let seller_address = trade_record.seller_address.clone().unwrap_or_default();
        let price_sats = trade_record.previous_price_sats.unwrap_or(0);
        let previous_price_sats = trade_record.previous_price_sats.unwrap_or(0);
        let tx_id = trade_record.tx_id.clone().unwrap_or_default();

        let now = Utc::now();
        let listing = Listing {
            name: name.clone(),
            seller_address,
            price_sats,
            listed_at: now,
            updated_at: now,
            tx_id,
            inscription_utxo_sats: trade_record.inscription_utxo_sats,
        };

        if let Err(e) = self.postgres.create_listing(&listing).await {
            tracing::error!("Failed to rollback delist for {}: {:?}", name, e);
        } else {
            tracing::info!("Rolled back delist for {} (rejected)", name);
        }

        self.update_listing_rankings(&listing, previous_price_sats)
            .await;
    }

    /// buy_and_relist rejected: restore seller and price to before the buy
    async fn handle_buy_and_relist_rejected(&self, trade_record: &TradeRecord) {
        let name = &trade_record.name;
        let original_seller = trade_record.seller_address.clone().unwrap_or_default();
        let original_price = trade_record.previous_price_sats.unwrap_or(0);

        // Restore the listing to the original seller and price
        // We need to figure out the original tx_id - it's lost in the update.
        // We'll use the trade_record's tx_id as fallback.
        let original_tx_id = trade_record.tx_id.clone().unwrap_or_default();

        if let Err(e) = self
            .postgres
            .update_listing_seller_price_tx(name, &original_seller, original_price, &original_tx_id)
            .await
        {
            tracing::error!("Failed to rollback buy_and_relist for {}: {:?}", name, e);
        } else {
            tracing::info!("Rolled back buy_and_relist for {} (rejected)", name);
        }

        // Update rankings
        self.remove_listing_rankings(name).await;
        if let Ok(Some(listing)) = self.postgres.get_listed_listing_by_name(name).await {
            self.update_listing_rankings(&listing, original_price).await;
        }

        // TODO: Also need to reverse sale rankings (recent-sales, top-earners, most-traded)
        // For now, broadcast updates for all affected channels
        self.broadcast(BroadcastEvent::RemoveNewListing(name.to_string()));
    }

    /// buy_and_delist rejected: re-insert listing from trade_record data
    async fn handle_buy_and_delist_rejected(&self, trade_record: &TradeRecord) {
        let name = &trade_record.name;
        let seller_address = trade_record.seller_address.clone().unwrap_or_default();
        let price_sats = trade_record.previous_price_sats.unwrap_or(0);
        let previous_price_sats = trade_record.previous_price_sats.unwrap_or(0);
        let tx_id = trade_record.tx_id.clone().unwrap_or_default();

        let now = Utc::now();
        let listing = Listing {
            name: name.clone(),
            seller_address,
            price_sats,
            listed_at: now,
            updated_at: now,
            tx_id,
            inscription_utxo_sats: trade_record.inscription_utxo_sats,
        };

        if let Err(e) = self.postgres.create_listing(&listing).await {
            tracing::error!("Failed to rollback buy_and_delist for {}: {:?}", name, e);
        } else {
            tracing::info!("Rolled back buy_and_delist for {} (rejected)", name);
        }

        self.update_listing_rankings(&listing, previous_price_sats)
            .await;
    }

    // ========================================================================
    // Ranking helpers
    // ========================================================================

    pub async fn add_seller_points(&self, trade_record: &TradeRecord) {
        if let Some(points) = trade_record.platform_fee {
            let seller = self
                .postgres
                .get_user(
                    trade_record
                        .seller_address
                        .clone()
                        .unwrap_or_default()
                        .as_str(),
                )
                .await;
            match seller {
                Ok(Some(seller)) => {
                    if let Some(primary_name) = seller.primary_name {
                        let result = self
                            .postgres
                            .add_nft_points(primary_name.as_str(), points as i64)
                            .await;
                        tracing::info!("add nft_points for seller: {:?}", result);
                    }
                }
                Ok(None) => {
                    tracing::error!("User not found: {:?}", trade_record.seller_address);
                }
                Err(e) => {
                    tracing::error!("Failed to retrieve seller address: {:?}", e);
                }
            }
        }
    }

    async fn update_sale_rankings(&self, prev_listing: &Listing, buyer_address: &str) {
        let name = &prev_listing.name;
        let now = Utc::now();

        let trade_count = self
            .postgres
            .get_listing_traded_count(name)
            .await
            .unwrap_or(0) as u32;

        // 1. recent_sales
        let recent_sale_item = RecentSaleItem {
            name: name.clone(),
            price_sats: prev_listing.price_sats,
            seller_address: prev_listing.seller_address.clone(),
            buyer_address: buyer_address.to_string(),
            sold_at: now.timestamp(),
        };
        if let Err(e) = self.redis.add_recent_sale(&recent_sale_item).await {
            tracing::error!("Failed to add {} to recent-sales ranking: {:?}", name, e);
        }
        self.broadcast(BroadcastEvent::RecentSale(recent_sale_item));

        // 2. most_traded
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
        }
        self.broadcast(BroadcastEvent::MostTraded(most_traded_item));

        // 3. top_sales
        let top_sale_item = TopSaleItem {
            name: name.to_string(),
            price_sats: prev_listing.price_sats,
            sold_at: prev_listing.updated_at.timestamp(),
            seller_address: prev_listing.seller_address.clone(),
            buyer_address: buyer_address.to_string(),
        };
        if let Err(e) = self.redis.add_top_sale(&top_sale_item).await {
            tracing::error!("Failed to add {} to top-sales ranking: {:?}", name, e);
        }
        self.broadcast(BroadcastEvent::TopSale(top_sale_item));

        // 4. top_earners
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
                }
                self.broadcast(BroadcastEvent::TopEarner(top_earner_item));
            }
            Err(e) => {
                tracing::error!("Failed to query top earner: {}", e);
            }
        }
    }

    async fn remove_listing_rankings(&self, name: &str) {
        if let Err(e) = self.redis.rem_new_listing(name).await {
            tracing::error!(
                "Failed to remove {} from new-listings ranking: {:?}",
                name,
                e
            );
        }
        self.broadcast(BroadcastEvent::RemoveNewListing(name.to_string()));

        if let Err(e) = self.redis.rem_best_deal(name).await {
            tracing::error!("Failed to remove {} from best-deals ranking: {:?}", name, e);
        }
        self.broadcast(BroadcastEvent::RemoveBestDeal(name.to_string()));

        tracing::info!("Removed {} from listing rankings", name);
    }

    pub async fn update_listing_rankings(&self, listing: &Listing, previous_price_sats: u64) {
        let name = listing.name.as_str();
        let price = listing.price_sats;
        let seller_address = &listing.seller_address;
        let listed_at = listing.listed_at.timestamp();

        let discount = crate::utils::calculate_discount(price, previous_price_sats);

        // 1. new_listings
        let new_listing_item = NewListingItem {
            name: name.to_string(),
            price_sats: price,
            listed_at,
            discount,
            seller_address: seller_address.clone(),
        };
        if let Err(e) = self.redis.add_new_listing(&new_listing_item).await {
            tracing::error!("Failed to add {} to new-listings ranking: {:?}", name, e);
        }
        self.broadcast(BroadcastEvent::NewListing(new_listing_item));

        // 2. best_deals
        let best_deal_item = BestDealItem {
            name: name.to_string(),
            price_sats: price,
            listed_at,
            discount,
            seller_address: seller_address.clone(),
        };
        if let Err(e) = self.redis.add_best_deal(&best_deal_item).await {
            tracing::error!("Failed to add {} to best-deals ranking: {:?}", name, e);
        }
        self.broadcast(BroadcastEvent::BestDeal(best_deal_item));
    }

    fn broadcast(&self, event: BroadcastEvent) {
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

    pub async fn calculate_etching_fee_price(&self, name: &str) -> Option<u64> {
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

        let etching_txid = match &rune_info.etching {
            Some(txid) => txid,
            None => {
                tracing::debug!("No etching tx for rune {}", name);
                return None;
            }
        };

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
