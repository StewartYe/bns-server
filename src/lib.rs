//! BNS Server - Bitcoin Name Service
//!
//! A Rust web server providing:
//! - BIP-322 authentication (Sign-In With Bitcoin)
//! - Name resolution (forward/reverse)
//! - Trading marketplace (list/delist/buy) [future]
//! - Market rankings and statistics [future]
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                          API Layer                              │
//! │  Auth (BIP-322) │ SDK API │ REST API │ WebSocket                │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                       Service Layer                             │
//! │  Auth │ Name │ Trading │ Market │ User │ ShoutOut │ Event       │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                    Infrastructure Layer                         │
//! │  PostgreSQL │ BIP-322 │ Redis │ Canister │ Blockchain           │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

pub mod api;
pub mod config;
pub mod domain;
pub mod error;
pub mod infra;
pub mod service;
pub mod state;

pub use config::Config;
pub use error::{AppError, Result};
pub use state::AppState;
