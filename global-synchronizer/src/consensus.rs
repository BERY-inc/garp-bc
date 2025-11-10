use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::hash::Hash;
use tokio::sync::{RwLock, Mutex, mpsc, oneshot};
use tokio::time::{interval, timeout};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};

use garp_common::{GarpResult, GarpError};
use garp_common::types::{TransactionId, ParticipantId};
use garp_common::consensus::{ValidationResult};

use crate::config::{GlobalSyncConfig, ConsensusAlgorithm};
use crate::cross_domain::{CrossDomainTransaction, CrossDomainTransactionType};
use crate::storage::{GlobalStorage, GlobalBlock, BlockHeader};
use crate::validator::{ValidatorInfo, ValidatorStatus};
use crate::network::NetworkManager;

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
struct BlockHashInput {
    height: u64,
    previous_hash: String,
    timestamp: chrono::DateTime<chrono::Utc>,
    merkle_root: String,
    transactions: Vec<String>,
    validator_signatures: usize,
}

/// BFT Consensus Engine for Global Synchronizer
pub struct ConsensusEngine {
    /// Configuration
    config: Arc<GlobalSyncConfig>,
    
    /// Storage layer
    storage: Arc<dyn GlobalStorage>,
    
    /// Network manager
    network_manager: Arc<NetworkManager>,
    
    /// Current consensus state
    consensus_state: Arc<RwLock<ConsensusState>>,
    
    /// Active consensus sessions
    active_sessions: Arc<RwLock<HashMap<String, ConsensusSession>>>,
    
    /// Validator set
    validator_set: Arc<RwLock<ValidatorSet>>,
    
    /// Message queue
    message_queue: Arc<Mutex<VecDeque<ConsensusMessage>>>,
    
    /// Event channels
    event_tx: mpsc::UnboundedSender<ConsensusEvent>,
    event_rx: Arc<Mutex<mpsc::UnboundedReceiver<ConsensusEvent>>>,
    
    /// Shutdown signal
    shutdown_tx: Option<oneshot::Sender<()>>,
    
    /// Metrics
    metrics: Arc<ConsensusMetrics>,
}

/// Consensus state
#[derive(Debug, Clone)]
pub struct ConsensusState {
    /// Current view/round
    pub current_view: u64,
    
    /// Current phase
    pub current_phase: ConsensusPhase,
    
    /// Current leader
    pub current_leader: Option<ParticipantId>,
    
    /// Last committed block
    pub last_committed_block: u64,
    
    /// Last committed hash
    pub last_committed_hash: String,
    
    /// View change in progress
    pub view_change_in_progress: bool,
    
    /// View change votes
    pub view_change_votes: HashMap<ParticipantId, ViewChangeVote>,
    
    /// Last updated
    pub last_updated: Instant,
}

/// Consensus phase
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConsensusPhase {
    /// Prepare phase
    Prepare,
    
    /// Pre-commit phase
    PreCommit,
    
    /// Commit phase
    Commit,
    
    /// View change phase
    ViewChange,
    
    /// Idle (no active consensus)
    Idle,
}

/// Consensus session for a specific proposal
#[derive(Debug, Clone)]
pub struct ConsensusSession {
    /// Session ID
    pub session_id: String,
    
    /// Proposal being decided
    pub proposal: ConsensusProposal,
    
    /// Current phase
    pub phase: ConsensusPhase,
    
    /// View number
    pub view: u64,
    
    /// Votes received
    pub votes: HashMap<ParticipantId, ConsensusVote>,
    
    /// Required votes for decision
    pub required_votes: usize,
    
    /// Session timeout
    pub timeout_at: Instant,
    
    /// Created timestamp
    pub created_at: Instant,
    
    /// Last activity
    pub last_activity: Instant,
}

/// Consensus proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusProposal {
    /// Proposal ID
    pub proposal_id: String,
    
    /// Proposal type
    pub proposal_type: ProposalType,
    
    /// Proposal data
    pub data: Vec<u8>,
    
    /// Proposer ID
    pub proposer_id: ParticipantId,
    
    /// View number
    pub view: u64,
    
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Signature
    pub signature: Vec<u8>,
}

/// Proposal type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalType {
    /// Block proposal
    Block(GlobalBlock),
    
    /// Transaction batch
    TransactionBatch(Vec<CrossDomainTransaction>),
    
    /// Validator set change
    ValidatorSetChange(ValidatorSetChange),
    
    /// Configuration change
    ConfigurationChange(ConfigurationChange),
    
    /// Emergency action
    EmergencyAction(EmergencyAction),
}

