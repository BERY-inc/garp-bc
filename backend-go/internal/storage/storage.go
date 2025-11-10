package storage

import (
	"context"
	"encoding/json"
	"time"

	"github.com/jackc/pgx/v5/pgxpool"
	"github.com/redis/go-redis/v9"
)

type Storage struct {
	PG    *pgxpool.Pool
	Redis *redis.Client
}

type Config struct {
	PostgresURL string
	RedisURL    string
}

func Init(ctx context.Context, cfg Config) (*Storage, error) {
	pgCfg, err := pgxpool.ParseConfig(cfg.PostgresURL)
	if err != nil {
		return nil, err
	}
	pgCfg.MaxConns = 10
	pg, err := pgxpool.NewWithConfig(ctx, pgCfg)
	if err != nil {
		return nil, err
	}

	rdbOpts, err := redis.ParseURL(cfg.RedisURL)
	if err != nil {
		pg.Close()
		return nil, err
	}
	rdb := redis.NewClient(rdbOpts)
	if err := rdb.Ping(ctx).Err(); err != nil {
		pg.Close()
		return nil, err
	}

	return &Storage{PG: pg, Redis: rdb}, nil
}

func (s *Storage) Close() {
	if s.PG != nil {
		s.PG.Close()
	}
	if s.Redis != nil {
		_ = s.Redis.Close()
	}
}

// SaveTx persists a transaction stub and enqueues it for processing.
type QueueMessage struct {
	Kind  string `json:"kind"`
	Hash  string `json:"hash"`
	Retry int    `json:"retry"`
}

func (s *Storage) SaveTx(ctx context.Context, hash string, payload []byte) error {
	_, err := s.PG.Exec(ctx, `CREATE TABLE IF NOT EXISTS transactions (
        tx_hash TEXT PRIMARY KEY,
        created_at TIMESTAMPTZ DEFAULT NOW(),
        payload BYTEA
    )`)
	if err != nil {
		return err
	}
	_, err = s.PG.Exec(ctx, `INSERT INTO transactions (tx_hash, payload) VALUES ($1,$2)
        ON CONFLICT (tx_hash) DO NOTHING`, hash, payload)
	if err != nil {
		return err
	}
	msg := QueueMessage{Kind: "tx", Hash: hash, Retry: 0}
	b, _ := json.Marshal(msg)
	return s.Redis.LPush(ctx, "tx_queue", b).Err()
}

// Ready checks DB and Redis connectivity.
func (s *Storage) Ready(ctx context.Context) bool {
	if s.PG == nil || s.Redis == nil {
		return false
	}
	ctx, cancel := context.WithTimeout(ctx, 2*time.Second)
	defer cancel()
	if err := s.Redis.Ping(ctx).Err(); err != nil {
		return false
	}
	if err := s.PG.Ping(ctx); err != nil {
		return false
	}
	return true
}
