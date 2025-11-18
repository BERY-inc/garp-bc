package integration

import (
    "context"
    "database/sql"
    "fmt"
    "time"

    _ "github.com/lib/pq"
    _ "github.com/go-sql-driver/mysql"
)

// DBIntegration provides database integration capabilities for external systems
type DBIntegration struct {
	db     *sql.DB
	driver string
}

// Config holds database configuration
type Config struct {
	Driver   string
	DSN      string
	MaxConns int
}

// TransactionRecord represents a blockchain transaction record in the database
type TransactionRecord struct {
	ID          string    `json:"id" db:"id"`
	Submitter   string    `json:"submitter" db:"submitter"`
	Status      string    `json:"status" db:"status"`
	CreatedAt   time.Time `json:"created_at" db:"created_at"`
	ConfirmedAt time.Time `json:"confirmed_at" db:"confirmed_at"`
	BlockNumber uint64    `json:"block_number" db:"block_number"`
	BlockHash   string    `json:"block_hash" db:"block_hash"`
	Data        string    `json:"data" db:"data"` // JSON-encoded transaction data
}

// BlockRecord represents a blockchain block record in the database
type BlockRecord struct {
	Number      uint64    `json:"number" db:"number"`
	Hash        string    `json:"hash" db:"hash"`
	ParentHash  string    `json:"parent_hash" db:"parent_hash"`
	Timestamp   time.Time `json:"timestamp" db:"timestamp"`
	TransactionCount int   `json:"transaction_count" db:"transaction_count"`
	Data        string    `json:"data" db:"data"` // JSON-encoded block data
}

// AccountRecord represents a blockchain account record in the database
type AccountRecord struct {
	Address     string    `json:"address" db:"address"`
	Balance     string    `json:"balance" db:"balance"`
	Nonce       uint64    `json:"nonce" db:"nonce"`
	UpdatedAt   time.Time `json:"updated_at" db:"updated_at"`
}

// NewDBIntegration creates a new database integration instance
func NewDBIntegration(config Config) (*DBIntegration, error) {
	db, err := sql.Open(config.Driver, config.DSN)
	if err != nil {
		return nil, fmt.Errorf("failed to open database: %w", err)
	}

	// Set connection pool settings
	if config.MaxConns > 0 {
		db.SetMaxOpenConns(config.MaxConns)
	}
	db.SetMaxIdleConns(10)
	db.SetConnMaxLifetime(time.Hour)

	// Test the connection
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	if err := db.PingContext(ctx); err != nil {
		return nil, fmt.Errorf("failed to ping database: %w", err)
	}

	return &DBIntegration{
		db:     db,
		driver: config.Driver,
	}, nil
}

// Close closes the database connection
func (dbi *DBIntegration) Close() error {
	return dbi.db.Close()
}

// InitializeSchema creates the necessary tables for blockchain data
func (dbi *DBIntegration) InitializeSchema(ctx context.Context) error {
	var schema string
	switch dbi.driver {
	case "postgres":
		schema = postgresSchema
	case "mysql":
		schema = mysqlSchema
	case "sqlite3":
		schema = sqliteSchema
	default:
		return fmt.Errorf("unsupported database driver: %s", dbi.driver)
	}

	_, err := dbi.db.ExecContext(ctx, schema)
	return err
}

// InsertTransaction inserts a transaction record into the database
func (dbi *DBIntegration) InsertTransaction(ctx context.Context, tx TransactionRecord) error {
	query := `
		INSERT INTO blockchain_transactions (id, submitter, status, created_at, confirmed_at, block_number, block_hash, data)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
	`
	_, err := dbi.db.ExecContext(ctx, query, tx.ID, tx.Submitter, tx.Status, tx.CreatedAt, tx.ConfirmedAt, tx.BlockNumber, tx.BlockHash, tx.Data)
	return err
}

// UpdateTransactionStatus updates the status of a transaction
func (dbi *DBIntegration) UpdateTransactionStatus(ctx context.Context, txID, status string, confirmedAt time.Time, blockNumber uint64, blockHash string) error {
	query := `
		UPDATE blockchain_transactions 
		SET status = $1, confirmed_at = $2, block_number = $3, block_hash = $4
		WHERE id = $5
	`
	_, err := dbi.db.ExecContext(ctx, query, status, confirmedAt, blockNumber, blockHash, txID)
	return err
}

// GetTransaction retrieves a transaction by ID
func (dbi *DBIntegration) GetTransaction(ctx context.Context, txID string) (*TransactionRecord, error) {
	var tx TransactionRecord
	query := `
		SELECT id, submitter, status, created_at, confirmed_at, block_number, block_hash, data
		FROM blockchain_transactions
		WHERE id = $1
	`
	err := dbi.db.QueryRowContext(ctx, query, txID).Scan(
		&tx.ID, &tx.Submitter, &tx.Status, &tx.CreatedAt, &tx.ConfirmedAt, &tx.BlockNumber, &tx.BlockHash, &tx.Data,
	)
	if err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	return &tx, nil
}

