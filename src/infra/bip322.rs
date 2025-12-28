//! BIP-322 Signature Verification
//!
//! Verifies BIP-322 "simple" signatures for Bitcoin message signing.
//! This is the standard used by UniSat and other modern wallets.

use crate::error::{AppError, Result};

/// The domain used in sign-in messages
pub const SIGN_IN_DOMAIN: &str = "bns.zone";

/// Maximum allowed time drift (5 minutes)
const MAX_TIME_DRIFT_SECS: i64 = 5 * 60;

/// Parsed sign-in message components
struct ParsedMessage {
    timestamp: i64,
    nonce: String,
}

/// Parse the sign-in message to extract timestamp and nonce
///
/// Message format: "Sign in to bns.zone at {timestamp} with nonce {nonce}"
fn parse_sign_in_message(message: &str) -> Result<ParsedMessage> {
    let prefix = format!("Sign in to {} at ", SIGN_IN_DOMAIN);
    let middle = " with nonce ";

    // Check prefix
    if !message.starts_with(&prefix) {
        return Err(AppError::BadRequest(format!(
            "Invalid message format: must start with '{}'",
            prefix
        )));
    }

    // Find the middle part
    let rest = &message[prefix.len()..];
    let parts: Vec<&str> = rest.splitn(2, middle).collect();
    if parts.len() != 2 {
        return Err(AppError::BadRequest(
            "Invalid message format: missing 'with nonce'".into(),
        ));
    }

    // Parse timestamp
    let timestamp: i64 = parts[0].parse().map_err(|_| {
        AppError::BadRequest(format!("Invalid timestamp in message: '{}'", parts[0]))
    })?;

    let nonce = parts[1].to_string();

    Ok(ParsedMessage { timestamp, nonce })
}

/// Verify a BIP-322 signature
///
/// ## Parameters
/// - `address`: Bitcoin address (P2WPKH, P2TR, etc.)
/// - `message`: The message that was signed (format: "Sign in to bns.zone at {timestamp} with nonce {nonce}")
/// - `signature`: Base64-encoded BIP-322 signature
pub fn verify_bip322_signature(address: &str, message: &str, signature: &str) -> Result<()> {
    // Step 1: Parse message to extract timestamp and nonce
    let parsed = parse_sign_in_message(message)?;

    // Step 2: Validate nonce format
    validate_nonce(&parsed.nonce)?;

    // Step 3: Check timestamp is within acceptable range
    let now = chrono::Utc::now().timestamp();
    let diff = (now - parsed.timestamp).abs();
    if diff > MAX_TIME_DRIFT_SECS {
        return Err(AppError::Unauthorized(format!(
            "Timestamp expired or invalid: diff={}s (max={}s)",
            diff, MAX_TIME_DRIFT_SECS
        )));
    }

    // Step 4: Verify BIP-322 signature using the bip322 crate
    bip322::verify_simple_encoded(address, message, signature).map_err(|e| {
        tracing::warn!(
            "BIP-322 signature verification failed for address {}: {:?}",
            address,
            e
        );
        AppError::Unauthorized(format!("Invalid BIP-322 signature: {:?}", e))
    })?;

    tracing::info!("BIP-322 signature verified for address: {}", address);
    Ok(())
}

/// Validate nonce format
fn validate_nonce(nonce: &str) -> Result<()> {
    // Nonce should be 8-64 characters, alphanumeric
    if nonce.len() < 8 || nonce.len() > 64 {
        return Err(AppError::BadRequest(format!(
            "Invalid nonce length: {} (expected 8-64)",
            nonce.len()
        )));
    }

    // Allow alphanumeric and hyphen
    if !nonce.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(AppError::BadRequest(
            "Invalid nonce format: must be alphanumeric".into(),
        ));
    }

    Ok(())
}

/// Generate expected message for signing (utility for clients)
pub fn generate_sign_in_message(timestamp: i64, nonce: &str) -> String {
    format!(
        "Sign in to {} at {} with nonce {}",
        SIGN_IN_DOMAIN, timestamp, nonce
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_sign_in_message() {
        let message = generate_sign_in_message(1735344000, "abc12345");
        assert_eq!(
            message,
            "Sign in to bns.zone at 1735344000 with nonce abc12345"
        );
    }

    #[test]
    fn test_parse_sign_in_message() {
        let message = "Sign in to bns.zone at 1735344000 with nonce abc12345";
        let parsed = parse_sign_in_message(message).unwrap();
        assert_eq!(parsed.timestamp, 1735344000);
        assert_eq!(parsed.nonce, "abc12345");
    }

    #[test]
    fn test_parse_sign_in_message_invalid() {
        // Wrong prefix
        assert!(parse_sign_in_message("Login to bns.zone at 123 with nonce abc").is_err());
        // Missing nonce
        assert!(parse_sign_in_message("Sign in to bns.zone at 123").is_err());
        // Invalid timestamp
        assert!(parse_sign_in_message("Sign in to bns.zone at abc with nonce 12345678").is_err());
    }

    #[test]
    fn test_validate_nonce() {
        // Valid nonces
        assert!(validate_nonce("abc12345").is_ok());
        assert!(validate_nonce("a1b2c3d4-e5f6").is_ok());
        assert!(validate_nonce("0123456789abcdef").is_ok());

        // Invalid nonces
        assert!(validate_nonce("short").is_err()); // too short
        assert!(validate_nonce("").is_err()); // empty
        assert!(validate_nonce("has spaces here").is_err()); // spaces
    }
}
