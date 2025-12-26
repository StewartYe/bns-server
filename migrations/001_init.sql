-- BNS Server Database Schema
-- Migration 001: Initial schema

-- Users table
CREATE TABLE IF NOT EXISTS users (
    btc_address VARCHAR(100) PRIMARY KEY,
    principal VARCHAR(100) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for principal lookups
CREATE INDEX IF NOT EXISTS idx_users_principal ON users(principal);

-- Sessions table
CREATE TABLE IF NOT EXISTS sessions (
    session_id VARCHAR(36) PRIMARY KEY,
    btc_address VARCHAR(100) NOT NULL REFERENCES users(btc_address) ON DELETE CASCADE,
    principal VARCHAR(100) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL
);

-- Index for session cleanup
CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);
CREATE INDEX IF NOT EXISTS idx_sessions_btc_address ON sessions(btc_address);

-- Listings table (for future use)
CREATE TABLE IF NOT EXISTS listings (
    id VARCHAR(36) PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    seller_address VARCHAR(100) NOT NULL,
    pool_address VARCHAR(100) NOT NULL,
    price_sats BIGINT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    listed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    previous_price_sats BIGINT
);

CREATE INDEX IF NOT EXISTS idx_listings_seller ON listings(seller_address);
CREATE INDEX IF NOT EXISTS idx_listings_status ON listings(status);
CREATE INDEX IF NOT EXISTS idx_listings_name ON listings(name);

-- Transaction history table (for future use)
CREATE TABLE IF NOT EXISTS transactions (
    id SERIAL PRIMARY KEY,
    btc_address VARCHAR(100) NOT NULL,
    tx_type VARCHAR(20) NOT NULL,
    name VARCHAR(255) NOT NULL,
    price_sats BIGINT,
    counterparty VARCHAR(100),
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    tx_id VARCHAR(100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_transactions_address ON transactions(btc_address);
CREATE INDEX IF NOT EXISTS idx_transactions_status ON transactions(status);

-- Canister events table (for future use)
CREATE TABLE IF NOT EXISTS canister_events (
    event_id VARCHAR(100) PRIMARY KEY,
    event_type VARCHAR(50) NOT NULL,
    name VARCHAR(255) NOT NULL,
    tx_id VARCHAR(100),
    addresses TEXT[], -- Array of affected addresses
    data JSONB,
    processed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_events_type ON canister_events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_name ON canister_events(name);

-- ShoutOuts table (for future use)
CREATE TABLE IF NOT EXISTS shoutouts (
    id VARCHAR(36) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    promoter_address VARCHAR(100) NOT NULL,
    message TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE INDEX IF NOT EXISTS idx_shoutouts_active ON shoutouts(is_active, expires_at);
