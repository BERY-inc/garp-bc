use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use garp_common::{GarpResult, Transaction, TransactionId, ParticipantId};
use crate::config::DatabaseConfig;

/// Transaction sequence entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequencedTransaction {
    /// Unique sequence number
    pub sequence_number: u64,
    
    /// Transaction ID
    pub transaction_id: TransactionId,
    
    /// Encrypted transaction data
    pub encrypted_data: Vec<u8>,
    
    /// Transaction metadata
    pub metadata: TransactionMetadata,
    
    /// Timestamp when sequenced
    pub sequenced_at: DateTime<Utc>,
    
    /// Domain ID
    pub domain_id: String,
    
    /// Batch ID (for batched transactions)
    pub batch_id: Option<Uuid>,
    
    /// Status of the transaction
    pub status: SequenceStatus,
}

/// Transaction metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionMetadata {
    /// Participating parties (encrypted)
    pub participants: Vec<ParticipantId>,
    
    /// Transaction type
    pub transaction_type: String,
    
    /// Priority level
    pub priority: u8,
    
    /// Size in bytes
    pub size: usize,
    
    /// Hash of the transaction
    pub hash: String,
    
    /// Dependencies (other transaction IDs)
    pub dependencies: Vec<TransactionId>,
    
    /// Expiration time
    pub expires_at: Option<DateTime<Utc>>,
}

/// Sequence status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SequenceStatus {
    /// Transaction is sequenced and pending consensus
    Sequenced,
    
    /// Transaction is in consensus phase
    InConsensus,
    
    /// Transaction is committed
    Committed,
    
    /// Transaction is rejected
    Rejected,
    
    /// Transaction expired
    Expired,
}

/// Consensus state for a transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusState {
    /// Transaction ID
    pub transaction_id: TransactionId,
    
    /// Consensus round
    pub round: u64,
    
    /// Current phase
    pub phase: ConsensusPhase,
    
    /// Votes received
    pub votes: HashMap<ParticipantId, ConsensusVote>,
    
    /// Required participants
    pub required_participants: Vec<ParticipantId>,
    
    /// Consensus result
    pub result: Option<ConsensusResult>,
    
    /// Started at
    pub started_at: DateTime<Utc>,
    
    /// Timeout
    pub timeout_at: DateTime<Utc>,
}

/// Consensus phase
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConsensusPhase {
    /// Phase 1: Validation
    Validation,
    
    /// Phase 2: Commit
    Commit,
    
    /// Completed
    Completed,
    
    /// Aborted
    Aborted,
}

/// Consensus vote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusVote {
    /// Participant ID
    pub participant_id: ParticipantId,
    
    /// Vote (approve/reject)
    pub vote: bool,
    
    /// Reason for rejection (if applicable)
    pub reason: Option<String>,
    
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Signature
    pub signature: String,
}

/// Consensus result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusResult {
    /// Transaction approved and committed
    Approved,
    
    /// Transaction rejected
    Rejected { reason: String },
    
    /// Consensus timed out
    Timeout,
}

/// Participant registration in domain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainParticipant {
    /// Participant ID
    pub participant_id: ParticipantId,
    
    /// Participant public key
    pub public_key: String,
    
    /// Participant endpoint
    pub endpoint: String,
    
    /// Registration status
    pub status: ParticipantStatus,
    
    /// Registered at
    pub registered_at: DateTime<Utc>,
    
    /// Last seen
    pub last_seen: DateTime<Utc>,
    
    /// Participant metadata
    pub metadata: ParticipantMetadata,
}

/// Participant status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ParticipantStatus {
    /// Pending approval
    Pending,
    
    /// Active participant
    Active,
    
    /// Suspended participant
    Suspended,
    
    /// Removed participant
    Removed,
}

/// Participant metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantMetadata {
    /// Participant name
    pub name: String,
    
    /// Organization
    pub organization: Option<String>,
    
    /// Contact information
    pub contact: Option<String>,
    
    /// Supported transaction types
    pub supported_types: Vec<String>,
    
    /// Capabilities
    pub capabilities: Vec<String>,
}

