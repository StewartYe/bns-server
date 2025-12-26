//! Event service
//!
//! Handles Canister event queue polling and transaction status updates.

use std::sync::Arc;
use tokio::sync::broadcast;

use crate::domain::{CanisterEvent, EventType, TransactionStatus, WebSocketEvent};
use crate::error::Result;
use crate::infra::{DynCanisterClient, DynPostgresClient, DynRedisClient};

/// Event service
pub struct EventService {
    canister: DynCanisterClient,
    postgres: DynPostgresClient,
    redis: DynRedisClient,
    /// Broadcast channel for WebSocket notifications
    event_tx: broadcast::Sender<WebSocketEvent>,
}

impl EventService {
    pub fn new(
        canister: DynCanisterClient,
        postgres: DynPostgresClient,
        redis: DynRedisClient,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(1024);
        Self {
            canister,
            postgres,
            redis,
            event_tx,
        }
    }

    /// Get a receiver for WebSocket events
    pub fn subscribe(&self) -> broadcast::Receiver<WebSocketEvent> {
        self.event_tx.subscribe()
    }

    /// Poll Canister event queue and process events
    ///
    /// Called periodically by background task
    pub async fn poll_and_process(&self) -> Result<u64> {
        // Get last processed event ID
        let last_id = self.postgres.get_last_processed_event_id().await?;

        // Poll new events from Canister
        let events = self.canister.poll_events(last_id.as_deref()).await?;
        let count = events.len() as u64;

        for event in events {
            self.process_event(&event).await?;
        }

        Ok(count)
    }

    /// Process a single event
    async fn process_event(&self, event: &CanisterEvent) -> Result<()> {
        // Save event to database
        self.postgres.save_event(event).await?;

        // Handle event based on type
        match event.event_type {
            EventType::TransactionConfirmed => {
                self.handle_tx_confirmed(event).await?;
            }
            EventType::TransactionFinalized => {
                self.handle_tx_finalized(event).await?;
            }
            EventType::TransactionFailed => {
                self.handle_tx_failed(event).await?;
            }
            _ => {
                // Other events are informational
            }
        }

        // Notify WebSocket clients
        self.notify_clients(event).await?;

        Ok(())
    }

    async fn handle_tx_confirmed(&self, event: &CanisterEvent) -> Result<()> {
        if let Some(tx_id) = &event.tx_id {
            self.postgres
                .update_transaction_status(tx_id, TransactionStatus::Confirmed)
                .await?;
        }
        Ok(())
    }

    async fn handle_tx_finalized(&self, event: &CanisterEvent) -> Result<()> {
        if let Some(tx_id) = &event.tx_id {
            self.postgres
                .update_transaction_status(tx_id, TransactionStatus::Finalized)
                .await?;
        }
        Ok(())
    }

    async fn handle_tx_failed(&self, event: &CanisterEvent) -> Result<()> {
        if let Some(tx_id) = &event.tx_id {
            self.postgres
                .update_transaction_status(tx_id, TransactionStatus::Failed)
                .await?;
        }
        // TODO: Revert market state (re-list name, etc.)
        Ok(())
    }

    async fn notify_clients(&self, event: &CanisterEvent) -> Result<()> {
        let ws_event = WebSocketEvent {
            event_type: format!("{:?}", event.event_type).to_lowercase(),
            names: vec![event.name.clone()],
            addresses: event.addresses.clone(),
            timestamp: event.timestamp,
        };

        // Ignore send errors (no receivers is fine)
        let _ = self.event_tx.send(ws_event);
        Ok(())
    }
}

pub type DynEventService = Arc<EventService>;
