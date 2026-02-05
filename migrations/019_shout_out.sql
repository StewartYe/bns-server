CREATE TABLE IF NOT EXISTS shout_outs (
    tx_id VARCHAR(100) PRIMARY KEY,
    listing_name VARCHAR(64) NOT NULL,
    user_address VARCHAR(64) NOT NULL,
    ad_words TEXT NOT NULL,
    status VARCHAR(64) NOT NULL,
    price BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
