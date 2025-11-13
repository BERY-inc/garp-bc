use std::collections::{HashMap, BTreeMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use std::path::PathBuf;
use tokio::sync::{RwLock, Mutex, mpsc, oneshot};
use tokio::time::{interval, timeout};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};
use sqlx::{Pool, Postgres, Row};

use garp_common::{GarpResult, GarpError};
use garp_common::types::{ParticipantId, TransactionId, Block, Transaction};
// Re-export canonical block header from garp_common
pub use garp_common::types::BlockHeader;
// Canonical type aliases to align with garp_common
pub type GlobalBlock = Block;
pub type GlobalTransaction = Transaction;
// Local aliases for IDs and hashes used in this module
pub type NodeId = String;
pub type DomainId = String;
pub type BlockHash = Vec<u8>;

use crate::config::GlobalSyncConfig;
use crate::consensus::FinalityCertificate;

/// Global storage manager for distributed data persistence
pub struct GlobalStorage {
    /// Configuration
    config: Arc<GlobalSyncConfig>,
    
    /// Node ID
    node_id: NodeId,
    
    /// Transaction storage
    transaction_storage: Arc<TransactionStorage>,
    
    /// Block storage
    block_storage: Arc<BlockStorage>,
    
    /// State storage
    state_storage: Arc<StateStorage>,
    
    /// Consensus storage
    consensus_storage: Arc<ConsensusStorage>,
    
    /// Cross-domain storage
    cross_domain_storage: Arc<CrossDomainStorage>,
    
    /// Settlement storage
    settlement_storage: Arc<SettlementStorage>,
    
    /// Metadata storage
    metadata_storage: Arc<MetadataStorage>,
    
    /// Cache manager
    cache_manager: Arc<CacheManager>,
    
    /// Backup manager
    backup_manager: Arc<BackupManager>,
    
    /// Replication manager
    replication_manager: Arc<ReplicationManager>,
    
    /// Storage metrics
    metrics: Arc<StorageMetrics>,
    
    /// Event channels
    event_tx: mpsc::UnboundedSender<StorageEvent>,
    event_rx: Arc<Mutex<mpsc::UnboundedReceiver<StorageEvent>>>,
    
    /// Shutdown signal
    shutdown_tx: Option<oneshot::Sender<()>>,
}

/// Transaction storage
pub struct TransactionStorage {
    /// Configuration
    config: Arc<GlobalSyncConfig>,
    
    /// Active transactions
    active_transactions: Arc<RwLock<HashMap<TransactionId, StoredTransaction>>>,
    
    /// Transaction history
    transaction_history: Arc<RwLock<BTreeMap<u64, Vec<TransactionId>>>>,
    
    /// Transaction index
    transaction_index: Arc<RwLock<HashMap<String, HashSet<TransactionId>>>>,
    
    /// Pending transactions
    pending_transactions: Arc<RwLock<VecDeque<TransactionId>>>,
    
    /// Transaction pool
    transaction_pool: Arc<RwLock<TransactionPool>>,
    
    /// Storage backend
    backend: Arc<dyn StorageBackend>,
    
    /// Metrics
    metrics: Arc<TransactionStorageMetrics>,
}

/// Stored transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTransaction {
    /// Transaction ID
    pub transaction_id: TransactionId,
    
    /// Transaction data
    pub transaction_data: Vec<u8>,
    
    /// Transaction type
    pub transaction_type: String,
    
    /// Source domain
    pub source_domain: DomainId,
    
    /// Target domains
    pub target_domains: Vec<DomainId>,
    
    /// Transaction status
    pub status: TransactionStatus,
    
    /// Consensus state
    pub consensus_state: ConsensusState,
    
    /// Settlement state
    pub settlement_state: SettlementState,
    
    /// Created timestamp
    pub created_at: SystemTime,
    
    /// Updated timestamp
    pub updated_at: SystemTime,
    
    /// Block height
    pub block_height: Option<u64>,
    
    /// Block hash
    pub block_hash: Option<BlockHash>,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
    
    /// Dependencies
    pub dependencies: Vec<TransactionId>,
    
    /// Dependents
    pub dependents: Vec<TransactionId>,
}

/// Transaction status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransactionStatus {
    /// Pending validation
    Pending,
    
    /// Validated
    Validated,
    
    /// In consensus
    InConsensus,
    
    /// Consensus reached
    ConsensusReached,
    
    /// In settlement
    InSettlement,
    
    /// Settled
    Settled,
    
    /// Failed
    Failed,
    
    /// Rejected
    Rejected,
    
    /// Rolled back
    RolledBack,
}

/// Consensus state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusState {
    /// Current phase
    pub phase: String,
    
    /// Votes received
    pub votes: HashMap<NodeId, bool>,
    
    /// Required votes
    pub required_votes: u32,
    
    /// Consensus result
    pub result: Option<bool>,
    
    /// Consensus proof
    pub proof: Option<Vec<u8>>,
    
    /// Started timestamp
    pub started_at: SystemTime,
    
    /// Completed timestamp
    pub completed_at: Option<SystemTime>,
}

/// Settlement state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementState {
    /// Settlement ID
    pub settlement_id: Option<String>,
    
    /// Settlement type
    pub settlement_type: String,
    
    /// Domain settlements
    pub domain_settlements: HashMap<DomainId, DomainSettlementState>,
    
    /// Settlement proof
    pub proof: Option<Vec<u8>>,
    
    /// Started timestamp
    pub started_at: Option<SystemTime>,
    
    /// Completed timestamp
    pub completed_at: Option<SystemTime>,
}

/// Domain settlement state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainSettlementState {
    /// Domain ID
    pub domain_id: DomainId,
    
    /// Settlement status
    pub status: String,
    
    /// Settlement data
    pub data: Vec<u8>,
    
    /// Settlement proof
    pub proof: Option<Vec<u8>>,
    
    /// Timestamp
    pub timestamp: SystemTime,
}

/// Transaction pool
#[derive(Debug, Clone)]
pub struct TransactionPool {
    /// Pool transactions
    pub transactions: HashMap<TransactionId, PoolTransaction>,
    
    /// Priority queue
    pub priority_queue: BTreeMap<u64, Vec<TransactionId>>,
    
    /// Pool size
    pub size: usize,
    
    /// Max pool size
    pub max_size: usize,
    
    /// Pool statistics
    pub stats: PoolStats,
}

/// Pool transaction
#[derive(Debug, Clone)]
pub struct PoolTransaction {
    /// Transaction ID
    pub transaction_id: TransactionId,
    
    /// Priority score
    pub priority: u64,
    
    /// Added timestamp
    pub added_at: Instant,
    
    /// Retry count
    pub retry_count: u32,
    
    /// Dependencies
    pub dependencies: Vec<TransactionId>,
}

/// Pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Total transactions added
    pub total_added: u64,
    
    /// Total transactions removed
    pub total_removed: u64,
    
    /// Average pool size
    pub avg_pool_size: f64,
    
    /// Average wait time
    pub avg_wait_time: Duration,
    
    /// Pool utilization
    pub utilization: f64,
}

/// Transaction storage metrics
#[derive(Debug, Clone)]
pub struct TransactionStorageMetrics {
    /// Total transactions stored
    pub total_transactions: Arc<RwLock<u64>>,
    
    /// Active transactions
    pub active_transactions: Arc<RwLock<usize>>,
    
    /// Storage operations per second
    pub storage_ops_per_sec: Arc<RwLock<f64>>,
    
    /// Average storage time
    pub avg_storage_time: Arc<RwLock<f64>>,
    
    /// Storage errors
    pub storage_errors: Arc<RwLock<u64>>,
}

/// Block storage
pub struct BlockStorage {
    /// Configuration
    config: Arc<GlobalSyncConfig>,
    
    /// Block chain
    blockchain: Arc<RwLock<BlockChain>>,
    
    /// Block index
    block_index: Arc<RwLock<HashMap<BlockHash, BlockInfo>>>,
    
    /// Height index
    height_index: Arc<RwLock<BTreeMap<u64, BlockHash>>>,
    
    /// Pending blocks
    pending_blocks: Arc<RwLock<HashMap<BlockHash, PendingBlock>>>,
    
    /// Storage backend
    backend: Arc<dyn StorageBackend>,
    
    /// Metrics
    metrics: Arc<BlockStorageMetrics>,
}

/// Block chain
#[derive(Debug, Clone)]
pub struct BlockChain {
    /// Genesis block
    pub genesis_block: BlockHash,
    
    /// Current head
    pub head: BlockHash,
    
    /// Current height
    pub height: u64,
    
    /// Total difficulty
    pub total_difficulty: u64,
    
    /// Chain statistics
    pub stats: ChainStats,
}

/// Chain statistics
#[derive(Debug, Clone)]
pub struct ChainStats {
    /// Total blocks
    pub total_blocks: u64,
    
    /// Total transactions
    pub total_transactions: u64,
    
    /// Average block time
    pub avg_block_time: Duration,
    
    /// Average block size
    pub avg_block_size: usize,
    
    /// Chain size
    pub chain_size: u64,
}

/// Block information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockInfo {
    /// Block hash
    pub block_hash: BlockHash,
    
    /// Block height
    pub height: u64,
    
    /// Parent hash
    pub parent_hash: BlockHash,
    
    /// Transaction count
    pub transaction_count: u32,
    
    /// Block size
    pub size: usize,
    
    /// Timestamp
    pub timestamp: SystemTime,
    
    /// Difficulty
    pub difficulty: u64,
    
    /// Nonce
    pub nonce: u64,
    
    /// Merkle root
    pub merkle_root: Vec<u8>,
    
    /// State root
    pub state_root: Vec<u8>,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Pending block
#[derive(Debug, Clone)]
pub struct PendingBlock {
    /// Block hash
    pub block_hash: BlockHash,
    
    /// Block data
    pub block_data: Vec<u8>,
    
