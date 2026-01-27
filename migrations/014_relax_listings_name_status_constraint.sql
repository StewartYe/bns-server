-- Migration 013: Relax listings name unique constraint
-- Drop the existing unique constraint on (name, status)
ALTER TABLE listings DROP CONSTRAINT IF EXISTS listings_name_status_key;
