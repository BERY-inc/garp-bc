package storage

import (
    "context"
    "crypto/sha256"
    "encoding/hex"
    "encoding/json"
    "time"
)

type Message struct {
    ID               int64      `json:"id"`
    Sender           string     `json:"sender"`
    Recipient        string     `json:"recipient"`
    ContentCiphertext []byte    `json:"content_ciphertext"`
    ContentNonce     []byte     `json:"content_nonce"`
    Hash             string     `json:"hash"`
    CreatedAt        time.Time  `json:"created_at"`
    AnchoredAtBlock  *int64     `json:"anchored_at_block,omitempty"`
}

func hashMessage(ciphertext, nonce []byte) string {
    h := sha256.Sum256(append(ciphertext, nonce...))
    return hex.EncodeToString(h[:])
}

func (s *Storage) CreateMessage(ctx context.Context, sender, recipient string, ciphertext, nonce []byte) (Message, error) {
    h := hashMessage(ciphertext, nonce)
    var id int64
    err := s.PG.QueryRow(ctx,
        `INSERT INTO messages(sender, recipient, content_ciphertext, content_nonce, hash)
         VALUES ($1,$2,$3,$4,$5)
         ON CONFLICT (hash) DO UPDATE SET sender = EXCLUDED.sender
         RETURNING id`, sender, recipient, ciphertext, nonce, h).Scan(&id)
    if err != nil { return Message{}, err }
    var m Message
    err = s.PG.QueryRow(ctx,
        `SELECT id, sender, recipient, content_ciphertext, content_nonce, hash, created_at, anchored_at_block
         FROM messages WHERE id = $1`, id).
         Scan(&m.ID, &m.Sender, &m.Recipient, &m.ContentCiphertext, &m.ContentNonce, &m.Hash, &m.CreatedAt, &m.AnchoredAtBlock)
    if err != nil { return Message{}, err }
    // Publish event for real-time streams
    if s.Redis != nil {
        b, _ := json.Marshal(map[string]any{
            "type": "message",
            "id": m.ID,
            "sender": m.Sender,
            "recipient": m.Recipient,
            "hash": m.Hash,
            "created_at": m.CreatedAt,
        })
        _ = s.Redis.Publish(ctx, "messages", b).Err()
    }
    return m, nil
}

func (s *Storage) ListMessages(ctx context.Context, a, b string, since *time.Time, limit int) ([]Message, error) {
    if limit <= 0 { limit = 100 }
    var rows pgRows
    var err error
    if since != nil {
        rows, err = s.PG.Query(ctx,
            `SELECT id, sender, recipient, content_ciphertext, content_nonce, hash, created_at, anchored_at_block
             FROM messages
             WHERE created_at >= $1 AND ((sender = $2 AND recipient = $3) OR (sender = $3 AND recipient = $2))
             ORDER BY created_at ASC
             LIMIT $4`, since.UTC(), a, b, limit)
    } else {
        rows, err = s.PG.Query(ctx,
            `SELECT id, sender, recipient, content_ciphertext, content_nonce, hash, created_at, anchored_at_block
             FROM messages
             WHERE (sender = $1 AND recipient = $2) OR (sender = $2 AND recipient = $1)
             ORDER BY created_at ASC
             LIMIT $3`, a, b, limit)
    }
    if err != nil { return nil, err }
    defer rows.Close()
    var out []Message
    for rows.Next() {
        var m Message
        if err := rows.Scan(&m.ID, &m.Sender, &m.Recipient, &m.ContentCiphertext, &m.ContentNonce, &m.Hash, &m.CreatedAt, &m.AnchoredAtBlock); err != nil {
            return nil, err
        }
        out = append(out, m)
    }
    return out, rows.Err()
}

func (s *Storage) AnchorMessage(ctx context.Context, id int64, block int64) error {
    _, err := s.PG.Exec(ctx, `UPDATE messages SET anchored_at_block = $2 WHERE id = $1`, id, block)
    return err
}

// minimal interface alias for pgx Rows to simplify testing
type pgRows interface{
    Next() bool
    Scan(dest ...any) error
    Close()
    Err() error
}