    /// Validation status
    pub validation_status: ValidationStatus,
    
    /// Consensus status
    pub consensus_status: ConsensusStatus,
    
    /// Added timestamp
    pub added_at: Instant,
    
    /// Dependencies
    pub dependencies: Vec<BlockHash>,
}

/// Validation status
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationStatus {
    /// Pending validation
    Pending,
    
    /// Validating
    Validating,
    
    /// Valid
    Valid,
    
    /// Invalid
    Invalid,
    
    /// Failed
    Failed,
}

/// Consensus status
#[derive(Debug, Clone, PartialEq)]
pub enum ConsensusStatus {
    /// Pending consensus
    Pending,
    
    /// In consensus
    InConsensus,
    
    /// Consensus reached
    ConsensusReached,
    
    /// Consensus failed
    ConsensusFailed,
}

/// Block storage metrics
#[derive(Debug, Clone)]
pub struct BlockStorageMetrics {
    /// Total blocks stored
    pub total_blocks: Arc<RwLock<u64>>,
    
    /// Current height
    pub current_height: Arc<RwLock<u64>>,
    
    /// Block storage rate
    pub block_storage_rate: Arc<RwLock<f64>>,
    
    /// Average block size
    pub avg_block_size: Arc<RwLock<f64>>,
    
    /// Storage errors
    pub storage_errors: Arc<RwLock<u64>>,
}

/// State storage
pub struct StateStorage {
    /// Configuration
    config: Arc<GlobalSyncConfig>,
    
    /// Global state
    global_state: Arc<RwLock<GlobalState>>,
    
    /// Domain states
    domain_states: Arc<RwLock<HashMap<DomainId, DomainState>>>,
    
    /// State snapshots
    state_snapshots: Arc<RwLock<HashMap<u64, StateSnapshot>>>,
    
    /// State transitions
    state_transitions: Arc<RwLock<VecDeque<StateTransition>>>,
    
    /// Storage backend
    backend: Arc<dyn StorageBackend>,
    
    /// Metrics
    metrics: Arc<StateStorageMetrics>,
}

/// Global state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalState {
    /// State version
    pub version: u64,
    
    /// State root hash
    pub root_hash: Vec<u8>,
    
    /// Domain states
    pub domain_states: HashMap<DomainId, Vec<u8>>,
    
    /// Global variables
    pub global_variables: HashMap<String, Vec<u8>>,
    
    /// Validator set
    pub validator_set: HashMap<NodeId, ValidatorInfo>,
    
    /// Configuration
    pub configuration: HashMap<String, String>,
    
    /// Last updated
    pub last_updated: SystemTime,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Domain state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainState {
    /// Domain ID
    pub domain_id: DomainId,
    
    /// State version
    pub version: u64,
    
    /// State data
    pub state_data: Vec<u8>,
    
    /// State hash
    pub state_hash: Vec<u8>,
    
    /// Last block height
    pub last_block_height: u64,
    
    /// Last block hash
    pub last_block_hash: BlockHash,
    
    /// Pending transactions
    pub pending_transactions: Vec<TransactionId>,
    
    /// Last updated
    pub last_updated: SystemTime,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Validator information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorInfo {
    /// Validator ID
    pub validator_id: NodeId,
    
    /// Public key
    pub public_key: Vec<u8>,
    
    /// Stake amount
    pub stake: u64,
    
    /// Voting power
    pub voting_power: u64,
    
    /// Status
    pub status: ValidatorStatus,
    
    /// Performance metrics
    pub performance: ValidatorPerformance,
    
    /// Joined timestamp
    pub joined_at: SystemTime,
    
    /// Last activity
    pub last_activity: SystemTime,
}

/// Validator status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidatorStatus {
    /// Active
    Active,
    
    /// Inactive
    Inactive,
    
    /// Jailed
    Jailed,
    
    /// Slashed
    Slashed,
    
    /// Leaving
    Leaving,
}

/// Validator performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorPerformance {
    /// Blocks proposed
    pub blocks_proposed: u64,
    
    /// Blocks validated
    pub blocks_validated: u64,
    
    /// Votes cast
    pub votes_cast: u64,
    
    /// Missed votes
    pub missed_votes: u64,
    
    /// Uptime percentage
    pub uptime_percentage: f64,
    
    /// Performance score
    pub performance_score: f64,
}

/// State snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Snapshot ID
    pub snapshot_id: String,
    
    /// Block height
    pub block_height: u64,
    
    /// Block hash
    pub block_hash: BlockHash,
    
    /// State root
    pub state_root: Vec<u8>,
    
    /// Snapshot data
    pub snapshot_data: Vec<u8>,
    
    /// Created timestamp
    pub created_at: SystemTime,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// State transition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    /// Transition ID
    pub transition_id: String,
    
    /// From state
    pub from_state: Vec<u8>,
    
    /// To state
    pub to_state: Vec<u8>,
    
    /// Transaction ID
    pub transaction_id: TransactionId,
    
    /// Block height
    pub block_height: u64,
    
    /// Timestamp
    pub timestamp: SystemTime,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// State storage metrics
#[derive(Debug, Clone)]
pub struct StateStorageMetrics {
    /// State updates
    pub state_updates: Arc<RwLock<u64>>,
    
    /// Snapshots created
    pub snapshots_created: Arc<RwLock<u64>>,
    
    /// State size
    pub state_size: Arc<RwLock<u64>>,
    
    /// Average update time
    pub avg_update_time: Arc<RwLock<f64>>,
    
    /// Storage errors
    pub storage_errors: Arc<RwLock<u64>>,
}

/// Consensus storage
pub struct ConsensusStorage {
    /// Configuration
    config: Arc<GlobalSyncConfig>,
    
    /// Consensus sessions
    consensus_sessions: Arc<RwLock<HashMap<String, ConsensusSession>>>,
    
    /// Consensus history
    consensus_history: Arc<RwLock<VecDeque<ConsensusRecord>>>,
    
    /// Vote records
    vote_records: Arc<RwLock<HashMap<String, VoteRecord>>>,
    
    /// View changes
    view_changes: Arc<RwLock<HashMap<u64, ViewChangeRecord>>>,
    
    /// Finality certificates indexed by block hash (string)
    finality_by_hash: Arc<RwLock<HashMap<String, FinalityCertificate>>>,
    
    /// Finality certificates indexed by block height
    finality_by_height: Arc<RwLock<BTreeMap<u64, FinalityCertificate>>>,
    
    /// Storage backend
    backend: Arc<dyn StorageBackend>,
    
    /// Metrics
    metrics: Arc<ConsensusStorageMetrics>,
}

/// Consensus session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusSession {
    /// Session ID
    pub session_id: String,
    
    /// Proposal ID
    pub proposal_id: String,
    
    /// Proposal data
    pub proposal_data: Vec<u8>,
    
    /// Current view
    pub current_view: u64,
    
    /// Current phase
    pub current_phase: String,
    
    /// Votes
    pub votes: HashMap<NodeId, Vote>,
    
    /// Required votes
    pub required_votes: u32,
    
    /// Session status
    pub status: SessionStatus,
    
    /// Started timestamp
    pub started_at: SystemTime,
    
    /// Completed timestamp
    pub completed_at: Option<SystemTime>,
    
    /// Result
    pub result: Option<ConsensusResult>,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Vote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    /// Voter ID
    pub voter_id: NodeId,
    
    /// Vote value
    pub vote: bool,
    
    /// Vote signature
    pub signature: Vec<u8>,
    
    /// Timestamp
    pub timestamp: SystemTime,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Session status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionStatus {
    /// Active
    Active,
    
    /// Completed
    Completed,
    
    /// Failed
    Failed,
    
    /// Timeout
    Timeout,
    
    /// Aborted
    Aborted,
}

/// Consensus result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusResult {
    /// Result
    pub result: bool,
    
    /// Proof
    pub proof: Vec<u8>,
    
    /// Vote count
    pub vote_count: u32,
    
    /// Timestamp
    pub timestamp: SystemTime,
}

/// Consensus record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusRecord {
    /// Record ID
    pub record_id: String,
    
    /// Session ID
    pub session_id: String,
    
    /// Proposal ID
    pub proposal_id: String,
    
    /// Result
    pub result: ConsensusResult,
    
    /// Participants
    pub participants: Vec<NodeId>,
    
    /// Duration
    pub duration: Duration,
    
    /// Timestamp
    pub timestamp: SystemTime,
}

/// Vote record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteRecord {
    /// Record ID
    pub record_id: String,
    
    /// Session ID
    pub session_id: String,
    
    /// Voter ID
    pub voter_id: NodeId,
    
    /// Vote
    pub vote: Vote,
    
    /// Timestamp
    pub timestamp: SystemTime,
}

/// View change record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewChangeRecord {
    /// View number
    pub view: u64,
    
    /// Previous view
    pub previous_view: u64,
    
    /// Reason
    pub reason: String,
    
    /// Initiator
    pub initiator: NodeId,
    
    /// Participants
    pub participants: Vec<NodeId>,
    
    /// Timestamp
    pub timestamp: SystemTime,
}

/// Consensus storage metrics
#[derive(Debug, Clone)]
pub struct ConsensusStorageMetrics {
    /// Total sessions
    pub total_sessions: Arc<RwLock<u64>>,
    
    /// Active sessions
    pub active_sessions: Arc<RwLock<usize>>,
    
    /// Successful consensus
    pub successful_consensus: Arc<RwLock<u64>>,
    
    /// Failed consensus
    pub failed_consensus: Arc<RwLock<u64>>,
    
    /// Average consensus time
    pub avg_consensus_time: Arc<RwLock<f64>>,
}

/// Cross-domain storage
pub struct CrossDomainStorage {
    /// Configuration
    config: Arc<GlobalSyncConfig>,
    
    /// Cross-domain transactions
    cross_domain_transactions: Arc<RwLock<HashMap<TransactionId, CrossDomainTransaction>>>,
    
