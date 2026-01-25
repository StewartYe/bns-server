-- Migration 012: Restructure pending_txs table
-- Drop old table and create new structure
-- Old data can be discarded during upgrade

DROP TABLE IF EXISTS pending_txs;

-- Pending transactions table with explicit columns
-- status: submitted (initial), pending, finalized, confirmed, rejected
-- action: list, buy_and_relist, buy_and_delist, delist
CREATE TABLE pending_txs (
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
