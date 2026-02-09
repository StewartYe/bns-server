-- Refactor: listings (simplified) + trade_history (replaces pending_txs)

-- Step 1: Create trade_history table from pending_txs data
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

-- Step 2: Migrate pending_txs data to trade_history
-- pending_txs has: tx_id (PK), created_at, name, action, status, previous_price_sats, price_sats, seller_address, buyer_address, inscription_utxo_sats, platform_fee
-- No updated_at or who columns in pending_txs
INSERT INTO trade_history (name, who, action, tx_id, created_at, updated_at, status, seller_address, previous_price_sats, price_sats, inscription_utxo_sats, buyer_address, platform_fee)
SELECT
    name,
    COALESCE(buyer_address, seller_address, ''),
    action,
    tx_id,
    created_at,
    created_at,
    status,
    seller_address,
    previous_price_sats,
    price_sats,
    inscription_utxo_sats,
    buyer_address,
    platform_fee
FROM pending_txs;

-- Step 3: Migrate listing history into trade_history
-- Insert confirmed buy_and_relist/buy_and_delist records from listings
INSERT INTO trade_history (name, who, action, tx_id, created_at, updated_at, status, seller_address, previous_price_sats, price_sats, inscription_utxo_sats, buyer_address)
SELECT
    name,
    COALESCE(buyer_address, ''),
    CASE
        WHEN status = 'bought_and_relisted' THEN 'buy_and_relist'
        WHEN status = 'bought_and_delisted' THEN 'buy_and_delist'
        WHEN status = 'delisted' THEN 'delist'
        WHEN status = 'relisted' THEN 'relist'
        ELSE 'list'
    END,
    tx_id,
    listed_at,
    updated_at,
    'confirmed',
    seller_address,
    previous_price_sats,
    price_sats,
    inscription_utxo_sats,
    buyer_address
FROM listings
WHERE status != 'listed';

-- Step 4: Drop pending_txs table
DROP TABLE IF EXISTS pending_txs;

-- Step 5: Simplify listings table - remove columns no longer needed
-- First, delete non-listed rows (they've been migrated to trade_history)
DELETE FROM listings WHERE status != 'listed';

-- Drop old constraints and indexes that reference removed columns
DROP INDEX IF EXISTS idx_listings_name_status;

-- Remove columns
ALTER TABLE listings DROP COLUMN IF EXISTS id;
ALTER TABLE listings DROP COLUMN IF EXISTS status;
ALTER TABLE listings DROP COLUMN IF EXISTS previous_price_sats;
ALTER TABLE listings DROP COLUMN IF EXISTS buyer_address;
ALTER TABLE listings DROP COLUMN IF EXISTS new_price_sats;

-- Make name the primary key (it may already be, but ensure)
-- Since we deleted non-listed rows, name should be unique
-- The old PK was id, so we need to set name as PK
-- First check if name already has a unique constraint
ALTER TABLE listings DROP CONSTRAINT IF EXISTS listings_pkey;
ALTER TABLE listings ADD PRIMARY KEY (name);
