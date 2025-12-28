//! BNS Server - Bitcoin Name Service
//!
//! Main entry point.

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use bns_server::{
    api,
    config::Config,
    service::{AuthConfig, AuthService},
    state::AppState,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,bns_server=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    dotenvy::dotenv().ok();
    let config = Config::from_env()?;

    tracing::info!("Starting BNS Server on port {}", config.port);

    if let Some(ord_url) = &config.ord_url {
        tracing::info!("Ord backend URL: {}", ord_url);
    } else {
        tracing::warn!("No Ord backend URL configured - name resolution will not work");
    }

    // Create HTTP client for Ord backend
    let http_client = reqwest::Client::new();

    // Connect to PostgreSQL
    tracing::info!("Connecting to PostgreSQL...");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;
    tracing::info!("Connected to PostgreSQL");

    // Run migrations
    tracing::info!("Running database migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("Migrations complete");

    // Initialize auth service
    let auth_config = AuthConfig {
        session_ttl_secs: config.session_ttl_secs,
    };
    let auth_service = Arc::new(AuthService::new(pool.clone(), auth_config));

    // Create application state
    let state = AppState::new(config.clone(), http_client, auth_service, pool);

    // Build router
    let app = api::build_router(state)
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    // Start server
    let addr = format!("0.0.0.0:{}", config.port);
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
