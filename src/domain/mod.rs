//! Domain models for BNS Server
//!
//! Core domain objects:
//! - Name: Rune name entity
//! - Listing: Market listing entity
//! - User: User identity (Bitcoin address via BIP-322)
//! - Event: Canister event queue events
//! - ShoutOut: Promotional messages

mod event;
mod listing;
mod name;
mod shoutout;
mod user;

pub use event::*;
pub use listing::*;
pub use name::*;
pub use shoutout::*;
pub use user::*;