    /// Domain coordination
    domain_coordination: Arc<RwLock<HashMap<String, CoordinationSession>>>,
    
    /// State synchronization
    state_synchronization: Arc<RwLock<HashMap<String, SyncSession>>>,
    
    /// Storage backend
    backend: Arc<dyn StorageBackend>,
    
    /// Metrics
    metrics: Arc<CrossDomainStorageMetrics>,
}

/// Cross-domain transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossDomainTransaction {
    /// Transaction ID
    pub transaction_id: TransactionId,
    
    /// Source domain
    pub source_domain: DomainId,
    
    /// Target domains
    pub target_domains: Vec<DomainId>,
    
    /// Transaction type
    pub transaction_type: String,
    
    /// Transaction data
    pub transaction_data: Vec<u8>,
    
    /// Coordination status
    pub coordination_status: CoordinationStatus,
    
    /// Domain confirmations
    pub domain_confirmations: HashMap<DomainId, DomainConfirmation>,
    
    /// Created timestamp
    pub created_at: SystemTime,
    
    /// Updated timestamp
    pub updated_at: SystemTime,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Coordination status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CoordinationStatus {
    /// Initiated
    Initiated,
    
    /// Coordinating
    Coordinating,
    
    /// Coordinated
    Coordinated,
    
    /// Failed
    Failed,
    
    /// Aborted
    Aborted,
}

/// Domain confirmation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainConfirmation {
    /// Domain ID
    pub domain_id: DomainId,
    
    /// Confirmation status
    pub status: ConfirmationStatus,
    
    /// Confirmation data
    pub data: Vec<u8>,
    
    /// Signature
    pub signature: Vec<u8>,
    
    /// Timestamp
    pub timestamp: SystemTime,
}

/// Confirmation status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConfirmationStatus {
    /// Pending
    Pending,
    
    /// Confirmed
    Confirmed,
    
    /// Rejected
    Rejected,
    
    /// Failed
    Failed,
}

/// Coordination session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationSession {
    /// Session ID
    pub session_id: String,
    
    /// Transaction ID
    pub transaction_id: TransactionId,
    
    /// Participating domains
    pub participating_domains: Vec<DomainId>,
    
    /// Session status
    pub status: SessionStatus,
    
    /// Coordination data
    pub coordination_data: HashMap<DomainId, Vec<u8>>,
    
    /// Started timestamp
    pub started_at: SystemTime,
    
    /// Completed timestamp
    pub completed_at: Option<SystemTime>,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Sync session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSession {
    /// Session ID
    pub session_id: String,
    
    /// Source domain
    pub source_domain: DomainId,
    
    /// Target domain
    pub target_domain: DomainId,
    
    /// Sync type
    pub sync_type: String,
    
    /// Sync data
    pub sync_data: Vec<u8>,
    
    /// Sync status
    pub status: SyncStatus,
    
    /// Started timestamp
    pub started_at: SystemTime,
    
    /// Completed timestamp
    pub completed_at: Option<SystemTime>,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Sync status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SyncStatus {
    /// Initiated
    Initiated,
    
    /// Syncing
    Syncing,
    
    /// Completed
    Completed,
    
    /// Failed
    Failed,
    
    /// Aborted
    Aborted,
}

/// Cross-domain storage metrics
#[derive(Debug, Clone)]
pub struct CrossDomainStorageMetrics {
    /// Cross-domain transactions
    pub cross_domain_transactions: Arc<RwLock<u64>>,
    
    /// Active coordination sessions
    pub active_coordination_sessions: Arc<RwLock<usize>>,
    
    /// Successful coordinations
    pub successful_coordinations: Arc<RwLock<u64>>,
    
    /// Failed coordinations
    pub failed_coordinations: Arc<RwLock<u64>>,
    
    /// Average coordination time
    pub avg_coordination_time: Arc<RwLock<f64>>,
}

/// Settlement storage
pub struct SettlementStorage {
    /// Configuration
    config: Arc<GlobalSyncConfig>,
    
    /// Settlements
    settlements: Arc<RwLock<HashMap<String, Settlement>>>,
    
    /// Settlement batches
    settlement_batches: Arc<RwLock<HashMap<String, SettlementBatch>>>,
    
    /// Rollback records
    rollback_records: Arc<RwLock<HashMap<String, RollbackRecord>>>,
    
    /// Storage backend
    backend: Arc<dyn StorageBackend>,
    
    /// Metrics
    metrics: Arc<SettlementStorageMetrics>,
}

/// Settlement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settlement {
    /// Settlement ID
    pub settlement_id: String,
    
    /// Transaction ID
    pub transaction_id: TransactionId,
    
    /// Settlement type
    pub settlement_type: String,
    
    /// Settlement data
    pub settlement_data: Vec<u8>,
    
    /// Settlement status
    pub status: SettlementStatus,
    
    /// Domain settlements
    pub domain_settlements: HashMap<DomainId, DomainSettlement>,
    
    /// Settlement proof
    pub proof: Option<Vec<u8>>,
    
    /// Created timestamp
    pub created_at: SystemTime,
    
    /// Completed timestamp
    pub completed_at: Option<SystemTime>,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Settlement status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SettlementStatus {
    /// Pending
    Pending,
    
    /// Processing
    Processing,
    
    /// Completed
    Completed,
    
    /// Failed
    Failed,
    
    /// Rolled back
    RolledBack,
}

/// Domain settlement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainSettlement {
    /// Domain ID
    pub domain_id: DomainId,
    
    /// Settlement data
    pub settlement_data: Vec<u8>,
    
    /// Settlement status
    pub status: SettlementStatus,
    
    /// Settlement proof
    pub proof: Option<Vec<u8>>,
    
    /// Timestamp
    pub timestamp: SystemTime,
}

/// Settlement batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementBatch {
    /// Batch ID
    pub batch_id: String,
    
    /// Settlement IDs
    pub settlement_ids: Vec<String>,
    
    /// Batch status
    pub status: BatchStatus,
    
    /// Created timestamp
    pub created_at: SystemTime,
    
    /// Processed timestamp
    pub processed_at: Option<SystemTime>,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Batch status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BatchStatus {
    /// Pending
    Pending,
    
    /// Processing
    Processing,
    
    /// Completed
    Completed,
    
    /// Failed
    Failed,
}

/// Rollback record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackRecord {
    /// Rollback ID
    pub rollback_id: String,
    
    /// Settlement ID
    pub settlement_id: String,
    
    /// Rollback reason
    pub reason: String,
    
    /// Rollback data
    pub rollback_data: Vec<u8>,
    
    /// Rollback status
    pub status: RollbackStatus,
    
    /// Created timestamp
    pub created_at: SystemTime,
    
    /// Completed timestamp
    pub completed_at: Option<SystemTime>,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Rollback status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RollbackStatus {
    /// Pending
    Pending,
    
    /// Processing
    Processing,
    
    /// Completed
    Completed,
    
    /// Failed
    Failed,
}

/// Settlement storage metrics
#[derive(Debug, Clone)]
pub struct SettlementStorageMetrics {
    /// Total settlements
    pub total_settlements: Arc<RwLock<u64>>,
    
    /// Successful settlements
    pub successful_settlements: Arc<RwLock<u64>>,
    
    /// Failed settlements
    pub failed_settlements: Arc<RwLock<u64>>,
    
    /// Rollbacks
    pub rollbacks: Arc<RwLock<u64>>,
    
    /// Average settlement time
    pub avg_settlement_time: Arc<RwLock<f64>>,
}

/// Metadata storage
pub struct MetadataStorage {
    /// Configuration
    config: Arc<GlobalSyncConfig>,
    
    /// Node metadata
    node_metadata: Arc<RwLock<HashMap<NodeId, NodeMetadata>>>,
    
    /// Domain metadata
    domain_metadata: Arc<RwLock<HashMap<DomainId, DomainMetadata>>>,
    
    /// System metadata
    system_metadata: Arc<RwLock<HashMap<String, SystemMetadata>>>,
    
    /// Storage backend
    backend: Arc<dyn StorageBackend>,
    
    /// Metrics
    metrics: Arc<MetadataStorageMetrics>,
}

/// Node metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    /// Node ID
    pub node_id: NodeId,
    
    /// Node type
    pub node_type: String,
    
    /// Node capabilities
    pub capabilities: HashMap<String, String>,
    
    /// Node configuration
    pub configuration: HashMap<String, String>,
    
    /// Node statistics
    pub statistics: HashMap<String, f64>,
    
    /// Last updated
    pub last_updated: SystemTime,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Domain metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainMetadata {
    /// Domain ID
    pub domain_id: DomainId,
    
    /// Domain name
    pub domain_name: String,
    
    /// Domain type
    pub domain_type: String,
    
    /// Domain configuration
    pub configuration: HashMap<String, String>,
    
    /// Domain statistics
    pub statistics: HashMap<String, f64>,
    
    /// Last updated
    pub last_updated: SystemTime,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// System metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetadata {
    /// Metadata key
    pub key: String,
    
    /// Metadata value
    pub value: String,
    
    /// Metadata type
    pub metadata_type: String,
    
    /// Created timestamp
    pub created_at: SystemTime,
    
    /// Updated timestamp
    pub updated_at: SystemTime,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Metadata storage metrics
#[derive(Debug, Clone)]
pub struct MetadataStorageMetrics {
    /// Total metadata entries
    pub total_entries: Arc<RwLock<u64>>,
    
    /// Metadata updates
    pub metadata_updates: Arc<RwLock<u64>>,
    
    /// Storage size
    pub storage_size: Arc<RwLock<u64>>,
    
    /// Average access time
    pub avg_access_time: Arc<RwLock<f64>>,
    
    /// Storage errors
    pub storage_errors: Arc<RwLock<u64>>,
}

/// Cache manager
pub struct CacheManager {
    /// Configuration
    config: Arc<GlobalSyncConfig>,
    
    /// Cache layers
    cache_layers: Arc<RwLock<HashMap<String, CacheLayer>>>,
    
    /// Cache policies
    cache_policies: Arc<RwLock<HashMap<String, CachePolicy>>>,
    
    /// Cache statistics
    cache_stats: Arc<RwLock<CacheStats>>,
    
    /// Metrics
    metrics: Arc<CacheMetrics>,
}

/// Cache layer
#[derive(Debug, Clone)]
pub struct CacheLayer {
    /// Layer name
    pub name: String,
    
    /// Cache entries
    pub entries: HashMap<String, CacheEntry>,
    
    /// Layer configuration
    pub config: CacheLayerConfig,
    
    /// Layer statistics
    pub stats: LayerStats,
}

/// Cache entry
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Entry key
    pub key: String,
    
    /// Entry value
    pub value: Vec<u8>,
    
    /// Entry metadata
    pub metadata: HashMap<String, String>,
    
    /// Created timestamp
    pub created_at: Instant,
    
    /// Last accessed
    pub last_accessed: Instant,
    
    /// Access count
    pub access_count: u64,
    
    /// TTL
    pub ttl: Option<Duration>,
}