/// Domain statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainStats {
    /// Total transactions sequenced
    pub total_transactions: u64,
    
    /// Transactions in last 24 hours
    pub transactions_24h: u64,
    
    /// Average transactions per second
    pub avg_tps: f64,
    
    /// Active participants
    pub active_participants: usize,
    
    /// Pending consensus transactions
    pub pending_consensus: usize,
    
    /// Current sequence number
    pub current_sequence: u64,
    
    /// Domain uptime
    pub uptime_seconds: u64,
}

/// Storage backend trait
#[async_trait]
pub trait StorageBackend: Send + Sync {
    // Transaction sequencing
    async fn store_sequenced_transaction(&self, transaction: &SequencedTransaction) -> GarpResult<()>;
    async fn get_sequenced_transaction(&self, sequence_number: u64) -> GarpResult<Option<SequencedTransaction>>;
    async fn get_transactions_by_batch(&self, batch_id: Uuid) -> GarpResult<Vec<SequencedTransaction>>;
    async fn get_transactions_in_range(&self, start: u64, end: u64) -> GarpResult<Vec<SequencedTransaction>>;
    async fn update_transaction_status(&self, sequence_number: u64, status: SequenceStatus) -> GarpResult<()>;
    async fn get_next_sequence_number(&self) -> GarpResult<u64>;
    
    // Consensus management
    async fn store_consensus_state(&self, state: &ConsensusState) -> GarpResult<()>;
    async fn get_consensus_state(&self, transaction_id: &TransactionId) -> GarpResult<Option<ConsensusState>>;
    async fn update_consensus_phase(&self, transaction_id: &TransactionId, phase: ConsensusPhase) -> GarpResult<()>;
    async fn add_consensus_vote(&self, transaction_id: &TransactionId, vote: &ConsensusVote) -> GarpResult<()>;
    async fn set_consensus_result(&self, transaction_id: &TransactionId, result: ConsensusResult) -> GarpResult<()>;
    async fn get_pending_consensus(&self) -> GarpResult<Vec<ConsensusState>>;
    async fn cleanup_expired_consensus(&self) -> GarpResult<u64>;
    
    // Participant management
    async fn register_participant(&self, participant: &DomainParticipant) -> GarpResult<()>;
    async fn get_participant(&self, participant_id: &ParticipantId) -> GarpResult<Option<DomainParticipant>>;
    async fn update_participant_status(&self, participant_id: &ParticipantId, status: ParticipantStatus) -> GarpResult<()>;
    async fn update_participant_last_seen(&self, participant_id: &ParticipantId) -> GarpResult<()>;
    async fn list_participants(&self, status: Option<ParticipantStatus>) -> GarpResult<Vec<DomainParticipant>>;
    async fn remove_participant(&self, participant_id: &ParticipantId) -> GarpResult<()>;
    
    // Statistics and monitoring
    async fn get_domain_stats(&self) -> GarpResult<DomainStats>;
    async fn increment_transaction_count(&self) -> GarpResult<()>;
    async fn record_tps_sample(&self, tps: f64) -> GarpResult<()>;
    
    // Maintenance
    async fn cleanup_old_transactions(&self, older_than: DateTime<Utc>) -> GarpResult<u64>;
    async fn compact_storage(&self) -> GarpResult<()>;
}

/// PostgreSQL storage implementation
pub struct PostgresStorage {
    pool: PgPool,
}

impl PostgresStorage {
    /// Create new PostgreSQL storage
    pub async fn new(config: &DatabaseConfig) -> GarpResult<Self> {
        let pool = PgPool::connect(&config.url).await?;
        
        let storage = Self { pool };
        storage.initialize_schema().await?;
        
        Ok(storage)
    }
    
