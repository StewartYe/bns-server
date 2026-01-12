//! Application configuration
//!
//! Loaded from environment variables.

use std::env;

/// Network type (testnet or mainnet)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Testnet,
    Mainnet,
}

impl Network {
    /// Get the Redis key prefix for this network
    pub fn key_prefix(&self) -> &'static str {
        match self {
            Network::Testnet => "testnet",
            Network::Mainnet => "mainnet",
        }
    }
}

impl std::fmt::Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Network::Testnet => write!(f, "testnet"),
            Network::Mainnet => write!(f, "mainnet"),
        }
    }
}

/// Redis/Valkey configuration
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// Redis host
    pub host: String,
    /// Redis port
    pub port: u16,
    /// Use TLS
    pub tls: bool,
    /// Use IAM authentication
    pub use_iam: bool,
    /// Service account for IAM auth
    pub service_account: Option<String>,
    /// CA certificate file path for TLS
    pub ca_file_path: Option<String>,
}

/// IC (Internet Computer) configuration
#[derive(Debug, Clone)]
pub struct IcConfig {
    /// Identity PEM content (secp256k1 private key in PEM format)
    pub identity_pem: String,
    /// BNS canister ID
    pub bns_canister_id: String,
}

impl RedisConfig {
    /// Build Redis connection URL
    pub fn connection_url(&self) -> String {
        let scheme = if self.tls { "rediss" } else { "redis" };
        format!("{}://{}:{}", scheme, self.host, self.port)
    }
}

/// Application configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Server port
    pub port: u16,

    /// Network (testnet or mainnet)
    pub network: Network,

    /// PostgreSQL URL (required)
    pub database_url: String,

    /// Bitcoin Core RPC URL (required for broadcasting transactions)
    pub bitcoind_url: String,

    /// Ord indexer URL (required for resolve_rune/resolve_address)
    pub ord_url: Option<String>,

    /// Redis/Valkey configuration
    pub redis: RedisConfig,

    /// IC (Internet Computer) configuration
    pub ic: IcConfig,

    /// Session TTL in seconds
    pub session_ttl_secs: i64,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url = env::var("DATABASE_URL")
            .map_err(|_| ConfigError::Missing("DATABASE_URL"))?;

        let bitcoind_url = env::var("BITCOIND_URL")
            .map_err(|_| ConfigError::Missing("BITCOIND_URL"))?;

        let redis_host = env::var("REDIS_HOST")
            .map_err(|_| ConfigError::Missing("REDIS_HOST"))?;

        let network = match env::var("NETWORK").as_deref() {
            Ok("mainnet") => Network::Mainnet,
            _ => Network::Testnet, // Default to testnet
        };

        let redis = RedisConfig {
            host: redis_host,
            port: env::var("REDIS_PORT")
                .unwrap_or_else(|_| "6379".to_string())
                .parse()
                .unwrap_or(6379),
            tls: env::var("REDIS_TLS")
                .map(|v| v == "true")
                .unwrap_or(false),
            use_iam: env::var("REDIS_USE_IAM")
                .map(|v| v == "true")
                .unwrap_or(false),
            service_account: env::var("REDIS_SERVICE_ACCOUNT").ok(),
            ca_file_path: env::var("REDIS_CA_FILE_PATH").ok(),
        };

        // IC configuration
        let ic_identity_pem = env::var("IC_IDENTITY_PEM")
            .map_err(|_| ConfigError::Missing("IC_IDENTITY_PEM"))?;
        let bns_canister_id = env::var("BNS_CANISTER_ID")
            .map_err(|_| ConfigError::Missing("BNS_CANISTER_ID"))?;

        let ic = IcConfig {
            identity_pem: ic_identity_pem,
            bns_canister_id,
        };

        Ok(Self {
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidPort)?,

            network,

            database_url,

            bitcoind_url,

            ord_url: env::var("ORD_URL")
                .or_else(|_| env::var("ORD_BACKEND_URL"))
                .ok(),

            redis,

            ic,

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