/// Cache layer configuration
#[derive(Debug, Clone)]
pub struct CacheLayerConfig {
    /// Max entries
    pub max_entries: usize,
    
    /// Max size
    pub max_size: usize,
    
    /// Default TTL
    pub default_ttl: Option<Duration>,
    
    /// Eviction policy
    pub eviction_policy: EvictionPolicy,
}

/// Eviction policy
#[derive(Debug, Clone)]
pub enum EvictionPolicy {
    /// Least Recently Used
    LRU,
    
    /// Least Frequently Used
    LFU,
    
    /// First In First Out
    FIFO,
    
    /// Time To Live
    TTL,
    
    /// Random
    Random,
}

/// Layer statistics
#[derive(Debug, Clone)]
pub struct LayerStats {
    /// Cache hits
    pub hits: u64,
    
    /// Cache misses
    pub misses: u64,
    
    /// Evictions
    pub evictions: u64,
    
    /// Current size
    pub current_size: usize,
    
    /// Current entries
    pub current_entries: usize,
}

/// Cache policy
#[derive(Debug, Clone)]
pub struct CachePolicy {
    /// Policy name
    pub name: String,
    
    /// Policy rules
    pub rules: Vec<CacheRule>,
    
    /// Priority
    pub priority: u32,
    
    /// Enabled
    pub enabled: bool,
}

/// Cache rule
#[derive(Debug, Clone)]
pub struct CacheRule {
    /// Rule condition
    pub condition: CacheCondition,
    
    /// Rule action
    pub action: CacheAction,
    
    /// Parameters
    pub parameters: HashMap<String, String>,
}

/// Cache condition
#[derive(Debug, Clone)]
pub enum CacheCondition {
    /// Key matches pattern
    KeyMatches(String),
    
    /// Size exceeds limit
    SizeExceeds(usize),
    
    /// Age exceeds limit
    AgeExceeds(Duration),
    
    /// Access count below threshold
    AccessCountBelow(u64),
    
    /// Custom condition
    Custom(String),
}

/// Cache action
#[derive(Debug, Clone)]
pub enum CacheAction {
    /// Cache entry
    Cache,
    
    /// Don't cache entry
    NoCache,
    
    /// Set TTL
    SetTTL(Duration),
    
    /// Evict entry
    Evict,
    
    /// Refresh entry
    Refresh,
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Total hits
    pub total_hits: u64,
    
    /// Total misses
    pub total_misses: u64,
    
    /// Hit rate
    pub hit_rate: f64,
    
    /// Total evictions
    pub total_evictions: u64,
    
    /// Total size
    pub total_size: usize,
    
    /// Total entries
    pub total_entries: usize,
}

/// Cache metrics
#[derive(Debug, Clone)]
pub struct CacheMetrics {
    /// Cache hits
    pub cache_hits: Arc<RwLock<u64>>,
    
    /// Cache misses
    pub cache_misses: Arc<RwLock<u64>>,
    
    /// Hit rate
    pub hit_rate: Arc<RwLock<f64>>,
    
    /// Cache size
    pub cache_size: Arc<RwLock<usize>>,
    
    /// Evictions
    pub evictions: Arc<RwLock<u64>>,
}

/// Backup manager
pub struct BackupManager {
    /// Configuration
    config: Arc<GlobalSyncConfig>,
    
    /// Backup schedules
    backup_schedules: Arc<RwLock<HashMap<String, BackupSchedule>>>,
    
    /// Backup history
    backup_history: Arc<RwLock<VecDeque<BackupRecord>>>,
    
    /// Active backups
    active_backups: Arc<RwLock<HashMap<String, ActiveBackup>>>,
    
    /// Metrics
    metrics: Arc<BackupMetrics>,
}

/// Backup schedule
#[derive(Debug, Clone)]
pub struct BackupSchedule {
    /// Schedule ID
    pub schedule_id: String,
    
    /// Schedule name
    pub name: String,
    
    /// Backup type
    pub backup_type: BackupType,
    
    /// Schedule pattern
    pub schedule: String,
    
    /// Backup configuration
    pub config: BackupConfig,
    
    /// Enabled
    pub enabled: bool,
    
    /// Last run
    pub last_run: Option<SystemTime>,
    
    /// Next run
    pub next_run: Option<SystemTime>,
}

/// Backup type
#[derive(Debug, Clone)]
pub enum BackupType {
    /// Full backup
    Full,
    
    /// Incremental backup
    Incremental,
    
    /// Differential backup
    Differential,
    
    /// Snapshot backup
    Snapshot,
}

/// Backup configuration
#[derive(Debug, Clone)]
pub struct BackupConfig {
    /// Backup destination
    pub destination: String,
    
    /// Compression enabled
    pub compression: bool,
    
    /// Encryption enabled
    pub encryption: bool,
    
    /// Retention policy
    pub retention: RetentionPolicy,
    
    /// Backup filters
    pub filters: Vec<BackupFilter>,
}

/// Retention policy
#[derive(Debug, Clone)]
pub struct RetentionPolicy {
    /// Keep daily backups for days
    pub daily_retention: u32,
    
    /// Keep weekly backups for weeks
    pub weekly_retention: u32,
    
    /// Keep monthly backups for months
    pub monthly_retention: u32,
    
    /// Keep yearly backups for years
    pub yearly_retention: u32,
}

/// Backup filter
#[derive(Debug, Clone)]
pub struct BackupFilter {
    /// Filter type
    pub filter_type: FilterType,
    
    /// Filter pattern
    pub pattern: String,
    
    /// Include or exclude
    pub include: bool,
}

/// Filter type
#[derive(Debug, Clone)]
pub enum FilterType {
    /// Path filter
    Path,
    
    /// File extension filter
    Extension,
    
    /// Size filter
    Size,
    
    /// Date filter
    Date,
    
    /// Custom filter
    Custom,
}

/// Backup record
#[derive(Debug, Clone)]
pub struct BackupRecord {
    /// Backup ID
    pub backup_id: String,
    
    /// Schedule ID
    pub schedule_id: String,
    
    /// Backup type
    pub backup_type: BackupType,
    
    /// Backup status
    pub status: BackupStatus,
    
    /// Backup size
    pub size: u64,
    
    /// Started timestamp
    pub started_at: SystemTime,
    
    /// Completed timestamp
    pub completed_at: Option<SystemTime>,
    
    /// Error message
    pub error_message: Option<String>,
    
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Backup status
#[derive(Debug, Clone, PartialEq)]
pub enum BackupStatus {
    /// Pending
    Pending,
    
    /// Running
    Running,
    
    /// Completed
    Completed,
    
    /// Failed
    Failed,
    
    /// Cancelled
    Cancelled,
}

/// Active backup
#[derive(Debug, Clone)]
pub struct ActiveBackup {
    /// Backup ID
    pub backup_id: String,
    
    /// Progress percentage
    pub progress: f64,
    
    /// Current operation
    pub current_operation: String,
    
    /// Bytes processed
    pub bytes_processed: u64,
    
    /// Total bytes
    pub total_bytes: u64,
    
    /// Started timestamp
    pub started_at: Instant,
}

/// Backup metrics
#[derive(Debug, Clone)]
pub struct BackupMetrics {
    /// Total backups
    pub total_backups: Arc<RwLock<u64>>,
    
    /// Successful backups
    pub successful_backups: Arc<RwLock<u64>>,
    
    /// Failed backups
    pub failed_backups: Arc<RwLock<u64>>,
    
    /// Total backup size
    pub total_backup_size: Arc<RwLock<u64>>,
    
    /// Average backup time
    pub avg_backup_time: Arc<RwLock<f64>>,
}

/// Replication manager
pub struct ReplicationManager {
    /// Configuration
    config: Arc<GlobalSyncConfig>,
    
    /// Replication targets
    replication_targets: Arc<RwLock<HashMap<String, ReplicationTarget>>>,
    
    /// Replication status
    replication_status: Arc<RwLock<HashMap<String, ReplicationStatus>>>,
    
    /// Replication queue
    replication_queue: Arc<RwLock<VecDeque<ReplicationTask>>>,
    
    /// Metrics
    metrics: Arc<ReplicationMetrics>,
}

/// Replication target
#[derive(Debug, Clone)]
pub struct ReplicationTarget {
    /// Target ID
    pub target_id: String,
    
    /// Target address
    pub address: String,
    
    /// Target type
    pub target_type: ReplicationTargetType,
    
    /// Replication mode
    pub mode: ReplicationMode,
    
    /// Configuration
    pub config: ReplicationConfig,
    
    /// Status
    pub status: TargetStatus,
    
    /// Last sync
    pub last_sync: Option<SystemTime>,
}

/// Replication target type
#[derive(Debug, Clone)]
pub enum ReplicationTargetType {
    /// Primary replica
    Primary,
    
