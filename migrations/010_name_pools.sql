-- Name to pool address mapping table
-- Caches the relationship between BNS names and their pool addresses

CREATE TABLE IF NOT EXISTS name_pools (
    name VARCHAR(255) PRIMARY KEY,
    pool_address VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_name_pools_pool_address ON name_pools(pool_address);
