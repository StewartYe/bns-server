//! Domain models for BNS Server
//!
//! Core domain objects:
//! - Name: Rune name entity
//! - Trading: Marketplace trading entities (listings, pools)
//! - User: User identity (Bitcoin address via BIP-322)

mod marketing;
mod name;
mod trading;
mod user;

pub use marketing::*;
pub use name::*;
pub use trading::*;
pub use user::*;
