use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::hash::Hash;
use tokio::sync::{RwLock, Mutex, mpsc, oneshot};
use tokio::time::{interval, timeout};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};
use ed25519_dalek::{SigningKey, Signer, Verifier, Signature, PublicKey};
use hex;
use garp_common::{ConsensusManager, ConsensusEngineType, ConsensusParams, ValidatorInfo, ValidatorStatus, EvidenceType};

// --- Canonicalization and signing helpers (module-level) ---
fn node_sign(message: &[u8]) -> Option<Vec<u8>> {
    let signer = std::env::var("SYNC_SIGNER").unwrap_or_else(|_| "env".to_string());
    match signer.as_str() {
        // Future: integrate KMS/Vault/HSM providers here
        // "vault" | "kms" | "hsm" => {
        //     tracing::warn!("External signer not yet configured; falling back to env");
        // }
        _ => {}
    }
    let sk_hex = std::env::var("SYNC_NODE_ED25519_SK_HEX").ok()?;
    let sk_bytes = match hex::decode(sk_hex) {
        Ok(b) => b,
        Err(e) => {
            error!("Invalid SYNC_NODE_ED25519_SK_HEX: {}", e);
            return None;
        }
    };
    let arr: [u8; 32] = match sk_bytes.try_into() {
        Ok(a) => a,
        Err(_) => {
            error!("SYNC_NODE_ED25519_SK_HEX must be 32 bytes hex");
            return None;
        }
    };
    let sk = SigningKey::from_bytes(&arr);
    let sig = sk.sign(message);
    Some(sig.to_bytes().to_vec())
}

fn canonical_proposal_message(p: &ConsensusProposal) -> Vec<u8> {
    let mut s = String::new();
    s.push_str(&p.proposal_id);
    s.push('|');
    s.push_str(&p.proposer_id.0);
    s.push('|');
    s.push_str(&p.view.to_string());
    s.push('|');
    s.push_str(&p.timestamp.timestamp_millis().to_string());
    s.push('|');
    match &p.proposal_type {
        ProposalType::Block(b) => {
            s.push_str(&b.header.height.to_string());
            s.push('|');
            s.push_str(&b.header.previous_hash.0);
            s.push('|');
            s.push_str(&b.header.merkle_root.0);
        }
        ProposalType::TransactionBatch(txs) => {
            s.push_str("batch:");
            s.push_str(&txs.len().to_string());
        }
        ProposalType::ValidatorSetChange(v) => {
            s.push_str("vset:");
            s.push_str(&v.effective_height.to_string());
        }
        ProposalType::ConfigurationChange(c) => {
            s.push_str("cfg:");
            s.push_str(&c.parameter);
            s.push('=');
            s.push_str(&c.value);
        }
        ProposalType::EmergencyAction(ea) => {
            s.push_str("emergency:");
            s.push_str(&ea.action_type.to_string());
        }
    }
    s.into_bytes()
}

fn canonical_vote_message(v: &ConsensusVote) -> Vec<u8> {
    let mut s = String::new();
    s.push_str(&v.voter_id.0);
    s.push('|');
    s.push_str(&v.proposal_id);
    s.push('|');
    s.push_str(&format!("{:?}", v.vote_type));
    s.push('|');
    s.push_str(if v.vote { "yes" } else { "no" });
    s.push('|');
    s.push_str(&v.view.to_string());
    s.push('|');
    s.push_str(&v.timestamp.timestamp_millis().to_string());
    s.into_bytes()
}

