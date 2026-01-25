use crate::AppError;
use crate::domain::Listing;
use crate::error::Result;
use crate::service::trading_validators::TradingValidator;
use bitcoin::Psbt;

pub struct ListValidator;
impl TradingValidator for ListValidator {
    fn validate_psbt(
        psbt: &Psbt,
        _initiator_address: &str,
        pool_address: &str,
        _name: &str,
        _listing: Option<&Listing>,
    ) -> Result<()> {
        let unsigned_tx = &psbt.unsigned_tx;
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
        //Validate ouput0 address
        Self::validate_output0_address(output0, pool_address)?;
        //Validate input0.value = output0.value
        Self::validate_input0_output0_value(&psbt.inputs[0], &unsigned_tx.input[0], output0)?;

        let prev_out = &unsigned_tx.input[0].previous_output;
        let input0_outpoint = format!("{}:{}", prev_out.txid, prev_out.vout);
        tracing::debug!(
            "PSBT validation passed: 2 inputs, 2 outputs, all signed, output[0]={} sats to {}, input[0]={}",
            output0.value.to_sat(),
            pool_address,
            input0_outpoint
        );

        Ok(())
    }
}