/// Consensus vote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusVote {
    /// Voter ID
    pub voter_id: ParticipantId,
    
    /// Proposal ID
    pub proposal_id: String,
    
    /// Vote type
    pub vote_type: VoteType,
    
    /// Vote value (approve/reject)
    pub vote: bool,
    
    /// Vote reason
    pub reason: Option<String>,
    
    /// View number
    pub view: u64,
    
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Signature
    pub signature: Vec<u8>,
}

/// Vote type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VoteType {
    /// Prepare vote
    Prepare,
    
    /// Pre-commit vote
    PreCommit,
    
    /// Commit vote
    Commit,
    
    /// View change vote
    ViewChange,
}

/// View change vote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewChangeVote {
    /// Voter ID
    pub voter_id: ParticipantId,
    
    /// New view number
    pub new_view: u64,
    
    /// Last prepared proposal
    pub last_prepared: Option<ConsensusProposal>,
    
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Signature
    pub signature: Vec<u8>,
}

/// Validator set
#[derive(Debug, Clone)]
pub struct ValidatorSet {
    /// Active validators
    pub validators: HashMap<ParticipantId, ValidatorInfo>,
    
    /// Total voting power
    pub total_voting_power: u64,
    
    /// Byzantine threshold (f in 3f+1)
    pub byzantine_threshold: usize,
    
    /// Required votes for consensus
    pub required_votes: usize,
    
    /// Last updated
    pub last_updated: Instant,
}

/// Validator set change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorSetChange {
    /// Change type
    pub change_type: ValidatorChangeType,
    
    /// Validator info
    pub validator: ValidatorInfo,
    
    /// Effective height
    pub effective_height: u64,
}

/// Validator change type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidatorChangeType {
    /// Add validator
    Add,
    
    /// Remove validator
    Remove,
    
    /// Update validator
    Update,
}

/// Configuration change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationChange {
    /// Parameter name
    pub parameter: String,
    
    /// New value
    pub value: String,
    
    /// Effective height
    pub effective_height: u64,
}

/// Emergency action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyAction {
    /// Action type
    pub action_type: EmergencyActionType,
    
    /// Action data
    pub data: Vec<u8>,
    
    /// Justification
    pub justification: String,
}

/// Emergency action type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmergencyActionType {
    /// Halt the network
    Halt,
    
    /// Emergency validator removal
    EmergencyValidatorRemoval,
    
    /// Emergency configuration change
    EmergencyConfigChange,
    
    /// Network reset
    NetworkReset,
}

/// Consensus message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusMessage {
    /// Message ID
    pub message_id: String,
    
    /// Message type
    pub message_type: ConsensusMessageType,
    
    /// Sender ID
    pub sender_id: ParticipantId,
    
    /// View number
    pub view: u64,
    
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Signature
    pub signature: Vec<u8>,
}

/// Consensus message type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusMessageType {
    /// Proposal message
    Proposal(ConsensusProposal),
    
    /// Vote message
    Vote(ConsensusVote),
    
    /// View change message
    ViewChange(ViewChangeVote),
    
    /// New view message
    NewView(NewViewMessage),
    
    /// Heartbeat message
    Heartbeat(HeartbeatMessage),
    
    /// Sync request
    SyncRequest(SyncRequestMessage),
    
    /// Sync response
    SyncResponse(SyncResponseMessage),
}

/// New view message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewViewMessage {
    /// New view number
    pub new_view: u64,
    
    /// View change proofs
    pub view_change_proofs: Vec<ViewChangeVote>,
    
    /// New proposal (if any)
    pub new_proposal: Option<ConsensusProposal>,
}

/// Heartbeat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatMessage {
    /// Current view
    pub current_view: u64,
    
    /// Last committed block
    pub last_committed_block: u64,
    
    /// Node status
    pub status: String,
}

/// Sync request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequestMessage {
    /// Requested start height
    pub start_height: u64,
    
    /// Requested end height
    pub end_height: u64,
}

/// Sync response message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResponseMessage {
    /// Blocks
    pub blocks: Vec<GlobalBlock>,
    
    /// More blocks available
    pub has_more: bool,
}

/// Consensus result
#[derive(Debug, Clone)]
pub struct ConsensusResult {
    /// Transaction ID
    pub transaction_id: TransactionId,
    
    /// Approved or rejected
    pub approved: bool,
    
    /// Consensus proof
    pub proof: ConsensusProof,
    
    /// Participating validators
    pub validators: Vec<ParticipantId>,
    
    /// Finalized at
    pub finalized_at: Instant,
}

