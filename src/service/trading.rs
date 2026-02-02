//! Trading service
//!
//! Handles trading operations:
//! - Get pool address for listing
//! - List name for sale
//! - Get all listings
//! - Future: Delist, Buy

use crate::AppError::BadRequest;
use crate::GLOBAL_MIN_PRICE;
use crate::api::rankings::NewListingItem;
use crate::domain::{
    BuyAndDelistParams, BuyAndDelistRequest, BuyAndRelistParams, BuyAndRelistRequest, DelistParams,
    DelistRequest, DelistResponse, GetListingResponse, GetPoolRequest, GetPoolResponse,
    ListNameParams, ListRequest, ListResponse, ListingHistory, ListingInfo,
    ListingPriceRangeResponse, ListingStatus, ListingsResponse, NameDealHistory,
    NameHistoriesResponse, PendingTx, PendingTxAction, PendingTxStatus, RelistRequest,
    RelistResponse, UserAction, UserHistoriesResponse,
};
use crate::error::{AppError, Result};
use crate::infra::orchestrator_canister::InvokeArgs;
use crate::infra::{DynBlockchainClient, DynPostgresClient, DynRedisClient, IcAgent};
use crate::service::trading_validators::buy_and_delist_validator::BuyAndDelistValidator;
use crate::service::trading_validators::buy_and_relist_validator::BuyAndRelistValidator;
use crate::service::trading_validators::delist_validator::DelistValidator;
use crate::service::trading_validators::list_validator::ListValidator;
use crate::service::trading_validators::{TradingValidator, parse_psbt};
use crate::service::{EventService, UserService};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::cmp::max;
use std::sync::Arc;
use uuid::Uuid;

/// BNS Rune info (serializable for responses)
#[derive(Debug, Serialize, Deserialize)]
pub struct BnsRuneInfo {
    pub address: String,
    pub inscription_id: String,
    pub rune_id: String,
    pub etching: String,
    pub inscription_number: i32,
    pub confirmations: u32,
}

/// Trading service
pub struct TradingService {
    event_service: Arc<EventService>,
    ic_agent: Arc<IcAgent>,
    postgres: DynPostgresClient,
    redis: DynRedisClient,
    blockchain: DynBlockchainClient,
    user_service: Arc<UserService>,
}

impl TradingService {
    pub fn new(
        event_service: Arc<EventService>,
        ic_agent: Arc<IcAgent>,
        postgres: DynPostgresClient,
        redis: DynRedisClient,
        blockchain: DynBlockchainClient,
        user_service: Arc<UserService>,
    ) -> Self {
        Self {
            event_service,
            ic_agent,
            postgres,
            redis,
            blockchain,
            user_service,
        }
    }

    // ========================================================================
    // Get Pool Address
    // ========================================================================

    /// Get or create a pool for a name
    ///
    /// Returns the pool address for the name.
    /// If pool already exists in cache, returns directly.
    /// If creating a new pool, requires the caller to own the name.
    ///
    /// Flow:
    /// 1. Check database cache for existing pool address (return if found)
    /// 2. Verify ownership of the name
    /// 3. Call canister to create pool
    /// 4. Save pool address to database cache
    /// 5. Return pool address
    pub async fn get_pool(
        &self,
        request: &GetPoolRequest,
        caller_address: &str,
    ) -> Result<GetPoolResponse> {
        // Check database cache first - if pool exists, return directly without ownership check
        if let Some(pool_address) = self.postgres.get_pool_address(&request.name).await? {
            tracing::debug!(
                "Pool address for '{}' found in cache: {}",
                request.name,
                pool_address
            );
            return Ok(GetPoolResponse {
                name: request.name.clone(),
                pool_address,
            });
        }

        // Verify ownership before creating pool: check if name belongs to caller's address
        self.user_service
            .verify_name_ownership(&request.name, caller_address)
            .await?;

        // Call canister to create pool
        let pool_address = match self.ic_agent.create_pool(&request.name).await {
            Ok(addr) => addr,
            Err(AppError::Canister(err))
                if err.contains("already exists") || err.contains("Pool exists") =>
            {
                return Err(AppError::PoolAlreadyExists(request.name.clone()));
            }
            Err(e) => return Err(e),
        };

        // Save to database cache
        if let Err(e) = self
            .postgres
            .save_pool_address(&request.name, &pool_address)
            .await
        {
            tracing::warn!(
                "Failed to cache pool address for '{}': {:?}",
                request.name,
                e
            );
        }

        Ok(GetPoolResponse {
            name: request.name.clone(),
            pool_address,
        })
    }

