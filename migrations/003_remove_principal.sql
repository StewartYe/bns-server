-- Migration 003: Remove principal column (switching from SIWB to BIP-322)
-- Principal is no longer needed as we use BIP-322 signature verification

-- Remove principal from users table
ALTER TABLE users DROP COLUMN IF EXISTS principal;

-- Remove principal from sessions table
ALTER TABLE sessions DROP COLUMN IF EXISTS principal;

-- Drop the index on principal
DROP INDEX IF EXISTS idx_users_principal;