/// Consensus proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusProof {
    /// Proposal ID
    pub proposal_id: String,
    
    /// View number
    pub view: u64,
    
    /// Votes
    pub votes: Vec<ConsensusVote>,
    
    /// Aggregated signature
    pub aggregated_signature: Vec<u8>,
}

/// Consensus events
#[derive(Debug, Clone)]
pub enum ConsensusEvent {
    /// New proposal received
    ProposalReceived(ConsensusProposal),
    
    /// Vote received
    VoteReceived(ConsensusVote),
    
    /// View change initiated
    ViewChangeInitiated(u64),
    
    /// New view established
    NewViewEstablished(u64),
    
    /// Consensus reached
    ConsensusReached(ConsensusResult),
    
    /// Consensus failed
    ConsensusFailed(String, String),
    
    /// Timeout occurred
    TimeoutOccurred(String),
    
    /// Validator joined
    ValidatorJoined(ValidatorInfo),
    
    /// Validator left
    ValidatorLeft(ParticipantId),
    
    /// Sync required
    SyncRequired(u64),
    
    /// Shutdown signal
    Shutdown,
}

/// Consensus metrics
#[derive(Debug, Clone)]
pub struct ConsensusMetrics {
    /// Total proposals
    pub total_proposals: Arc<RwLock<u64>>,
    
    /// Successful consensus
    pub successful_consensus: Arc<RwLock<u64>>,
    
    /// Failed consensus
    pub failed_consensus: Arc<RwLock<u64>>,
    
    /// View changes
    pub view_changes: Arc<RwLock<u64>>,
    
    /// Average consensus time
    pub avg_consensus_time: Arc<RwLock<f64>>,
    
    /// Current view
    pub current_view: Arc<RwLock<u64>>,
    
    /// Active sessions
    pub active_sessions: Arc<RwLock<usize>>,
}

/// Lightweight snapshot for API serialization
#[derive(Debug, Clone, Serialize)]
pub struct ConsensusMetricsSnapshot {
    pub total_proposals: u64,
    pub successful_consensus: u64,
    pub failed_consensus: u64,
    pub view_changes: u64,
    pub avg_consensus_time_ms: f64,
    pub current_view: u64,
    pub active_sessions: usize,
    pub current_phase: String,
    pub current_leader: Option<String>,
    pub last_committed_block: u64,
}

impl ConsensusEngine {
    /// Create new consensus engine
    pub async fn new(config: Arc<GlobalSyncConfig>) -> GarpResult<Self> {
        let storage = Arc::new(crate::storage::MemoryGlobalStorage::new());
        let network_manager = Arc::new(NetworkManager::new(config.clone()).await?);
        
        let consensus_state = Arc::new(RwLock::new(ConsensusState {
            current_view: 0,
            current_phase: ConsensusPhase::Idle,
            current_leader: None,
            last_committed_block: 0,
            last_committed_hash: String::new(),
            view_change_in_progress: false,
            view_change_votes: HashMap::new(),
            last_updated: Instant::now(),
        }));
        
        let validator_set = Arc::new(RwLock::new(ValidatorSet {
            validators: HashMap::new(),
            total_voting_power: 0,
            byzantine_threshold: config.consensus.byzantine_threshold,
            required_votes: 0,
            last_updated: Instant::now(),
        }));
        
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let event_rx = Arc::new(Mutex::new(event_rx));
        
        let metrics = Arc::new(ConsensusMetrics {
            total_proposals: Arc::new(RwLock::new(0)),
            successful_consensus: Arc::new(RwLock::new(0)),
            failed_consensus: Arc::new(RwLock::new(0)),
            view_changes: Arc::new(RwLock::new(0)),
            avg_consensus_time: Arc::new(RwLock::new(0.0)),
            current_view: Arc::new(RwLock::new(0)),
            active_sessions: Arc::new(RwLock::new(0)),
        });
        
        Ok(Self {
            config,
            storage,
            network_manager,
            consensus_state,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            validator_set,
            message_queue: Arc::new(Mutex::new(VecDeque::new())),
            event_tx,
            event_rx,
            shutdown_tx: None,
            metrics,
        })
    }
    
    /// Start the consensus engine
    pub async fn start(&self) -> GarpResult<()> {
        info!("Starting Consensus Engine");
        
        // Initialize validator set
        self.initialize_validator_set().await?;
        
        // Start message processor
        let message_processor = self.start_message_processor().await?;
        
        // Start view change monitor
        let view_change_monitor = self.start_view_change_monitor().await?;
        
        // Start heartbeat sender
        let heartbeat_sender = self.start_heartbeat_sender().await?;
        
        // Start timeout monitor
        let timeout_monitor = self.start_timeout_monitor().await?;
        
        info!("Consensus Engine started successfully");
        Ok(())
    }
    