fn canonical_consensus_message(m: &ConsensusMessage) -> Vec<u8> {
    let mut s = String::new();
    s.push_str(&m.message_id);
    s.push('|');
    s.push_str(&m.sender_id.0);
    s.push('|');
    s.push_str(&m.view.to_string());
    s.push('|');
    s.push_str(&m.timestamp.timestamp_millis().to_string());
    s.push('|');
    s.push_str(match &m.message_type {
        ConsensusMessageType::Proposal(p) => {
            let mut inner = String::from("proposal:");
            inner.push_str(&p.proposal_id);
            inner
        }
        ConsensusMessageType::Vote(v) => {
            let mut inner = String::from("vote:");
            inner.push_str(&v.proposal_id);
            inner
        }
        ConsensusMessageType::ViewChange(vc) => {
            let mut inner = String::from("view_change:");
            inner.push_str(&vc.new_view.to_string());
            inner
        }
        ConsensusMessageType::NewView(nv) => {
            let mut inner = String::from("new_view:");
            inner.push_str(&nv.new_view.to_string());
            inner
        }
        ConsensusMessageType::Heartbeat(hb) => {
            let mut inner = String::from("heartbeat:");
            inner.push_str(&hb.current_view.to_string());
            inner
        }
        ConsensusMessageType::SyncRequest(sr) => {
            let mut inner = String::from("sync_req:");
            inner.push_str(&sr.start_height.to_string());
            inner
        }
        ConsensusMessageType::SyncResponse(_) => "sync_resp".to_string(),
    });
    s.into_bytes()
}

use garp_common::{GarpResult, GarpError};
use garp_common::crypto::CryptoService;
use garp_common::types::{TransactionId, ParticipantId};
use garp_common::consensus::{ValidationResult};

use crate::config::{GlobalSyncConfig, ConsensusAlgorithm};
use crate::cross_domain::{CrossDomainTransaction, CrossDomainTransactionType};
use crate::storage::{GlobalStorage, GlobalBlock, BlockHeader};
use crate::validator::{ValidatorInfo, ValidatorStatus};
use crate::network::NetworkManager;
use crate::network::InboundMessage;

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
    struct BlockHashInput {
        height: u64,
        previous_hash: String,
        timestamp: chrono::DateTime<chrono::Utc>,
        merkle_root: String,
        transactions: Vec<String>,
    }

/// BFT Consensus Engine for Global Synchronizer
pub struct ConsensusEngine {
    /// Configuration
    config: Arc<GlobalSyncConfig>,
    
    /// Storage layer
    storage: Arc<GlobalStorage>,
    
    /// Network manager
    network_manager: Arc<NetworkManager>,
    
    /// Current consensus state
    consensus_state: Arc<RwLock<ConsensusState>>,
    
    /// Active consensus sessions
    active_sessions: Arc<RwLock<HashMap<String, ConsensusSession>>>,
    
    /// Consensus manager with pluggable engines and validator management
    consensus_manager: Arc<ConsensusManager>,
    
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
        let storage = Arc::new(crate::storage::GlobalStorage::new(config.clone()).await?);
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
        
        // Initialize consensus manager with Tendermint consensus as default for BFT
        let consensus_params = ConsensusParams {
            timeout_seconds: config.consensus_timeout().as_secs(),
            threshold: 0.67,
            max_validators: 100,
            min_validators: 1,
            weighted_voting: true,
            require_unanimous: false,
            allow_abstain: false,
            max_concurrent_sessions: 1000,
            byzantine_threshold: config.consensus.byzantine_threshold,
            quorum_ratio_thousandths: 667,
            max_view_changes: 10,
            liveness_timeout_ms: 10000,
        };
        
        let consensus_manager = Arc::new(ConsensusManager::new(
            ParticipantId::new(config.node.node_id.clone()),
            ConsensusEngineType::Tendermint,
            consensus_params
        ));
        
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
            consensus_manager,
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

