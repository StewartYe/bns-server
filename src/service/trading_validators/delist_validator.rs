use crate::AppError;
use crate::domain::Listing;
use crate::service::trading_validators::TradingValidator;
use bitcoin::Psbt;

pub struct DelistValidator;

impl TradingValidator for DelistValidator {
    fn validate_psbt(
        psbt: &Psbt,
        _initiator_address: &str,
        pool_address: &str,
        _name: &str,
        listing: Option<&Listing>,
    ) -> crate::Result<()> {
        let db_listing = listing.ok_or(AppError::BadRequest("Listing not found".to_owned()))?;
        let unsigned_tx = &psbt.unsigned_tx;

        // Verify outputs count >= 2
        if unsigned_tx.output.len() < 2 {
            return Err(AppError::BadRequest(format!(
                "PSBT must have exactly 2 more outputs, got {}",
                unsigned_tx.output.len()
            )));
        }

        // Verify all inputs are signed (except input 0)
        for (i, input) in psbt.inputs.iter().enumerate() {
            if i == 0 {
                continue;
            }
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

        // Verify outputs[0] goes to seller_address
        let output0 = &unsigned_tx.output[0];
        //Validate ouput0 address (goes back to seller)
        Self::validate_output0_address(output0, &db_listing.seller_address)?;
        // Verify outputs[0].sats == inputs[0].sats
        Self::validate_input0_output0_value(&psbt.inputs[0], &unsigned_tx.input[0], output0)?;
        //Validate input0 is the listing's outpoint
        Self::validate_input0_utxo(&unsigned_tx.input[0], db_listing)?;

        let expected_outpoint = format!("{}:0", db_listing.tx_id);
        tracing::debug!(
            "PSBT validation passed: {} inputs, {} outputs, all signed, output[0]={} sats to {}, input[0]={}",
            psbt.inputs.len(),
            psbt.outputs.len(),
            output0.value.to_sat(),
            pool_address,
            expected_outpoint
        );
        Ok(())
    }
}