    /// Stop the consensus engine
    pub async fn stop(&self) -> GarpResult<()> {
        info!("Stopping Consensus Engine");
        
        // Send shutdown signal
        if let Some(shutdown_tx) = &self.shutdown_tx {
            let _ = shutdown_tx.send(());
        }
        
        info!("Consensus Engine stopped");
        Ok(())
    }
    
    /// Start consensus for a cross-domain transaction
    pub async fn start_consensus(&self, transaction: CrossDomainTransaction) -> GarpResult<()> {
        let session_id = Uuid::new_v4().to_string();
        let proposal_id = Uuid::new_v4().to_string();
        
        debug!("Starting consensus for transaction: {}", transaction.transaction_id);
        
        // Create proposal
        let proposal = ConsensusProposal {
            proposal_id: proposal_id.clone(),
            proposal_type: ProposalType::TransactionBatch(vec![transaction]),
            data: Vec::new(), // Serialized transaction data
            proposer_id: self.get_node_id().await,
            view: self.get_current_view().await,
            timestamp: chrono::Utc::now(),
            signature: Vec::new(), // TODO: Sign proposal
        };
        
        // Create consensus session
        let session = ConsensusSession {
            session_id: session_id.clone(),
            proposal: proposal.clone(),
            phase: ConsensusPhase::Prepare,
            view: self.get_current_view().await,
            votes: HashMap::new(),
            required_votes: self.get_required_votes().await,
            timeout_at: Instant::now() + self.config.consensus_timeout(),
            created_at: Instant::now(),
            last_activity: Instant::now(),
        };
        
        // Store session
        {
            let mut sessions = self.active_sessions.write().await;
            sessions.insert(session_id, session);
        }
        
        // Broadcast proposal
        self.broadcast_proposal(proposal).await?;
        
        // Update metrics
        {
            let mut total_proposals = self.metrics.total_proposals.write().await;
            *total_proposals += 1;
        }
        
        Ok(())
    }
    
    /// Vote on a block
    pub async fn vote_on_block(&self, block: GlobalBlock) -> GarpResult<()> {
        let proposal_id = block.header.block_id.clone();
        
        // Validate block
        let is_valid = self.validate_block(&block).await?;
        
        // Create vote
        let vote = ConsensusVote {
            voter_id: self.get_node_id().await,
            proposal_id,
            vote_type: VoteType::Prepare,
            vote: is_valid,
            reason: if is_valid { None } else { Some("Block validation failed".to_string()) },
            view: self.get_current_view().await,
            timestamp: chrono::Utc::now(),
            signature: Vec::new(), // TODO: Sign vote
        };
        
        // Broadcast vote
        self.broadcast_vote(vote).await?;
        
        Ok(())
    }
    
    /// Get required votes for consensus
    pub async fn get_required_votes(&self) -> usize {
        let validator_set = self.validator_set.read().await;
        validator_set.required_votes
    }
    
    /// Get current view
    pub async fn get_current_view(&self) -> u64 {
        let state = self.consensus_state.read().await;
        state.current_view
    }

    /// Get metrics snapshot for API
    pub async fn get_metrics_snapshot(&self) -> GarpResult<ConsensusMetricsSnapshot> {
        let state = self.consensus_state.read().await;
        Ok(ConsensusMetricsSnapshot {
            total_proposals: *self.metrics.total_proposals.read().await,
            successful_consensus: *self.metrics.successful_consensus.read().await,
            failed_consensus: *self.metrics.failed_consensus.read().await,
            view_changes: *self.metrics.view_changes.read().await,
            avg_consensus_time_ms: *self.metrics.avg_consensus_time.read().await,
            current_view: state.current_view,
            active_sessions: *self.metrics.active_sessions.read().await,
            current_phase: format!("{:?}", state.current_phase),
            current_leader: state.current_leader.as_ref().map(|p| p.0.clone()),
            last_committed_block: state.last_committed_block,
        })
    }
    
    /// Get node ID
    async fn get_node_id(&self) -> ParticipantId {
        ParticipantId::new(self.config.node.node_id.clone())
    }
    
