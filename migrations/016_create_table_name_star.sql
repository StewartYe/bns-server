
CREATE TABLE IF NOT EXISTS stars (
    id SERIAL PRIMARY KEY,
    user_address VARCHAR(64) NOT NULL,
    target VARCHAR(100) NOT NULL,
    target_type VARCHAR NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE stars ADD CONSTRAINT user_target_key UNIQUE (user_address, target);
