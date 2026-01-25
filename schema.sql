-- BNS Server Database Schema
-- Generated from migrations 001-009
-- This file represents the current database schema and can be used to initialize a new database

-- Users table
CREATE TABLE IF NOT EXISTS users (
    btc_address VARCHAR(100) PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    primary_name VARCHAR(64)
);

-- Listings table
-- Status values: listed, bought_and_relisted, bought_and_delisted, relisted, delisted
CREATE TABLE IF NOT EXISTS listings (
    id VARCHAR(36) PRIMARY KEY,
    name VARCHAR(64) NOT NULL,
    seller_address VARCHAR(100) NOT NULL,
    price_sats BIGINT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'listed',
    listed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    previous_price_sats BIGINT,
    tx_id VARCHAR(64),
    buyer_address VARCHAR(100),
    new_price_sats BIGINT,
    CONSTRAINT listings_name_status_key UNIQUE (name, status)
);

CREATE INDEX IF NOT EXISTS idx_listings_seller ON listings(seller_address);
CREATE INDEX IF NOT EXISTS idx_listings_status ON listings(status);
CREATE INDEX IF NOT EXISTS idx_listings_name ON listings(name);
CREATE INDEX IF NOT EXISTS idx_listings_tx_id ON listings(tx_id);
CREATE INDEX IF NOT EXISTS idx_listings_buyer ON listings(buyer_address);

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
-- status: submitted (initial), pending, finalized, confirmed, rejected
-- action: list, buy_and_relist, buy_and_delist, delist
CREATE TABLE IF NOT EXISTS pending_txs (
    tx_id VARCHAR(100) PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    name VARCHAR(64) NOT NULL,
    action VARCHAR(20) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'submitted',
    previous_price_sats BIGINT,
    price_sats BIGINT,
    seller_address VARCHAR(100),
    buyer_address VARCHAR(100)
);

CREATE INDEX IF NOT EXISTS idx_pending_txs_created ON pending_txs(created_at);
CREATE INDEX IF NOT EXISTS idx_pending_txs_status ON pending_txs(status);
CREATE INDEX IF NOT EXISTS idx_pending_txs_name ON pending_txs(name);