    /// Initialize database schema
    async fn initialize_schema(&self) -> GarpResult<()> {
        // Create sequenced_transactions table
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS sequenced_transactions (
                sequence_number BIGSERIAL PRIMARY KEY,
                transaction_id TEXT NOT NULL,
                encrypted_data BYTEA NOT NULL,
                metadata JSONB NOT NULL,
                sequenced_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                domain_id TEXT NOT NULL,
                batch_id UUID,
                status TEXT NOT NULL DEFAULT 'Sequenced',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
        "#).execute(&self.pool).await?;
        
        // Create consensus_states table
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS consensus_states (
                transaction_id TEXT PRIMARY KEY,
                round BIGINT NOT NULL,
                phase TEXT NOT NULL,
                votes JSONB NOT NULL DEFAULT '{}',
                required_participants JSONB NOT NULL,
                result JSONB,
                started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                timeout_at TIMESTAMPTZ NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
        "#).execute(&self.pool).await?;
        
        // Create domain_participants table
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS domain_participants (
                participant_id TEXT PRIMARY KEY,
                public_key TEXT NOT NULL,
                endpoint TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'Pending',
                registered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                last_seen TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                metadata JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
        "#).execute(&self.pool).await?;
        
        // Create domain_stats table
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS domain_stats (
                id SERIAL PRIMARY KEY,
                total_transactions BIGINT NOT NULL DEFAULT 0,
                transactions_24h BIGINT NOT NULL DEFAULT 0,
                avg_tps DOUBLE PRECISION NOT NULL DEFAULT 0.0,
                current_sequence BIGINT NOT NULL DEFAULT 0,
                uptime_seconds BIGINT NOT NULL DEFAULT 0,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
        "#).execute(&self.pool).await?;
        
        // Insert initial stats record if not exists
        sqlx::query(r#"
            INSERT INTO domain_stats (id, total_transactions, transactions_24h, avg_tps, current_sequence, uptime_seconds)
            VALUES (1, 0, 0, 0.0, 0, 0)
            ON CONFLICT (id) DO NOTHING
        "#).execute(&self.pool).await?;
        
        // Create indexes
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_sequenced_transactions_domain_id ON sequenced_transactions(domain_id)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_sequenced_transactions_batch_id ON sequenced_transactions(batch_id)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_sequenced_transactions_status ON sequenced_transactions(status)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_consensus_states_phase ON consensus_states(phase)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_consensus_states_timeout ON consensus_states(timeout_at)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_domain_participants_status ON domain_participants(status)")
            .execute(&self.pool).await?;
        
        Ok(())
    }
}

#[async_trait]
impl StorageBackend for PostgresStorage {
    async fn store_sequenced_transaction(&self, transaction: &SequencedTransaction) -> GarpResult<()> {
        sqlx::query(r#"
            INSERT INTO sequenced_transactions 
            (sequence_number, transaction_id, encrypted_data, metadata, sequenced_at, domain_id, batch_id, status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#)
        .bind(transaction.sequence_number as i64)
        .bind(&transaction.transaction_id)
        .bind(&transaction.encrypted_data)
        .bind(serde_json::to_value(&transaction.metadata)?)
        .bind(transaction.sequenced_at)
        .bind(&transaction.domain_id)
        .bind(transaction.batch_id)
        .bind(serde_json::to_string(&transaction.status)?)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn get_sequenced_transaction(&self, sequence_number: u64) -> GarpResult<Option<SequencedTransaction>> {
        let row = sqlx::query(r#"
            SELECT sequence_number, transaction_id, encrypted_data, metadata, sequenced_at, domain_id, batch_id, status
            FROM sequenced_transactions WHERE sequence_number = $1
        "#)
        .bind(sequence_number as i64)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row {
            let transaction = SequencedTransaction {
                sequence_number: row.get::<i64, _>("sequence_number") as u64,
                transaction_id: row.get("transaction_id"),
                encrypted_data: row.get("encrypted_data"),
                metadata: serde_json::from_value(row.get("metadata"))?,
                sequenced_at: row.get("sequenced_at"),
                domain_id: row.get("domain_id"),
                batch_id: row.get("batch_id"),
                status: serde_json::from_str(&row.get::<String, _>("status"))?,
            };
            Ok(Some(transaction))
        } else {
            Ok(None)
        }
    }
    
    async fn get_transactions_by_batch(&self, batch_id: Uuid) -> GarpResult<Vec<SequencedTransaction>> {
        let rows = sqlx::query(r#"
            SELECT sequence_number, transaction_id, encrypted_data, metadata, sequenced_at, domain_id, batch_id, status
            FROM sequenced_transactions WHERE batch_id = $1 ORDER BY sequence_number
        "#)
        .bind(batch_id)
        .fetch_all(&self.pool)
        .await?;
        
        let mut transactions = Vec::new();
        for row in rows {
            transactions.push(SequencedTransaction {
                sequence_number: row.get::<i64, _>("sequence_number") as u64,
                transaction_id: row.get("transaction_id"),
                encrypted_data: row.get("encrypted_data"),
                metadata: serde_json::from_value(row.get("metadata"))?,
                sequenced_at: row.get("sequenced_at"),
                domain_id: row.get("domain_id"),
                batch_id: row.get("batch_id"),
                status: serde_json::from_str(&row.get::<String, _>("status"))?,
            });
        }
        
        Ok(transactions)
    }
    
    async fn get_transactions_in_range(&self, start: u64, end: u64) -> GarpResult<Vec<SequencedTransaction>> {
        let rows = sqlx::query(r#"
            SELECT sequence_number, transaction_id, encrypted_data, metadata, sequenced_at, domain_id, batch_id, status
            FROM sequenced_transactions 
            WHERE sequence_number >= $1 AND sequence_number <= $2 
            ORDER BY sequence_number
        "#)
        .bind(start as i64)
        .bind(end as i64)
        .fetch_all(&self.pool)
        .await?;
        
        let mut transactions = Vec::new();
        for row in rows {
            transactions.push(SequencedTransaction {
                sequence_number: row.get::<i64, _>("sequence_number") as u64,
                transaction_id: row.get("transaction_id"),
                encrypted_data: row.get("encrypted_data"),
                metadata: serde_json::from_value(row.get("metadata"))?,
                sequenced_at: row.get("sequenced_at"),
                domain_id: row.get("domain_id"),
                batch_id: row.get("batch_id"),
                status: serde_json::from_str(&row.get::<String, _>("status"))?,
            });
        }
        
        Ok(transactions)
    }
    
    async fn update_transaction_status(&self, sequence_number: u64, status: SequenceStatus) -> GarpResult<()> {
        sqlx::query("UPDATE sequenced_transactions SET status = $1, updated_at = NOW() WHERE sequence_number = $2")
            .bind(serde_json::to_string(&status)?)
            .bind(sequence_number as i64)
            .execute(&self.pool)
            .await?;
        
        Ok(())
    }
    
    async fn get_next_sequence_number(&self) -> GarpResult<u64> {
        let row = sqlx::query("SELECT COALESCE(MAX(sequence_number), 0) + 1 as next_seq FROM sequenced_transactions")
            .fetch_one(&self.pool)
            .await?;
        
        Ok(row.get::<i64, _>("next_seq") as u64)
    }
    
    async fn store_consensus_state(&self, state: &ConsensusState) -> GarpResult<()> {
        sqlx::query(r#"
            INSERT INTO consensus_states 
            (transaction_id, round, phase, votes, required_participants, result, started_at, timeout_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (transaction_id) DO UPDATE SET
                round = EXCLUDED.round,
                phase = EXCLUDED.phase,
                votes = EXCLUDED.votes,
                result = EXCLUDED.result,
                updated_at = NOW()
        "#)
        .bind(&state.transaction_id)
        .bind(state.round as i64)
        .bind(serde_json::to_string(&state.phase)?)
        .bind(serde_json::to_value(&state.votes)?)
        .bind(serde_json::to_value(&state.required_participants)?)
        .bind(serde_json::to_value(&state.result)?)
        .bind(state.started_at)
        .bind(state.timeout_at)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn get_consensus_state(&self, transaction_id: &TransactionId) -> GarpResult<Option<ConsensusState>> {
        let row = sqlx::query(r#"
            SELECT transaction_id, round, phase, votes, required_participants, result, started_at, timeout_at
            FROM consensus_states WHERE transaction_id = $1
        "#)
        .bind(transaction_id)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row {
            let state = ConsensusState {
                transaction_id: row.get("transaction_id"),
                round: row.get::<i64, _>("round") as u64,
                phase: serde_json::from_str(&row.get::<String, _>("phase"))?,
                votes: serde_json::from_value(row.get("votes"))?,
                required_participants: serde_json::from_value(row.get("required_participants"))?,
                result: serde_json::from_value(row.get("result"))?,
                started_at: row.get("started_at"),
                timeout_at: row.get("timeout_at"),
            };
            Ok(Some(state))
        } else {
            Ok(None)
        }
    }
    
    async fn update_consensus_phase(&self, transaction_id: &TransactionId, phase: ConsensusPhase) -> GarpResult<()> {
        sqlx::query("UPDATE consensus_states SET phase = $1, updated_at = NOW() WHERE transaction_id = $2")
            .bind(serde_json::to_string(&phase)?)
            .bind(transaction_id)
            .execute(&self.pool)
            .await?;
        
        Ok(())
    }
    
    async fn add_consensus_vote(&self, transaction_id: &TransactionId, vote: &ConsensusVote) -> GarpResult<()> {
        sqlx::query(r#"
            UPDATE consensus_states 
            SET votes = votes || jsonb_build_object($2, $3), updated_at = NOW()
            WHERE transaction_id = $1
        "#)
        .bind(transaction_id)
        .bind(&vote.participant_id)
        .bind(serde_json::to_value(vote)?)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn set_consensus_result(&self, transaction_id: &TransactionId, result: ConsensusResult) -> GarpResult<()> {
        sqlx::query("UPDATE consensus_states SET result = $1, updated_at = NOW() WHERE transaction_id = $2")
            .bind(serde_json::to_value(&result)?)
            .bind(transaction_id)
            .execute(&self.pool)
            .await?;
        
        Ok(())
    }
    
    async fn get_pending_consensus(&self) -> GarpResult<Vec<ConsensusState>> {
        let rows = sqlx::query(r#"
            SELECT transaction_id, round, phase, votes, required_participants, result, started_at, timeout_at
            FROM consensus_states 
            WHERE phase IN ('Validation', 'Commit') AND timeout_at > NOW()
            ORDER BY started_at
        "#)
        .fetch_all(&self.pool)
        .await?;
        
        let mut states = Vec::new();
        for row in rows {
            states.push(ConsensusState {
                transaction_id: row.get("transaction_id"),
                round: row.get::<i64, _>("round") as u64,
                phase: serde_json::from_str(&row.get::<String, _>("phase"))?,
                votes: serde_json::from_value(row.get("votes"))?,
                required_participants: serde_json::from_value(row.get("required_participants"))?,
                result: serde_json::from_value(row.get("result"))?,
                started_at: row.get("started_at"),
                timeout_at: row.get("timeout_at"),
            });
        }
        
        Ok(states)
    }
    
    async fn cleanup_expired_consensus(&self) -> GarpResult<u64> {
        let result = sqlx::query(r#"
            UPDATE consensus_states 
            SET phase = 'Aborted', result = '{"Timeout": null}', updated_at = NOW()
            WHERE timeout_at <= NOW() AND phase IN ('Validation', 'Commit')
        "#)
        .execute(&self.pool)
        .await?;
        
        Ok(result.rows_affected())
    }
    
    async fn register_participant(&self, participant: &DomainParticipant) -> GarpResult<()> {
        sqlx::query(r#"
            INSERT INTO domain_participants 
            (participant_id, public_key, endpoint, status, registered_at, last_seen, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (participant_id) DO UPDATE SET
                public_key = EXCLUDED.public_key,
                endpoint = EXCLUDED.endpoint,
                status = EXCLUDED.status,
                metadata = EXCLUDED.metadata,
                updated_at = NOW()
        "#)
        .bind(&participant.participant_id)
        .bind(&participant.public_key)
        .bind(&participant.endpoint)
        .bind(serde_json::to_string(&participant.status)?)
        .bind(participant.registered_at)
        .bind(participant.last_seen)
        .bind(serde_json::to_value(&participant.metadata)?)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn get_participant(&self, participant_id: &ParticipantId) -> GarpResult<Option<DomainParticipant>> {
        let row = sqlx::query(r#"
            SELECT participant_id, public_key, endpoint, status, registered_at, last_seen, metadata
            FROM domain_participants WHERE participant_id = $1
        "#)
        .bind(participant_id)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row {
            let participant = DomainParticipant {
                participant_id: row.get("participant_id"),
                public_key: row.get("public_key"),
                endpoint: row.get("endpoint"),
                status: serde_json::from_str(&row.get::<String, _>("status"))?,
                registered_at: row.get("registered_at"),
                last_seen: row.get("last_seen"),
                metadata: serde_json::from_value(row.get("metadata"))?,
            };
            Ok(Some(participant))
        } else {
            Ok(None)
        }
    }
    
    async fn update_participant_status(&self, participant_id: &ParticipantId, status: ParticipantStatus) -> GarpResult<()> {
        sqlx::query("UPDATE domain_participants SET status = $1, updated_at = NOW() WHERE participant_id = $2")
            .bind(serde_json::to_string(&status)?)
            .bind(participant_id)
            .execute(&self.pool)
            .await?;
        
        Ok(())
    }
    
    async fn update_participant_last_seen(&self, participant_id: &ParticipantId) -> GarpResult<()> {
        sqlx::query("UPDATE domain_participants SET last_seen = NOW(), updated_at = NOW() WHERE participant_id = $2")
            .bind(participant_id)
            .execute(&self.pool)
            .await?;
        
        Ok(())
    }
    
    async fn list_participants(&self, status: Option<ParticipantStatus>) -> GarpResult<Vec<DomainParticipant>> {
        let rows = if let Some(status) = status {
            sqlx::query(r#"
                SELECT participant_id, public_key, endpoint, status, registered_at, last_seen, metadata
                FROM domain_participants WHERE status = $1 ORDER BY registered_at
            "#)
            .bind(serde_json::to_string(&status)?)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(r#"
                SELECT participant_id, public_key, endpoint, status, registered_at, last_seen, metadata
                FROM domain_participants ORDER BY registered_at
            "#)
            .fetch_all(&self.pool)
            .await?
        };
        
        let mut participants = Vec::new();
        for row in rows {
            participants.push(DomainParticipant {
                participant_id: row.get("participant_id"),
                public_key: row.get("public_key"),
                endpoint: row.get("endpoint"),
                status: serde_json::from_str(&row.get::<String, _>("status"))?,
                registered_at: row.get("registered_at"),
                last_seen: row.get("last_seen"),
                metadata: serde_json::from_value(row.get("metadata"))?,
            });
        }
        
        Ok(participants)
    }
    
    async fn remove_participant(&self, participant_id: &ParticipantId) -> GarpResult<()> {
        sqlx::query("DELETE FROM domain_participants WHERE participant_id = $1")
            .bind(participant_id)
            .execute(&self.pool)
            .await?;
        
        Ok(())
    }
    
    async fn get_domain_stats(&self) -> GarpResult<DomainStats> {
        let row = sqlx::query(r#"
            SELECT total_transactions, transactions_24h, avg_tps, current_sequence, uptime_seconds
            FROM domain_stats WHERE id = 1
        "#)
        .fetch_one(&self.pool)
        .await?;
        
        let active_participants = sqlx::query("SELECT COUNT(*) as count FROM domain_participants WHERE status = 'Active'")
            .fetch_one(&self.pool)
            .await?
            .get::<i64, _>("count") as usize;
        
        let pending_consensus = sqlx::query("SELECT COUNT(*) as count FROM consensus_states WHERE phase IN ('Validation', 'Commit')")
            .fetch_one(&self.pool)
            .await?
            .get::<i64, _>("count") as usize;
        
        Ok(DomainStats {
            total_transactions: row.get::<i64, _>("total_transactions") as u64,
            transactions_24h: row.get::<i64, _>("transactions_24h") as u64,
            avg_tps: row.get("avg_tps"),
            active_participants,
            pending_consensus,
            current_sequence: row.get::<i64, _>("current_sequence") as u64,
            uptime_seconds: row.get::<i64, _>("uptime_seconds") as u64,
        })
    }
    
    async fn increment_transaction_count(&self) -> GarpResult<()> {
        sqlx::query(r#"
            UPDATE domain_stats 
            SET total_transactions = total_transactions + 1,
                transactions_24h = transactions_24h + 1,
                updated_at = NOW()
            WHERE id = 1
        "#)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn record_tps_sample(&self, tps: f64) -> GarpResult<()> {
        sqlx::query("UPDATE domain_stats SET avg_tps = $1, updated_at = NOW() WHERE id = 1")
            .bind(tps)
            .execute(&self.pool)
            .await?;
        
        Ok(())
    }
    
    async fn cleanup_old_transactions(&self, older_than: DateTime<Utc>) -> GarpResult<u64> {
        let result = sqlx::query("DELETE FROM sequenced_transactions WHERE sequenced_at < $1")
            .bind(older_than)
            .execute(&self.pool)
            .await?;
        
        Ok(result.rows_affected())
    }
    
    async fn compact_storage(&self) -> GarpResult<()> {
        sqlx::query("VACUUM ANALYZE sequenced_transactions").execute(&self.pool).await?;
        sqlx::query("VACUUM ANALYZE consensus_states").execute(&self.pool).await?;
        sqlx::query("VACUUM ANALYZE domain_participants").execute(&self.pool).await?;
        
        Ok(())
    }
}

/// In-memory storage implementation for testing
pub struct MemoryStorage {
    transactions: Arc<RwLock<HashMap<u64, SequencedTransaction>>>,
    consensus_states: Arc<RwLock<HashMap<TransactionId, ConsensusState>>>,
    participants: Arc<RwLock<HashMap<ParticipantId, DomainParticipant>>>,
    next_sequence: Arc<RwLock<u64>>,
    stats: Arc<RwLock<DomainStats>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            consensus_states: Arc::new(RwLock::new(HashMap::new())),
            participants: Arc::new(RwLock::new(HashMap::new())),
            next_sequence: Arc::new(RwLock::new(1)),
            stats: Arc::new(RwLock::new(DomainStats {
                total_transactions: 0,
                transactions_24h: 0,
                avg_tps: 0.0,
                active_participants: 0,
                pending_consensus: 0,
                current_sequence: 0,
                uptime_seconds: 0,
            })),
        }
    }
}

#[async_trait]
impl StorageBackend for MemoryStorage {
    async fn store_sequenced_transaction(&self, transaction: &SequencedTransaction) -> GarpResult<()> {
        let mut transactions = self.transactions.write().await;
        transactions.insert(transaction.sequence_number, transaction.clone());
        Ok(())
    }
    
    async fn get_sequenced_transaction(&self, sequence_number: u64) -> GarpResult<Option<SequencedTransaction>> {
        let transactions = self.transactions.read().await;
        Ok(transactions.get(&sequence_number).cloned())
    }
    
    async fn get_transactions_by_batch(&self, batch_id: Uuid) -> GarpResult<Vec<SequencedTransaction>> {
        let transactions = self.transactions.read().await;
        let mut result = Vec::new();
        
        for transaction in transactions.values() {
            if transaction.batch_id == Some(batch_id) {
                result.push(transaction.clone());
            }
        }
        
        result.sort_by_key(|t| t.sequence_number);
        Ok(result)
    }
    
    async fn get_transactions_in_range(&self, start: u64, end: u64) -> GarpResult<Vec<SequencedTransaction>> {
        let transactions = self.transactions.read().await;
        let mut result = Vec::new();
        
        for seq in start..=end {
            if let Some(transaction) = transactions.get(&seq) {
                result.push(transaction.clone());
            }
        }
        
        Ok(result)
    }
    
    async fn update_transaction_status(&self, sequence_number: u64, status: SequenceStatus) -> GarpResult<()> {
        let mut transactions = self.transactions.write().await;
        if let Some(transaction) = transactions.get_mut(&sequence_number) {
            transaction.status = status;
        }
        Ok(())
    }
    
    async fn get_next_sequence_number(&self) -> GarpResult<u64> {
        let mut next_seq = self.next_sequence.write().await;
        let current = *next_seq;
        *next_seq += 1;
        Ok(current)
    }
    
    async fn store_consensus_state(&self, state: &ConsensusState) -> GarpResult<()> {
        let mut states = self.consensus_states.write().await;
        states.insert(state.transaction_id.clone(), state.clone());
        Ok(())
    }
    
    async fn get_consensus_state(&self, transaction_id: &TransactionId) -> GarpResult<Option<ConsensusState>> {
        let states = self.consensus_states.read().await;
        Ok(states.get(transaction_id).cloned())
    }
    
    async fn update_consensus_phase(&self, transaction_id: &TransactionId, phase: ConsensusPhase) -> GarpResult<()> {
        let mut states = self.consensus_states.write().await;
        if let Some(state) = states.get_mut(transaction_id) {
            state.phase = phase;
        }
        Ok(())
    }
    
    async fn add_consensus_vote(&self, transaction_id: &TransactionId, vote: &ConsensusVote) -> GarpResult<()> {
        let mut states = self.consensus_states.write().await;
        if let Some(state) = states.get_mut(transaction_id) {
            state.votes.insert(vote.participant_id.clone(), vote.clone());
        }
        Ok(())
    }
    
    async fn set_consensus_result(&self, transaction_id: &TransactionId, result: ConsensusResult) -> GarpResult<()> {
        let mut states = self.consensus_states.write().await;
        if let Some(state) = states.get_mut(transaction_id) {
            state.result = Some(result);
        }
        Ok(())
    }
    
    async fn get_pending_consensus(&self) -> GarpResult<Vec<ConsensusState>> {
        let states = self.consensus_states.read().await;
        let now = Utc::now();
        
        let result: Vec<ConsensusState> = states
            .values()
            .filter(|state| {
                matches!(state.phase, ConsensusPhase::Validation | ConsensusPhase::Commit) 
                && state.timeout_at > now
            })
            .cloned()
            .collect();
        
        Ok(result)
    }
    
    async fn cleanup_expired_consensus(&self) -> GarpResult<u64> {
        let mut states = self.consensus_states.write().await;
        let now = Utc::now();
        let mut count = 0;
        
        for state in states.values_mut() {
            if state.timeout_at <= now && matches!(state.phase, ConsensusPhase::Validation | ConsensusPhase::Commit) {
                state.phase = ConsensusPhase::Aborted;
                state.result = Some(ConsensusResult::Timeout);
                count += 1;
            }
        }
        
        Ok(count)
    }
    
    async fn register_participant(&self, participant: &DomainParticipant) -> GarpResult<()> {
        let mut participants = self.participants.write().await;
        participants.insert(participant.participant_id.clone(), participant.clone());
        Ok(())
    }
    
    async fn get_participant(&self, participant_id: &ParticipantId) -> GarpResult<Option<DomainParticipant>> {
        let participants = self.participants.read().await;
        Ok(participants.get(participant_id).cloned())
    }
    
    async fn update_participant_status(&self, participant_id: &ParticipantId, status: ParticipantStatus) -> GarpResult<()> {
        let mut participants = self.participants.write().await;
        if let Some(participant) = participants.get_mut(participant_id) {
            participant.status = status;
        }
        Ok(())
    }
    
    async fn update_participant_last_seen(&self, participant_id: &ParticipantId) -> GarpResult<()> {
        let mut participants = self.participants.write().await;
        if let Some(participant) = participants.get_mut(participant_id) {
            participant.last_seen = Utc::now();
        }
        Ok(())
    }
    
    async fn list_participants(&self, status: Option<ParticipantStatus>) -> GarpResult<Vec<DomainParticipant>> {
        let participants = self.participants.read().await;
        
        let result: Vec<DomainParticipant> = if let Some(status) = status {
            participants
                .values()
                .filter(|p| p.status == status)
                .cloned()
                .collect()
        } else {
            participants.values().cloned().collect()
        };
        
        Ok(result)
    }
    
    async fn remove_participant(&self, participant_id: &ParticipantId) -> GarpResult<()> {
        let mut participants = self.participants.write().await;
        participants.remove(participant_id);
        Ok(())
    }
    
    async fn get_domain_stats(&self) -> GarpResult<DomainStats> {
        let stats = self.stats.read().await;
        Ok(stats.clone())
    }
    
    async fn increment_transaction_count(&self) -> GarpResult<()> {
        let mut stats = self.stats.write().await;
        stats.total_transactions += 1;
        stats.transactions_24h += 1;
        Ok(())
    }
    
    async fn record_tps_sample(&self, tps: f64) -> GarpResult<()> {
        let mut stats = self.stats.write().await;
        stats.avg_tps = tps;
        Ok(())
    }
    
    async fn cleanup_old_transactions(&self, older_than: DateTime<Utc>) -> GarpResult<u64> {
        let mut transactions = self.transactions.write().await;
        let mut count = 0;
        
        transactions.retain(|_, transaction| {
            if transaction.sequenced_at < older_than {
                count += 1;
                false
            } else {
                true
            }
        });
        
        Ok(count)
    }
    
    async fn compact_storage(&self) -> GarpResult<()> {
        // No-op for memory storage
        Ok(())
    }
}

/// Storage factory
pub struct Storage;

impl Storage {
    /// Create PostgreSQL storage
    pub async fn postgres(config: &DatabaseConfig) -> GarpResult<Box<dyn StorageBackend>> {
        let storage = PostgresStorage::new(config).await?;
        Ok(Box::new(storage))
    }
    
    /// Create in-memory storage
    pub fn memory() -> Box<dyn StorageBackend> {
        Box::new(MemoryStorage::new())
    }
}