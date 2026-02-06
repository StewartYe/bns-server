use crate::AppError;
use crate::config::CONFIG;
use crate::domain::{ShoutOut, ShoutOutRequest, ShoutOutStatus};
use crate::error::Result;
use crate::infra::{DynBlockchainClient, DynPostgresClient};
use crate::service::trading_validators::parse_psbt;
use base64::Engine;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;

pub struct ShoutOutService {
    pub blockchain: DynBlockchainClient,
    pub postgres: DynPostgresClient,
}

impl ShoutOutService {
    pub fn new(blockchain: DynBlockchainClient, postgres: DynPostgresClient) -> Self {
        Self {
            blockchain,
            postgres,
        }
    }

    pub async fn shout_out(&self, user: &str, req: ShoutOutRequest) -> Result<()> {
        let psbt = parse_psbt(&req.psbt_hex)?;
        let _tx = psbt
            .clone()
            .extract_tx()
            .map_err(|e| AppError::BadRequest(format!("Failed to extract tx from PSBT: {}", e)))?;

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

        let mut total_payto_platform = 0;
        for (_, output) in psbt.unsigned_tx.output.iter().enumerate() {
            let output_addr =
                bitcoin::Address::from_script(&output.script_pubkey, CONFIG.bitcoin_network())
                    .map_err(|e| AppError::BadRequest(e.to_string()))?
                    .to_string();
            if output_addr == CONFIG.fee_collector {
                total_payto_platform += output.value.to_sat();
            }
        }
        if total_payto_platform < 10_000 {
            return Err(AppError::BadRequest("fee is too low".to_owned()));
        }
        let db_listing = self
            .postgres
            .get_listed_listing_by_name(&req.name)
            .await?
            .ok_or_else(|| AppError::BadRequest(format!("Listing for '{}' not found", req.name)))?;
        if db_listing.seller_address != user {
            return Err(AppError::Forbidden(
                "The listing of the name is not yours".to_string(),
            ));
        }
        let psbt_base64 =
            base64::engine::general_purpose::STANDARD.encode(psbt.serialize().as_slice());
        self.blockchain.broadcast_psbt(&psbt_base64).await?;
        let now = Utc::now();
        let shout_out = ShoutOut {
            tx_id: psbt.unsigned_tx.compute_txid().to_string(),
            listing_name: req.name,
            user_address: user.to_string(),
            ad_words: req.ad_words,
            status: ShoutOutStatus::Pending.to_string(),
            price: db_listing.price_sats as i64,
            created_at: now,
            updated_at: now,
        };
        self.postgres.insert_shout_out(&shout_out).await?;
        Ok(())
    }

    pub async fn polling_loop(&self) {
        loop {
            tokio::time::sleep(Duration::from_secs(20)).await;
            if let Ok(pending_shout_outs) = self.postgres.get_pending_shout_out().await {
                for shout_out in pending_shout_outs {
                    if let Ok(Some(confirms)) = self
                        .blockchain
                        .get_transaction_confirmations(shout_out.tx_id.as_str())
                        .await
                    {
                        if confirms > 0 {
                            let _ = self
                                .postgres
                                .confirm_shout_out(shout_out.tx_id.as_str())
                                .await;
                        } else {
                            tracing::debug!("Transaction confirmations 0: {}", shout_out.tx_id);
                        }
                    } else {
                        tracing::warn!("Failed to confirm shout transaction: {}", shout_out.tx_id);
                    }
                }
            }
        }
    }

    pub fn start_confirming(self: Arc<Self>) {
        tokio::spawn(async move {
            self.polling_loop().await;
        });
    }
}

pub type DynShoutOutService = Arc<ShoutOutService>;
