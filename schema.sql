-- BNS Server Database Schema
-- Generated from migrations 001-019
-- This file represents the current database schema and can be used to initialize a new database

-- Users table
CREATE TABLE IF NOT EXISTS users (
    btc_address VARCHAR(100) PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    primary_name VARCHAR(64)
);

-- Listings table (only currently listed names)
CREATE TABLE IF NOT EXISTS listings (
    name VARCHAR(64) PRIMARY KEY,
    seller_address VARCHAR(100) NOT NULL,
    price_sats BIGINT NOT NULL,
    listed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    tx_id VARCHAR(64) NOT NULL UNIQUE,
    inscription_utxo_sats BIGINT NOT NULL DEFAULT 546
);

CREATE INDEX IF NOT EXISTS idx_listings_seller ON listings(seller_address);
CREATE INDEX IF NOT EXISTS idx_listings_tx_id ON listings(tx_id);

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

-- Trade history table (all trade actions for names)
-- status: submitted (default), pending, finalized, confirmed, rejected
-- action: list, delist, relist, buy_and_relist, buy_and_delist
CREATE TABLE IF NOT EXISTS trade_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(64) NOT NULL,
    who VARCHAR(100) NOT NULL,
    action VARCHAR(20) NOT NULL,
    tx_id VARCHAR(100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    status VARCHAR(20) NOT NULL DEFAULT 'submitted',
    seller_address VARCHAR(100),
    previous_price_sats BIGINT,
    price_sats BIGINT,
    inscription_utxo_sats BIGINT NOT NULL DEFAULT 546,
    buyer_address VARCHAR(100),
    platform_fee BIGINT
);

CREATE INDEX IF NOT EXISTS idx_trade_history_name ON trade_history(name);
CREATE INDEX IF NOT EXISTS idx_trade_history_tx_id ON trade_history(tx_id);
CREATE INDEX IF NOT EXISTS idx_trade_history_status ON trade_history(status);
CREATE INDEX IF NOT EXISTS idx_trade_history_who ON trade_history(who);
CREATE INDEX IF NOT EXISTS idx_trade_history_created ON trade_history(created_at);

-- Name to pool address mapping table
-- Caches the relationship between BNS names and their pool addresses
CREATE TABLE IF NOT EXISTS name_pools (
    name VARCHAR(255) PRIMARY KEY,
    pool_address VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_name_pools_pool_address ON name_pools(pool_address);

-- Stars table (user bookmarks for names and collectors)
CREATE TABLE IF NOT EXISTS stars (
    id SERIAL PRIMARY KEY,
    user_address VARCHAR(64) NOT NULL,
    target VARCHAR(100) NOT NULL,
    target_type VARCHAR NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT user_target_key UNIQUE (user_address, target)
);

-- NFT points table (points associated with names)
CREATE TABLE IF NOT EXISTS nft_points (
    name VARCHAR(100) PRIMARY KEY NOT NULL,
    points BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Shout outs table
CREATE TABLE IF NOT EXISTS shout_outs (
    tx_id VARCHAR(100) PRIMARY KEY,
    listing_name VARCHAR(64) NOT NULL,
    user_address VARCHAR(64) NOT NULL,
    ad_words TEXT NOT NULL,
    status VARCHAR(64) NOT NULL,
    price BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
