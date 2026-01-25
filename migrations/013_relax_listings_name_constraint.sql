-- Migration 013: Relax listings name unique constraint
-- Drop the existing unique constraint on name
ALTER TABLE listings DROP CONSTRAINT IF EXISTS listings_name_key;

-- Add a new unique constraint on (name, status)
ALTER TABLE listings ADD CONSTRAINT listings_name_status_key UNIQUE (name, status);