    /// Secondary replica
    Secondary,
    
    /// Backup replica
    Backup,
    
    /// Archive replica
    Archive,
}

/// Replication mode
#[derive(Debug, Clone)]
pub enum ReplicationMode {
    /// Synchronous replication
    Synchronous,
    
    /// Asynchronous replication
    Asynchronous,
    
    /// Semi-synchronous replication
    SemiSynchronous,
}

/// Replication configuration
#[derive(Debug, Clone)]
pub struct ReplicationConfig {
    /// Batch size
    pub batch_size: usize,
    
    /// Sync interval
    pub sync_interval: Duration,
    
    /// Retry attempts
    pub retry_attempts: u32,
    
    /// Timeout
    pub timeout: Duration,
    
    /// Compression enabled
    pub compression: bool,
    
    /// Encryption enabled
    pub encryption: bool,
}

/// Target status
#[derive(Debug, Clone, PartialEq)]
pub enum TargetStatus {
    /// Online
    Online,
    
    /// Offline
    Offline,
    
    /// Syncing
    Syncing,
    
    /// Error
    Error,
    
    /// Disabled
    Disabled,
}

/// Replication status
#[derive(Debug, Clone)]
pub struct ReplicationStatus {
    /// Target ID
    pub target_id: String,
    
    /// Last sync timestamp
    pub last_sync: Option<SystemTime>,
    
    /// Sync progress
    pub sync_progress: f64,
    
    /// Lag
    pub lag: Duration,
    
    /// Error count
    pub error_count: u32,
    
    /// Last error
    pub last_error: Option<String>,
}

/// Replication task
#[derive(Debug, Clone)]
pub struct ReplicationTask {
    /// Task ID
    pub task_id: String,
    
    /// Target ID
    pub target_id: String,
    
    /// Task type
    pub task_type: TaskType,
    
    /// Task data
    pub data: Vec<u8>,
    
    /// Priority
    pub priority: u32,
    
    /// Created timestamp
    pub created_at: Instant,
    
    /// Retry count
    pub retry_count: u32,
}

/// Task type
#[derive(Debug, Clone)]
pub enum TaskType {
    /// Full sync
    FullSync,
    
    /// Incremental sync
    IncrementalSync,
    
    /// Data update
    DataUpdate,
    
    /// Schema update
    SchemaUpdate,
    
    /// Configuration update
    ConfigUpdate,
}

/// Replication metrics
#[derive(Debug, Clone)]
pub struct ReplicationMetrics {
    /// Total replications
    pub total_replications: Arc<RwLock<u64>>,
    
    /// Successful replications
    pub successful_replications: Arc<RwLock<u64>>,
    
    /// Failed replications
    pub failed_replications: Arc<RwLock<u64>>,
    
    /// Average replication time
    pub avg_replication_time: Arc<RwLock<f64>>,
    
    /// Replication lag
    pub replication_lag: Arc<RwLock<f64>>,
}

/// Storage backend trait
#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    /// Get data by key
    async fn get(&self, key: &str) -> GarpResult<Option<Vec<u8>>>;
    
    /// Set data by key
    async fn set(&self, key: &str, value: Vec<u8>) -> GarpResult<()>;
    
    /// Delete data by key
    async fn delete(&self, key: &str) -> GarpResult<()>;
    
    /// Check if key exists
    async fn exists(&self, key: &str) -> GarpResult<bool>;
    
    /// List keys with prefix
    async fn list_keys(&self, prefix: &str) -> GarpResult<Vec<String>>;
    
    /// Batch operations
    async fn batch(&self, operations: Vec<BatchOperation>) -> GarpResult<()>;
    
    /// Create snapshot
    async fn create_snapshot(&self, snapshot_id: &str) -> GarpResult<()>;
    
    /// Restore from snapshot
    async fn restore_snapshot(&self, snapshot_id: &str) -> GarpResult<()>;
    
    /// Get storage statistics
    async fn get_stats(&self) -> GarpResult<StorageStats>;
}

/// Batch operation
#[derive(Debug, Clone)]
pub enum BatchOperation {
    /// Set operation
    Set { key: String, value: Vec<u8> },
    
    /// Delete operation
    Delete { key: String },
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct StorageStats {
    /// Total keys
    pub total_keys: u64,
    
    /// Total size
    pub total_size: u64,
    
    /// Free space
    pub free_space: u64,
    
    /// Read operations
    pub read_ops: u64,
    
    /// Write operations
    pub write_ops: u64,
    
    /// Delete operations
    pub delete_ops: u64,
}

/// Storage events
#[derive(Debug, Clone)]
pub enum StorageEvent {
    /// Transaction stored
    TransactionStored(TransactionId),
    
    /// Block stored
    BlockStored(BlockHash),
    
    /// State updated
    StateUpdated(DomainId),
    
    /// Consensus completed
    ConsensusCompleted(String),
    
    /// Settlement completed
    SettlementCompleted(String),
    
    /// Backup completed
    BackupCompleted(String),
    
    /// Replication completed
    ReplicationCompleted(String),
    
    /// Storage error
    StorageError(String),
    
    /// Shutdown signal
    Shutdown,
}

/// Storage metrics
#[derive(Debug, Clone)]
pub struct StorageMetrics {
    /// Total storage operations
    pub total_operations: Arc<RwLock<u64>>,
    
    /// Storage size
    pub storage_size: Arc<RwLock<u64>>,
    
    /// Read operations per second
    pub read_ops_per_sec: Arc<RwLock<f64>>,
    
    /// Write operations per second
    pub write_ops_per_sec: Arc<RwLock<f64>>,
    
    /// Average operation time
    pub avg_operation_time: Arc<RwLock<f64>>,
    
    /// Storage errors
    pub storage_errors: Arc<RwLock<u64>>,
}

impl GlobalStorage {
    /// Create new global storage
    pub async fn new(config: Arc<GlobalSyncConfig>) -> GarpResult<Self> {
        let node_id = config.node.node_id.clone();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let event_rx = Arc::new(Mutex::new(event_rx));
        
        // Select storage backend based on configuration
        let backend: Arc<dyn StorageBackend> = if config.database.url.starts_with("postgres://") || config.database.url.starts_with("postgresql://") {
            info!("Using PostgresStorageBackend for persistence");
            Arc::new(PostgresStorageBackend::new(config.clone()).await?)
        } else {
            warn!("Unknown database URL '{}', falling back to in-memory storage", config.database.url);
            Arc::new(MemoryStorageBackend::new())
        };
        
        let transaction_storage = Arc::new(TransactionStorage::new(config.clone(), backend.clone()).await?);
        let block_storage = Arc::new(BlockStorage::new(config.clone(), backend.clone()).await?);
        let state_storage = Arc::new(StateStorage::new(config.clone(), backend.clone()).await?);
        let consensus_storage = Arc::new(ConsensusStorage::new(config.clone(), backend.clone()).await?);
        let cross_domain_storage = Arc::new(CrossDomainStorage::new(config.clone(), backend.clone()).await?);
        let settlement_storage = Arc::new(SettlementStorage::new(config.clone(), backend.clone()).await?);
        let metadata_storage = Arc::new(MetadataStorage::new(config.clone(), backend.clone()).await?);
        
        let cache_manager = Arc::new(CacheManager::new(config.clone()).await?);
        let backup_manager = Arc::new(BackupManager::new(config.clone()).await?);
        let replication_manager = Arc::new(ReplicationManager::new(config.clone()).await?);
        
        let metrics = Arc::new(StorageMetrics {
            total_operations: Arc::new(RwLock::new(0)),
            storage_size: Arc::new(RwLock::new(0)),
            read_ops_per_sec: Arc::new(RwLock::new(0.0)),
            write_ops_per_sec: Arc::new(RwLock::new(0.0)),
            avg_operation_time: Arc::new(RwLock::new(0.0)),
            storage_errors: Arc::new(RwLock::new(0)),
        });
        
        Ok(Self {
            config,
            node_id,
            transaction_storage,
            block_storage,
            state_storage,
            consensus_storage,
            cross_domain_storage,
            settlement_storage,
            metadata_storage,
            cache_manager,
            backup_manager,
            replication_manager,
            metrics,
            event_tx,
            event_rx,
            shutdown_tx: None,
        })
    }
    
    /// Start the global storage
    pub async fn start(&self) -> GarpResult<()> {
        info!("Starting Global Storage");
        
        // Start storage components
        self.cache_manager.start().await?;
        self.backup_manager.start().await?;
        self.replication_manager.start().await?;
        
        // Start background tasks
        let event_processor = self.start_event_processor().await?;
        let metrics_collector = self.start_metrics_collector().await?;
        let maintenance_task = self.start_maintenance_task().await?;
        
        info!("Global Storage started successfully");
        Ok(())
    }
    
    /// Stop the global storage
    pub async fn stop(&self) -> GarpResult<()> {
        info!("Stopping Global Storage");
        
        // Send shutdown signal
        if let Some(shutdown_tx) = &self.shutdown_tx {
            let _ = shutdown_tx.send(());
        }
        
        info!("Global Storage stopped");
        Ok(())
    }
    
    /// Store transaction
    pub async fn store_transaction(&self, transaction: StoredTransaction) -> GarpResult<()> {
        self.transaction_storage.store_transaction(transaction).await
    }
    
    /// Get transaction
    pub async fn get_transaction(&self, transaction_id: &TransactionId) -> GarpResult<Option<StoredTransaction>> {
        self.transaction_storage.get_transaction(transaction_id).await
    }
    
    /// Store block
    pub async fn store_block(&self, block_hash: BlockHash, block_info: BlockInfo) -> GarpResult<()> {
        self.block_storage.store_block(block_hash, block_info).await
    }
    
    /// Get block
    pub async fn get_block(&self, block_hash: &BlockHash) -> GarpResult<Option<BlockInfo>> {
        self.block_storage.get_block(block_hash).await
    }
    
