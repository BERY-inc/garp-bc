use garp_common::{
    Contract, Transaction, TransactionId, ContractId, ParticipantId, Asset, WalletBalance,
    Block, BlockHeader,
    GarpResult, GarpError, DatabaseError
};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use serde_json;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use async_trait::async_trait;
use std::collections::HashMap;

/// Database storage trait
#[async_trait]
pub trait StorageBackend: Send + Sync {
    // Contract operations
    async fn store_contract(&self, contract: &Contract) -> GarpResult<()>;
    async fn get_contract(&self, contract_id: &ContractId) -> GarpResult<Option<Contract>>;
    async fn archive_contract(&self, contract_id: &ContractId) -> GarpResult<()>;
    async fn list_contracts(&self, participant_id: &ParticipantId, active_only: bool) -> GarpResult<Vec<Contract>>;

    // Transaction operations
    async fn store_transaction(&self, transaction: &Transaction) -> GarpResult<()>;
    async fn get_transaction(&self, transaction_id: &TransactionId) -> GarpResult<Option<Transaction>>;
    async fn list_transactions(&self, participant_id: &ParticipantId, limit: Option<u32>) -> GarpResult<Vec<Transaction>>;

    // Wallet operations
    async fn store_wallet_balance(&self, balance: &WalletBalance) -> GarpResult<()>;
    async fn get_wallet_balance(&self, participant_id: &ParticipantId) -> GarpResult<Option<WalletBalance>>;
    async fn update_asset_balance(&self, participant_id: &ParticipantId, asset: &Asset, delta: i64) -> GarpResult<()>;

    // Ledger operations
    async fn get_ledger_state(&self, participant_id: &ParticipantId) -> GarpResult<LedgerState>;
    async fn store_ledger_checkpoint(&self, participant_id: &ParticipantId, state: &LedgerState) -> GarpResult<()>;

    // Block operations
    async fn store_block(&self, block: &Block) -> GarpResult<()>;
    async fn get_block_by_slot(&self, slot: u64) -> GarpResult<Option<Block>>;
    async fn get_latest_block(&self) -> GarpResult<Option<Block>>;
    async fn get_block_by_hash_hex(&self, hash_hex: &str) -> GarpResult<Option<Block>>;
    async fn list_blocks(&self, limit: Option<u32>, offset: Option<u32>) -> GarpResult<Vec<Block>>;
    async fn list_blocks_filtered(&self, epoch: Option<u64>, proposer: Option<String>, limit: Option<u32>, offset: Option<u32>) -> GarpResult<Vec<Block>>;
    async fn get_block_state_changes(&self, slot: u64) -> GarpResult<Vec<crate::state_commitments::StateChangeItem>>;

    // Event operations
    async fn store_contract_event(&self, event: &ContractEvent) -> GarpResult<()>;
    async fn get_contract_events(&self, contract_id: &ContractId, limit: Option<u32>) -> GarpResult<Vec<ContractEvent>>;
    async fn get_participant_events(&self, participant_id: &ParticipantId, limit: Option<u32>) -> GarpResult<Vec<ContractEvent>>;
    async fn query_events(&self, query: &EventQuery) -> GarpResult<Vec<ContractEvent>>;
}

/// Contract event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractEvent {
    pub id: String,
    pub contract_id: ContractId,
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub emitter: ParticipantId,
}

/// Event query parameters
#[derive(Debug, Clone)]
pub struct EventQuery {
    pub contract_id: Option<ContractId>,
    pub event_type: Option<String>,
    pub participant_id: Option<ParticipantId>,
    pub from_timestamp: Option<DateTime<Utc>>,
    pub to_timestamp: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
}

/// PostgreSQL storage implementation
pub struct PostgresStorage {
    pool: PgPool,
}

/// Ledger state snapshot
#[derive(Debug, Clone)]
pub struct LedgerState {
    pub participant_id: ParticipantId,
    pub active_contracts: Vec<ContractId>,
    pub total_transactions: u64,
    pub last_transaction_id: Option<TransactionId>,
    pub wallet_balance: Option<WalletBalance>,
    pub checkpoint_time: DateTime<Utc>,
}

impl PostgresStorage {
    /// Create new PostgreSQL storage
    pub async fn new(database_url: &str, max_connections: u32) -> GarpResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await
            .map_err(|e| DatabaseError::ConnectionFailed(e.to_string()))?;

        let storage = Self { pool };
        storage.initialize_schema().await?;
        
