//! Background tasks
//!
//! Periodic tasks:
//! - Event polling from Canister
//! - Ranking cleanup (expired entries)
//! - ShoutOut expiration

use std::time::Duration;
use tokio::time::interval;

use crate::service::{DynEventService, DynMarketService, DynShoutOutService};

/// Start background tasks
pub fn start_background_tasks(
    event_service: DynEventService,
    market_service: DynMarketService,
    shoutout_service: DynShoutOutService,
    event_poll_interval: Duration,
    cleanup_interval: Duration,
) {
    // Event polling task
    let event_svc = event_service.clone();
    tokio::spawn(async move {
        let mut interval = interval(event_poll_interval);
        loop {
            interval.tick().await;
            if let Err(e) = event_svc.poll_and_process().await {
                tracing::error!("Event polling error: {}", e);
            }
        }
    });

    // Ranking cleanup task
    let market_svc = market_service.clone();
    tokio::spawn(async move {
        let mut interval = interval(cleanup_interval);
        loop {
            interval.tick().await;
            if let Err(e) = market_svc.cleanup_expired_rankings().await {
                tracing::error!("Ranking cleanup error: {}", e);
            }
        }
    });

    // ShoutOut expiration task
    let shoutout_svc = shoutout_service.clone();
    tokio::spawn(async move {
        let mut interval = interval(cleanup_interval);
        loop {
            interval.tick().await;
            if let Err(e) = shoutout_svc.cleanup_expired().await {
                tracing::error!("ShoutOut cleanup error: {}", e);
            }
        }
    });
}