    /// Get block by height
    pub async fn get_block_by_height(&self, height: u64) -> GarpResult<Option<BlockInfo>> {
        let height_index = self.block_storage.height_index.read().await;
        if let Some(block_hash) = height_index.get(&height) {
            drop(height_index);
            return self.get_block(block_hash).await;
        }
        Ok(None)
    }
    
    /// Get latest block info
    pub async fn get_latest_block(&self) -> GarpResult<Option<BlockInfo>> {
        let height_index = self.block_storage.height_index.read().await;
        if let Some((_, block_hash)) = height_index.iter().next_back() {
            drop(height_index);
            return self.get_block(block_hash).await;
        }
        Ok(None)
    }

    /// Get transactions by block height
    pub async fn get_transactions_by_height(&self, height: u64) -> GarpResult<Vec<TransactionId>> {
        self.transaction_storage.get_transactions_by_height(height).await
    }
    
    /// Update state
    pub async fn update_state(&self, domain_id: &DomainId, state: DomainState) -> GarpResult<()> {
        self.state_storage.update_domain_state(domain_id, state).await
    }
    
    /// Get state
    pub async fn get_state(&self, domain_id: &DomainId) -> GarpResult<Option<DomainState>> {
        self.state_storage.get_domain_state(domain_id).await
    }

    /// Assign transactions to a finalized block
    pub async fn assign_block_transactions(
        &self,
        height: u64,
        block_hash: BlockHash,
        tx_ids: &[TransactionId],
    ) -> GarpResult<()> {
        self.transaction_storage.assign_block(height, block_hash, tx_ids).await
    }
    
    /// Get metrics
    pub async fn get_metrics(&self) -> StorageMetrics {
        self.metrics.clone()
    }
    
    /// Start event processor
    async fn start_event_processor(&self) -> GarpResult<tokio::task::JoinHandle<()>> {
        let event_rx = self.event_rx.clone();
        let metrics = self.metrics.clone();
        
        let handle = tokio::spawn(async move {
            let mut event_rx = event_rx.lock().await;
            
            while let Some(event) = event_rx.recv().await {
                match event {
                    StorageEvent::TransactionStored(_) => {
                        // Handle transaction stored event
                    }
                    StorageEvent::BlockStored(_) => {
                        // Handle block stored event
                    }
                    StorageEvent::StateUpdated(_) => {
                        // Handle state updated event
                    }
                    StorageEvent::StorageError(error) => {
                        error!("Storage error: {}", error);
                        let mut errors = metrics.storage_errors.write().await;
                        *errors += 1;
                    }
                    StorageEvent::Shutdown => {
                        info!("Storage event processor shutting down");
                        break;
                    }
                    _ => {
                        // Handle other events
                    }
                }
            }
        });
        
        Ok(handle)
    }
    
    /// Start metrics collector
    async fn start_metrics_collector(&self) -> GarpResult<tokio::task::JoinHandle<()>> {
        let metrics = self.metrics.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                // TODO: Collect and update metrics
            }
        });
        
        Ok(handle)
    }
    
    /// Start maintenance task
    async fn start_maintenance_task(&self) -> GarpResult<tokio::task::JoinHandle<()>> {
        let cache_manager = self.cache_manager.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                // Perform cache cleanup
                if let Err(e) = cache_manager.cleanup().await {
                    error!("Cache cleanup failed: {}", e);
                }
                
                // TODO: Perform other maintenance tasks
            }
        });
        
        Ok(handle)
    }

    /// Store a finality certificate via consensus storage
    pub async fn store_finality_certificate(&self, cert: FinalityCertificate) -> GarpResult<()> {
        self.consensus_storage.store_finality_certificate(cert).await
    }

    /// Get a finality certificate by block hash string
    pub async fn get_finality_certificate_by_hash(&self, hash: &str) -> GarpResult<Option<FinalityCertificate>> {
        self.consensus_storage.get_finality_certificate_by_hash(hash).await
    }

    /// Get a finality certificate by block height
    pub async fn get_finality_certificate_by_height(&self, height: u64) -> GarpResult<Option<FinalityCertificate>> {
        self.consensus_storage.get_finality_certificate_by_height(height).await
    }
}

// Implementation stubs for storage components
impl TransactionStorage {
    pub async fn new(config: Arc<GlobalSyncConfig>, backend: Arc<dyn StorageBackend>) -> GarpResult<Self> {
        let metrics = Arc::new(TransactionStorageMetrics {
            total_transactions: Arc::new(RwLock::new(0)),
            active_transactions: Arc::new(RwLock::new(0)),
            storage_ops_per_sec: Arc::new(RwLock::new(0.0)),
            avg_storage_time: Arc::new(RwLock::new(0.0)),
            storage_errors: Arc::new(RwLock::new(0)),
        });
        
        Ok(Self {
            config,
            active_transactions: Arc::new(RwLock::new(HashMap::new())),
            transaction_history: Arc::new(RwLock::new(BTreeMap::new())),
            transaction_index: Arc::new(RwLock::new(HashMap::new())),
            pending_transactions: Arc::new(RwLock::new(VecDeque::new())),
            transaction_pool: Arc::new(RwLock::new(TransactionPool {
                transactions: HashMap::new(),
                priority_queue: BTreeMap::new(),
                size: 0,
                max_size: 10000,
                stats: PoolStats {
                    total_added: 0,
                    total_removed: 0,
                    avg_pool_size: 0.0,
                    avg_wait_time: Duration::from_secs(0),
                    utilization: 0.0,
                },
            })),
            backend,
            metrics,
        })
    }
    
    pub async fn store_transaction(&self, transaction: StoredTransaction) -> GarpResult<()> {
        let mut active = self.active_transactions.write().await;
        active.insert(transaction.transaction_id.clone(), transaction);
        Ok(())
    }
    
    pub async fn get_transaction(&self, transaction_id: &TransactionId) -> GarpResult<Option<StoredTransaction>> {
        let active = self.active_transactions.read().await;
        Ok(active.get(transaction_id).cloned())
    }

    /// Assign a set of transactions to a finalized block height and hash
    pub async fn assign_block(
        &self,
        height: u64,
        block_hash: BlockHash,
        tx_ids: &[TransactionId],
    ) -> GarpResult<()> {
        // Update height -> tx_ids index
        {
            let mut history = self.transaction_history.write().await;
            let entry = history.entry(height).or_insert_with(Vec::new);
            for tid in tx_ids {
                if !entry.contains(tid) {
                    entry.push(tid.clone());
                }
            }
        }

        // Tag active transactions with block metadata and mark as settled
        {
            let mut active = self.active_transactions.write().await;
            for tid in tx_ids {
                if let Some(tx) = active.get_mut(tid) {
                    tx.block_height = Some(height);
                    tx.block_hash = Some(block_hash.clone());
                    tx.updated_at = SystemTime::now();
                    // Mark transaction as settled upon finalization
                    tx.status = TransactionStatus::Settled;
                }
            }
        }

        // Maintain generic index keys for convenience (height and block hash)
        {
            let mut index = self.transaction_index.write().await;
            let height_key = format!("height:{}", height);
            let block_key = format!("block:{}", hex::encode(&block_hash));

            let height_set = index.entry(height_key).or_insert_with(HashSet::new);
            let block_set = index.entry(block_key).or_insert_with(HashSet::new);
            for tid in tx_ids {
                height_set.insert(tid.clone());
                block_set.insert(tid.clone());
            }
        }

        Ok(())
    }

    /// Get transaction IDs assigned to a given block height
    pub async fn get_transactions_by_height(&self, height: u64) -> GarpResult<Vec<TransactionId>> {
        let history = self.transaction_history.read().await;
        Ok(history.get(&height).cloned().unwrap_or_default())
    }
}

impl BlockStorage {
    pub async fn new(config: Arc<GlobalSyncConfig>, backend: Arc<dyn StorageBackend>) -> GarpResult<Self> {
        let metrics = Arc::new(BlockStorageMetrics {
            total_blocks: Arc::new(RwLock::new(0)),
            current_height: Arc::new(RwLock::new(0)),
            block_storage_rate: Arc::new(RwLock::new(0.0)),
            avg_block_size: Arc::new(RwLock::new(0.0)),
            storage_errors: Arc::new(RwLock::new(0)),
        });
        
        Ok(Self {
            config,
            blockchain: Arc::new(RwLock::new(BlockChain {
                genesis_block: BlockHash::default(),
                head: BlockHash::default(),
                height: 0,
                total_difficulty: 0,
                stats: ChainStats {
                    total_blocks: 0,
                    total_transactions: 0,
                    avg_block_time: Duration::from_secs(0),
                    avg_block_size: 0,
                    chain_size: 0,
                },
            })),
            block_index: Arc::new(RwLock::new(HashMap::new())),
            height_index: Arc::new(RwLock::new(BTreeMap::new())),
            pending_blocks: Arc::new(RwLock::new(HashMap::new())),
            backend,
            metrics,
        })
    }
    
    pub async fn store_block(&self, block_hash: BlockHash, block_info: BlockInfo) -> GarpResult<()> {
        let mut index = self.block_index.write().await;
        let mut height_index = self.height_index.write().await;
        
        index.insert(block_hash.clone(), block_info.clone());
        height_index.insert(block_info.height, block_hash);
        
        Ok(())
    }
    
    pub async fn get_block(&self, block_hash: &BlockHash) -> GarpResult<Option<BlockInfo>> {
        let index = self.block_index.read().await;
        Ok(index.get(block_hash).cloned())
    }
}

