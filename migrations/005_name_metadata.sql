-- BNS Server Database Schema
-- Migration 005: Add primary_name to users and name_metadata table

-- Add primary_name column to users table
ALTER TABLE users ADD COLUMN IF NOT EXISTS primary_name VARCHAR(64);

-- Create name_metadata table for storing name metadata
CREATE TABLE IF NOT EXISTS name_metadata (
    name VARCHAR(64) PRIMARY KEY,
    owner_address VARCHAR(100) NOT NULL,
    description TEXT,
    url VARCHAR(500),
    twitter VARCHAR(100),
    email VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for owner lookups
CREATE INDEX IF NOT EXISTS idx_name_metadata_owner ON name_metadata(owner_address);

-- Update existing tables to use VARCHAR(64) for name columns
ALTER TABLE listings ALTER COLUMN name TYPE VARCHAR(64);
ALTER TABLE transactions ALTER COLUMN name TYPE VARCHAR(64);
ALTER TABLE canister_events ALTER COLUMN name TYPE VARCHAR(64);
ALTER TABLE shoutouts ALTER COLUMN name TYPE VARCHAR(64);