// ListTransactions lists transactions with optional filters
func (dbi *DBIntegration) ListTransactions(ctx context.Context, limit, offset int, status string) ([]TransactionRecord, error) {
	query := `
		SELECT id, submitter, status, created_at, confirmed_at, block_number, block_hash, data
		FROM blockchain_transactions
	`
	args := []interface{}{}
	
	if status != "" {
		query += " WHERE status = $1"
		args = append(args, status)
	}
	
	query += " ORDER BY created_at DESC LIMIT $2 OFFSET $3"
	args = append(args, limit, offset)
	
	rows, err := dbi.db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	
	var transactions []TransactionRecord
	for rows.Next() {
		var tx TransactionRecord
		err := rows.Scan(&tx.ID, &tx.Submitter, &tx.Status, &tx.CreatedAt, &tx.ConfirmedAt, &tx.BlockNumber, &tx.BlockHash, &tx.Data)
		if err != nil {
			return nil, err
		}
		transactions = append(transactions, tx)
	}
	
	return transactions, rows.Err()
}

// InsertBlock inserts a block record into the database
func (dbi *DBIntegration) InsertBlock(ctx context.Context, block BlockRecord) error {
	query := `
		INSERT INTO blockchain_blocks (number, hash, parent_hash, timestamp, transaction_count, data)
		VALUES ($1, $2, $3, $4, $5, $6)
	`
	_, err := dbi.db.ExecContext(ctx, query, block.Number, block.Hash, block.ParentHash, block.Timestamp, block.TransactionCount, block.Data)
	return err
}

// GetBlockByNumber retrieves a block by number
func (dbi *DBIntegration) GetBlockByNumber(ctx context.Context, number uint64) (*BlockRecord, error) {
	var block BlockRecord
	query := `
		SELECT number, hash, parent_hash, timestamp, transaction_count, data
		FROM blockchain_blocks
		WHERE number = $1
	`
	err := dbi.db.QueryRowContext(ctx, query, number).Scan(
		&block.Number, &block.Hash, &block.ParentHash, &block.Timestamp, &block.TransactionCount, &block.Data,
	)
	if err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	return &block, nil
}

// GetBlockByHash retrieves a block by hash
func (dbi *DBIntegration) GetBlockByHash(ctx context.Context, hash string) (*BlockRecord, error) {
	var block BlockRecord
	query := `
		SELECT number, hash, parent_hash, timestamp, transaction_count, data
		FROM blockchain_blocks
		WHERE hash = $1
	`
	err := dbi.db.QueryRowContext(ctx, query, hash).Scan(
		&block.Number, &block.Hash, &block.ParentHash, &block.Timestamp, &block.TransactionCount, &block.Data,
	)
	if err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	return &block, nil
}

// UpdateAccountBalance updates an account's balance
func (dbi *DBIntegration) UpdateAccountBalance(ctx context.Context, address, balance string) error {
	query := `
		INSERT INTO blockchain_accounts (address, balance, nonce, updated_at)
		VALUES ($1, $2, 0, $3)
		ON CONFLICT (address) DO UPDATE
		SET balance = $2, updated_at = $3
	`
	_, err := dbi.db.ExecContext(ctx, query, address, balance, time.Now().UTC())
	return err
}

// GetAccount retrieves an account by address
func (dbi *DBIntegration) GetAccount(ctx context.Context, address string) (*AccountRecord, error) {
	var account AccountRecord
	query := `
		SELECT address, balance, nonce, updated_at
		FROM blockchain_accounts
		WHERE address = $1
	`
	err := dbi.db.QueryRowContext(ctx, query, address).Scan(
		&account.Address, &account.Balance, &account.Nonce, &account.UpdatedAt,
	)
	if err != nil {
		if err == sql.ErrNoRows {
			return nil, nil
		}
		return nil, err
	}
	return &account, nil
}

