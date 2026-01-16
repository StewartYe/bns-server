//! Listing service
//!
//! Handles list_name operations via orchestrator canister invoke.

use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use bitcoin::Address;
use bitcoin::psbt::Psbt;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{
    ListNameParams, ListNameRequest, ListNameResponse, ListedNamesResponse, ListingInfo,
};
use crate::error::{AppError, Result};
use crate::infra::orchestrator_canister::InvokeArgs;
use crate::infra::{DynPostgresClient, DynRedisClient, IcAgent, ListingMeta};

/// BNS Rune info from ord backend
#[derive(Debug, Serialize, Deserialize)]
pub struct BnsRuneInfo {
    pub address: String,
    pub inscription_id: String,
    pub rune_id: String,
    pub etching: String,
    pub inscription_number: i32,
    pub confirmations: u32,
}

/// BNS Rune response from /bns/rune/{rune}
#[derive(Debug, Serialize, Deserialize)]
pub struct BnsRuneResponse {
    pub result: Option<BnsRuneInfo>,
}

/// Ord output response from /output/{output}
#[derive(Debug, Serialize, Deserialize)]
pub struct OrdOutputResponse {
    pub address: Option<String>,
    pub confirmations: u32,
    pub indexed: bool,
    pub inscriptions: Option<Vec<String>>,
    pub outpoint: String,
    pub spent: bool,
    pub value: u64,
}

/// Listing service
pub struct ListingService {
    ic_agent: Arc<IcAgent>,
    postgres: DynPostgresClient,
    redis: DynRedisClient,
    http_client: reqwest::Client,
    ord_url: Option<String>,
}

impl ListingService {
    pub fn new(
        ic_agent: Arc<IcAgent>,
        postgres: DynPostgresClient,
        redis: DynRedisClient,
        http_client: reqwest::Client,
        ord_url: Option<String>,
    ) -> Self {
        Self {
            ic_agent,
            postgres,
            redis,
            http_client,
            ord_url,
        }
    }

    /// List a name for sale via orchestrator canister
    ///
    /// Flow:
    /// 1. Parse and validate PSBT
    /// 2. Verify pool_address belongs to the name
    /// 3. Check for duplicate pending listings
    /// 4. Validate inscription ownership
    /// 5. Build InvokeArgs and call orchestrator.invoke()
    /// 6. Store tx_id in database for tracking
    /// 7. Return response with tx_id
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

        // === PSBT Validation ===
        // Parse and validate PSBT, returns the input[0] outpoint for inscription validation
        let input0_outpoint = self
            .validate_psbt(&request.psbt_hex, &intention.pool_address)
            .await?;

        // === Pool Address Validation ===
        // Verify pool_address belongs to this name
        let pool_info = self.ic_agent.get_pool_info(&intention.pool_address).await?;
        if pool_info.name != params.name {
            return Err(AppError::BadRequest(format!(
                "Pool address {} belongs to name '{}', not '{}'",
                intention.pool_address, pool_info.name, params.name
            )));
        }
        tracing::debug!(
            "Pool address {} verified for name '{}'",
            intention.pool_address,
            params.name
        );

        // === Duplicate Listing Check ===
        // Check if there's already a pending listing for this name
        let pending_txs = self.postgres.get_pending_txs().await?;
        for (_tx_id, data) in &pending_txs {
            if let Ok(tracking_data) = serde_json::from_str::<serde_json::Value>(data) {
                if tracking_data["name"].as_str() == Some(&params.name) {
                    return Err(AppError::BadRequest(format!(
                        "Name '{}' already has a pending listing transaction",
                        params.name
                    )));
                }
            }
        }

        // === Price Validation ===
        self.validate_price(params.price)?;

        // === Inscription Validation ===
        // Verify inputs[0] contains the inscription for this name
        self.validate_inscription(&params.name, &input0_outpoint)
            .await?;

        // Decode initiator_utxo_proof from base64
        let proof_bytes = STANDARD
            .decode(&request.initiator_utxo_proof)
            .map_err(|e| {
                AppError::BadRequest(format!("Invalid initiator_utxo_proof base64: {}", e))
            })?;

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

        // Store tx_id in database for tracking by get_events background task
        let tracking_data = serde_json::json!({
            "name": params.name,
            "price": params.price,
            "seller_address": params.seller_address,
            "pool_address": intention.pool_address,
            "submitted_at": Utc::now().to_rfc3339(),
        });

        if let Err(e) = self
            .postgres
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