impl StateStorage {
    pub async fn new(config: Arc<GlobalSyncConfig>, backend: Arc<dyn StorageBackend>) -> GarpResult<Self> {
        let metrics = Arc::new(StateStorageMetrics {
            state_updates: Arc::new(RwLock::new(0)),
            snapshots_created: Arc::new(RwLock::new(0)),
            state_size: Arc::new(RwLock::new(0)),
            avg_update_time: Arc::new(RwLock::new(0.0)),
            storage_errors: Arc::new(RwLock::new(0)),
        });
        
        Ok(Self {
            config,
            global_state: Arc::new(RwLock::new(GlobalState {
                version: 0,
                root_hash: Vec::new(),
                domain_states: HashMap::new(),
                global_variables: HashMap::new(),
                validator_set: HashMap::new(),
                configuration: HashMap::new(),
                last_updated: SystemTime::now(),
                metadata: HashMap::new(),
            })),
            domain_states: Arc::new(RwLock::new(HashMap::new())),
            state_snapshots: Arc::new(RwLock::new(HashMap::new())),
            state_transitions: Arc::new(RwLock::new(VecDeque::new())),
            backend,
            metrics,
        })
    }
    
    pub async fn update_domain_state(&self, domain_id: &DomainId, state: DomainState) -> GarpResult<()> {
        let mut states = self.domain_states.write().await;
        states.insert(domain_id.clone(), state);
        Ok(())
    }
    
    pub async fn get_domain_state(&self, domain_id: &DomainId) -> GarpResult<Option<DomainState>> {
        let states = self.domain_states.read().await;
        Ok(states.get(domain_id).cloned())
    }
}

impl ConsensusStorage {
    pub async fn new(config: Arc<GlobalSyncConfig>, backend: Arc<dyn StorageBackend>) -> GarpResult<Self> {
        let metrics = Arc::new(ConsensusStorageMetrics {
            total_sessions: Arc::new(RwLock::new(0)),
            active_sessions: Arc::new(RwLock::new(0)),
            successful_consensus: Arc::new(RwLock::new(0)),
            failed_consensus: Arc::new(RwLock::new(0)),
            avg_consensus_time: Arc::new(RwLock::new(0.0)),
        });
        
        Ok(Self {
            config,
            consensus_sessions: Arc::new(RwLock::new(HashMap::new())),
            consensus_history: Arc::new(RwLock::new(VecDeque::new())),
            vote_records: Arc::new(RwLock::new(HashMap::new())),
            view_changes: Arc::new(RwLock::new(HashMap::new())),
            finality_by_hash: Arc::new(RwLock::new(HashMap::new())),
            finality_by_height: Arc::new(RwLock::new(BTreeMap::new())),
            backend,
            metrics,
        })
    }

    /// Store a finality certificate and index it by hash and height
    pub async fn store_finality_certificate(&self, cert: FinalityCertificate) -> GarpResult<()> {
        let mut by_hash = self.finality_by_hash.write().await;
        let mut by_height = self.finality_by_height.write().await;
        by_hash.insert(cert.block_hash.clone(), cert.clone());
        by_height.insert(cert.height, cert);
        Ok(())
    }

    /// Retrieve a finality certificate by block hash string
    pub async fn get_finality_certificate_by_hash(&self, hash: &str) -> GarpResult<Option<FinalityCertificate>> {
        let by_hash = self.finality_by_hash.read().await;
        Ok(by_hash.get(hash).cloned())
    }

    /// Retrieve a finality certificate by block height
    pub async fn get_finality_certificate_by_height(&self, height: u64) -> GarpResult<Option<FinalityCertificate>> {
        let by_height = self.finality_by_height.read().await;
        Ok(by_height.get(&height).cloned())
    }
}

impl CrossDomainStorage {
    pub async fn new(config: Arc<GlobalSyncConfig>, backend: Arc<dyn StorageBackend>) -> GarpResult<Self> {
        let metrics = Arc::new(CrossDomainStorageMetrics {
            cross_domain_transactions: Arc::new(RwLock::new(0)),
            active_coordination_sessions: Arc::new(RwLock::new(0)),
            successful_coordinations: Arc::new(RwLock::new(0)),
            failed_coordinations: Arc::new(RwLock::new(0)),
            avg_coordination_time: Arc::new(RwLock::new(0.0)),
        });
        
        Ok(Self {
            config,
            cross_domain_transactions: Arc::new(RwLock::new(HashMap::new())),
            domain_coordination: Arc::new(RwLock::new(HashMap::new())),
            state_synchronization: Arc::new(RwLock::new(HashMap::new())),
            backend,
            metrics,
        })
    }
}

impl SettlementStorage {
    pub async fn new(config: Arc<GlobalSyncConfig>, backend: Arc<dyn StorageBackend>) -> GarpResult<Self> {
        let metrics = Arc::new(SettlementStorageMetrics {
            total_settlements: Arc::new(RwLock::new(0)),
            successful_settlements: Arc::new(RwLock::new(0)),
            failed_settlements: Arc::new(RwLock::new(0)),
            rollbacks: Arc::new(RwLock::new(0)),
            avg_settlement_time: Arc::new(RwLock::new(0.0)),
        });
        
        Ok(Self {
            config,
            settlements: Arc::new(RwLock::new(HashMap::new())),
            settlement_batches: Arc::new(RwLock::new(HashMap::new())),
            rollback_records: Arc::new(RwLock::new(HashMap::new())),
            backend,
            metrics,
        })
    }
}

impl MetadataStorage {
    pub async fn new(config: Arc<GlobalSyncConfig>, backend: Arc<dyn StorageBackend>) -> GarpResult<Self> {
        let metrics = Arc::new(MetadataStorageMetrics {
            total_entries: Arc::new(RwLock::new(0)),
            metadata_updates: Arc::new(RwLock::new(0)),
            storage_size: Arc::new(RwLock::new(0)),
            avg_access_time: Arc::new(RwLock::new(0.0)),
            storage_errors: Arc::new(RwLock::new(0)),
        });
        
        Ok(Self {
            config,
            node_metadata: Arc::new(RwLock::new(HashMap::new())),
            domain_metadata: Arc::new(RwLock::new(HashMap::new())),
            system_metadata: Arc::new(RwLock::new(HashMap::new())),
            backend,
            metrics,
        })
    }
}

impl CacheManager {
    pub async fn new(config: Arc<GlobalSyncConfig>) -> GarpResult<Self> {
        let metrics = Arc::new(CacheMetrics {
            cache_hits: Arc::new(RwLock::new(0)),
            cache_misses: Arc::new(RwLock::new(0)),
            hit_rate: Arc::new(RwLock::new(0.0)),
            cache_size: Arc::new(RwLock::new(0)),
            evictions: Arc::new(RwLock::new(0)),
        });
        
        Ok(Self {
            config,
            cache_layers: Arc::new(RwLock::new(HashMap::new())),
            cache_policies: Arc::new(RwLock::new(HashMap::new())),
            cache_stats: Arc::new(RwLock::new(CacheStats {
                total_hits: 0,
                total_misses: 0,
                hit_rate: 0.0,
                total_evictions: 0,
                total_size: 0,
                total_entries: 0,
            })),
            metrics,
        })
    }
    
    pub async fn start(&self) -> GarpResult<()> {
        info!("Starting Cache Manager");
        Ok(())
    }
    
    pub async fn cleanup(&self) -> GarpResult<()> {
        // Perform cache cleanup
        Ok(())
    }
}

impl BackupManager {
    pub async fn new(config: Arc<GlobalSyncConfig>) -> GarpResult<Self> {
        let metrics = Arc::new(BackupMetrics {
            total_backups: Arc::new(RwLock::new(0)),
            successful_backups: Arc::new(RwLock::new(0)),
            failed_backups: Arc::new(RwLock::new(0)),
            total_backup_size: Arc::new(RwLock::new(0)),
            avg_backup_time: Arc::new(RwLock::new(0.0)),
        });
        
        Ok(Self {
            config,
            backup_schedules: Arc::new(RwLock::new(HashMap::new())),
            backup_history: Arc::new(RwLock::new(VecDeque::new())),
            active_backups: Arc::new(RwLock::new(HashMap::new())),
            metrics,
        })
    }
    
    pub async fn start(&self) -> GarpResult<()> {
        info!("Starting Backup Manager");
        Ok(())
    }
}

impl ReplicationManager {
    pub async fn new(config: Arc<GlobalSyncConfig>) -> GarpResult<Self> {
        let metrics = Arc::new(ReplicationMetrics {
            total_replications: Arc::new(RwLock::new(0)),
            successful_replications: Arc::new(RwLock::new(0)),
            failed_replications: Arc::new(RwLock::new(0)),
            avg_replication_time: Arc::new(RwLock::new(0.0)),
            replication_lag: Arc::new(RwLock::new(0.0)),
        });
        
        Ok(Self {
            config,
            replication_targets: Arc::new(RwLock::new(HashMap::new())),
            replication_status: Arc::new(RwLock::new(HashMap::new())),
            replication_queue: Arc::new(RwLock::new(VecDeque::new())),
            metrics,
        })
    }
    
    pub async fn start(&self) -> GarpResult<()> {
        info!("Starting Replication Manager");
        Ok(())
    }
}