    /// Initialize validator set
    async fn initialize_validator_set(&self) -> GarpResult<()> {
        let mut validator_set = self.validator_set.write().await;
        
        // Add initial validators from config
        for peer in &self.config.consensus.cluster_peers {
            let validator = ValidatorInfo {
                validator_id: ParticipantId::new(peer.clone()),
                public_key: Vec::new(), // TODO: Load from config
                voting_power: 1,
                status: ValidatorStatus::Active,
                endpoint: peer.clone(),
                last_seen: Instant::now(),
                metadata: HashMap::new(),
            };
            
            validator_set.validators.insert(validator.validator_id.clone(), validator);
            validator_set.total_voting_power += 1;
        }
        
        // Calculate required votes (2f+1 for BFT)
        let total_validators = validator_set.validators.len();
        validator_set.required_votes = (2 * validator_set.byzantine_threshold) + 1;
        
        info!("Initialized validator set with {} validators", total_validators);
        Ok(())
    }
    
    /// Validate block
    async fn validate_block(&self, block: &GlobalBlock) -> GarpResult<bool> {
        // Basic validation
        if block.header.height == 0 {
            return Ok(false);
        }
        
        // Validate transactions
        for transaction in &block.transactions {
            if !self.validate_transaction(transaction).await? {
                return Ok(false);
            }
        }
        
        // Validate block hash
        let calculated_hash = self.calculate_block_hash(block).await?;
        if calculated_hash != block.header.block_hash {
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// Validate cross-domain transaction
    async fn validate_transaction(&self, transaction: &CrossDomainTransaction) -> GarpResult<bool> {
        debug!("Validating cross-domain transaction: {:?}", transaction.transaction_id);
        
        // Basic validation checks
        if transaction.target_domains.is_empty() {
            warn!("Invalid transaction: no target domains specified");
            return Ok(false);
        }
        
        // Check if source domain is in target domains (should not be)
        if transaction.target_domains.contains(&transaction.source_domain) {
            warn!("Invalid transaction: source domain cannot be in target domains");
            return Ok(false);
        }
        
        // Check if transaction is not expired (using timeout_at if available)
        if let Ok(timeout) = transaction.timeout_at.duration_since(std::time::UNIX_EPOCH) {
            let timeout_chrono = chrono::DateTime::<chrono::Utc>::from_timestamp(
                timeout.as_secs() as i64, 
                timeout.subsec_nanos()
            );
            if let Some(timeout_dt) = timeout_chrono {
                if timeout_dt < chrono::Utc::now() {
                    warn!("Transaction expired: {:?}", transaction.transaction_id);
                    return Ok(false);
                }
            }
        }
        
        // Validate transaction data is not empty
        if transaction.data.is_empty() {
            warn!("Invalid transaction: empty transaction data");
            return Ok(false);
        }
        
        // Check minimum required confirmations
        let validator_set = self.validator_set.read().await;
        let required_confirmations = (validator_set.validators.len() * 2) / 3 + 1; // 2/3 + 1 majority
        if transaction.required_confirmations < required_confirmations {
            warn!("Insufficient required confirmations: got {}, minimum {}", 
                  transaction.required_confirmations, required_confirmations);
            return Ok(false);
        }
        
        // Validate domain-specific constraints
        if !self.validate_domain_constraints(transaction).await? {
            warn!("Domain constraints validation failed for transaction: {:?}", transaction.transaction_id);
            return Ok(false);
        }
        
        debug!("Transaction validation successful: {:?}", transaction.transaction_id);
        Ok(true)
    }
    
    /// Validate domain-specific constraints for cross-domain transactions
    async fn validate_domain_constraints(&self, transaction: &CrossDomainTransaction) -> GarpResult<bool> {
        debug!("Validating domain constraints for transaction: {:?}", transaction.transaction_id);
        
        // Validate source domain exists and is active
        if !self.is_domain_active(&transaction.source_domain.to_string()).await? {
            warn!("Source domain is not active: {:?}", transaction.source_domain);
            return Ok(false);
        }
        
        // Validate all target domains exist and are active
        for target_domain in &transaction.target_domains {
            if !self.is_domain_active(&target_domain.to_string()).await? {
                warn!("Target domain is not active: {:?}", target_domain);
                return Ok(false);
            }
        }
        
        // Validate transaction type is supported
        match transaction.transaction_type {
            CrossDomainTransactionType::AssetTransfer { amount, .. } => {
                if amount == 0 {
                    warn!("Invalid asset transfer amount: {}", amount);
                    return Ok(false);
                }
                // Check if amount exceeds domain limits
                if amount > 1_000_000 { // Example limit
                    warn!("Asset transfer amount exceeds domain limit: {}", amount);
                    return Ok(false);
                }
            }
            CrossDomainTransactionType::ContractCall { contract_address, .. } => {
                if contract_address.is_empty() {
                    warn!("Invalid contract call: empty contract address");
                    return Ok(false);
                }
            }
            CrossDomainTransactionType::StateSynchronization { state_key, .. } => {
                if state_key.is_empty() {
                    warn!("Invalid state synchronization: empty state key");
                    return Ok(false);
                }
            }
            CrossDomainTransactionType::AtomicSwap { amount_a, amount_b, .. } => {
                if amount_a == 0 || amount_b == 0 {
                    warn!("Invalid atomic swap amounts: {} <-> {}", amount_a, amount_b);
                    return Ok(false);
                }
            }
            CrossDomainTransactionType::GovernanceProposal { proposal_id, .. } => {
                if proposal_id.is_empty() {
                    warn!("Invalid governance proposal: empty proposal ID");
                    return Ok(false);
                }
            }
            CrossDomainTransactionType::EmergencyAction { action_type, justification, .. } => {
                if action_type.is_empty() || justification.is_empty() {
                    warn!("Invalid emergency action: missing type or justification");
                    return Ok(false);
                }
            }
        }
        
        // Check transaction data size limits
        if transaction.data.len() > 1_048_576 { // 1MB limit
            warn!("Transaction data exceeds size limit: {} bytes", transaction.data.len());
            return Ok(false);
        }
        
        debug!("Domain constraints validation passed for transaction: {:?}", transaction.transaction_id);
        Ok(true)
    }
    
    /// Check if a domain is active and participating in consensus
    async fn is_domain_active(&self, domain_id: &str) -> GarpResult<bool> {
        // In a real implementation, this would check with the domain registry
        // For now, we'll assume all domains are active except for a blacklist
        let inactive_domains = vec!["test_domain", "deprecated_domain"];
        Ok(!inactive_domains.contains(&domain_id))
    }
    
    /// Calculate block hash using SHA-256
    async fn calculate_block_hash(&self, block: &GlobalBlock) -> GarpResult<String> {
        debug!("Calculating hash for block: {}", block.header.height);
        
        // Create a deterministic representation of the block for hashing
        let hash_input = BlockHashInput {
            height: block.header.height,
            previous_hash: block.header.previous_hash.clone(),
            timestamp: block.header.timestamp,
            merkle_root: block.header.merkle_root.clone(),
            transactions: block.transactions.iter().map(|tx| tx.transaction_id.clone()).collect(),
            validator_signatures: block.signatures.len(),
        };
        
        // Serialize the hash input
        let serialized = bincode::serialize(&hash_input)
            .map_err(|e| GarpError::Internal(format!("Block serialization failed: {}", e)))?;
        
        // Get crypto service from storage and calculate hash
        match self.storage.get_crypto_service().await {
            Ok(crypto_service) => {
                let hash_bytes = crypto_service.hash(&serialized);
                let hash_hex = hex::encode(hash_bytes);
                debug!("Calculated block hash: {}", hash_hex);
                Ok(hash_hex)
            }
            Err(_) => {
                // Fallback to simple hash calculation
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                
                let mut hasher = DefaultHasher::new();
                hash_input.hash(&mut hasher);
                let hash = hasher.finish();
                let hash_hex = format!("{:016x}", hash);
                debug!("Calculated fallback block hash: {}", hash_hex);
                Ok(hash_hex)
            }
        }
    }
    
    /// Broadcast proposal
    async fn broadcast_proposal(&self, proposal: ConsensusProposal) -> GarpResult<()> {
        let message = ConsensusMessage {
            message_id: Uuid::new_v4().to_string(),
            message_type: ConsensusMessageType::Proposal(proposal),
            sender_id: self.get_node_id().await,
            view: self.get_current_view().await,
            timestamp: chrono::Utc::now(),
            signature: Vec::new(), // TODO: Sign message
        };
        
        self.network_manager.broadcast_consensus_message(message).await?;
        Ok(())
    }
    
    /// Broadcast vote
    async fn broadcast_vote(&self, vote: ConsensusVote) -> GarpResult<()> {
        let message = ConsensusMessage {
            message_id: Uuid::new_v4().to_string(),
            message_type: ConsensusMessageType::Vote(vote),
            sender_id: self.get_node_id().await,
            view: self.get_current_view().await,
            timestamp: chrono::Utc::now(),
            signature: Vec::new(), // TODO: Sign message
        };
        
        self.network_manager.broadcast_consensus_message(message).await?;
        Ok(())
    }
    
    /// Start message processor
    async fn start_message_processor(&self) -> GarpResult<tokio::task::JoinHandle<()>> {
        let event_rx = self.event_rx.clone();
        let active_sessions = self.active_sessions.clone();
        let consensus_state = self.consensus_state.clone();
        let validator_set = self.validator_set.clone();
        let metrics = self.metrics.clone();
        
        let handle = tokio::spawn(async move {
            let mut event_rx = event_rx.lock().await;
            
            while let Some(event) = event_rx.recv().await {
                match event {
                    ConsensusEvent::ProposalReceived(proposal) => {
                        Self::handle_proposal_received(
                            proposal,
                            &active_sessions,
                            &consensus_state,
                            &metrics,
                        ).await;
                    }
                    
                    ConsensusEvent::VoteReceived(vote) => {
                        Self::handle_vote_received(
                            vote,
                            &active_sessions,
                            &validator_set,
                            &metrics,
                        ).await;
                    }
                    
                    ConsensusEvent::ViewChangeInitiated(new_view) => {
                        Self::handle_view_change_initiated(
                            new_view,
                            &consensus_state,
                            &metrics,
                        ).await;
                    }
                    
                    ConsensusEvent::Shutdown => {
                        info!("Received shutdown signal in consensus message processor");
                        break;
                    }
                    
                    _ => {
                        debug!("Received consensus event: {:?}", event);
                    }
                }
            }
        });
        
        Ok(handle)
    }
    
    /// Handle proposal received
    async fn handle_proposal_received(
        proposal: ConsensusProposal,
        active_sessions: &Arc<RwLock<HashMap<String, ConsensusSession>>>,
        consensus_state: &Arc<RwLock<ConsensusState>>,
        metrics: &Arc<ConsensusMetrics>,
    ) {
        debug!("Handling proposal: {}", proposal.proposal_id);
        
        // Create or update session
        let session_id = proposal.proposal_id.clone();
        let session = ConsensusSession {
            session_id: session_id.clone(),
            proposal: proposal.clone(),
            phase: ConsensusPhase::Prepare,
            view: proposal.view,
            votes: HashMap::new(),
            required_votes: 3, // TODO: Get from validator set
            timeout_at: Instant::now() + Duration::from_secs(30),
            created_at: Instant::now(),
            last_activity: Instant::now(),
        };
        
        {
            let mut sessions = active_sessions.write().await;
            sessions.insert(session_id, session);
        }
        
        // Update metrics
        {
            let mut active_sessions_count = metrics.active_sessions.write().await;
            *active_sessions_count = active_sessions.read().await.len();
        }
    }
    
    /// Handle vote received
    async fn handle_vote_received(
        vote: ConsensusVote,
        active_sessions: &Arc<RwLock<HashMap<String, ConsensusSession>>>,
        validator_set: &Arc<RwLock<ValidatorSet>>,
        metrics: &Arc<ConsensusMetrics>,
    ) {
        debug!("Handling vote from {}: {}", vote.voter_id, vote.vote);
        
        let mut sessions = active_sessions.write().await;
        if let Some(session) = sessions.get_mut(&vote.proposal_id) {
            // Add vote to session
            session.votes.insert(vote.voter_id.clone(), vote.clone());
            session.last_activity = Instant::now();
            
            // Check if consensus reached
            let validator_set = validator_set.read().await;
            let required_votes = validator_set.required_votes;
            
            let approve_votes = session.votes.values().filter(|v| v.vote).count();
            let reject_votes = session.votes.values().filter(|v| !v.vote).count();
            
            if approve_votes >= required_votes {
                // Consensus reached - approved
                info!("Consensus reached for proposal: {} (approved)", vote.proposal_id);
                
                // Update metrics
                {
                    let mut successful = metrics.successful_consensus.write().await;
                    *successful += 1;
                }
            } else if reject_votes >= required_votes {
                // Consensus reached - rejected
                info!("Consensus reached for proposal: {} (rejected)", vote.proposal_id);
                
                // Update metrics
                {
                    let mut failed = metrics.failed_consensus.write().await;
                    *failed += 1;
                }
            }
        }
    }
    
    /// Handle view change initiated
    async fn handle_view_change_initiated(
        new_view: u64,
        consensus_state: &Arc<RwLock<ConsensusState>>,
        metrics: &Arc<ConsensusMetrics>,
    ) {
        info!("View change initiated to view: {}", new_view);
        
        {
            let mut state = consensus_state.write().await;
            state.view_change_in_progress = true;
            state.current_view = new_view;
            state.last_updated = Instant::now();
        }
        
        // Update metrics
        {
            let mut view_changes = metrics.view_changes.write().await;
            *view_changes += 1;
            
            let mut current_view = metrics.current_view.write().await;
            *current_view = new_view;
        }
    }
    
    /// Start view change monitor
    async fn start_view_change_monitor(&self) -> GarpResult<tokio::task::JoinHandle<()>> {
        let consensus_state = self.consensus_state.clone();
        let event_tx = self.event_tx.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));
            
            loop {
                interval.tick().await;
                
                // Check if view change is needed
                let needs_view_change = {
                    let state = consensus_state.read().await;
                    // TODO: Implement view change detection logic
                    false
                };
                
                if needs_view_change {
                    let new_view = {
                        let state = consensus_state.read().await;
                        state.current_view + 1
                    };
                    
                    if let Err(e) = event_tx.send(ConsensusEvent::ViewChangeInitiated(new_view)) {
                        error!("Failed to send view change event: {}", e);
                        break;
                    }
                }
            }
        });
        
        Ok(handle)
    }
    
    /// Start heartbeat sender
    async fn start_heartbeat_sender(&self) -> GarpResult<tokio::task::JoinHandle<()>> {
        let network_manager = self.network_manager.clone();
        let consensus_state = self.consensus_state.clone();
        let node_id = self.get_node_id().await;
        
        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                let (current_view, last_committed_block) = {
                    let state = consensus_state.read().await;
                    (state.current_view, state.last_committed_block)
                };
                
                let heartbeat = HeartbeatMessage {
                    current_view,
                    last_committed_block,
                    status: "active".to_string(),
                };
                
                let message = ConsensusMessage {
                    message_id: Uuid::new_v4().to_string(),
                    message_type: ConsensusMessageType::Heartbeat(heartbeat),
                    sender_id: node_id.clone(),
                    view: current_view,
                    timestamp: chrono::Utc::now(),
                    signature: Vec::new(),
                };
                
                if let Err(e) = network_manager.broadcast_consensus_message(message).await {
                    error!("Failed to send heartbeat: {}", e);
                }
            }
        });
        
        Ok(handle)
    }
    
    /// Start timeout monitor
    async fn start_timeout_monitor(&self) -> GarpResult<tokio::task::JoinHandle<()>> {
        let active_sessions = self.active_sessions.clone();
        let event_tx = self.event_tx.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(1));
            
            loop {
                interval.tick().await;
                
                let now = Instant::now();
                let mut timed_out_sessions = Vec::new();
                
                // Check for timed out sessions
                {
                    let sessions = active_sessions.read().await;
                    for (session_id, session) in sessions.iter() {
                        if now > session.timeout_at {
                            timed_out_sessions.push(session_id.clone());
                        }
                    }
                }
                
                // Handle timeouts
                for session_id in timed_out_sessions {
                    warn!("Consensus session timed out: {}", session_id);
                    
                    if let Err(e) = event_tx.send(ConsensusEvent::TimeoutOccurred(session_id)) {
                        error!("Failed to send timeout event: {}", e);
                    }
                }
            }
        });
        
        Ok(handle)
    }
}