        // Register inbound consensus message handler with signature verification
        let validator_set = self.validator_set.clone();
        let active_sessions = self.active_sessions.clone();
        let consensus_state = self.consensus_state.clone();
        let metrics = self.metrics.clone();
        self.network_manager.register_message_handler(
            "consensus".to_string(),
            move |inbound: &InboundMessage| {
                let data = inbound.data.clone();
                let validator_set = validator_set.clone();
                let active_sessions = active_sessions.clone();
                let consensus_state = consensus_state.clone();
                let metrics = metrics.clone();
                tokio::spawn(async move {
                    // Parse consensus message
                    let parsed: Result<ConsensusMessage, serde_json::Error> = serde_json::from_slice(&data);
                    let message = match parsed {
                        Ok(m) => m,
                        Err(e) => {
                            error!("Invalid consensus message JSON: {}", e);
                            return;
                        }
                    };

                    // Resolve sender's public key
                    let sender_id = message.sender_id.clone();
                    let public_key_hex = {
                        let vs = validator_set.read().await;
                        vs.validators.get(&sender_id).map(|vi| vi.public_key_hex.clone())
                    };
                    let public_key_hex = match public_key_hex {
                        Some(h) => h,
                        None => {
                            warn!("Consensus message from unknown validator: {}", sender_id.0);
                            return;
                        }
                    };

                    // Build ed25519 public key
                    let pk_bytes = match hex::decode(&public_key_hex) {
                        Ok(b) => b,
                        Err(e) => {
                            warn!("Invalid validator public key hex: {}", e);
                            return;
                        }
                    };
                    let pk_arr: [u8; 32] = match pk_bytes.try_into() {
                        Ok(a) => a,
                        Err(_) => {
                            warn!("Validator public key length is not 32 bytes");
                            return;
                        }
                    };
                    let pk = match PublicKey::from_bytes(&pk_arr) {
                        Ok(pk) => pk,
                        Err(e) => {
                            warn!("Invalid ed25519 public key: {}", e);
                            return;
                        }
                    };

                    // Verify envelope signature
                    let env_sig_arr: [u8; 64] = match message.signature.clone().try_into() {
                        Ok(a) => a,
                        Err(_) => {
                            warn!("Consensus envelope signature length is not 64 bytes");
                            return;
                        }
                    };
                    let env_sig = match Signature::from_bytes(&env_sig_arr) {
                        Ok(s) => s,
                        Err(e) => {
                            warn!("Invalid envelope signature: {}", e);
                            return;
                        }
                    };
                    let env_bytes = canonical_consensus_message(&message);
                    if let Err(e) = pk.verify_strict(&env_bytes, &env_sig) {
                        warn!("Consensus envelope signature verification failed: {}", e);
                        return;
                    }

                    // Dispatch based on message type and verify inner signatures
                    match message.message_type {
                        ConsensusMessageType::Proposal(p) => {
                            if p.proposer_id != sender_id {
                                warn!("Proposal sender mismatch: envelope {} vs proposal {}", sender_id.0, p.proposer_id.0);
                                return;
                            }
                            let sig_arr: [u8; 64] = match p.signature.clone().try_into() {
                                Ok(a) => a,
                                Err(_) => {
                                    warn!("Proposal signature length is not 64 bytes");
                                    return;
                                }
                            };
                            let sig = match Signature::from_bytes(&sig_arr) {
                                Ok(s) => s,
                                Err(e) => {
                                    warn!("Invalid proposal signature: {}", e);
                                    return;
                                }
                            };
                            let msg_bytes = canonical_proposal_message(&p);
                            if let Err(e) = pk.verify_strict(&msg_bytes, &sig) {
                                warn!("Proposal signature verification failed: {}", e);
                                return;
                            }
                            Self::handle_proposal_received(p, &active_sessions, &consensus_state, &metrics, &validator_set).await;
                        }
                        ConsensusMessageType::Vote(v) => {
                            if v.voter_id != sender_id {
                                warn!("Vote sender mismatch: envelope {} vs vote {}", sender_id.0, v.voter_id.0);
                                return;
                            }
                            let sig_arr: [u8; 64] = match v.signature.clone().try_into() {
                                Ok(a) => a,
                                Err(_) => {
                                    warn!("Vote signature length is not 64 bytes");
                                    return;
                                }
                            };
                            let sig = match Signature::from_bytes(&sig_arr) {
                                Ok(s) => s,
                                Err(e) => {
                                    warn!("Invalid vote signature: {}", e);
                                    return;
                                }
                            };
                            let msg_bytes = canonical_vote_message(&v);
                            if let Err(e) = pk.verify_strict(&msg_bytes, &sig) {
                                warn!("Vote signature verification failed: {}", e);
                                return;
                            }
                            Self::handle_vote_received(v, &active_sessions, &consensus_state, &metrics).await;
                        }
                        other => {
                            debug!("Inbound consensus message ignored for now: {:?}", other);
                        }
                    }
                });
                Ok(())
            }
        ).await?;
        
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
        let mut vote = ConsensusVote {
            voter_id: self.get_node_id().await,
            proposal_id,
            vote_type: VoteType::Prepare,
            vote: is_valid,
            reason: if is_valid { None } else { Some("Block validation failed".to_string()) },
            view: self.get_current_view().await,
            timestamp: chrono::Utc::now(),
            signature: Vec::new(),
        };