// Database schemas for different drivers
const postgresSchema = `
CREATE TABLE IF NOT EXISTS blockchain_transactions (
	id TEXT PRIMARY KEY,
	submitter TEXT NOT NULL,
	status TEXT NOT NULL,
	created_at TIMESTAMP WITH TIME ZONE NOT NULL,
	confirmed_at TIMESTAMP WITH TIME ZONE,
	block_number BIGINT,
	block_hash TEXT,
	data JSONB
);

CREATE INDEX IF NOT EXISTS idx_transactions_submitter ON blockchain_transactions(submitter);
CREATE INDEX IF NOT EXISTS idx_transactions_status ON blockchain_transactions(status);
CREATE INDEX IF NOT EXISTS idx_transactions_created_at ON blockchain_transactions(created_at);
CREATE INDEX IF NOT EXISTS idx_transactions_block_number ON blockchain_transactions(block_number);

CREATE TABLE IF NOT EXISTS blockchain_blocks (
	number BIGINT PRIMARY KEY,
	hash TEXT UNIQUE NOT NULL,
	parent_hash TEXT NOT NULL,
	timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
	transaction_count INTEGER NOT NULL,
	data JSONB
);

CREATE INDEX IF NOT EXISTS idx_blocks_timestamp ON blockchain_blocks(timestamp);
CREATE INDEX IF NOT EXISTS idx_blocks_parent_hash ON blockchain_blocks(parent_hash);

CREATE TABLE IF NOT EXISTS blockchain_accounts (
	address TEXT PRIMARY KEY,
	balance TEXT NOT NULL,
	nonce BIGINT NOT NULL,
	updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_accounts_balance ON blockchain_accounts(balance);
CREATE INDEX IF NOT EXISTS idx_accounts_updated_at ON blockchain_accounts(updated_at);
`

const mysqlSchema = `
CREATE TABLE IF NOT EXISTS blockchain_transactions (
	id VARCHAR(255) PRIMARY KEY,
	submitter VARCHAR(255) NOT NULL,
	status VARCHAR(50) NOT NULL,
	created_at TIMESTAMP NOT NULL,
	confirmed_at TIMESTAMP NULL,
	block_number BIGINT UNSIGNED,
	block_hash VARCHAR(255),
	data JSON
);

CREATE INDEX idx_transactions_submitter ON blockchain_transactions(submitter);
CREATE INDEX idx_transactions_status ON blockchain_transactions(status);
CREATE INDEX idx_transactions_created_at ON blockchain_transactions(created_at);
CREATE INDEX idx_transactions_block_number ON blockchain_transactions(block_number);

CREATE TABLE IF NOT EXISTS blockchain_blocks (
	number BIGINT UNSIGNED PRIMARY KEY,
	hash VARCHAR(255) UNIQUE NOT NULL,
	parent_hash VARCHAR(255) NOT NULL,
	timestamp TIMESTAMP NOT NULL,
	transaction_count INT NOT NULL,
	data JSON
);

CREATE INDEX idx_blocks_timestamp ON blockchain_blocks(timestamp);
CREATE INDEX idx_blocks_parent_hash ON blockchain_blocks(parent_hash);

CREATE TABLE IF NOT EXISTS blockchain_accounts (
	address VARCHAR(255) PRIMARY KEY,
	balance VARCHAR(255) NOT NULL,
	nonce BIGINT UNSIGNED NOT NULL,
	updated_at TIMESTAMP NOT NULL
);

CREATE INDEX idx_accounts_balance ON blockchain_accounts(balance);
CREATE INDEX idx_accounts_updated_at ON blockchain_accounts(updated_at);
`

const sqliteSchema = `
CREATE TABLE IF NOT EXISTS blockchain_transactions (
	id TEXT PRIMARY KEY,
	submitter TEXT NOT NULL,
	status TEXT NOT NULL,
	created_at TEXT NOT NULL,
	confirmed_at TEXT,
	block_number INTEGER,
	block_hash TEXT,
	data TEXT
);

CREATE INDEX IF NOT EXISTS idx_transactions_submitter ON blockchain_transactions(submitter);
CREATE INDEX IF NOT EXISTS idx_transactions_status ON blockchain_transactions(status);
CREATE INDEX IF NOT EXISTS idx_transactions_created_at ON blockchain_transactions(created_at);
CREATE INDEX IF NOT EXISTS idx_transactions_block_number ON blockchain_transactions(block_number);

CREATE TABLE IF NOT EXISTS blockchain_blocks (
	number INTEGER PRIMARY KEY,
	hash TEXT UNIQUE NOT NULL,
	parent_hash TEXT NOT NULL,
	timestamp TEXT NOT NULL,
	transaction_count INTEGER NOT NULL,
	data TEXT
);

CREATE INDEX IF NOT EXISTS idx_blocks_timestamp ON blockchain_blocks(timestamp);
CREATE INDEX IF NOT EXISTS idx_blocks_parent_hash ON blockchain_blocks(parent_hash);

CREATE TABLE IF NOT EXISTS blockchain_accounts (
	address TEXT PRIMARY KEY,
	balance TEXT NOT NULL,
	nonce INTEGER NOT NULL,
	updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_accounts_balance ON blockchain_accounts(balance);
CREATE INDEX IF NOT EXISTS idx_accounts_updated_at ON blockchain_accounts(updated_at);
`