impl ConsensusMetrics {
    /// Create new consensus metrics
    pub fn new() -> Self {
        Self {
            total_proposals: Arc::new(RwLock::new(0)),
            successful_consensus: Arc::new(RwLock::new(0)),
            failed_consensus: Arc::new(RwLock::new(0)),
            view_changes: Arc::new(RwLock::new(0)),
            avg_consensus_time: Arc::new(RwLock::new(0.0)),
            current_view: Arc::new(RwLock::new(0)),
            active_sessions: Arc::new(RwLock::new(0)),
        }
    }
    
    /// Get success rate
    pub async fn get_success_rate(&self) -> f64 {
        let successful = *self.successful_consensus.read().await;
        let total = *self.total_proposals.read().await;
        
        if total > 0 {
            successful as f64 / total as f64
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GlobalSyncConfig;
    
    #[tokio::test]
    async fn test_consensus_engine_creation() {
        let config = Arc::new(GlobalSyncConfig::default());
        let engine = ConsensusEngine::new(config).await;
        assert!(engine.is_ok());
    }
    
    #[tokio::test]
    async fn test_validator_set_initialization() {
        let config = Arc::new(GlobalSyncConfig::default());
        let engine = ConsensusEngine::new(config).await.unwrap();
        
        let result = engine.initialize_validator_set().await;
        assert!(result.is_ok());
        
        let validator_set = engine.validator_set.read().await;
        assert!(!validator_set.validators.is_empty());
    }
    
    #[tokio::test]
    async fn test_consensus_metrics() {
        let metrics = ConsensusMetrics::new();
        
        {
            let mut total = metrics.total_proposals.write().await;
            *total = 10;
        }
        
        {
            let mut successful = metrics.successful_consensus.write().await;
            *successful = 8;
        }
        
        let success_rate = metrics.get_success_rate().await;
        assert_eq!(success_rate, 0.8);
    }
}