        // Sign vote (end-to-end)
        let vote_bytes = canonical_vote_message(&vote);
        if let Some(sig) = node_sign(&vote_bytes) {
            vote.signature = sig;
        } else {
            warn!("No node signing key configured; broadcasting unsigned vote");
        }
        
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

    /// List current validators
    pub async fn list_validators(&self) -> GarpResult<Vec<ValidatorInfo>> {
        Ok(self.consensus_manager.get_active_validators().await)
    }

    /// Add a validator to the set
    pub async fn add_validator(&self, v: ValidatorInfo) -> GarpResult<()> {
        self.consensus_manager.add_validator(v).await
    }

    /// Remove a validator from the set
    pub async fn remove_validator(&self, id: ParticipantId) -> GarpResult<()> {
        self.consensus_manager.remove_validator(&id).await
    }

    /// Update a validator's status
    pub async fn update_validator_status(&self, id: ParticipantId, status: ValidatorStatus) -> GarpResult<()> {
        self.consensus_manager.update_validator_status(&id, status).await
    }

    fn calculate_required_votes(total_voting_power: u64) -> usize {
        ((total_voting_power as f64 * 0.67).ceil() as u64) as usize
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
                id: ParticipantId::new(peer.clone()),
                public_key_hex: String::new(), // TODO: Load from config
                voting_power: 1,
                status: ValidatorStatus::Active,
                joined_at: chrono::Utc::now(),
                metadata: HashMap::new(),
            };
            
