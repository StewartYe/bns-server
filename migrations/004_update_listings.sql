-- Migration 004: Update listings table for list_name API
-- Add confirmations and tx_id columns for transaction tracking

-- Add confirmations column (tracks Bitcoin confirmations)
ALTER TABLE listings ADD COLUMN IF NOT EXISTS confirmations INTEGER NOT NULL DEFAULT 0;

-- Add tx_id column (Bitcoin transaction ID)
ALTER TABLE listings ADD COLUMN IF NOT EXISTS tx_id VARCHAR(64);

-- Add index for confirmation tracking
CREATE INDEX IF NOT EXISTS idx_listings_confirmations ON listings(confirmations);
CREATE INDEX IF NOT EXISTS idx_listings_tx_id ON listings(tx_id);
