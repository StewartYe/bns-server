//! BNS Server - Bitcoin Name Service
//!
//! A Rust web server providing:
//! - BIP-322 authentication (Sign-In With Bitcoin)
//! - Name resolution (forward/reverse)
//! - Name listing marketplace
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                          API Layer                              │
//! │  Auth (BIP-322) │ SDK API │ Rankings │ WebSocket                │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                       Service Layer                             │
//! │  Auth │ Name │ Event │ Listing                                  │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                    Infrastructure Layer                         │
//! │  PostgreSQL │ BIP-322 │ Redis │ Canister │ Blockchain           │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

pub mod api;
pub mod config;
pub mod constants;
pub mod domain;
pub mod error;
pub mod infra;
pub mod service;
pub mod state;

pub use config::Config;
pub use error::{AppError, Result};
pub use state::AppState;
