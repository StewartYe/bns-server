//! Application constants
//!
//! Shared constants used throughout the application.

/// Minimum number of confirmations required before a name can be:
/// - Set as primary name
/// - Have metadata updated
/// - Be listed for sale
pub const FINALIZE_THRESHOLD: u64 = 3;

/// Session cookie name
pub const SESSION_COOKIE_NAME: &str = "bns_session";