/// Memory storage backend for testing
pub struct MemoryStorageBackend {
    data: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl MemoryStorageBackend {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl StorageBackend for MemoryStorageBackend {
    async fn get(&self, key: &str) -> GarpResult<Option<Vec<u8>>> {
        let data = self.data.read().await;
        Ok(data.get(key).cloned())
    }
    
    async fn set(&self, key: &str, value: Vec<u8>) -> GarpResult<()> {
        let mut data = self.data.write().await;
        data.insert(key.to_string(), value);
        Ok(())
    }
    
    async fn delete(&self, key: &str) -> GarpResult<()> {
        let mut data = self.data.write().await;
        data.remove(key);
        Ok(())
    }
    
    async fn exists(&self, key: &str) -> GarpResult<bool> {
        let data = self.data.read().await;
        Ok(data.contains_key(key))
    }
    
    async fn list_keys(&self, prefix: &str) -> GarpResult<Vec<String>> {
        let data = self.data.read().await;
        Ok(data.keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect())
    }
    
    async fn batch(&self, operations: Vec<BatchOperation>) -> GarpResult<()> {
        let mut data = self.data.write().await;
        
        for op in operations {
            match op {
                BatchOperation::Set { key, value } => {
                    data.insert(key, value);
                }
                BatchOperation::Delete { key } => {
                    data.remove(&key);
                }
            }
        }
        
        Ok(())
    }
    
    async fn create_snapshot(&self, _snapshot_id: &str) -> GarpResult<()> {
        // TODO: Implement snapshot creation
        Ok(())
    }
    
    async fn restore_snapshot(&self, _snapshot_id: &str) -> GarpResult<()> {
        // TODO: Implement snapshot restoration
        Ok(())
    }
    
    async fn get_stats(&self) -> GarpResult<StorageStats> {
        let data = self.data.read().await;
        let total_size = data.values().map(|v| v.len() as u64).sum();
        
        Ok(StorageStats {
            total_keys: data.len() as u64,
            total_size,
            free_space: u64::MAX - total_size,
            read_ops: 0,
            write_ops: 0,
            delete_ops: 0,
        })
    }
}

// ---------------------------
// Postgres storage backend
// ---------------------------

pub struct PostgresStorageBackend {
    pool: Pool<Postgres>,
}

impl PostgresStorageBackend {
    pub async fn new(config: Arc<GlobalSyncConfig>) -> GarpResult<Self> {
        // Build connection pool
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(config.database.max_connections)
            .min_connections(config.database.min_connections)
            .connect_timeout(std::time::Duration::from_millis(config.database.connect_timeout_ms))
            .idle_timeout(std::time::Duration::from_millis(config.database.idle_timeout_ms))
            .connect(&config.database.url)
            .await
            .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres connect error: {}", e)))?;

        let backend = Self { pool };
        if config.database.enable_migrations {
            backend.run_migrations().await?;
        }
        Ok(backend)
    }

    async fn run_migrations(&self) -> GarpResult<()> {
        // Create a simple key-value store and snapshot tables if they do not exist
        let queries = [
            r#"CREATE TABLE IF NOT EXISTS kv_store (
                    key TEXT PRIMARY KEY,
                    value BYTEA NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                )"#,
            r#"CREATE TABLE IF NOT EXISTS kv_snapshots (
                    snapshot_id TEXT PRIMARY KEY,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                )"#,
            r#"CREATE TABLE IF NOT EXISTS kv_snapshot_entries (
                    snapshot_id TEXT NOT NULL,
                    key TEXT NOT NULL,
                    value BYTEA NOT NULL,
                    PRIMARY KEY (snapshot_id, key),
                    FOREIGN KEY (snapshot_id) REFERENCES kv_snapshots(snapshot_id) ON DELETE CASCADE
                )"#,
        ];

        for q in queries {
            sqlx::query(q)
                .execute(&self.pool)
                .await
                .map_err(|e| garp_common::GarpError::StorageError(format!("Migration error: {}", e)))?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl StorageBackend for PostgresStorageBackend {
    async fn get(&self, key: &str) -> GarpResult<Option<Vec<u8>>> {
        let res = sqlx::query("SELECT value FROM kv_store WHERE key = $1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres get error: {}", e)))?;
        Ok(res.map(|row| row.get::<Vec<u8>, _>("value")))
    }

    async fn set(&self, key: &str, value: Vec<u8>) -> GarpResult<()> {
        sqlx::query(r#"
            INSERT INTO kv_store(key, value, created_at, updated_at)
            VALUES ($1, $2, NOW(), NOW())
            ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()
        "#)
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await
            .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres set error: {}", e)))?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> GarpResult<()> {
        sqlx::query("DELETE FROM kv_store WHERE key = $1")
            .bind(key)
            .execute(&self.pool)
            .await
            .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres delete error: {}", e)))?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> GarpResult<bool> {
        let res = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM kv_store WHERE key = $1")
            .bind(key)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres exists error: {}", e)))?;
        Ok(res > 0)
    }

    async fn list_keys(&self, prefix: &str) -> GarpResult<Vec<String>> {
        let like = format!("{}%", prefix);
        let rows = sqlx::query("SELECT key FROM kv_store WHERE key LIKE $1")
            .bind(&like)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres list_keys error: {}", e)))?;
        Ok(rows.into_iter().map(|r| r.get::<String, _>("key")).collect())
    }

    async fn batch(&self, operations: Vec<BatchOperation>) -> GarpResult<()> {
        let mut tx = self.pool.begin().await
            .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres begin tx error: {}", e)))?;
        for op in operations {
            match op {
                BatchOperation::Set { key, value } => {
                    sqlx::query(r#"
                        INSERT INTO kv_store(key, value, created_at, updated_at)
                        VALUES ($1, $2, NOW(), NOW())
                        ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()
                    "#)
                        .bind(&key)
                        .bind(&value)
                        .execute(&mut tx)
                        .await
                        .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres batch set error: {}", e)))?;
                }
                BatchOperation::Delete { key } => {
                    sqlx::query("DELETE FROM kv_store WHERE key = $1")
                        .bind(&key)
                        .execute(&mut tx)
                        .await
                        .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres batch delete error: {}", e)))?;
                }
            }
        }
        tx.commit().await
            .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres commit tx error: {}", e)))?;
        Ok(())
    }

    async fn create_snapshot(&self, snapshot_id: &str) -> GarpResult<()> {
        let mut tx = self.pool.begin().await
            .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres begin snapshot tx error: {}", e)))?;
        sqlx::query("INSERT INTO kv_snapshots(snapshot_id, created_at) VALUES ($1, NOW()) ON CONFLICT DO NOTHING")
            .bind(snapshot_id)
            .execute(&mut tx)
            .await
            .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres snapshot header error: {}", e)))?;
        sqlx::query(r#"
            INSERT INTO kv_snapshot_entries(snapshot_id, key, value)
            SELECT $1, key, value FROM kv_store
            ON CONFLICT (snapshot_id, key) DO UPDATE SET value = EXCLUDED.value
        "#)
            .bind(snapshot_id)
            .execute(&mut tx)
            .await
            .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres snapshot entries error: {}", e)))?;
        tx.commit().await
            .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres commit snapshot tx error: {}", e)))?;
        Ok(())
    }

    async fn restore_snapshot(&self, snapshot_id: &str) -> GarpResult<()> {
        let mut tx = self.pool.begin().await
            .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres begin restore tx error: {}", e)))?;
        sqlx::query("DELETE FROM kv_store")
            .execute(&mut tx)
            .await
            .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres clear kv_store error: {}", e)))?;
        sqlx::query(r#"
            INSERT INTO kv_store(key, value, created_at, updated_at)
            SELECT key, value, NOW(), NOW()
            FROM kv_snapshot_entries
            WHERE snapshot_id = $1
            ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()
        "#)
            .bind(snapshot_id)
            .execute(&mut tx)
            .await
            .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres restore entries error: {}", e)))?;
        tx.commit().await
            .map_err(|e| garp_common::GarpError::StorageError(format!("Postgres commit restore tx error: {}", e)))?;
        Ok(())
    }

    async fn get_stats(&self) -> GarpResult<StorageStats> {
        let total_keys = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM kv_store")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);
        // Size info is not trivial; approximate by sum of octet_length(value)
        let total_size = sqlx::query_scalar::<_, Option<i64>>("SELECT SUM(octet_length(value)) FROM kv_store")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(None)
            .unwrap_or(0);
        Ok(StorageStats {
            total_keys: total_keys as u64,
            total_size: total_size as u64,
            free_space: 0,
            read_ops: 0,
            write_ops: 0,
            delete_ops: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use garp_common::types::*;
    
    #[tokio::test]
    async fn test_global_storage_creation() {
        let config = Arc::new(GlobalSyncConfig::default());
        let storage = GlobalStorage::new(config).await;
        assert!(storage.is_ok());
    }
    
    #[tokio::test]
    async fn test_transaction_storage() {
        let config = Arc::new(GlobalSyncConfig::default());
        let backend = Arc::new(MemoryStorageBackend::new());
        let storage = TransactionStorage::new(config, backend).await.unwrap();
        
        let transaction = StoredTransaction {
            transaction_id: TransactionId(uuid::Uuid::new_v4()),
            transaction_data: vec![1, 2, 3],
            transaction_type: "test".to_string(),
            source_domain: "domain1".to_string(),
            target_domains: vec!["domain2".to_string()],
            status: TransactionStatus::Pending,
            consensus_state: ConsensusState {
                phase: "prepare".to_string(),
                votes: HashMap::new(),
                required_votes: 3,
                result: None,
                proof: None,
                started_at: SystemTime::now(),
                completed_at: None,
            },
            settlement_state: SettlementState {
                settlement_id: None,
                settlement_type: "atomic".to_string(),
                domain_settlements: HashMap::new(),
                proof: None,
                started_at: None,
                completed_at: None,
            },
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            block_height: None,
            block_hash: None,
            metadata: HashMap::new(),
            dependencies: Vec::new(),
            dependents: Vec::new(),
        };
        
        let tx_id = transaction.transaction_id.clone();
        storage.store_transaction(transaction).await.unwrap();
        
        let retrieved = storage.get_transaction(&tx_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().transaction_id, tx_id);
    }
    
    #[tokio::test]
    async fn test_memory_storage_backend() {
        let backend = MemoryStorageBackend::new();
        
        // Test set and get
        backend.set("key1", vec![1, 2, 3]).await.unwrap();
        let value = backend.get("key1").await.unwrap();
        assert_eq!(value, Some(vec![1, 2, 3]));
        
        // Test exists
        assert!(backend.exists("key1").await.unwrap());
        assert!(!backend.exists("key2").await.unwrap());
        
        // Test delete
        backend.delete("key1").await.unwrap();
        assert!(!backend.exists("key1").await.unwrap());
    }
}