-- Messages table for chat feature
CREATE TABLE IF NOT EXISTS messages (
    id BIGSERIAL PRIMARY KEY,
    sender TEXT NOT NULL,
    recipient TEXT NOT NULL,
    content_ciphertext BYTEA NOT NULL,
    content_nonce BYTEA NOT NULL,
    hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    anchored_at_block BIGINT NULL,
    deleted_at TIMESTAMPTZ NULL
);

CREATE INDEX IF NOT EXISTS idx_messages_sender_recipient_created ON messages(sender, recipient, created_at);
CREATE INDEX IF NOT EXISTS idx_messages_recipient_created ON messages(recipient, created_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_hash ON messages(hash);