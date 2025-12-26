//! Service layer
//!
//! Business logic services:
//! - AuthService: SIWB authentication and session management
//! - NameService: Name resolution, search, details
//! - TradingService: List, delist, buy operations
//! - MarketService: Rankings, statistics
//! - UserService: Inventory, history
//! - ShoutOutService: Promotional messages
//! - EventService: Canister event synchronization

mod auth;
mod event;
mod market;
mod name;
mod shoutout;
mod trading;
mod user;

pub use auth::*;
pub use event::*;
pub use market::*;
pub use name::*;
pub use shoutout::*;
pub use trading::*;
pub use user::*;