            validator_set.validators.insert(validator.id.clone(), validator);
            validator_set.total_voting_power += 1;
        }
        
        // Calculate required votes using quorum params over validator count
        let total_validators = validator_set.validators.len();
        validator_set.required_votes = self.config.params.required_votes(total_validators);
        
        info!("Initialized validator set with {} validators", total_validators);
        Ok(())
    }
    
    /// Validate block
    async fn validate_block(&self, block: &GlobalBlock) -> GarpResult<bool> {
        // Basic validation
        if block.header.slot == 0 {
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
        if calculated_hash != hex::encode(&block.hash) {
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
        let required_confirmations = self.consensus_manager.get_required_votes().await;
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
        debug!("Calculating hash for block: epoch={}, slot={}", block.header.epoch, block.header.slot);
        
        // Create a deterministic representation of the block for hashing
        let hash_input = BlockHashInput {
            height: block.header.slot,
            previous_hash: hex::encode(&block.header.parent_hash),
            timestamp: block.timestamp,
            merkle_root: hex::encode(&block.header.tx_root),
            transactions: block.transactions.iter().map(|tx| tx.id.0.to_string()).collect(),
        };
        
        // Serialize the hash input
        let serialized = bincode::serialize(&hash_input)
            .map_err(|e| GarpError::Internal(format!("Block serialization failed: {}", e)))?;
        
        // Calculate hash using the common CryptoService (SHA-256)
        let crypto_service = CryptoService::new();
        let hash_bytes = crypto_service.hash(&serialized);
        let hash_hex = hex::encode(hash_bytes);
        debug!("Calculated block hash: {}", hash_hex);
        Ok(hash_hex)
    }
    
    /// Broadcast proposal
    async fn broadcast_proposal(&self, proposal: ConsensusProposal) -> GarpResult<()> {
        // Sign proposal itself
        let mut proposal = proposal;
        let proposal_bytes = canonical_proposal_message(&proposal);
        if let Some(sig) = node_sign(&proposal_bytes) {
            proposal.signature = sig;
        } else {
            warn!("No node signing key configured; broadcasting unsigned proposal");
        }

        let mut message = ConsensusMessage {
            message_id: Uuid::new_v4().to_string(),
            message_type: ConsensusMessageType::Proposal(proposal),
            sender_id: self.get_node_id().await,
            view: self.get_current_view().await,
            timestamp: chrono::Utc::now(),
            signature: Vec::new(),
        };

        // Sign envelope
        let envelope_bytes = canonical_consensus_message(&message);
        if let Some(sig) = node_sign(&envelope_bytes) {
            message.signature = sig;
        }
        
        self.network_manager.broadcast_consensus_message(message).await?;
        Ok(())
    }
    
    /// Broadcast vote
    async fn broadcast_vote(&self, vote: ConsensusVote) -> GarpResult<()> {
        let mut message = ConsensusMessage {
            message_id: Uuid::new_v4().to_string(),
            message_type: ConsensusMessageType::Vote(vote),
            sender_id: self.get_node_id().await,
            view: self.get_current_view().await,
            timestamp: chrono::Utc::now(),
            signature: Vec::new(),
        };

        // Sign envelope
        let envelope_bytes = canonical_consensus_message(&message);
        if let Some(sig) = node_sign(&envelope_bytes) {
            message.signature = sig;
        } else {
            warn!("No node signing key configured; broadcasting unsigned vote envelope");
        }
        
        self.network_manager.broadcast_consensus_message(message).await?;
        Ok(())
    }
    
    /// Start message processor
    async fn start_message_processor(&self) -> GarpResult<tokio::task::JoinHandle<()>> {
        let event_rx = self.event_rx.clone();
        let event_tx = self.event_tx.clone();
        let active_sessions = self.active_sessions.clone();
        let consensus_state = self.consensus_state.clone();
        let validator_set = self.validator_set.clone();
        let storage = self.storage.clone();
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
                            &validator_set,
                        ).await;
                    }
                    
                    ConsensusEvent::VoteReceived(vote) => {
                        Self::handle_vote_received(
                            vote,
                            &active_sessions,
                            &validator_set,
                            &metrics,
                        ).await;

                        // After processing the vote, check if approval consensus was reached
                        // and persist a finality certificate for block proposals.
                        {
                            let sessions = active_sessions.read().await;
                            if let Some(session) = sessions.get(&vote.proposal_id) {
                                let approve_votes = session
                                    .votes
                                    .values()
                                    .filter(|v| v.vote)
                                    .count();
                                let required_votes = session.required_votes;

                                if approve_votes >= required_votes {
                                    if let ProposalType::Block(block) = &session.proposal.proposal_type {
                                        let height = block.header.slot;
                                        let block_hash_hex = hex::encode(&block.hash);

                                        let signatures = session
                                            .votes
                                            .values()
                                            .filter(|v| v.vote)
                                            .map(|v| (v.voter_id.clone(), v.signature.clone()))
                                            .collect::<Vec<(ParticipantId, Vec<u8>)>>();

                                        let vset_power = {
                                            let vs = validator_set.read().await;
                                            vs.total_voting_power
                                        };
                                        let vset_hash = format!(
                                            "vset-{:#x}",
                                            blake3::hash(vset_power.to_le_bytes().as_slice())
                                        );

                                        let certificate = FinalityCertificate {
                                            height,
                                            block_hash: block_hash_hex.clone(),
                                            signatures,
                                            validator_set_hash: vset_hash,
                                            timestamp: chrono::Utc::now(),
                                        };

                                        if let Err(e) = storage.store_finality_certificate(certificate).await {
                                            error!(
                                                "Failed to store finality certificate for block {}: {}",
                                                block_hash_hex,
                                                e
                                            );
                                        } else {
                                            info!(
                                                "Stored finality certificate for block {} at height {}",
                                                block_hash_hex,
                                                height
                                            );

                                            // Emit a ConsensusReached event with minimal proof data for block approvals
                                            let approving_votes: Vec<ConsensusVote> = session
                                                .votes
                                                .values()
                                                .filter(|v| v.vote)
                                                .cloned()
                                                .collect();
                                            let validators: Vec<ParticipantId> = approving_votes
                                                .iter()
                                                .map(|v| v.voter_id.clone())
                                                .collect();
                                            let proof = ConsensusProof {
                                                proposal_id: session.proposal.proposal_id.clone(),
                                                view: session.view,
                                                votes: approving_votes,
                                                aggregated_signature: Vec::new(),
                                            };
                                            let result = ConsensusResult {
                                                transaction_id: format!("block:{}", block_hash_hex),
                                                approved: true,
                                                proof,
                                                validators,
                                                finalized_at: Instant::now(),
                                            };
                                            if let Err(e) = event_tx.send(ConsensusEvent::ConsensusReached(result)) {
                                                warn!("Failed to emit ConsensusReached event: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
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
        validator_set: &Arc<RwLock<ValidatorSet>>,
    ) {
        debug!("Handling proposal: {}", proposal.proposal_id);
        
        // Create or update session
        let session_id = proposal.proposal_id.clone();
        // Fetch current required votes from validator set
        let rv = {
            let vs = validator_set.read().await;
            vs.required_votes
        };

        let session = ConsensusSession {
            session_id: session_id.clone(),
            proposal: proposal.clone(),
            phase: ConsensusPhase::Prepare,
            view: proposal.view,
            votes: HashMap::new(),
            required_votes: rv,
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
            let required_votes = validator_set.get_required_votes().await;
            
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
                
                // Record successful proposal for validator
                let _ = validator_set.record_successful_proposal(&vote.voter_id).await;
            } else if reject_votes >= required_votes {
                // Consensus reached - rejected
                info!("Consensus reached for proposal: {} (rejected)", vote.proposal_id);
                
                // Update metrics
                {
                    let mut failed = metrics.failed_consensus.write().await;
                    *failed += 1;
                }
                
                // Record failed proposal for validator
                let _ = validator_set.record_failed_proposal(&vote.voter_id).await;
            } else {
                // Record vote for validator
                let _ = validator_set.record_missed_vote(&vote.voter_id).await;
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
// -----------------------------------------------------------------------------
// Consensus protocol specification, finality, evidence, and slashing scaffolding
// -----------------------------------------------------------------------------

/// Supported consensus protocol flavors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConsensusProtocol {
    /// Raft-style leader-based consensus (non-Byzantine, crash-fault tolerant)
    Raft,
    /// Tendermint-like BFT (Byzantine-fault tolerant, with votes and commits)
    TendermintLike,
}

/// Parameters that influence quorum, view changes, and liveness.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConsensusParams {
    pub protocol: ConsensusProtocol,
    /// Quorum ratio in thousandths (e.g., 667 => 2/3 supermajority)
    pub quorum_ratio_thousandths: u16,
    /// Maximum consecutive view changes before declaring an epoch incident
    pub max_view_changes: u32,
    /// Liveness timeout in milliseconds to trigger a view change
    pub liveness_timeout_ms: u64,
}

/// DoS/backpressure limits for the consensus networking layer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConsensusNetworkLimits {
    pub max_message_rate_per_sec: u32,
    pub max_peer_connections: u32,
    pub max_inflight_votes: u32,
}

/// Finality certificate proving a block reached the required threshold.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FinalityCertificate {
    pub height: u64,
    pub block_hash: String,
    pub signatures: Vec<(garp_common::types::ParticipantId, Vec<u8>)>,
    pub validator_set_hash: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Evidence for slashing conditions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum EvidenceType {
    Equivocation,   // Conflicting votes for the same height/view
    DoubleSign,     // Two different blocks signed at same height/view
    LivenessFault,  // Missed N consecutive rounds beyond policy threshold
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Evidence {
    pub validator: garp_common::types::ParticipantId,
    pub evidence_type: EvidenceType,
    pub details: String,
    pub height: u64,
    pub view: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Slashing policy parameters.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SlashingPolicy {
    pub double_sign_penalty_bp: u32, // basis points of stake/voting power
    pub equivocation_penalty_bp: u32,
    pub liveness_penalty_bp: u32,
    pub jail_duration_secs: u64,
}

impl ConsensusParams {
    /// Calculate required votes for a given validator set size.
    pub fn required_votes(&self, total_validators: usize) -> usize {
        // quorum_ratio_thousandths e.g., 667 => ceil(2/3 * N)
        let num = (self.quorum_ratio_thousandths as usize) * total_validators;
        let denom = 1000usize;
        (num + denom - 1) / denom
    }
}

impl ConsensusEngine {
    /// Returns required votes based on provided params and current validator set.
    pub fn compute_required_votes_with(&self, params: &ConsensusParams) -> usize {
        let total = self.validator_set.validators.len();
        params.required_votes(total)
    }

    /// Suggests whether a view change should occur given inactivity and params.
    pub fn should_view_change(&self, params: &ConsensusParams, inactive_ms: u64, consecutive_view_changes: u32) -> bool {
        inactive_ms >= params.liveness_timeout_ms || consecutive_view_changes >= params.max_view_changes
    }

    /// Submits evidence to be adjudicated; returns whether it was accepted.
    pub async fn submit_evidence(&self, evidence: Evidence) -> garp_common::GarpResult<bool> {
        // TODO: persist evidence, broadcast to peers, and throttle per limits
        // For now, accept all syntactically valid evidence.
        let _ = evidence;
        Ok(true)
    }

    /// Applies slashing according to policy; updates validator status and power.
    pub async fn adjudicate_evidence(&self, evidence: Evidence, policy: SlashingPolicy) -> garp_common::GarpResult<()> {
        // Convert policy penalties to evidence type
        let penalty_bp = match evidence.evidence_type {
            EvidenceType::DoubleSign => policy.double_sign_penalty_bp,
            EvidenceType::Equivocation => policy.equivocation_penalty_bp,
            EvidenceType::LivenessFault => policy.liveness_penalty_bp,
            EvidenceType::InvalidBehavior => 15, // Default penalty for invalid behavior
        };
        
        // Apply slashing through consensus manager
        self.consensus_manager
            .apply_slashing(
                &evidence.validator, 
                evidence.evidence_type, 
                penalty_bp, 
                evidence.details
            )
            .await
    }

    /// Forges a finality certificate from collected signatures for a block.
    pub fn forge_finality_certificate(&self, height: u64, block_hash: String, sigs: Vec<(garp_common::types::ParticipantId, Vec<u8>)>) -> FinalityCertificate {
        let vset_hash = format!("vset-{:#x}", blake3::hash(self.validator_set.total_voting_power.to_le_bytes().as_slice()));
        FinalityCertificate {
            height,
            block_hash,
            signatures: sigs,
            validator_set_hash: vset_hash,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Simple fork-choice: prefer chain with higher height, then by certificate signatures count.
    pub fn fork_choice(&self, current: &FinalityCertificate, candidate: &FinalityCertificate) -> &FinalityCertificate {
        if candidate.height > current.height { return candidate; }
        if candidate.height < current.height { return current; }
        if candidate.signatures.len() >= current.signatures.len() { candidate } else { current }
    }
}