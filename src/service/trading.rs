//! Trading service
//!
//! Handles trading operations:
//! - Get pool address for listing
//! - List name for sale
//! - Get all listings
//! - Delist, Relist, Buy

use crate::AppError::BadRequest;
use crate::GLOBAL_MIN_PRICE;
use crate::api::rankings::NewListingItem;
use crate::domain::{
    BuyAndDelistParams, BuyAndDelistRequest, BuyAndRelistParams, BuyAndRelistRequest, DelistParams,
    DelistRequest, DelistResponse, GetListingResponse, GetPoolRequest, GetPoolResponse,
    ListNameParams, ListRequest, ListResponse, Listing, ListingInfo, ListingsResponse,
    NameDealHistory, NameHistoriesResponse, RelistRequest, RelistResponse, TradeAction,
    TradeHistoryItem, TradeRecord, TradeStatus, UserHistoriesResponse, UserSession,
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

    pub async fn get_pool(
        &self,
        request: &GetPoolRequest,
        caller_address: &str,
    ) -> Result<GetPoolResponse> {
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

        self.user_service
            .verify_name_ownership(&request.name, caller_address)
            .await?;

        let pool_address = match self.ic_agent.create_pool(&request.name).await {
            Ok(addr) => addr,
            Err(AppError::Canister(err))
                if err.contains("already exists") || err.contains("Pool exists") =>
            {
                return Err(AppError::PoolAlreadyExists(request.name.clone()));
            }
            Err(e) => return Err(e),
        };

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
        let intention = request
            .intention_set
            .intentions
            .first()
            .ok_or_else(|| AppError::BadRequest("No intention in intention_set".to_string()))?;

        let psbt = parse_psbt(&request.psbt_hex)?;
        let _tx = psbt
            .clone()
            .extract_tx()
            .map_err(|e| AppError::BadRequest(format!("Failed to extract tx from PSBT: {}", e)))?;

        let params: BuyAndRelistParams = serde_json::from_str(&intention.action_params)
            .map_err(|e| AppError::BadRequest(format!("Invalid action_params JSON: {}", e)))?;

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
        if params.buyer_address != caller_address {
            return Err(AppError::Forbidden(format!(
                "Caller address {} does not match buyer address {}",
                caller_address, params.buyer_address
            )));
        }

        let db_listing = self
            .postgres
            .get_listed_listing_by_name(&params.name)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!("Listing for '{}' not found", params.name))
            })?;

        let fee_sats = self
            .calculate_platform_fee(&db_listing, &params.buyer_address)
            .await?;
        if fee_sats != params.fee_sats || params.payment_sats != db_listing.price_sats - fee_sats {
            return Err(AppError::BadRequest(
                "fee_sats + payment_sats != price_sats".to_string(),
            ));
        }

        BuyAndRelistValidator::validate_psbt(&psbt, &db_pool_address, Some(&db_listing), &params)?;

        self.validate_price(db_listing.price_sats, params.new_price)?;

        let proof_bytes = request.initiator_utxo_proof.clone();
        let invoke_args = InvokeArgs {
            client_info: None,
            intention_set: request.intention_set.clone(),
            initiator_utxo_proof: serde_bytes::ByteBuf::from(proof_bytes),
            psbt_hex: request.psbt_hex.clone(),
        };

        let tx_id = self.ic_agent.invoke(invoke_args).await?;

        tracing::info!(
            "Invoke succeeded for name '{}', tx_id: {}",
            params.name,
            tx_id
        );
        let output0_value = psbt.unsigned_tx.output[0].value.to_sat();

        // Create trade history record
        let now = Utc::now();
        let trade_record = TradeRecord {
            id: Uuid::new_v4().to_string(),
            name: params.name.clone(),
            who: params.buyer_address.clone(),
            action: TradeAction::BuyAndRelist,
            tx_id: Some(tx_id.clone()),
            created_at: now,
            updated_at: now,
            status: TradeStatus::Submitted,
            seller_address: Some(db_listing.seller_address),
            previous_price_sats: Some(db_listing.price_sats),
            price_sats: Some(params.new_price),
            inscription_utxo_sats: output0_value,
            buyer_address: Some(params.buyer_address.clone()),
            platform_fee: Some(fee_sats),
        };

        if let Err(e) = self.postgres.add_trade_record(&trade_record).await {
            tracing::error!("Failed to add trade record for tx_id {}: {:?}", tx_id, e);
        }

        Ok(ListResponse {
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
        let old_listing = self
            .postgres
            .get_listed_listing_by_name(&request.name)
            .await?
            .ok_or_else(|| BadRequest(format!("Listing for '{}' not found", &request.name)))?;

        if old_listing.seller_address != caller_address {
            return Err(BadRequest(format!(
                "The listing named {} is not belong to you, you can't relist it",
                &request.name
            )));
        }

        // Calculate previous price for validation
        let previous_price_sats = self
            .calculate_previous_price(request.name.as_str())
            .await
            .unwrap_or(old_listing.price_sats);
        self.validate_price(previous_price_sats, request.new_price)?;

        // Call canister to relist
        self.ic_agent
            .relist_name(request.name.as_str(), request.new_price)
            .await?;

        // Create trade history record (status=finalized, no tx_id)
        let now = Utc::now();
        let trade_record = TradeRecord {
            id: Uuid::new_v4().to_string(),
            name: request.name.clone(),
            who: old_listing.seller_address.clone(),
            action: TradeAction::Relist,
            tx_id: None,
            created_at: now,
            updated_at: now,
            status: TradeStatus::Finalized,
            seller_address: Some(old_listing.seller_address.clone()),
            previous_price_sats: Some(old_listing.price_sats),
            price_sats: Some(request.new_price),
            inscription_utxo_sats: old_listing.inscription_utxo_sats,
            buyer_address: None,
            platform_fee: None,
        };

        if let Err(e) = self.postgres.add_trade_record(&trade_record).await {
            tracing::error!("Failed to add trade record for relist: {:?}", e);
        }

        // Update listing price
        if let Err(e) = self
            .postgres
            .update_listing_price(&request.name, request.new_price)
            .await
        {
            tracing::error!("Failed to update listing price for relist: {:?}", e);
        }

        // Re-read the updated listing for ranking updates
        if let Ok(Some(updated_listing)) = self
            .postgres
            .get_listed_listing_by_name(&request.name)
            .await
        {
            self.event_service
                .update_listing_rankings(&updated_listing, previous_price_sats)
                .await;
        }

        Ok(RelistResponse {
            name: request.name.clone(),
            new_price: request.new_price,
        })
    }

    pub async fn buy_and_delist(
        &self,
        request: &BuyAndDelistRequest,
        caller_address: &str,
    ) -> Result<ListResponse> {
        let intention = request
            .intention_set
            .intentions
            .first()
            .ok_or_else(|| AppError::BadRequest("No intention in intention_set".to_string()))?;

        let psbt = parse_psbt(&request.psbt_hex)?;
        let _tx = psbt
            .clone()
            .extract_tx()
            .map_err(|e| AppError::BadRequest(format!("Failed to extract tx from PSBT: {}", e)))?;

        let params: BuyAndDelistParams = serde_json::from_str(&intention.action_params)
            .map_err(|e| AppError::BadRequest(format!("Invalid action_params JSON: {}", e)))?;

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

        if intention.pool_address != db_pool_address {
            return Err(AppError::BadRequest(format!(
                "Pool address mismatch: intention has '{}', database has '{}'",
                intention.pool_address, db_pool_address
            )));
        }

        tracing::info!("Processing buy_and_delist: name={}", params.name);
        if params.buyer_address != caller_address {
            return Err(AppError::Forbidden(format!(
                "Caller address {} does not match buyer address {}",
                caller_address, params.buyer_address
            )));
        }

        let db_listing = self
            .postgres
            .get_listed_listing_by_name(&params.name)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!("Listing for '{}' not found", params.name))
            })?;

        let fee_sats = self
            .calculate_platform_fee(&db_listing, &params.buyer_address)
            .await?;
        if fee_sats != params.fee_sats || params.payment_sats != db_listing.price_sats - fee_sats {
            return Err(AppError::BadRequest(
                "fee_sats + payment_sats != price_sats".to_string(),
            ));
        }

        BuyAndDelistValidator::validate_psbt(&psbt, &db_pool_address, Some(&db_listing), &params)?;
        let proof_bytes = request.initiator_utxo_proof.clone();
        let invoke_args = InvokeArgs {
            client_info: None,
            intention_set: request.intention_set.clone(),
            initiator_utxo_proof: serde_bytes::ByteBuf::from(proof_bytes),
            psbt_hex: request.psbt_hex.clone(),
        };

        let tx_id = self.ic_agent.invoke(invoke_args).await?;

        tracing::info!(
            "Invoke succeeded for name '{}', tx_id: {}",
            params.name,
            tx_id
        );

        // Create trade history record
        let now = Utc::now();
        let trade_record = TradeRecord {
            id: Uuid::new_v4().to_string(),
            name: params.name.clone(),
            who: params.buyer_address.clone(),
            action: TradeAction::BuyAndDelist,
            tx_id: Some(tx_id.clone()),
            created_at: now,
            updated_at: now,
            status: TradeStatus::Submitted,
            seller_address: Some(db_listing.seller_address),
            previous_price_sats: Some(db_listing.price_sats),
            price_sats: Some(db_listing.price_sats),
            inscription_utxo_sats: BuyAndDelistValidator::get_input0_value(
                &psbt.inputs[0],
                &psbt.unsigned_tx.input[0],
            )
            .unwrap_or_default(),
            buyer_address: Some(params.buyer_address.clone()),
            platform_fee: Some(fee_sats),
        };

        if let Err(e) = self.postgres.add_trade_record(&trade_record).await {
            tracing::error!("Failed to add trade record for tx_id {}: {:?}", tx_id, e);
        }

        Ok(ListResponse {
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
        let intention = request
            .intention_set
            .intentions
            .first()
            .ok_or_else(|| AppError::BadRequest("No intention in intention_set".to_string()))?;

        let psbt = parse_psbt(&request.psbt_hex)?;
        let _tx = psbt
            .clone()
            .extract_tx()
            .map_err(|e| AppError::BadRequest(format!("Failed to extract tx from PSBT: {}", e)))?;

        let params: DelistParams = serde_json::from_str(&intention.action_params)
            .map_err(|e| AppError::BadRequest(format!("Invalid action_params JSON: {}", e)))?;

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

        if intention.pool_address != db_pool_address {
            return Err(AppError::BadRequest(format!(
                "Pool address mismatch: intention has '{}', database has '{}'",
                intention.pool_address, db_pool_address
            )));
        }

        tracing::info!("Processing delist: name={}", params.name);
        let db_listing = self
            .postgres
            .get_listed_listing_by_name(&params.name)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!("Listing for '{}' not found", params.name))
            })?;

        if db_listing.seller_address != caller_address {
            return Err(AppError::Forbidden(format!(
                "Only the seller can delist. Caller {} is not the seller {}",
                caller_address, db_listing.seller_address
            )));
        }

        DelistValidator::validate_psbt(&psbt, &db_pool_address, Some(&db_listing), &params)?;

        let proof_bytes = request.initiator_utxo_proof.clone();
        let invoke_args = InvokeArgs {
            client_info: None,
            intention_set: request.intention_set.clone(),
            initiator_utxo_proof: serde_bytes::ByteBuf::from(proof_bytes),
            psbt_hex: request.psbt_hex.clone(),
        };

        let tx_id = self.ic_agent.invoke(invoke_args).await?;

        tracing::info!(
            "Invoke succeeded for name '{}', tx_id: {}",
            params.name,
            tx_id
        );

        // Create trade history record
        let now = Utc::now();
        let trade_record = TradeRecord {
            id: Uuid::new_v4().to_string(),
            name: params.name.clone(),
            who: db_listing.seller_address.clone(),
            action: TradeAction::Delist,
            tx_id: Some(tx_id.clone()),
            created_at: now,
            updated_at: now,
            status: TradeStatus::Submitted,
            seller_address: Some(db_listing.seller_address),
            previous_price_sats: Some(db_listing.price_sats),
            price_sats: Some(db_listing.price_sats),
            inscription_utxo_sats: DelistValidator::get_input0_value(
                &psbt.inputs[0],
                &psbt.unsigned_tx.input[0],
            )
            .unwrap_or_default(),
            buyer_address: None,
            platform_fee: None,
        };

        if let Err(e) = self.postgres.add_trade_record(&trade_record).await {
            tracing::error!("Failed to add trade record for tx_id {}: {:?}", tx_id, e);
        }

        Ok(DelistResponse {
            tx_id,
            name: params.name,
        })
    }

    // ========================================================================
    // List Name
    // ========================================================================

    pub async fn list(&self, request: &ListRequest, caller_address: &str) -> Result<ListResponse> {
        let intention = request
            .intention_set
            .intentions
            .first()
            .ok_or_else(|| AppError::BadRequest("No intention in intention_set".to_string()))?;

        let psbt = parse_psbt(&request.psbt_hex)?;
        let _tx = psbt
            .clone()
            .extract_tx()
            .map_err(|e| AppError::BadRequest(format!("Failed to extract tx from PSBT: {}", e)))?;

        let params: ListNameParams = serde_json::from_str(&intention.action_params)
            .map_err(|e| AppError::BadRequest(format!("Invalid action_params JSON: {}", e)))?;

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

        self.user_service
            .verify_name_ownership(&params.name, caller_address)
            .await?;

        if params.seller_address != caller_address {
            return Err(AppError::Forbidden(format!(
                "Caller address {} does not match seller address {}",
                caller_address, params.seller_address
            )));
        }

        ListValidator::validate_psbt(&psbt, &db_pool_address, None, &params)?;

        let prev_out = &psbt.unsigned_tx.input[0].previous_output;
        let input0_outpoint = format!("{}:{}", prev_out.txid, prev_out.vout);
        self.validate_inscription(&params.name, &input0_outpoint)
            .await?;

        let previous_price_sats = self
            .calculate_previous_price(params.name.as_str())
            .await
            .ok_or(AppError::Internal(
                "Failed to calculate previous price".to_string(),
            ))?;
        self.validate_price(previous_price_sats, params.price)?;

        let invoke_args = InvokeArgs {
            client_info: None,
            intention_set: request.intention_set.clone(),
            initiator_utxo_proof: serde_bytes::ByteBuf::from(request.initiator_utxo_proof.clone()),
            psbt_hex: request.psbt_hex.clone(),
        };

        let tx_id = self.ic_agent.invoke(invoke_args).await?;

        tracing::info!(
            "Invoke succeeded for name '{}', tx_id: {}",
            params.name,
            tx_id
        );
        let output0_value = psbt.unsigned_tx.output[0].value.to_sat();

        // Create trade history record
        let now = Utc::now();
        let trade_record = TradeRecord {
            id: Uuid::new_v4().to_string(),
            name: params.name.clone(),
            who: params.seller_address.clone(),
            action: TradeAction::List,
            tx_id: Some(tx_id.clone()),
            created_at: now,
            updated_at: now,
            status: TradeStatus::Submitted,
            seller_address: Some(params.seller_address.clone()),
            previous_price_sats: Some(previous_price_sats),
            price_sats: Some(params.price),
            inscription_utxo_sats: output0_value,
            buyer_address: None,
            platform_fee: None,
        };

        if let Err(e) = self.postgres.add_trade_record(&trade_record).await {
            tracing::error!("Failed to add trade record for tx_id {}: {:?}", tx_id, e);
        }

        Ok(ListResponse {
            tx_id,
            name: params.name,
            price_sats: params.price,
            seller_address: params.seller_address,
        })
    }

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
            Ok(None) => self.event_service.calculate_etching_fee_price(name).await,
            Err(e) => {
                tracing::warn!("Failed to get last bought price for {}: {:?}", name, e);
                self.event_service.calculate_etching_fee_price(name).await
            }
        }
    }

    pub async fn get_listing(
        &self,
        name: &str,
        session: &UserSession,
    ) -> Result<GetListingResponse> {
        let listing = self.postgres.get_listed_listing_by_name(&name).await?;
        let listing_info = listing.clone().map(|l| ListingInfo::from(l));
        let pool = self.postgres.get_pool_address(name).await?;
        let price = self.calculate_previous_price(&name).await;
        let fee_sats = if listing.is_some() {
            let fee = self
                .calculate_platform_fee(&listing.unwrap(), session.btc_address.as_str())
                .await?;
            Some(fee)
        } else {
            None
        };
        Ok(GetListingResponse {
            listing: listing_info,
            last_price_sat: price.unwrap_or(GLOBAL_MIN_PRICE),
            pool_address: pool,
            fee_sats,
        })
    }

    // ========================================================================
    // Get Listings
    // ========================================================================

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

        let (records, total) = self.postgres.get_name_history(name, limit, offset).await?;
        let histories = records
            .into_iter()
            .map(|r| NameDealHistory {
                seller_address: r.seller_address.unwrap_or_default(),
                buyer_address: r.buyer_address.unwrap_or_default(),
                price_sats: r.price_sats.unwrap_or(0),
                time: r.updated_at,
            })
            .collect();
        Ok(NameHistoriesResponse { histories, total })
    }

    pub async fn get_user_history(
        &self,
        user: &str,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<UserHistoriesResponse> {
        let limit = limit.unwrap_or(20).min(50);
        let offset = offset.unwrap_or(0);

        let (records, total) = self.postgres.get_user_history(user, limit, offset).await?;
        let histories = records
            .into_iter()
            .map(|r| TradeHistoryItem {
                id: r.id,
                name: r.name,
                action: r.action.to_string(),
                price_sats: r.price_sats,
                status: r.status.to_string(),
                time: r.created_at,
            })
            .collect();
        Ok(UserHistoriesResponse { histories, total })
    }

    /// Get newest listings from Redis (for real-time display)
    pub async fn get_new_listings(&self, count: usize) -> Result<Vec<NewListingItem>> {
        self.redis.get_new_listings(count).await
    }

    /// Validate that input[0] contains the inscription for the given name
    async fn validate_inscription(&self, name: &str, input0_outpoint: &str) -> Result<()> {
        let rune_info = self.blockchain.ord_bns_rune(name).await?.ok_or_else(|| {
            AppError::BadRequest(format!("Name '{}' not found in ord backend", name))
        })?;

        let expected_inscription_id = &rune_info.inscription_id;
        tracing::debug!(
            "Name '{}' has inscription_id: {}",
            name,
            expected_inscription_id
        );

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

    fn validate_price(&self, base_price: u64, new_price: u64) -> Result<()> {
        let max = base_price * 126 / 100;
        let min = GLOBAL_MIN_PRICE.max(max / 100);
        if new_price < min || new_price > max {
            return Err(AppError::BadRequest(format!(
                "Price {} sats isn't between {} and {} sats",
                new_price, min, max
            )));
        }
        Ok(())
    }

    async fn calculate_platform_fee(&self, listing: &Listing, buyer: &str) -> Result<u64> {
        let mut seller_level = 0u64;
        if let Some(seller_primary_name) = self
            .postgres
            .get_user(listing.seller_address.as_str())
            .await?
            .unwrap()
            .primary_name
        {
            let points = self
                .postgres
                .get_nft_points(seller_primary_name.as_str())
                .await?;
            let points = if points.is_some() {
                points.unwrap().points
            } else {
                0
            };
            seller_level = Self::points_to_level(points as u64);
        }
        let mut buyer_level = 0u64;
        if let Some(buyer_primary_name) = self.postgres.get_user(buyer).await?.unwrap().primary_name
        {
            let points = self
                .postgres
                .get_nft_points(buyer_primary_name.as_str())
                .await?;
            let points = if points.is_some() {
                points.unwrap().points
            } else {
                0
            };
            buyer_level = Self::points_to_level(points as u64);
        }
        let fee = 330.max(listing.price_sats * (20u64 - seller_level - buyer_level) / 1000);
        Ok(fee)
    }

    fn points_to_level(points: u64) -> u64 {
        if points < 50_000 {
            0
        } else if points < 500_000 {
            1
        } else if points < 5_000_000 {
            2
        } else if points < 50_000_000 {
            3
        } else if points < 500_000_000 {
            4
        } else {
            5
        }
    }
}

pub type DynTradingService = Arc<TradingService>;
