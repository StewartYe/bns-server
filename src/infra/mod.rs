//! Infrastructure layer
//!
//! External system clients:
//! - Redis: Rankings, caching, sessions, pub/sub
//! - PostgreSQL: Persistent storage
//! - Canister: ICP smart contract interactions
//! - Blockchain: Ord indexer and Bitcoin fullnode
//! - BIP-322: Bitcoin message signature verification

pub mod bip322;
pub mod bns_canister;
pub mod orchestrator_canister;
mod blockchain;
mod canister;
mod postgres;
mod redis;

pub use blockchain::*;
pub use canister::*;
pub use postgres::*;
pub use redis::*;
