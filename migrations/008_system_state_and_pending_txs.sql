-- Migration 008: Add system_state and pending_txs tables
-- These tables replace Redis keys for persistent state tracking

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
