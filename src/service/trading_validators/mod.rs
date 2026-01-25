use crate::AppError;
use crate::AppError::BadRequest;
use crate::config::CONFIG;
use crate::domain::Listing;
use crate::error::Result;
use bitcoin::psbt::Input;
use bitcoin::{Address, Psbt, TxIn, TxOut};

pub mod buy_and_delist_validator;
pub mod buy_and_relist_validator;
pub mod delist_validator;
pub mod list_validator;

/// Parse PSBT from hex string (shared utility function)
pub fn parse_psbt(psbt_hex: &str) -> Result<Psbt> {
    let psbt_bytes = hex::decode(psbt_hex)
        .map_err(|e| AppError::BadRequest(format!("Invalid PSBT hex: {}", e)))?;

    let psbt = Psbt::deserialize(&psbt_bytes)
        .map_err(|e| AppError::BadRequest(format!("Failed to parse PSBT: {}", e)))?;
    Ok(psbt)
}

pub trait TradingValidator {
    fn validate_psbt(
        psbt: &Psbt,
        initiator_address: &str,
        pool_address: &str,
        name: &str,
        listing: Option<&Listing>,
    ) -> Result<()>;

    fn validate_input0_output0_value(input0: &Input, txin0: &TxIn, output0: &TxOut) -> Result<()> {
        // Verify outputs[0].sats == inputs[0].sats
        let input0_value = Self::get_input0_value(input0, txin0)?;
        let output0_value = output0.value.to_sat();
        if output0_value != input0_value {
            return Err(AppError::BadRequest(format!(
                "PSBT output[0] value ({} sats) does not match input[0] value ({} sats)",
                output0_value, input0_value
            )));
        }
        Ok(())
    }

    fn validate_output0_address(output0: &TxOut, address: &str) -> Result<()> {
        let output0_address =
            Address::from_script(&output0.script_pubkey, CONFIG.bitcoin_network())
                .map_err(|e| AppError::BadRequest(format!("Invalid output[0] script: {}", e)))?;

        if output0_address.to_string() != address.to_string() {
            return Err(AppError::BadRequest(format!(
                "PSBT output[0] address '{}' does not match pool_address '{}'",
                output0_address, address
            )));
        }
        Ok(())
    }

    fn validate_input0_utxo(txin0: &TxIn, listing: &Listing) -> Result<()> {
        let prev_out = &txin0.previous_output;
        let input0_outpoint = format!("{}:{}", prev_out.txid, prev_out.vout);
        let expected_outpoint = listing
            .tx_id
            .as_ref()
            .map(|tx_id| format!("{}:0", tx_id))
            .unwrap_or_default();
        if input0_outpoint != expected_outpoint {
            return Err(AppError::BadRequest(format!(
                "input0 outpoint '{}' does not match listing outpoint '{}'",
                input0_outpoint, expected_outpoint
            )));
        }
        Ok(())
    }

    fn validate_payto_seller(output1: &TxOut, listing: &Listing) -> Result<()> {
        let output1_address =
            Address::from_script(&output1.script_pubkey, CONFIG.bitcoin_network())
                .map_err(|e| BadRequest(format!("Invalid output1 script: {}", e)))?;
        if output1_address.to_string() != listing.seller_address
            || output1.value.to_sat() != listing.price_sats
        {
            return Err(BadRequest(
                "output1 don't pay to seller or value is incorrect".to_owned(),
            ));
        }
        Ok(())
    }

    fn get_input0_value(input0: &Input, txin0: &TxIn) -> Result<u64> {
        let input0_value = if let Some(ref witness_utxo) = input0.witness_utxo {
            witness_utxo.value.to_sat()
        } else if let Some(ref non_witness_utxo) = input0.non_witness_utxo {
            let vout = txin0.previous_output.vout as usize;
            non_witness_utxo.output[vout].value.to_sat()
        } else {
            return Err(AppError::BadRequest(
                "PSBT input[0] missing witness_utxo or non_witness_utxo".to_string(),
            ));
        };
        Ok(input0_value)
    }
}
