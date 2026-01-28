//! Service layer
//!
//! Business logic services:
//! - AuthService: BIP-322 authentication and session management
//! - NameService: Name resolution
//! - UserService: User operations and name metadata
//! - TradingService: Marketplace trading (pools, listings)
//! - EventService: Canister event polling and transaction status updates

mod auth;
mod event;
mod marketing;
mod name;
mod trading;
mod trading_validators;
mod user;

pub use auth::*;
pub use event::*;
pub use marketing::*;
pub use name::*;
pub use trading::*;
pub use user::*;