    pub async fn buy_and_relist(
        &self,
        request: &BuyAndRelistRequest,
        caller_address: &str,
    ) -> Result<ListResponse> {
        // === Step 1: Get first intention ===
        let intention = request
            .intention_set
            .intentions
            .first()
            .ok_or_else(|| AppError::BadRequest("No intention in intention_set".to_string()))?;

        // === Step 2: Parse PSBT and call extract_tx() ===
        let psbt = parse_psbt(&request.psbt_hex)?;
        let _tx = psbt
            .clone()
            .extract_tx()
            .map_err(|e| AppError::BadRequest(format!("Failed to extract tx from PSBT: {}", e)))?;

        // === Step 3: Parse intention.action_params ===
        let params: BuyAndRelistParams = serde_json::from_str(&intention.action_params)
            .map_err(|e| AppError::BadRequest(format!("Invalid action_params JSON: {}", e)))?;

        // === Step 4: Query database get_pool_address(params.name) ===
        let db_pool_address = self
            .postgres
            .get_pool_address(&params.name)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!(
                    "Pool address for '{}' not found in cache",
                    params.name
                ))
            })?;

        // === Step 5: Check intention.pool_address == database pool_address ===
        if intention.pool_address != db_pool_address {
            return Err(AppError::BadRequest(format!(
                "Pool address mismatch: intention has '{}', database has '{}'",
                intention.pool_address, db_pool_address
            )));
        }

        tracing::info!(
            "Processing buy_and_relist: name={}, price={}, buyer={}",
            params.name,
            params.new_price,
            params.buyer_address
        );

        // === Step 6: Verify name ownership - params.name belongs to db_pool_address ===
        self.user_service
            .verify_name_ownership(&params.name, &db_pool_address)
            .await?;

        // === Step 7: Verify params.buyer_address = caller_address ===
        if params.buyer_address != caller_address {
            return Err(AppError::Forbidden(format!(
                "Caller address {} does not match buyer address {}",
                caller_address, params.buyer_address
            )));
        }

        // Get listing from database (needed for Step 8)
        let db_listing = self
            .postgres
            .get_listed_listing_by_name(&params.name)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!("Listing for '{}' not found", params.name))
            })?;

        //verify action_parmas:
        let fee_sats = max(330, db_listing.price_sats * 2 / 100);
        if fee_sats != params.fee_sats || params.payment_sats != db_listing.price_sats - fee_sats {
            return Err(AppError::BadRequest(
                "fee_sats + payment_sats != price_sats".to_string(),
            ));
        }

        // === Step 8: PSBT Validation ===
        BuyAndRelistValidator::validate_psbt(&psbt, &db_pool_address, Some(&db_listing), &params)?;

        // === Step 9: Inscription Validation ===
        let prev_out = &psbt.unsigned_tx.input[0].previous_output;
        let input0_outpoint = format!("{}:{}", prev_out.txid, prev_out.vout);
        self.validate_inscription(&params.name, &input0_outpoint)
            .await?;

        // === Step 10: Price Validation ===
        self.validate_price(params.name.as_str(), params.new_price)
            .await?;

        tracing::debug!(
            "Pool address {} verified for name '{}'",
            intention.pool_address,
            params.name
        );

        // Create ByteBuf from raw Vec<u8> (no base64 decode needed)
        let proof_bytes = request.initiator_utxo_proof.clone();

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
        let output0_value = psbt.unsigned_tx.output[0].value.to_sat();
        let pending_tx = PendingTx {
            tx_id: tx_id.clone(),
            created_at: Utc::now(),
            name: params.name.clone(),
            action: PendingTxAction::BuyAndRelist,
            status: PendingTxStatus::Submitted,
            previous_price_sats: None,
            price_sats: Some(params.new_price),
            seller_address: Some(db_listing.seller_address),
            buyer_address: Some(params.buyer_address.clone()),
            inscription_utxo_sats: output0_value,
        };

        if let Err(e) = self.postgres.add_pending_tx(&pending_tx).await {
            tracing::error!("Failed to add tx_id {} to pending tracking: {:?}", tx_id, e);
        }

        let listing_id = Uuid::new_v4().to_string();

        Ok(ListResponse {
            id: listing_id,
            tx_id,
            name: params.name,
            price_sats: params.new_price,
            seller_address: params.buyer_address,
        })
    }

    pub async fn relist(
        &self,
        request: &RelistRequest,
        caller_address: &str,
    ) -> Result<RelistResponse> {
        // Get listing from database
        let old_listing = self
            .postgres
            .get_listed_listing_by_name(&request.name)
            .await?
            .ok_or_else(|| BadRequest(format!("Listing for '{}' not found", &request.name)))?;

        // Check seller
        if old_listing.seller_address != caller_address {
            return Err(BadRequest(format!(
                "The listing named {} is not belong to you, you can't relist it",
                &request.name
            )));
        }
        self.validate_price(request.name.as_str(), request.new_price)
            .await?;

        // Call canister to relist
        self.ic_agent
            .relist_name(request.name.as_str(), request.new_price)
            .await?;

        // Update old listing to 'relisted' and create new 'listed' record
        {
            // Update old listing to 'relisted' with new_price_sats
            self.postgres
                .update_listing_to_relisted(&old_listing.id, request.new_price)
                .await?;

            // Create new listing with 'listed' status
            let now = Utc::now();
            let new_listing = crate::domain::Listing {
                id: Uuid::new_v4().to_string(),
                name: request.name.clone(),
                seller_address: old_listing.seller_address.clone(),
                price_sats: request.new_price,
                status: crate::domain::ListingStatus::Listed,
                listed_at: now,
                updated_at: now,
                previous_price_sats: old_listing.price_sats,
                tx_id: old_listing.tx_id.clone(),
                buyer_address: None,
                new_price_sats: None,
                inscription_utxo_sats: old_listing.inscription_utxo_sats,
            };

            if let Err(e) = self.postgres.create_listing(&new_listing).await {
                tracing::error!("Failed to create new listing for relist: {:?}", e);
            }

            // Update Redis for the new listing
            self.event_service
                .update_listing_rankings(&new_listing)
                .await;
        }

        let resp = RelistResponse {
            name: request.name.clone(),
            new_price: request.new_price,
        };
        Ok(resp)
    }

    pub async fn buy_and_delist(
        &self,
        request: &BuyAndDelistRequest,
        caller_address: &str,
    ) -> Result<ListResponse> {
        // === Step 1: Get first intention ===
        let intention = request
            .intention_set
            .intentions
            .first()
            .ok_or_else(|| AppError::BadRequest("No intention in intention_set".to_string()))?;

        // === Step 2: Parse PSBT and call extract_tx() ===
        let psbt = parse_psbt(&request.psbt_hex)?;
        let _tx = psbt
            .clone()
            .extract_tx()
            .map_err(|e| AppError::BadRequest(format!("Failed to extract tx from PSBT: {}", e)))?;

        // === Step 3: Parse intention.action_params ===
        let params: BuyAndDelistParams = serde_json::from_str(&intention.action_params)
            .map_err(|e| AppError::BadRequest(format!("Invalid action_params JSON: {}", e)))?;

        // === Step 4: Query database get_pool_address(params.name) ===
        let db_pool_address = self
            .postgres
            .get_pool_address(&params.name)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!(
                    "Pool address for '{}' not found in cache",
                    params.name
                ))
            })?;

        // === Step 5: Check intention.pool_address == database pool_address ===
        if intention.pool_address != db_pool_address {
            return Err(AppError::BadRequest(format!(
                "Pool address mismatch: intention has '{}', database has '{}'",
                intention.pool_address, db_pool_address
            )));
        }

        tracing::info!("Processing buy_and_delist: name={}", params.name);

        // === Step 6: Verify name ownership - params.name belongs to db_pool_address ===
        self.user_service
            .verify_name_ownership(&params.name, &db_pool_address)
            .await?;

        // === Step 7: Verify params.buyer_address = caller_address ===
        if params.buyer_address != caller_address {
            return Err(AppError::Forbidden(format!(
                "Caller address {} does not match buyer address {}",
                caller_address, params.buyer_address
            )));
        }

        // Get listing from database (needed for Step 8)
        let db_listing = self
            .postgres
            .get_listed_listing_by_name(&params.name)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!("Listing for '{}' not found", params.name))
            })?;

        //verify action_parmas:
        let fee_sats = max(330, db_listing.price_sats * 2 / 100);
        if fee_sats != params.fee_sats || params.payment_sats != db_listing.price_sats - fee_sats {
            return Err(AppError::BadRequest(
                "fee_sats + payment_sats != price_sats".to_string(),
            ));
        }

        // === Step 8: PSBT Validation ===
        BuyAndDelistValidator::validate_psbt(&psbt, &db_pool_address, Some(&db_listing), &params)?;

        // === Step 9: Inscription Validation ===
        let prev_out = &psbt.unsigned_tx.input[0].previous_output;
        let input0_outpoint = format!("{}:{}", prev_out.txid, prev_out.vout);
        self.validate_inscription(&params.name, &input0_outpoint)
            .await?;

        // === Step 10: (empty for buy_and_delist) ===

        tracing::info!(
            "Processing buy_and_delist: name={}, buyer={}",
            params.name,
            params.buyer_address
        );

        // Create ByteBuf from raw Vec<u8> (no base64 decode needed)
        let proof_bytes = request.initiator_utxo_proof.clone();

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

        let pending_tx = PendingTx {
            tx_id: tx_id.clone(),
            created_at: Utc::now(),
            name: params.name.clone(),
            action: PendingTxAction::BuyAndDelist,
            status: PendingTxStatus::Submitted,
            previous_price_sats: None,
            price_sats: None,
            seller_address: Some(db_listing.seller_address),
            buyer_address: Some(params.buyer_address.clone()),
            inscription_utxo_sats: BuyAndDelistValidator::get_input0_value(
                &psbt.inputs[0],
                &psbt.unsigned_tx.input[0],
            )
            .unwrap_or_default(),
        };

        if let Err(e) = self.postgres.add_pending_tx(&pending_tx).await {
            tracing::error!("Failed to add tx_id {} to pending tracking: {:?}", tx_id, e);
        }

        let listing_id = Uuid::new_v4().to_string();

        Ok(ListResponse {
            id: listing_id,
            tx_id,
            name: params.name,
            price_sats: 0,
            seller_address: params.buyer_address,
        })
    }

    pub async fn delist(
        &self,
        request: &DelistRequest,
        caller_address: &str,
    ) -> Result<DelistResponse> {
        // === Step 1: Get first intention ===
        let intention = request
            .intention_set
            .intentions
            .first()
            .ok_or_else(|| AppError::BadRequest("No intention in intention_set".to_string()))?;

        // === Step 2: Parse PSBT and call extract_tx() ===
        let psbt = parse_psbt(&request.psbt_hex)?;
        let _tx = psbt
            .clone()
            .extract_tx()
            .map_err(|e| AppError::BadRequest(format!("Failed to extract tx from PSBT: {}", e)))?;

        // === Step 3: Parse intention.action_params ===
        let params: DelistParams = serde_json::from_str(&intention.action_params)
            .map_err(|e| AppError::BadRequest(format!("Invalid action_params JSON: {}", e)))?;

        // === Step 4: Query database get_pool_address(params.name) ===
        let db_pool_address = self
            .postgres
            .get_pool_address(&params.name)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!(
                    "Pool address for '{}' not found in cache",
                    params.name
                ))
            })?;

        // === Step 5: Check intention.pool_address == database pool_address ===
        if intention.pool_address != db_pool_address {
            return Err(AppError::BadRequest(format!(
                "Pool address mismatch: intention has '{}', database has '{}'",
                intention.pool_address, db_pool_address
            )));
        }

        tracing::info!("Processing delist: name={}", params.name);

        // === Step 6: Verify name ownership - params.name belongs to db_pool_address ===
        self.user_service
            .verify_name_ownership(&params.name, &db_pool_address)
            .await?;

        // === Step 7: (empty for delist) ===

        // Get listing from database (needed for Step 8 and seller verification)
        let db_listing = self
            .postgres
            .get_listed_listing_by_name(&params.name)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!("Listing for '{}' not found", params.name))
            })?;

        // Verify caller is the seller of this listing
        if db_listing.seller_address != caller_address {
            return Err(AppError::Forbidden(format!(
                "Only the seller can delist. Caller {} is not the seller {}",
                caller_address, db_listing.seller_address
            )));
        }

        // === Step 8: PSBT Validation ===
        DelistValidator::validate_psbt(&psbt, &db_pool_address, Some(&db_listing), &params)?;

        // === Step 9: Inscription Validation ===
        let prev_out = &psbt.unsigned_tx.input[0].previous_output;
        let input0_outpoint = format!("{}:{}", prev_out.txid, prev_out.vout);
        self.validate_inscription(&params.name, &input0_outpoint)
            .await?;

        // === Step 10: (empty for delist) ===

        // Create ByteBuf from raw Vec<u8> (no base64 decode needed)
        let proof_bytes = request.initiator_utxo_proof.clone();

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

        let pending_tx = PendingTx {
            tx_id: tx_id.clone(),
            created_at: Utc::now(),
            name: params.name.clone(),
            action: PendingTxAction::Delist,
            status: PendingTxStatus::Submitted,
            previous_price_sats: None,
            price_sats: None,
            seller_address: Some(db_listing.seller_address),
            buyer_address: None,
            inscription_utxo_sats: DelistValidator::get_input0_value(
                &psbt.inputs[0],
                &psbt.unsigned_tx.input[0],
            )
            .unwrap_or_default(),
        };

        if let Err(e) = self.postgres.add_pending_tx(&pending_tx).await {
            tracing::error!("Failed to add tx_id {} to pending tracking: {:?}", tx_id, e);
        }

        let listing_id = Uuid::new_v4().to_string();

        Ok(DelistResponse {
            id: listing_id,
            tx_id,
            name: params.name,
        })
    }

    // ========================================================================
    // List Name
    // ========================================================================

    /// List a name for sale via orchestrator canister
    ///
    /// Flow:
    /// 1. Get first intention
    /// 2. Parse PSBT and call extract_tx()
    /// 3. Parse intention.action_params
    /// 4. Query database get_pool_address(params.name)
    /// 5. Check intention.pool_address == database pool_address
    /// 6. Validate PSBT, check duplicate, validate price and inscription
    /// 7. Build InvokeArgs and call orchestrator.invoke()
    /// 8. Store tx_id in database for tracking
    /// 9. Return response with tx_id
    ///
    /// Note: Listing is NOT saved to PostgreSQL here.
    /// The background get_events task will save it when status becomes Pending.
    pub async fn list(&self, request: &ListRequest, caller_address: &str) -> Result<ListResponse> {
        // === Step 1: Get first intention ===
        let intention = request
            .intention_set
            .intentions
            .first()
            .ok_or_else(|| AppError::BadRequest("No intention in intention_set".to_string()))?;

        // === Step 2: Parse PSBT and call extract_tx() ===
        let psbt = parse_psbt(&request.psbt_hex)?;
        let _tx = psbt
            .clone()
            .extract_tx()
            .map_err(|e| AppError::BadRequest(format!("Failed to extract tx from PSBT: {}", e)))?;

        // === Step 3: Parse intention.action_params ===
        let params: ListNameParams = serde_json::from_str(&intention.action_params)
            .map_err(|e| AppError::BadRequest(format!("Invalid action_params JSON: {}", e)))?;

        // === Step 4: Query database get_pool_address(params.name) ===
        let db_pool_address = self
            .postgres
            .get_pool_address(&params.name)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!(
                    "Pool address for '{}' not found in cache",
                    params.name
                ))
            })?;

        // === Step 5: Check intention.pool_address == database pool_address ===
        if intention.pool_address != db_pool_address {
            return Err(AppError::BadRequest(format!(
                "Pool address mismatch: intention has '{}', database has '{}'",
                intention.pool_address, db_pool_address
            )));
        }

        tracing::info!(
            "Processing list: name={}, price={}, seller={}",
            params.name,
            params.price,
            params.seller_address
        );

        // === Step 6: Verify name ownership - params.name belongs to caller_address ===
        self.user_service
            .verify_name_ownership(&params.name, caller_address)
            .await?;

        // === Step 7: Verify params.seller_address = caller_address ===
        if params.seller_address != caller_address {
            return Err(AppError::Forbidden(format!(
                "Caller address {} does not match seller address {}",
                caller_address, params.seller_address
            )));
        }

        // === Step 8: PSBT Validation ===
        ListValidator::validate_psbt(&psbt, &db_pool_address, None, &params)?;

        // === Step 9: Inscription Validation ===
        let prev_out = &psbt.unsigned_tx.input[0].previous_output;
        let input0_outpoint = format!("{}:{}", prev_out.txid, prev_out.vout);
        self.validate_inscription(&params.name, &input0_outpoint)
            .await?;

        // === Step 10: Price Validation ===
        self.validate_price(params.name.as_str(), params.price)
            .await?;

        tracing::debug!(
            "Pool address {} verified for name '{}'",
            intention.pool_address,
            params.name
        );

        let previous_price_sats = self.calculate_previous_price(params.name.as_str()).await;
        // Build InvokeArgs
        let invoke_args = InvokeArgs {
            client_info: None,
            intention_set: request.intention_set.clone(),
            initiator_utxo_proof: serde_bytes::ByteBuf::from(request.initiator_utxo_proof.clone()),
            psbt_hex: request.psbt_hex.clone(),
        };

        // Call orchestrator canister invoke
        let tx_id = self.ic_agent.invoke(invoke_args).await?;

        tracing::info!(
            "Invoke succeeded for name '{}', tx_id: {}",
            params.name,
            tx_id
        );
        let output0_value = psbt.unsigned_tx.output[0].value.to_sat();
        let pending_tx = PendingTx {
            tx_id: tx_id.clone(),
            created_at: Utc::now(),
            name: params.name.clone(),
            action: PendingTxAction::List,
            status: PendingTxStatus::Submitted,
            previous_price_sats,
            price_sats: Some(params.price),
            seller_address: Some(params.seller_address.clone()),
            buyer_address: None,
            inscription_utxo_sats: output0_value,
        };

        if let Err(e) = self.postgres.add_pending_tx(&pending_tx).await {
            tracing::error!("Failed to add tx_id {} to pending tracking: {:?}", tx_id, e);
        }

        let listing_id = Uuid::new_v4().to_string();

        Ok(ListResponse {
            id: listing_id,
            tx_id,
            name: params.name,
            price_sats: params.price,
            seller_address: params.seller_address,
        })
    }

    // === Calculate previous_price_sats ===
    // Use last bought price (bought_and_relisted or bought_and_delisted) if exists,
    // otherwise calculate from etching fee and whether init list
    pub async fn calculate_previous_price(&self, name: &str) -> Option<u64> {
        match self.postgres.get_last_bought_price(name).await {
            Ok(Some(price)) => {
                tracing::info!(
                    "Name {} has previous bought price {}, using as previous_price_sats",
                    name,
                    price
                );
                Some(price)
            }
            Ok(None) => {
                // First time listing - calculate from etching fee
                self.event_service.calculate_etching_fee_price(name).await
            }
            Err(e) => {
                tracing::warn!("Failed to get last bought price for {}: {:?}", name, e);
                self.event_service.calculate_etching_fee_price(name).await
            }
        }
    }

    pub async fn get_listing(&self, name: &str) -> Result<GetListingResponse> {
        let listing = self
            .postgres
            .get_listed_listing_by_name(&name)
            .await?
            .map(|l| ListingInfo::from(l));
        let pool = self.postgres.get_pool_address(name).await?;
        let price = self.calculate_previous_price(&name).await;
        let fee_sats = if listing.is_some() {
            let fee = max(330, listing.clone().unwrap().price_sats * 2 / 100);
            Some(fee)
        } else {
            None
        };
        Ok(GetListingResponse {
            listing,
            last_price_sat: price.unwrap_or(GLOBAL_MIN_PRICE),
            pool_address: pool,
            fee_sats,
        })
    }

    pub async fn name_price_range(&self, name: &str) -> Result<ListingPriceRangeResponse> {
        let previous_price_sats = self
            .calculate_previous_price(name)
            .await
            .unwrap_or(GLOBAL_MIN_PRICE);
        let max = previous_price_sats * 126 / 100;
        Ok(ListingPriceRangeResponse {
            min: GLOBAL_MIN_PRICE.max(max / 100),
            max,
        })
    }

    // ========================================================================
    // Get Listings
    // ========================================================================

    /// Get all listed names with pagination
    pub async fn get_listings(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<ListingsResponse> {
        let limit = limit.unwrap_or(50).min(100);
        let offset = offset.unwrap_or(0);

        let (listings, total) = self.postgres.get_all_listings(limit, offset).await?;

        let listing_infos: Vec<ListingInfo> =
            listings.into_iter().map(|l| ListingInfo::from(l)).collect();

        Ok(ListingsResponse {
            listings: listing_infos,
            total,
        })
    }

    pub async fn get_name_history(
        &self,
        name: &str,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<NameHistoriesResponse> {
        let limit = limit.unwrap_or(20).min(50);
        let offset = offset.unwrap_or(0);

        let (listings, total) = self.postgres.get_name_history(name, limit, offset).await?;
        let mut histories = vec![];
        for listing in listings {
            let history = NameDealHistory {
                seller_address: listing.seller_address,
                buyer_address: listing.buyer_address.unwrap_or_default(),
                price_sats: listing.price_sats,
                time: listing.updated_at,
            };
            histories.push(history);
        }
        Ok(NameHistoriesResponse {
            listings: histories,
            total,
        })
    }
    // ========================================================================
    // Get user history
    // ========================================================================
    pub async fn get_user_history(
        &self,
        user: &str,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<UserHistoriesResponse> {
        let limit = limit.unwrap_or(20).min(50);
        let offset = offset.unwrap_or(0);

        let (listings, total) = self.postgres.get_user_history(user, limit, offset).await?;
        let mut histories = vec![];
        for listing in listings {
            let price_sats = if listing.status == ListingStatus::Delisted {
                None
            } else {
                Some(listing.price_sats)
            };
            let id = listing.id.clone();
            let time = listing.listed_at;
            let action = match listing.status {
                ListingStatus::Listed => UserAction::List,
                ListingStatus::BoughtAndRelisted | ListingStatus::BoughtAndDelisted => {
                    if listing.seller_address == user {
                        UserAction::Sell
                    } else {
                        UserAction::Buy
                    }
                }
                ListingStatus::Relisted => UserAction::List,
                ListingStatus::Delisted => UserAction::Delist,
                ListingStatus::List => UserAction::List,
            }
            .to_string();
            let name = listing.name.clone();
            let mut status = PendingTxStatus::Finalized;
            match self
                .postgres
                .get_pending_tx_by_id(listing.tx_id.as_str())
                .await
            {
                Ok(Some(tx)) => {
                    status = tx.status;
                }
                Err(e) => {
                    tracing::error!("Failed to get pending tx: {:?}", e);
                }
                _ => {}
            }
            histories.push(ListingHistory {
                id,
                name,
                action,
                price_sats,
                time,
                status: status.to_string(),
            });
        }
        Ok(UserHistoriesResponse {
            listings: histories,
            total,
        })
    }

    /// Get newest listings from Redis (for real-time display)
    pub async fn get_new_listings(&self, count: usize) -> Result<Vec<NewListingItem>> {
        self.redis.get_new_listings(count).await
    }

    /// Validate that input[0] contains the inscription for the given name
    async fn validate_inscription(&self, name: &str, input0_outpoint: &str) -> Result<()> {
        // Step 1: Get inscription_id for this name
        let rune_info = self.blockchain.ord_bns_rune(name).await?.ok_or_else(|| {
            AppError::BadRequest(format!("Name '{}' not found in ord backend", name))
        })?;

        let expected_inscription_id = &rune_info.inscription_id;
        tracing::debug!(
            "Name '{}' has inscription_id: {}",
            name,
            expected_inscription_id
        );

        // Step 2: Get inscriptions on input[0] output
        let output_data = self
            .blockchain
            .ord_output(input0_outpoint)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!(
                    "Output {} not found in ord backend",
                    input0_outpoint
                ))
            })?;

        // Step 3: Verify inscription_id is in the output's inscriptions
        let inscriptions = output_data.inscriptions.unwrap_or_default();

        if !inscriptions.contains(expected_inscription_id) {
            return Err(AppError::BadRequest(format!(
                "Input[0] ({}) does not contain the inscription for name '{}'. Expected inscription: {}, found: {:?}",
                input0_outpoint, name, expected_inscription_id, inscriptions
            )));
        }

        tracing::debug!(
            "Inscription validation passed: input[0] ({}) contains inscription {} for name '{}'",
            input0_outpoint,
            expected_inscription_id,
            name
        );

        Ok(())
    }

    /// Validate listing price is within acceptable range
    async fn validate_price(&self, name: &str, price: u64) -> Result<()> {
        let price_range = self.name_price_range(name).await?;
        if price < price_range.min || price > price_range.max {
            return Err(AppError::BadRequest(format!(
                "Price {} sats isn't between {} and {} sats",
                price, price_range.min, price_range.max
            )));
        }
        Ok(())
    }
}

pub type DynTradingService = Arc<TradingService>;