        Ok(storage)
    }

    /// Initialize database schema
    async fn initialize_schema(&self) -> GarpResult<()> {
        // Create contracts table
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS contracts (
                id UUID PRIMARY KEY,
                template_id VARCHAR NOT NULL,
                signatories JSONB NOT NULL,
                observers JSONB NOT NULL,
                argument JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL,
                archived BOOLEAN NOT NULL DEFAULT FALSE,
                archived_at TIMESTAMPTZ
            )
        "#)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        // Create transactions table
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS transactions (
                id UUID PRIMARY KEY,
                submitter VARCHAR NOT NULL,
                command JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL,
                signatures JSONB NOT NULL,
                encrypted_payload JSONB
            )
        "#)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        // Create wallet_balances table
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS wallet_balances (
                participant_id VARCHAR PRIMARY KEY,
                assets JSONB NOT NULL,
                last_updated TIMESTAMPTZ NOT NULL
            )
        "#)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        // Create ledger_checkpoints table
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS ledger_checkpoints (
                participant_id VARCHAR PRIMARY KEY,
                active_contracts JSONB NOT NULL,
                total_transactions BIGINT NOT NULL,
                last_transaction_id UUID,
                wallet_balance JSONB,
                checkpoint_time TIMESTAMPTZ NOT NULL
            )
        "#)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        // Create indexes
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_contracts_signatories ON contracts USING GIN (signatories)")
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_contracts_observers ON contracts USING GIN (observers)")
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_transactions_submitter ON transactions (submitter)")
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_transactions_created_at ON transactions (created_at)")
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        // Create blocks table
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS blocks (
                hash_hex VARCHAR PRIMARY KEY,
                parent_hash BYTEA NOT NULL,
                slot BIGINT NOT NULL,
                epoch BIGINT NOT NULL,
                proposer VARCHAR NOT NULL,
                state_root BYTEA NOT NULL,
                tx_root BYTEA NOT NULL,
                receipt_root BYTEA NOT NULL,
                timestamp TIMESTAMPTZ NOT NULL,
                transactions JSONB NOT NULL
            )
        "#)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_blocks_slot ON blocks (slot)")
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_blocks_epoch ON blocks (epoch)")
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_blocks_proposer ON blocks (proposer)")
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        // Create block_state_changes table to persist coalesced state changes per block slot
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS block_state_changes (
                slot BIGINT NOT NULL,
                state_key VARCHAR NOT NULL,
                value_hash BYTEA NOT NULL,
                PRIMARY KEY (slot, state_key)
            )
        "#)
        .execute(&self.pool)
        .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_bsc_slot ON block_state_changes (slot)")
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        // Create contract_events table
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS contract_events (
                id VARCHAR PRIMARY KEY,
                contract_id UUID NOT NULL,
                event_type VARCHAR NOT NULL,
                data JSONB NOT NULL,
                timestamp TIMESTAMPTZ NOT NULL,
                emitter VARCHAR NOT NULL
            )
        "#)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_contract_events_contract_id ON contract_events (contract_id)")
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_contract_events_event_type ON contract_events (event_type)")
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_contract_events_emitter ON contract_events (emitter)")
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_contract_events_timestamp ON contract_events (timestamp)")
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl StorageBackend for PostgresStorage {
    async fn store_contract(&self, contract: &Contract) -> GarpResult<()> {
        let signatories_json = serde_json::to_value(&contract.signatories)
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        let observers_json = serde_json::to_value(&contract.observers)
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        sqlx::query(r#"
            INSERT INTO contracts (id, template_id, signatories, observers, argument, created_at, archived)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id) DO UPDATE SET
                template_id = EXCLUDED.template_id,
                signatories = EXCLUDED.signatories,
                observers = EXCLUDED.observers,
                argument = EXCLUDED.argument,
                archived = EXCLUDED.archived
        "#)
        .bind(contract.id.0)
        .bind(&contract.template_id)
        .bind(signatories_json)
        .bind(observers_json)
        .bind(&contract.argument)
        .bind(contract.created_at)
        .bind(contract.archived)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        Ok(())
    }

    async fn get_contract(&self, contract_id: &ContractId) -> GarpResult<Option<Contract>> {
        let row = sqlx::query(r#"
            SELECT id, template_id, signatories, observers, argument, created_at, archived
            FROM contracts WHERE id = $1
        "#)
        .bind(contract_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        if let Some(row) = row {
            let signatories: Vec<ParticipantId> = serde_json::from_value(row.get("signatories"))
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
            let observers: Vec<ParticipantId> = serde_json::from_value(row.get("observers"))
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

            let contract = Contract {
                id: ContractId(row.get("id")),
                template_id: row.get("template_id"),
                signatories,
                observers,
                argument: row.get("argument"),
                created_at: row.get("created_at"),
                archived: row.get("archived"),
            };

            Ok(Some(contract))
        } else {
            Ok(None)
        }
    }

    async fn archive_contract(&self, contract_id: &ContractId) -> GarpResult<()> {
        sqlx::query(r#"
            UPDATE contracts SET archived = TRUE, archived_at = NOW()
            WHERE id = $1
        "#)
        .bind(contract_id.0)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        Ok(())
    }

    async fn list_contracts(&self, participant_id: &ParticipantId, active_only: bool) -> GarpResult<Vec<Contract>> {
        let query = if active_only {
            r#"
            SELECT id, template_id, signatories, observers, argument, created_at, archived
            FROM contracts 
            WHERE (signatories ? $1 OR observers ? $1) AND archived = FALSE
            ORDER BY created_at DESC
            "#
        } else {
            r#"
            SELECT id, template_id, signatories, observers, argument, created_at, archived
            FROM contracts 
            WHERE (signatories ? $1 OR observers ? $1)
            ORDER BY created_at DESC
            "#
        };

        let rows = sqlx::query(query)
            .bind(&participant_id.0)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        let mut contracts = Vec::new();
        for row in rows {
            let signatories: Vec<ParticipantId> = serde_json::from_value(row.get("signatories"))
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
            let observers: Vec<ParticipantId> = serde_json::from_value(row.get("observers"))
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

            let contract = Contract {
                id: ContractId(row.get("id")),
                template_id: row.get("template_id"),
                signatories,
                observers,
                argument: row.get("argument"),
                created_at: row.get("created_at"),
                archived: row.get("archived"),
            };

            contracts.push(contract);
        }

        Ok(contracts)
    }

    async fn store_transaction(&self, transaction: &Transaction) -> GarpResult<()> {
        let command_json = serde_json::to_value(&transaction.command)
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        let signatures_json = serde_json::to_value(&transaction.signatures)
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        let encrypted_payload_json = serde_json::to_value(&transaction.encrypted_payload)
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        sqlx::query(r#"
            INSERT INTO transactions (id, submitter, command, created_at, signatures, encrypted_payload)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO NOTHING
        "#)
        .bind(transaction.id.0)
        .bind(&transaction.submitter.0)
        .bind(command_json)
        .bind(transaction.created_at)
        .bind(signatures_json)
        .bind(encrypted_payload_json)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        Ok(())
    }

    async fn get_transaction(&self, transaction_id: &TransactionId) -> GarpResult<Option<Transaction>> {
        let row = sqlx::query(r#"
            SELECT id, submitter, command, created_at, signatures, encrypted_payload
            FROM transactions WHERE id = $1
        "#)
        .bind(transaction_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        if let Some(row) = row {
            let command = serde_json::from_value(row.get("command"))
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
            let signatures = serde_json::from_value(row.get("signatures"))
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
            let encrypted_payload = serde_json::from_value(row.get("encrypted_payload"))
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

            let transaction = Transaction {
                id: TransactionId(row.get("id")),
                submitter: ParticipantId(row.get("submitter")),
                command,
                created_at: row.get("created_at"),
                signatures,
                encrypted_payload,
            };

            Ok(Some(transaction))
        } else {
            Ok(None)
        }
    }

    async fn list_transactions(&self, participant_id: &ParticipantId, limit: Option<u32>) -> GarpResult<Vec<Transaction>> {
        let query = match limit {
            Some(_) => r#"
                SELECT id, submitter, command, created_at, signatures, encrypted_payload
                FROM transactions WHERE submitter = $1
                ORDER BY created_at DESC LIMIT $2
            "#,
            None => r#"
                SELECT id, submitter, command, created_at, signatures, encrypted_payload
                FROM transactions WHERE submitter = $1
                ORDER BY created_at DESC
            "#,
        };

        let rows = if let Some(limit) = limit {
            sqlx::query(query)
                .bind(&participant_id.0)
                .bind(limit as i64)
                .fetch_all(&self.pool)
                .await
        } else {
            sqlx::query(query)
                .bind(&participant_id.0)
                .fetch_all(&self.pool)
                .await
        }
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        let mut transactions = Vec::new();
        for row in rows {
            let command = serde_json::from_value(row.get("command"))
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
            let signatures = serde_json::from_value(row.get("signatures"))
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
            let encrypted_payload = serde_json::from_value(row.get("encrypted_payload"))
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

            let transaction = Transaction {
                id: TransactionId(row.get("id")),
                submitter: ParticipantId(row.get("submitter")),
                command,
                created_at: row.get("created_at"),
                signatures,
                encrypted_payload,
            };

            transactions.push(transaction);
        }

        Ok(transactions)
    }

    async fn store_wallet_balance(&self, balance: &WalletBalance) -> GarpResult<()> {
        let assets_json = serde_json::to_value(&balance.assets)
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        sqlx::query(r#"
            INSERT INTO wallet_balances (participant_id, assets, last_updated)
            VALUES ($1, $2, $3)
            ON CONFLICT (participant_id) DO UPDATE SET
                assets = EXCLUDED.assets,
                last_updated = EXCLUDED.last_updated
        "#)
        .bind(&balance.participant_id.0)
        .bind(assets_json)
        .bind(balance.last_updated)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        Ok(())
    }

    async fn get_wallet_balance(&self, participant_id: &ParticipantId) -> GarpResult<Option<WalletBalance>> {
        let row = sqlx::query(r#"
            SELECT participant_id, assets, last_updated
            FROM wallet_balances WHERE participant_id = $1
        "#)
        .bind(&participant_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        if let Some(row) = row {
            let assets = serde_json::from_value(row.get("assets"))
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

            let balance = WalletBalance {
                participant_id: ParticipantId(row.get("participant_id")),
                assets,
                last_updated: row.get("last_updated"),
            };

            Ok(Some(balance))
        } else {
            Ok(None)
        }
    }

    async fn update_asset_balance(&self, participant_id: &ParticipantId, asset: &Asset, delta: i64) -> GarpResult<()> {
        // This is a simplified implementation
        // In practice, you'd need more sophisticated asset balance management
        let mut balance = self.get_wallet_balance(participant_id).await?
            .unwrap_or_else(|| WalletBalance {
                participant_id: participant_id.clone(),
                assets: Vec::new(),
                last_updated: Utc::now(),
            });

        // Find and update the asset
        let mut found = false;
        for existing_asset in &mut balance.assets {
            if existing_asset.id == asset.id {
                existing_asset.amount = (existing_asset.amount as i64 + delta).max(0) as u64;
                found = true;
                break;
            }
        }

        if !found && delta > 0 {
            let mut new_asset = asset.clone();
            new_asset.amount = delta as u64;
            balance.assets.push(new_asset);
        }

        balance.last_updated = Utc::now();
        self.store_wallet_balance(&balance).await?;

        Ok(())
    }

    async fn get_ledger_state(&self, participant_id: &ParticipantId) -> GarpResult<LedgerState> {
        let row = sqlx::query(r#"
            SELECT participant_id, active_contracts, total_transactions, last_transaction_id, wallet_balance, checkpoint_time
            FROM ledger_checkpoints WHERE participant_id = $1
        "#)
        .bind(&participant_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        if let Some(row) = row {
            let active_contracts: Vec<ContractId> = serde_json::from_value(row.get("active_contracts"))
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
            let wallet_balance: Option<WalletBalance> = serde_json::from_value(row.get("wallet_balance"))
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
            let last_transaction_id: Option<Uuid> = row.get("last_transaction_id");

            Ok(LedgerState {
                participant_id: ParticipantId(row.get("participant_id")),
                active_contracts,
                total_transactions: row.get::<i64, _>("total_transactions") as u64,
                last_transaction_id: last_transaction_id.map(TransactionId),
                wallet_balance,
                checkpoint_time: row.get("checkpoint_time"),
            })
        } else {
            // Return empty state for new participant
            Ok(LedgerState {
                participant_id: participant_id.clone(),
                active_contracts: Vec::new(),
                total_transactions: 0,
                last_transaction_id: None,
                wallet_balance: None,
                checkpoint_time: Utc::now(),
            })
        }
    }

    async fn store_ledger_checkpoint(&self, participant_id: &ParticipantId, state: &LedgerState) -> GarpResult<()> {
        let active_contracts_json = serde_json::to_value(&state.active_contracts)
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        let wallet_balance_json = serde_json::to_value(&state.wallet_balance)
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        sqlx::query(r#"
            INSERT INTO ledger_checkpoints (participant_id, active_contracts, total_transactions, last_transaction_id, wallet_balance, checkpoint_time)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (participant_id) DO UPDATE SET
                active_contracts = EXCLUDED.active_contracts,
                total_transactions = EXCLUDED.total_transactions,
                last_transaction_id = EXCLUDED.last_transaction_id,
                wallet_balance = EXCLUDED.wallet_balance,
                checkpoint_time = EXCLUDED.checkpoint_time
        "#)
        .bind(&participant_id.0)
        .bind(active_contracts_json)
        .bind(state.total_transactions as i64)
        .bind(state.last_transaction_id.as_ref().map(|id| id.0))
        .bind(wallet_balance_json)
        .bind(state.checkpoint_time)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        Ok(())
    }

    async fn store_block(&self, block: &Block) -> GarpResult<()> {
        let txs_json = serde_json::to_value(&block.transactions)
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        sqlx::query(r#"
            INSERT INTO blocks (
                hash_hex, parent_hash, slot, epoch, proposer, state_root, tx_root, receipt_root, timestamp, transactions
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (hash_hex) DO NOTHING
        "#)
        .bind(hex::encode(&block.hash))
        .bind(&block.header.parent_hash)
        .bind(block.header.slot as i64)
        .bind(block.header.epoch as i64)
        .bind(&block.header.proposer.0)
        .bind(&block.header.state_root)
        .bind(&block.header.tx_root)
        .bind(&block.header.receipt_root)
        .bind(block.timestamp)
        .bind(txs_json)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        // Persist coalesced state changes for this block
        use crate::state_commitments::derive_state_changes;
        let changes = derive_state_changes(&block.transactions);
        // Remove any existing changes for this slot (idempotent on re-store)
        sqlx::query(r#"DELETE FROM block_state_changes WHERE slot = $1"#)
            .bind(block.header.slot as i64)
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        // Insert changes
        for c in changes {
            sqlx::query(r#"
                INSERT INTO block_state_changes (slot, state_key, value_hash)
                VALUES ($1, $2, $3)
                ON CONFLICT (slot, state_key) DO UPDATE SET value_hash = EXCLUDED.value_hash
            "#)
            .bind(block.header.slot as i64)
            .bind(&c.key)
            .bind(&c.value_hash)
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        }
        Ok(())
    }

    async fn get_block_by_slot(&self, slot: u64) -> GarpResult<Option<Block>> {
        let row = sqlx::query(r#"
            SELECT hash_hex, parent_hash, slot, epoch, proposer, state_root, tx_root, receipt_root, timestamp, transactions
            FROM blocks WHERE slot = $1 LIMIT 1
        "#)
        .bind(slot as i64)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        Ok(row.map(|row| {
            let header = BlockHeader {
                parent_hash: row.get::<Vec<u8>, _>("parent_hash"),
                slot: row.get::<i64, _>("slot") as u64,
                epoch: row.get::<i64, _>("epoch") as u64,
                proposer: ParticipantId(row.get::<String, _>("proposer")),
                state_root: row.get::<Vec<u8>, _>("state_root"),
                tx_root: row.get::<Vec<u8>, _>("tx_root"),
                receipt_root: row.get::<Vec<u8>, _>("receipt_root"),
            };
            let hash_hex: String = row.get("hash_hex");
            let hash = hex::decode(hash_hex).unwrap_or_default();
            let timestamp = row.get("timestamp");
            let txs: Vec<Transaction> = serde_json::from_value(row.get("transactions")).unwrap_or_default();
            Block { header, hash, timestamp, transactions: txs }
        }))
    }

    async fn get_latest_block(&self) -> GarpResult<Option<Block>> {
        let row = sqlx::query(r#"
            SELECT hash_hex, parent_hash, slot, epoch, proposer, state_root, tx_root, receipt_root, timestamp, transactions
            FROM blocks ORDER BY slot DESC LIMIT 1
        "#)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        Ok(row.map(|row| {
            let header = BlockHeader {
                parent_hash: row.get::<Vec<u8>, _>("parent_hash"),
                slot: row.get::<i64, _>("slot") as u64,
                epoch: row.get::<i64, _>("epoch") as u64,
                proposer: ParticipantId(row.get::<String, _>("proposer")),
                state_root: row.get::<Vec<u8>, _>("state_root"),
                tx_root: row.get::<Vec<u8>, _>("tx_root"),
                receipt_root: row.get::<Vec<u8>, _>("receipt_root"),
            };
            let hash_hex: String = row.get("hash_hex");
            let hash = hex::decode(hash_hex).unwrap_or_default();
            let timestamp = row.get("timestamp");
            let txs: Vec<Transaction> = serde_json::from_value(row.get("transactions")).unwrap_or_default();
            Block { header, hash, timestamp, transactions: txs }
        }))
    }

    async fn get_block_by_hash_hex(&self, hash_hex: &str) -> GarpResult<Option<Block>> {
        let row = sqlx::query(r#"
            SELECT hash_hex, parent_hash, slot, epoch, proposer, state_root, tx_root, receipt_root, timestamp, transactions
            FROM blocks WHERE hash_hex = $1 LIMIT 1
        "#)
        .bind(hash_hex)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        Ok(row.map(|row| {
            let header = BlockHeader {
                parent_hash: row.get::<Vec<u8>, _>("parent_hash"),
                slot: row.get::<i64, _>("slot") as u64,
                epoch: row.get::<i64, _>("epoch") as u64,
                proposer: ParticipantId(row.get::<String, _>("proposer")),
                state_root: row.get::<Vec<u8>, _>("state_root"),
                tx_root: row.get::<Vec<u8>, _>("tx_root"),
                receipt_root: row.get::<Vec<u8>, _>("receipt_root"),
            };
            let h_hex: String = row.get("hash_hex");
            let hash = hex::decode(h_hex).unwrap_or_default();
            let timestamp = row.get("timestamp");
            let txs: Vec<Transaction> = serde_json::from_value(row.get("transactions")).unwrap_or_default();
            Block { header, hash, timestamp, transactions: txs }
        }))
    }

    async fn list_blocks(&self, limit: Option<u32>, offset: Option<u32>) -> GarpResult<Vec<Block>> {
        let lim = limit.unwrap_or(25) as i64;
        let off = offset.unwrap_or(0) as i64;
        let rows = sqlx::query(r#"
            SELECT hash_hex, parent_hash, slot, epoch, proposer, state_root, tx_root, receipt_root, timestamp, transactions
            FROM blocks ORDER BY slot DESC LIMIT $1 OFFSET $2
        "#)
        .bind(lim)
        .bind(off)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        let mut blocks = Vec::with_capacity(rows.len());
        for row in rows {
            let header = BlockHeader {
                parent_hash: row.get::<Vec<u8>, _>("parent_hash"),
                slot: row.get::<i64, _>("slot") as u64,
                epoch: row.get::<i64, _>("epoch") as u64,
                proposer: ParticipantId(row.get::<String, _>("proposer")),
                state_root: row.get::<Vec<u8>, _>("state_root"),
                tx_root: row.get::<Vec<u8>, _>("tx_root"),
                receipt_root: row.get::<Vec<u8>, _>("receipt_root"),
            };
            let hash_hex: String = row.get("hash_hex");
            let hash = hex::decode(hash_hex).unwrap_or_default();
            let timestamp = row.get("timestamp");
            let txs: Vec<Transaction> = serde_json::from_value(row.get("transactions")).unwrap_or_default();
            blocks.push(Block { header, hash, timestamp, transactions: txs });
        }
        Ok(blocks)
    }

    async fn list_blocks_filtered(&self, epoch: Option<u64>, proposer: Option<String>, limit: Option<u32>, offset: Option<u32>) -> GarpResult<Vec<Block>> {
        let lim = limit.unwrap_or(25) as i64;
        let off = offset.unwrap_or(0) as i64;
        // Build dynamic WHERE clause
        let mut where_clauses: Vec<&str> = Vec::new();
        if epoch.is_some() { where_clauses.push("epoch = $3"); }
        if proposer.is_some() { where_clauses.push("proposer = $4"); }
        let where_sql = if where_clauses.is_empty() { "" } else { " WHERE "; };
        let where_joined = if where_clauses.is_empty() { String::new() } else { where_clauses.join(" AND ") };
        let sql = format!(
            "SELECT hash_hex, parent_hash, slot, epoch, proposer, state_root, tx_root, receipt_root, timestamp, transactions FROM blocks{}{} ORDER BY slot DESC LIMIT $1 OFFSET $2",
            where_sql,
            where_joined
        );

        let mut query = sqlx::query(&sql).bind(lim).bind(off);
        if let Some(e) = epoch { query = query.bind(e as i64); }
        if let Some(p) = proposer.as_ref() { query = query.bind(p); }

        let rows = query.fetch_all(&self.pool).await.map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        let mut blocks = Vec::with_capacity(rows.len());
        for row in rows {
            let header = BlockHeader {
                parent_hash: row.get::<Vec<u8>, _>("parent_hash"),
                slot: row.get::<i64, _>("slot") as u64,
                epoch: row.get::<i64, _>("epoch") as u64,
                proposer: ParticipantId(row.get::<String, _>("proposer")),
                state_root: row.get::<Vec<u8>, _>("state_root"),
                tx_root: row.get::<Vec<u8>, _>("tx_root"),
                receipt_root: row.get::<Vec<u8>, _>("receipt_root"),
            };
            let hash_hex: String = row.get("hash_hex");
            let hash = hex::decode(hash_hex).unwrap_or_default();
            let timestamp = row.get("timestamp");
            let txs: Vec<Transaction> = serde_json::from_value(row.get("transactions")).unwrap_or_default();
            blocks.push(Block { header, hash, timestamp, transactions: txs });
        }
        Ok(blocks)
    }

    async fn get_block_state_changes(&self, slot: u64) -> GarpResult<Vec<crate::state_commitments::StateChangeItem>> {
        let rows = sqlx::query(r#"SELECT state_key, value_hash FROM block_state_changes WHERE slot = $1 ORDER BY state_key"#)
            .bind(slot as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let key: String = row.get("state_key");
            let value_hash: Vec<u8> = row.get("value_hash");
            items.push(crate::state_commitments::StateChangeItem { key, value_hash });
        }
        Ok(items)
    }

    async fn store_contract_event(&self, event: &ContractEvent) -> GarpResult<()> {
        let data_json = serde_json::to_value(&event.data)
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        sqlx::query(r#"
            INSERT INTO contract_events (id, contract_id, event_type, data, timestamp, emitter)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO NOTHING
        "#)
        .bind(&event.id)
        .bind(event.contract_id.0)
        .bind(&event.event_type)
        .bind(data_json)
        .bind(event.timestamp)
        .bind(&event.emitter.0)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        Ok(())
    }

    async fn get_contract_events(&self, contract_id: &ContractId, limit: Option<u32>) -> GarpResult<Vec<ContractEvent>> {
        let query = match limit {
            Some(_) => r#"
                SELECT id, contract_id, event_type, data, timestamp, emitter
                FROM contract_events 
                WHERE contract_id = $1
                ORDER BY timestamp DESC 
                LIMIT $2
            "#,
            None => r#"
                SELECT id, contract_id, event_type, data, timestamp, emitter
                FROM contract_events 
                WHERE contract_id = $1
                ORDER BY timestamp DESC
            "#,
        };

        let rows = if let Some(limit) = limit {
            sqlx::query(query)
                .bind(contract_id.0)
                .bind(limit as i64)
                .fetch_all(&self.pool)
                .await
        } else {
            sqlx::query(query)
                .bind(contract_id.0)
                .fetch_all(&self.pool)
                .await
        }
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        let mut events = Vec::new();
        for row in rows {
            let data = serde_json::from_value(row.get("data"))
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

            let event = ContractEvent {
                id: row.get("id"),
                contract_id: ContractId(row.get("contract_id")),
                event_type: row.get("event_type"),
                data,
                timestamp: row.get("timestamp"),
                emitter: ParticipantId(row.get("emitter")),
            };

            events.push(event);
        }

        Ok(events)
    }

    async fn get_participant_events(&self, participant_id: &ParticipantId, limit: Option<u32>) -> GarpResult<Vec<ContractEvent>> {
        let query = match limit {
            Some(_) => r#"
                SELECT id, contract_id, event_type, data, timestamp, emitter
                FROM contract_events 
                WHERE emitter = $1
                ORDER BY timestamp DESC 
                LIMIT $2
            "#,
            None => r#"
                SELECT id, contract_id, event_type, data, timestamp, emitter
                FROM contract_events 
                WHERE emitter = $1
                ORDER BY timestamp DESC
            "#,
        };

        let rows = if let Some(limit) = limit {
            sqlx::query(query)
                .bind(&participant_id.0)
                .bind(limit as i64)
                .fetch_all(&self.pool)
                .await
        } else {
            sqlx::query(query)
                .bind(&participant_id.0)
                .fetch_all(&self.pool)
                .await
        }
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        let mut events = Vec::new();
        for row in rows {
            let data = serde_json::from_value(row.get("data"))
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

            let event = ContractEvent {
                id: row.get("id"),
                contract_id: ContractId(row.get("contract_id")),
                event_type: row.get("event_type"),
                data,
                timestamp: row.get("timestamp"),
                emitter: ParticipantId(row.get("emitter")),
            };

            events.push(event);
        }

        Ok(events)
    }

    async fn query_events(&self, query: &EventQuery) -> GarpResult<Vec<ContractEvent>> {
        let mut sql = "SELECT id, contract_id, event_type, data, timestamp, emitter FROM contract_events WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn sqlx::Encode<'_, sqlx::Postgres> + Send>> = Vec::new();
        let mut param_index = 1;

        if let Some(contract_id) = &query.contract_id {
            sql.push_str(&format!(" AND contract_id = ${}", param_index));
            params.push(Box::new(contract_id.0));
            param_index += 1;
        }

        if let Some(event_type) = &query.event_type {
            sql.push_str(&format!(" AND event_type = ${}", param_index));
            params.push(Box::new(event_type.clone()));
            param_index += 1;
        }

        if let Some(participant_id) = &query.participant_id {
            sql.push_str(&format!(" AND emitter = ${}", param_index));
            params.push(Box::new(participant_id.0.clone()));
            param_index += 1;
        }

        if let Some(from_timestamp) = query.from_timestamp {
            sql.push_str(&format!(" AND timestamp >= ${}", param_index));
            params.push(Box::new(from_timestamp));
            param_index += 1;
        }

        if let Some(to_timestamp) = query.to_timestamp {
            sql.push_str(&format!(" AND timestamp <= ${}", param_index));
            params.push(Box::new(to_timestamp));
            param_index += 1;
        }

        sql.push_str(" ORDER BY timestamp DESC");

        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT ${}", param_index));
            params.push(Box::new(limit as i64));
        }

        // Build the query dynamically
        let mut final_query = sqlx::query(&sql);
        for param in params {
            final_query = final_query.bind(param);
        }

        let rows = final_query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        let mut events = Vec::new();
        for row in rows {
            let data = serde_json::from_value(row.get("data"))
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

            let event = ContractEvent {
                id: row.get("id"),
                contract_id: ContractId(row.get("contract_id")),
                event_type: row.get("event_type"),
                data,
                timestamp: row.get("timestamp"),
                emitter: ParticipantId(row.get("emitter")),
            };

            events.push(event);
        }

        Ok(events)
    }
}

/// In-memory storage for testing
pub struct MemoryStorage {
    contracts: parking_lot::RwLock<HashMap<ContractId, Contract>>,
    transactions: parking_lot::RwLock<HashMap<TransactionId, Transaction>>,
    wallet_balances: parking_lot::RwLock<HashMap<ParticipantId, WalletBalance>>,
    ledger_states: parking_lot::RwLock<HashMap<ParticipantId, LedgerState>>,
    blocks_by_hash: parking_lot::RwLock<HashMap<String, Block>>,
    blocks_by_slot: parking_lot::RwLock<HashMap<u64, Block>>,
    block_state_changes_by_slot: parking_lot::RwLock<HashMap<u64, Vec<crate::state_commitments::StateChangeItem>>>,
    contract_events: parking_lot::RwLock<HashMap<String, ContractEvent>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            contracts: parking_lot::RwLock::new(HashMap::new()),
            transactions: parking_lot::RwLock::new(HashMap::new()),
            wallet_balances: parking_lot::RwLock::new(HashMap::new()),
            ledger_states: parking_lot::RwLock::new(HashMap::new()),
            blocks_by_hash: parking_lot::RwLock::new(HashMap::new()),
            blocks_by_slot: parking_lot::RwLock::new(HashMap::new()),
            block_state_changes_by_slot: parking_lot::RwLock::new(HashMap::new()),
            contract_events: parking_lot::RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl StorageBackend for MemoryStorage {
    async fn store_contract(&self, contract: &Contract) -> GarpResult<()> {
        let mut contracts = self.contracts.write();
        contracts.insert(contract.id.clone(), contract.clone());
        Ok(())
    }

    async fn get_contract(&self, contract_id: &ContractId) -> GarpResult<Option<Contract>> {
        let contracts = self.contracts.read();
        Ok(contracts.get(contract_id).cloned())
    }

    async fn archive_contract(&self, contract_id: &ContractId) -> GarpResult<()> {
        let mut contracts = self.contracts.write();
        if let Some(contract) = contracts.get_mut(contract_id) {
            contract.archived = true;
        }
        Ok(())
    }

    async fn list_contracts(&self, participant_id: &ParticipantId, active_only: bool) -> GarpResult<Vec<Contract>> {
        let contracts = self.contracts.read();
        let filtered: Vec<Contract> = contracts
            .values()
            .filter(|c| {
                let is_stakeholder = c.signatories.contains(participant_id) || c.observers.contains(participant_id);
                let is_active = !active_only || !c.archived;
                is_stakeholder && is_active
            })
            .cloned()
            .collect();
        Ok(filtered)
    }

    async fn store_transaction(&self, transaction: &Transaction) -> GarpResult<()> {
        let mut transactions = self.transactions.write();
        transactions.insert(transaction.id.clone(), transaction.clone());
        Ok(())
    }

    async fn get_transaction(&self, transaction_id: &TransactionId) -> GarpResult<Option<Transaction>> {
        let transactions = self.transactions.read();
        Ok(transactions.get(transaction_id).cloned())
    }

    async fn list_transactions(&self, participant_id: &ParticipantId, limit: Option<u32>) -> GarpResult<Vec<Transaction>> {
        let transactions = self.transactions.read();
        let mut filtered: Vec<Transaction> = transactions
            .values()
            .filter(|t| t.submitter == *participant_id)
            .cloned()
            .collect();
        
        filtered.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        
        if let Some(limit) = limit {
            filtered.truncate(limit as usize);
        }
        
        Ok(filtered)
    }

    async fn store_wallet_balance(&self, balance: &WalletBalance) -> GarpResult<()> {
        let mut balances = self.wallet_balances.write();
        balances.insert(balance.participant_id.clone(), balance.clone());
        Ok(())
    }

    async fn get_wallet_balance(&self, participant_id: &ParticipantId) -> GarpResult<Option<WalletBalance>> {
        let balances = self.wallet_balances.read();
        Ok(balances.get(participant_id).cloned())
    }

    async fn update_asset_balance(&self, participant_id: &ParticipantId, asset: &Asset, delta: i64) -> GarpResult<()> {
        let mut balances = self.wallet_balances.write();
        let balance = balances.entry(participant_id.clone()).or_insert_with(|| WalletBalance {
            participant_id: participant_id.clone(),
            assets: Vec::new(),
            last_updated: Utc::now(),
        });

        let mut found = false;
        for existing_asset in &mut balance.assets {
            if existing_asset.id == asset.id {
                existing_asset.amount = (existing_asset.amount as i64 + delta).max(0) as u64;
                found = true;
                break;
            }
        }

        if !found && delta > 0 {
            let mut new_asset = asset.clone();
            new_asset.amount = delta as u64;
            balance.assets.push(new_asset);
        }

        balance.last_updated = Utc::now();
        Ok(())
    }

    async fn get_ledger_state(&self, participant_id: &ParticipantId) -> GarpResult<LedgerState> {
        let states = self.ledger_states.read();
        Ok(states.get(participant_id).cloned().unwrap_or_else(|| LedgerState {
            participant_id: participant_id.clone(),
            active_contracts: Vec::new(),
            total_transactions: 0,
            last_transaction_id: None,
            wallet_balance: None,
            checkpoint_time: Utc::now(),
        }))
    }

    async fn store_ledger_checkpoint(&self, participant_id: &ParticipantId, state: &LedgerState) -> GarpResult<()> {
        let mut states = self.ledger_states.write();
        states.insert(participant_id.clone(), state.clone());
        Ok(())
    }

    async fn store_block(&self, block: &Block) -> GarpResult<()> {
        let hash_hex = hex::encode(&block.hash);
        {
            let mut by_hash = self.blocks_by_hash.write();
            by_hash.insert(hash_hex, block.clone());
        }
        {
            let mut by_slot = self.blocks_by_slot.write();
            by_slot.insert(block.header.slot, block.clone());
        }
        // Persist coalesced state changes for this block
        use crate::state_commitments::derive_state_changes;
        let changes = derive_state_changes(&block.transactions);
        {
            let mut map = self.block_state_changes_by_slot.write();
            map.insert(block.header.slot, changes);
        }
        Ok(())
    }

    async fn get_block_by_slot(&self, slot: u64) -> GarpResult<Option<Block>> {
        Ok(self.blocks_by_slot.read().get(&slot).cloned())
    }

    async fn get_latest_block(&self) -> GarpResult<Option<Block>> {
        let by_slot = self.blocks_by_slot.read();
        let latest = by_slot.keys().max().and_then(|s| by_slot.get(s)).cloned();
        Ok(latest)
    }

    async fn get_block_by_hash_hex(&self, hash_hex: &str) -> GarpResult<Option<Block>> {
        Ok(self.blocks_by_hash.read().get(hash_hex).cloned())
    }

    async fn list_blocks(&self, limit: Option<u32>, offset: Option<u32>) -> GarpResult<Vec<Block>> {
        let lim = limit.unwrap_or(25) as usize;
        let off = offset.unwrap_or(0) as usize;
        let blocks_map = self.blocks_by_slot.read();
        let mut blocks: Vec<(u64, Block)> = blocks_map.iter().map(|(s, b)| (*s, b.clone())).collect();
        blocks.sort_by(|a, b| b.0.cmp(&a.0));
        let sliced = blocks.into_iter().skip(off).take(lim).map(|(_, b)| b).collect();
        Ok(sliced)
    }

    async fn list_blocks_filtered(&self, epoch: Option<u64>, proposer: Option<String>, limit: Option<u32>, offset: Option<u32>) -> GarpResult<Vec<Block>> {
        let lim = limit.unwrap_or(25) as usize;
        let off = offset.unwrap_or(0) as usize;
        let blocks_map = self.blocks_by_slot.read();
        let mut blocks: Vec<(u64, Block)> = blocks_map.iter().map(|(s, b)| (*s, b.clone())).collect();
        // Filter by epoch and proposer
        blocks.retain(|(_, b)| match epoch { Some(e) => b.header.epoch == e, None => true });
        blocks.retain(|(_, b)| match proposer.as_ref() { Some(p) => &b.header.proposer.0 == p, None => true });
        blocks.sort_by(|a, b| b.0.cmp(&a.0));
        let sliced = blocks.into_iter().skip(off).take(lim).map(|(_, b)| b).collect();
        Ok(sliced)
    }

    async fn get_block_state_changes(&self, slot: u64) -> GarpResult<Vec<crate::state_commitments::StateChangeItem>> {
        let map = self.block_state_changes_by_slot.read();
        Ok(map.get(&slot).cloned().unwrap_or_default())
    }

    async fn store_contract_event(&self, event: &ContractEvent) -> GarpResult<()> {
        let mut events = self.contract_events.write();
        events.insert(event.id.clone(), event.clone());
        Ok(())
    }

    async fn get_contract_events(&self, contract_id: &ContractId, limit: Option<u32>) -> GarpResult<Vec<ContractEvent>> {
        let events = self.contract_events.read();
        let mut filtered: Vec<ContractEvent> = events
            .values()
            .filter(|e| e.contract_id == *contract_id)
            .cloned()
            .collect();
        
        filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        if let Some(limit) = limit {
            filtered.truncate(limit as usize);
        }
        
        Ok(filtered)
    }

    async fn get_participant_events(&self, participant_id: &ParticipantId, limit: Option<u32>) -> GarpResult<Vec<ContractEvent>> {
        let events = self.contract_events.read();
        let mut filtered: Vec<ContractEvent> = events
            .values()
            .filter(|e| e.emitter == *participant_id)
            .cloned()
            .collect();
        
        filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        if let Some(limit) = limit {
            filtered.truncate(limit as usize);
        }
        
        Ok(filtered)
    }

    async fn query_events(&self, query: &EventQuery) -> GarpResult<Vec<ContractEvent>> {
        let events = self.contract_events.read();
        let mut filtered: Vec<ContractEvent> = events.values().cloned().collect();
        
        if let Some(contract_id) = &query.contract_id {
            filtered.retain(|e| e.contract_id == *contract_id);
        }
        
        if let Some(event_type) = &query.event_type {
            filtered.retain(|e| &e.event_type == event_type);
        }
        
        if let Some(participant_id) = &query.participant_id {
            filtered.retain(|e| e.emitter == *participant_id);
        }
        
        if let Some(from_timestamp) = query.from_timestamp {
            filtered.retain(|e| e.timestamp >= from_timestamp);
        }
        
        if let Some(to_timestamp) = query.to_timestamp {
            filtered.retain(|e| e.timestamp <= to_timestamp);
        }
        
        filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        if let Some(limit) = query.limit {
            filtered.truncate(limit as usize);
        }
        
        Ok(filtered)
    }
}

/// Storage factory
pub struct Storage;

impl Storage {
    /// Create PostgreSQL storage
    pub async fn postgres(database_url: &str, max_connections: u32) -> GarpResult<Box<dyn StorageBackend>> {
        let storage = PostgresStorage::new(database_url, max_connections).await?;
        Ok(Box::new(storage))
    }

    /// Create in-memory storage for testing
    pub fn memory() -> Box<dyn StorageBackend> {
        Box::new(MemoryStorage::new())
    }
}