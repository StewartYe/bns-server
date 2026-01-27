
ALTER TABLE listings ADD column inscription_utxo_sats BIGINT NOT NULL DEFAULT 546;
ALTER TABLE listings ALTER column tx_id set not null;
ALTER TABLE pending_txs ADD column inscription_utxo_sats BIGINT NOT NULL DEFAULT 546;