    /// Validate PSBT structure and content
    ///
    /// Checks:
    /// - PSBT can be parsed
    /// - inputs count == 2, outputs count == 2
    /// - All inputs are signed
    /// - outputs[0] goes to pool_address
    /// - outputs[0].sats == inputs[0].sats
    ///
    /// Returns the input[0] outpoint (txid:vout) for inscription validation
    async fn validate_psbt(&self, psbt_hex: &str, pool_address: &str) -> Result<String> {
        // Parse PSBT from hex
        let psbt_bytes = hex::decode(psbt_hex)
            .map_err(|e| AppError::BadRequest(format!("Invalid PSBT hex: {}", e)))?;

        let psbt = Psbt::deserialize(&psbt_bytes)
            .map_err(|e| AppError::BadRequest(format!("Failed to parse PSBT: {}", e)))?;

        // Verify inputs count == 2
        if psbt.inputs.len() != 2 {
            return Err(AppError::BadRequest(format!(
                "PSBT must have exactly 2 inputs, got {}",
                psbt.inputs.len()
            )));
        }

        // Verify outputs count == 2
        let unsigned_tx = &psbt.unsigned_tx;
        if unsigned_tx.output.len() != 2 {
            return Err(AppError::BadRequest(format!(
                "PSBT must have exactly 2 outputs, got {}",
                unsigned_tx.output.len()
            )));
        }

        // Verify all inputs are signed
        for (i, input) in psbt.inputs.iter().enumerate() {
            let is_signed = input.final_script_sig.is_some()
                || input.final_script_witness.is_some()
                || !input.partial_sigs.is_empty()
                || input.tap_key_sig.is_some()
                || !input.tap_script_sigs.is_empty();

            if !is_signed {
                return Err(AppError::BadRequest(format!(
                    "PSBT input {} is not signed",
                    i
                )));
            }
        }

        // Verify outputs[0] goes to pool_address
        let output0 = &unsigned_tx.output[0];
        let output0_address =
            Address::from_script(&output0.script_pubkey, bitcoin::Network::Bitcoin)
                .map_err(|e| AppError::BadRequest(format!("Invalid output[0] script: {}", e)))?;

        if output0_address.to_string() != pool_address {
            return Err(AppError::BadRequest(format!(
                "PSBT output[0] address '{}' does not match pool_address '{}'",
                output0_address, pool_address
            )));
        }

        // Verify outputs[0].sats == inputs[0].sats
        // Need to get the input value from witness_utxo or non_witness_utxo
        let input0 = &psbt.inputs[0];
        let input0_value = if let Some(ref witness_utxo) = input0.witness_utxo {
            witness_utxo.value.to_sat()
        } else if let Some(ref non_witness_utxo) = input0.non_witness_utxo {
            let vout = unsigned_tx.input[0].previous_output.vout as usize;
            non_witness_utxo.output[vout].value.to_sat()
        } else {
            return Err(AppError::BadRequest(
                "PSBT input[0] missing witness_utxo or non_witness_utxo".to_string(),
            ));
        };

        let output0_value = output0.value.to_sat();
        if output0_value != input0_value {
            return Err(AppError::BadRequest(format!(
                "PSBT output[0] value ({} sats) does not match input[0] value ({} sats)",
                output0_value, input0_value
            )));
        }

        // Get input[0] outpoint (txid:vout) for inscription validation
        let prev_out = &unsigned_tx.input[0].previous_output;
        let input0_outpoint = format!("{}:{}", prev_out.txid, prev_out.vout);

        tracing::debug!(
            "PSBT validation passed: 2 inputs, 2 outputs, all signed, output[0]={} sats to {}, input[0]={}",
            output0_value,
            pool_address,
            input0_outpoint
        );

        Ok(input0_outpoint)
    }

    /// Validate that input[0] contains the inscription for the given name
    ///
    /// Steps:
    /// 1. Query ord /bns/rune/{name} to get the inscription_id for this name
    /// 2. Query ord /output/{outpoint} to get the inscriptions on input[0]
    /// 3. Verify the inscription_id from step 1 is in the list from step 2
    async fn validate_inscription(&self, name: &str, input0_outpoint: &str) -> Result<()> {
        let ord_url = self
            .ord_url
            .as_ref()
            .ok_or_else(|| AppError::Internal("Ord backend URL not configured".to_string()))?;

        // Step 1: Get inscription_id for this name
        let rune_url = format!("{}/bns/rune/{}", ord_url, name);
        tracing::debug!("Querying ord for rune info: {}", rune_url);

        let rune_response = self
            .http_client
            .get(&rune_url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to query ord /bns/rune: {}", e)))?;

        if !rune_response.status().is_success() {
            return Err(AppError::BadRequest(format!(
                "Name '{}' not found in ord backend",
                name
            )));
        }

        let rune_data: BnsRuneResponse = rune_response.json().await.map_err(|e| {
            AppError::Internal(format!("Failed to parse ord /bns/rune response: {}", e))
        })?;

        let rune_info = rune_data
            .result
            .ok_or_else(|| AppError::BadRequest(format!("Name '{}' not found", name)))?;

        let expected_inscription_id = &rune_info.inscription_id;
        tracing::debug!(
            "Name '{}' has inscription_id: {}",
            name,
            expected_inscription_id
        );

        // Step 2: Get inscriptions on input[0] output
        let output_url = format!("{}/output/{}", ord_url, input0_outpoint);
        tracing::debug!("Querying ord for output info: {}", output_url);

        let output_response = self
            .http_client
            .get(&output_url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to query ord /output: {}", e)))?;

        if !output_response.status().is_success() {
            return Err(AppError::BadRequest(format!(
                "Output {} not found in ord backend",
                input0_outpoint
            )));
        }

        let output_data: OrdOutputResponse = output_response.json().await.map_err(|e| {
            AppError::Internal(format!("Failed to parse ord /output response: {}", e))
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
    ///
    /// TODO: Implement specific price validation rules
    fn validate_price(&self, price: u64) -> Result<()> {
        // Minimum price check (e.g., 1000 sats = 0.00001 BTC)
        const MIN_PRICE: u64 = 1000;
        if price < MIN_PRICE {
            return Err(AppError::BadRequest(format!(
                "Price {} sats is below minimum {} sats",
                price, MIN_PRICE
            )));
        }

        // TODO: Add maximum price check if needed
        // TODO: Add other price validation rules

        Ok(())
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
