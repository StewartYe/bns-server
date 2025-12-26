//! Application configuration
//!
//! Loaded from environment variables.

use std::env;

/// Application configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Server port
    pub port: u16,

    /// PostgreSQL URL (Cloud SQL) - optional for proxy-only mode
    pub database_url: Option<String>,

    /// Ord indexer URL (required for resolve_rune/resolve_address)
    pub ord_url: Option<String>,

    /// Session TTL in seconds
    pub session_ttl_secs: i64,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidPort)?,

            database_url: env::var("DATABASE_URL").ok(),

            ord_url: env::var("ORD_URL")
                .or_else(|_| env::var("ORD_BACKEND_URL"))
                .ok(),

            session_ttl_secs: env::var("SESSION_TTL_SECS")
                .unwrap_or_else(|_| "86400".to_string()) // 24 hours
                .parse()
                .unwrap_or(86400),
        })
    }
}

/// Configuration error
#[derive(Debug)]
pub enum ConfigError {
    Missing(&'static str),
    InvalidPort,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Missing(var) => write!(f, "Missing environment variable: {}", var),
            ConfigError::InvalidPort => write!(f, "Invalid PORT value"),
        }
    }
}

impl std::error::Error for ConfigError {}
