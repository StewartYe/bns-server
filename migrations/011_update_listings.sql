-- Migration: Update listings table structure
-- 1. Remove pool_address column
-- 2. Change status values to: listed, bought_and_relisted, bought_and_delisted, relisted, delisted
-- 3. Add buyer_address and new_price_sats optional fields

-- Add new columns
ALTER TABLE listings ADD COLUMN IF NOT EXISTS buyer_address VARCHAR(100);
ALTER TABLE listings ADD COLUMN IF NOT EXISTS new_price_sats BIGINT;

-- Migrate existing status values to new values
-- active/pending -> listed (these are currently on sale)
-- sold -> bought_and_delisted (assume sold items were delisted after purchase)
-- delisted -> delisted (keep as is)
-- cancelled -> delisted (treat cancelled as delisted)
UPDATE listings SET status = 'listed' WHERE status IN ('active', 'pending');
UPDATE listings SET status = 'bought_and_delisted' WHERE status = 'sold';
UPDATE listings SET status = 'delisted' WHERE status IN ('delisted', 'cancelled');

-- Change default status to 'listed'
ALTER TABLE listings ALTER COLUMN status SET DEFAULT 'listed';

-- Remove pool_address column
ALTER TABLE listings DROP COLUMN IF EXISTS pool_address;

-- Add index for buyer_address
CREATE INDEX IF NOT EXISTS idx_listings_buyer ON listings(buyer_address);
