CREATE TABLE IF NOT EXISTS transactions (
  tx_hash TEXT PRIMARY KEY,
  created_at TIMESTAMPTZ DEFAULT NOW(),
  payload BYTEA
);

CREATE TABLE IF NOT EXISTS contracts (
  id TEXT PRIMARY KEY,
  template_id TEXT,
  signatories JSONB,
  observers JSONB,
  argument JSONB,
  created_at TIMESTAMPTZ DEFAULT NOW(),
  archived BOOLEAN DEFAULT FALSE
);

CREATE TABLE IF NOT EXISTS wallet_balances (
  participant_id TEXT,
  asset_id TEXT,
  balance NUMERIC,
  last_updated TIMESTAMPTZ DEFAULT NOW(),
  PRIMARY KEY (participant_id, asset_id)
);

CREATE TABLE IF NOT EXISTS events (
  id TEXT PRIMARY KEY,
  contract_id TEXT,
  event_type TEXT,
  data JSONB,
  timestamp TIMESTAMPTZ DEFAULT NOW()
);