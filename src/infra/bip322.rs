//! BIP-322 Signature Verification
//!
//! Verifies BIP-322 "simple" signatures for Bitcoin message signing.
//! This is the standard used by UniSat and other modern wallets.

use crate::error::{AppError, Result};

/// Verify a BIP-322 signature
///
/// ## Parameters
/// - `address`: Bitcoin address (P2WPKH, P2TR, etc.)
/// - `message`: The message that was signed
/// - `signature`: Base64-encoded BIP-322 signature
/// - `timestamp`: Timestamp in milliseconds
///
/// ## Verification Steps
/// 1. Check timestamp is within 5 minutes
/// 2. Verify message format matches "bns-login:{timestamp}"
/// 3. Verify the BIP-322 signature (TODO: implement full verification)
pub fn verify_bip322_signature(
    address: &str,
    message: &str,
    signature: &str,
    timestamp: i64,
) -> Result<()> {
    // Step 1: Check timestamp is within 5 minutes
    let now = chrono::Utc::now().timestamp_millis();
    let diff = (now - timestamp).abs();
    if diff > 5 * 60 * 1000 {
        return Err(AppError::Unauthorized(format!(
            "Timestamp expired: diff={}ms",
            diff
        )));
    }

    // Step 2: Verify message format
    let expected_message = format!("bns-login:{}", timestamp);
    if message != expected_message {
        return Err(AppError::BadRequest(format!(
            "Invalid message format: expected '{}', got '{}'",
            expected_message, message
        )));
    }

    // Step 3: Verify BIP-322 signature
    // TODO: Implement full BIP-322 verification
    // For now, we do basic validation and log a warning

    // Validate signature is valid base64
    let _signature_bytes = base64_decode(signature)?;

    // Validate address format
    validate_bitcoin_address(address)?;

    tracing::warn!(
        "BIP-322 signature verification not fully implemented - accepting signature for address: {}",
        address
    );

    Ok(())
}

/// Decode base64 signature
fn base64_decode(s: &str) -> Result<Vec<u8>> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD
        .decode(s)
        .map_err(|e| AppError::BadRequest(format!("Invalid base64 signature: {}", e)))
}

/// Validate Bitcoin address format
fn validate_bitcoin_address(address: &str) -> Result<()> {
    // Basic validation for common address formats
    let valid = address.starts_with("bc1")    // mainnet bech32 (P2WPKH, P2WSH, P2TR)
        || address.starts_with("tb1")         // testnet bech32
        || address.starts_with("bcrt1")       // regtest bech32
        || address.starts_with('1')           // mainnet P2PKH
        || address.starts_with('3')           // mainnet P2SH
        || address.starts_with('m')           // testnet P2PKH
        || address.starts_with('n')           // testnet P2PKH
        || address.starts_with('2');          // testnet P2SH

    if !valid {
        return Err(AppError::BadRequest(format!(
            "Invalid Bitcoin address format: {}",
            address
        )));
    }

    // Check length
    if address.len() < 26 || address.len() > 90 {
        return Err(AppError::BadRequest(format!(
            "Invalid Bitcoin address length: {}",
            address.len()
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_bitcoin_address() {
        // Valid addresses
        assert!(validate_bitcoin_address("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq").is_ok());
        assert!(validate_bitcoin_address("tb1q837dfu2xmthlx6a6c59dvw6v4t0erg6c4mn4e2").is_ok());
        assert!(validate_bitcoin_address("1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2").is_ok());

        // Invalid addresses
        assert!(validate_bitcoin_address("invalid").is_err());
        assert!(validate_bitcoin_address("").is_err());
    }
}
