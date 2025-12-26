-- Migration 002: Add secret_hash to sessions for enhanced security
-- The session token is now: session_id:session_secret
-- Only the hash of session_secret is stored in the database
-- This prevents database administrators from impersonating users

ALTER TABLE sessions ADD COLUMN secret_hash VARCHAR(64) NOT NULL DEFAULT '';

-- Remove the default after adding the column
ALTER TABLE sessions ALTER COLUMN secret_hash DROP DEFAULT;

-- Add index for faster lookups
CREATE INDEX IF NOT EXISTS idx_sessions_id_hash ON sessions(session_id, secret_hash);
