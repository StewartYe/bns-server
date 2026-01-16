-- BNS Server Database Schema
-- Generated from migrations 001-007
-- This file represents the current database schema and can be used to initialize a new database

-- Users table
CREATE TABLE IF NOT EXISTS users (
    btc_address VARCHAR(100) PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    primary_name VARCHAR(64)
);

-- Listings table
CREATE TABLE IF NOT EXISTS listings (
    id VARCHAR(36) PRIMARY KEY,
    name VARCHAR(64) NOT NULL UNIQUE,
    seller_address VARCHAR(100) NOT NULL,
    pool_address VARCHAR(100) NOT NULL,
    price_sats BIGINT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    listed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    previous_price_sats BIGINT,
    tx_id VARCHAR(64)
);

CREATE INDEX IF NOT EXISTS idx_listings_seller ON listings(seller_address);
CREATE INDEX IF NOT EXISTS idx_listings_status ON listings(status);
CREATE INDEX IF NOT EXISTS idx_listings_name ON listings(name);
CREATE INDEX IF NOT EXISTS idx_listings_tx_id ON listings(tx_id);

-- Transaction history table
CREATE TABLE IF NOT EXISTS transactions (
    id SERIAL PRIMARY KEY,
    btc_address VARCHAR(100) NOT NULL,
    tx_type VARCHAR(20) NOT NULL,
    name VARCHAR(64) NOT NULL,
    price_sats BIGINT,
    counterparty VARCHAR(100),
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    tx_id VARCHAR(100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_transactions_address ON transactions(btc_address);
CREATE INDEX IF NOT EXISTS idx_transactions_status ON transactions(status);

-- Canister events table
CREATE TABLE IF NOT EXISTS canister_events (
    event_id VARCHAR(100) PRIMARY KEY,
    event_type VARCHAR(50) NOT NULL,
    name VARCHAR(64) NOT NULL,
    tx_id VARCHAR(100),
    addresses TEXT[],
    data JSONB,
    processed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_events_type ON canister_events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_name ON canister_events(name);

-- ShoutOuts table
CREATE TABLE IF NOT EXISTS shoutouts (
    id VARCHAR(36) PRIMARY KEY,
    name VARCHAR(64) NOT NULL,
    promoter_address VARCHAR(100) NOT NULL,
    message TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE INDEX IF NOT EXISTS idx_shoutouts_active ON shoutouts(is_active, expires_at);

-- Name metadata table
CREATE TABLE IF NOT EXISTS name_metadata (
    name VARCHAR(64) PRIMARY KEY,
    owner_address VARCHAR(100) NOT NULL,
    description TEXT,
    url VARCHAR(500),
    twitter VARCHAR(100),
    email VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_name_metadata_owner ON name_metadata(owner_address);

-- System state table (for event polling offset, etc.)
CREATE TABLE IF NOT EXISTS system_state (
    key VARCHAR(50) PRIMARY KEY,
    value_int BIGINT,
    value_text TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Pending transactions table (tracking tx_ids waiting for canister events)
CREATE TABLE IF NOT EXISTS pending_txs (
    tx_id VARCHAR(100) PRIMARY KEY,
    tracking_data JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pending_txs_created ON pending_txs(created_at);
