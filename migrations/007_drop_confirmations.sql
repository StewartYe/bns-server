-- Migration 007: Drop confirmations column from listings table
-- Confirmations are no longer tracked by the application

-- Drop the index first
DROP INDEX IF EXISTS idx_listings_confirmations;

-- Drop the column
ALTER TABLE listings DROP COLUMN IF EXISTS confirmations;
