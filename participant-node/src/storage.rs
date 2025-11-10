use garp_common::{
    Contract, Transaction, TransactionId, ContractId, ParticipantId, Asset, WalletBalance,
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
}

/// In-memory storage for testing
pub struct MemoryStorage {
    contracts: parking_lot::RwLock<HashMap<ContractId, Contract>>,
    transactions: parking_lot::RwLock<HashMap<TransactionId, Transaction>>,
    wallet_balances: parking_lot::RwLock<HashMap<ParticipantId, WalletBalance>>,
    ledger_states: parking_lot::RwLock<HashMap<ParticipantId, LedgerState>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            contracts: parking_lot::RwLock::new(HashMap::new()),
            transactions: parking_lot::RwLock::new(HashMap::new()),
            wallet_balances: parking_lot::RwLock::new(HashMap::new()),
            ledger_states: parking_lot::RwLock::new(HashMap::new()),
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