//! Listing service
//!
//! Handles list_name operations via orchestrator canister invoke.

use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::Utc;
use uuid::Uuid;

use crate::domain::{
    ListNameParams, ListNameRequest, ListNameResponse, ListedNamesResponse, ListingInfo,
};
use crate::error::{AppError, Result};
use crate::infra::orchestrator_canister::InvokeArgs;
use crate::infra::{DynPostgresClient, DynRedisClient, IcAgent, ListingMeta};

/// Listing service
pub struct ListingService {
    ic_agent: Arc<IcAgent>,
    postgres: DynPostgresClient,
    redis: DynRedisClient,
}

impl ListingService {
    pub fn new(
        ic_agent: Arc<IcAgent>,
        postgres: DynPostgresClient,
        redis: DynRedisClient,
    ) -> Self {
        Self {
            ic_agent,
            postgres,
            redis,
        }
    }

    /// List a name for sale via orchestrator canister
    ///
    /// Flow:
    /// 1. Parse ListNameParams from intention action_params
    /// 2. Build InvokeArgs and call orchestrator.invoke()
    /// 3. Store tx_id in Redis for tracking
    /// 4. Return response with tx_id
    ///
    /// Note: Listing is NOT saved to PostgreSQL here.
    /// The background get_events task will save it when status becomes Pending.
    pub async fn list_name(&self, request: &ListNameRequest) -> Result<ListNameResponse> {
        // Extract and validate intention
        let intention = request
            .intention_set
            .intentions
            .first()
            .ok_or_else(|| AppError::BadRequest("No intention in intention_set".to_string()))?;

        // Parse action_params to get listing details
        let params: ListNameParams = serde_json::from_str(&intention.action_params)
            .map_err(|e| AppError::BadRequest(format!("Invalid action_params JSON: {}", e)))?;

        tracing::info!(
            "Processing list_name: name={}, price={}, seller={}",
            params.name,
            params.price,
            params.seller_address
        );

        // Decode initiator_utxo_proof from base64
        let proof_bytes = STANDARD
            .decode(&request.initiator_utxo_proof)
            .map_err(|e| AppError::BadRequest(format!("Invalid initiator_utxo_proof base64: {}", e)))?;

        // Build InvokeArgs
        let invoke_args = InvokeArgs {
            client_info: None,
            intention_set: request.intention_set.clone(),
            initiator_utxo_proof: serde_bytes::ByteBuf::from(proof_bytes),
            psbt_hex: request.psbt_hex.clone(),
        };

        // Call orchestrator canister invoke
        let tx_id = self.ic_agent.invoke(invoke_args).await?;

        tracing::info!(
            "Invoke succeeded for name '{}', tx_id: {}",
            params.name,
            tx_id
        );

        // Store tx_id in Redis for tracking by get_events background task
        // Key: pending_tx:<tx_id>, Value: JSON with listing details
        let tracking_data = serde_json::json!({
            "name": params.name,
            "price": params.price,
            "seller_address": params.seller_address,
            "pool_address": intention.pool_address,
            "submitted_at": Utc::now().to_rfc3339(),
        });

        if let Err(e) = self
            .redis
            .add_pending_tx(&tx_id, &tracking_data.to_string())
            .await
        {
            tracing::error!("Failed to add tx_id {} to pending tracking: {:?}", tx_id, e);
            // Don't fail the request - the invoke already succeeded
        }

        // Generate a listing ID for the response
        let listing_id = Uuid::new_v4().to_string();

        Ok(ListNameResponse {
            id: listing_id,
            tx_id,
            name: params.name,
            price_sats: params.price,
            seller_address: params.seller_address,
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
            })
            .collect();

        Ok(ListedNamesResponse {
            listings: listing_infos,
            total,
        })
    }

    /// Get newest listings from Redis (for real-time display)
    pub async fn get_new_listings(&self, count: usize) -> Result<Vec<ListingMeta>> {
        self.redis.get_new_listings(count).await
    }
}

pub type DynListingService = Arc<ListingService>;
