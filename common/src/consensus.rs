use crate::types::{ParticipantId, TransactionId, ValidationResult};
use crate::error::{ConsensusError, GarpResult};
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use async_trait::async_trait;
use crate::types::{BlockHeader, Block};
use std::sync::Arc;

/// Consensus proposal for transaction validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusProposal {
    pub proposal_id: Uuid,
    pub proposer: ParticipantId,
    pub transactions: Vec<TransactionId>,
    pub stakeholders: Vec<ParticipantId>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Vote on a consensus proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusVote {
    pub proposal_id: Uuid,
    pub voter: ParticipantId,
    pub vote: VoteType,
    pub reason: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Types of votes in consensus
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VoteType {
    Approve,
    Reject,
    Abstain,
}

/// Result of a consensus round
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusResult {
    pub proposal_id: Uuid,
    pub outcome: ConsensusOutcome,
    pub votes: Vec<ConsensusVote>,
    pub finalized_at: DateTime<Utc>,
}

/// Possible outcomes of consensus
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConsensusOutcome {
    Approved,
    Rejected,
    Timeout,
    InsufficientVotes,
}

/// Two-phase commit coordinator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoPhaseCommit {
    pub transaction_id: TransactionId,
    pub coordinator: ParticipantId,
    pub participants: Vec<ParticipantId>,
    pub phase: CommitPhase,
    pub votes: HashMap<ParticipantId, PhaseVote>,
    pub created_at: DateTime<Utc>,
    pub timeout_at: DateTime<Utc>,
}

/// Phases of two-phase commit
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CommitPhase {
    Prepare,
    Commit,
    Abort,
    Completed,
}

/// Vote in two-phase commit protocol
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PhaseVote {
    Prepared,
    NotPrepared,
    Committed,
    Aborted,
}

/// Consensus engine trait
#[async_trait]
pub trait ConsensusEngine: Send + Sync {
    /// Submit a proposal for consensus
    async fn submit_proposal(&self, proposal: ConsensusProposal) -> GarpResult<()>;

    /// Cast a vote on a proposal
    async fn cast_vote(&self, vote: ConsensusVote) -> GarpResult<()>;

    /// Get the result of a consensus round
    async fn get_consensus_result(&self, proposal_id: Uuid) -> GarpResult<Option<ConsensusResult>>;

    /// Start a two-phase commit
    async fn start_two_phase_commit(&self, commit: TwoPhaseCommit) -> GarpResult<()>;

    /// Vote in two-phase commit
    async fn vote_two_phase_commit(
        &self,
        transaction_id: TransactionId,
        participant: ParticipantId,
        vote: PhaseVote,
    ) -> GarpResult<()>;

    /// Get two-phase commit status
    async fn get_two_phase_commit_status(&self, transaction_id: TransactionId) -> GarpResult<Option<TwoPhaseCommit>>;

    /// Get consensus engine type
    fn engine_type(&self) -> ConsensusEngineType;

    /// Get consensus parameters
    fn get_params(&self) -> ConsensusParams;
}

/// Consensus engine types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConsensusEngineType {
    ProofOfStakeholder,
    Raft,
    Tendermint,
    HotStuff,
    PracticalBFT,
    Pbft,
    HoneyBadgerBft,
    Streamlet,
}

/// Consensus parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusParams {
    /// Consensus timeout in seconds
    pub timeout_seconds: u64,
    
    /// Consensus threshold (0.0 - 1.0)
    pub threshold: f64,
    
    /// Maximum number of validators
    pub max_validators: usize,
    
    /// Minimum number of validators
    pub min_validators: usize,
    
    /// Enable weighted voting
    pub weighted_voting: bool,
    
    /// Require unanimous consensus
    pub require_unanimous: bool,
    
    /// Allow abstain votes
    pub allow_abstain: bool,
    
    /// Maximum concurrent sessions
    pub max_concurrent_sessions: usize,
    
    /// Byzantine fault tolerance threshold
    pub byzantine_threshold: usize,
    
    /// Quorum ratio in thousandths (e.g., 667 => 2/3 supermajority)
    pub quorum_ratio_thousandths: u16,
    
    /// Maximum consecutive view changes before declaring an epoch incident
    pub max_view_changes: u32,
    
    /// Liveness timeout in milliseconds to trigger a view change
    pub liveness_timeout_ms: u64,
}

impl Default for ConsensusParams {
    fn default() -> Self {
        Self {
            timeout_seconds: 30,
            threshold: 0.67,
            max_validators: 100,
            min_validators: 1,
            weighted_voting: false,
            require_unanimous: false,
            allow_abstain: false,
            max_concurrent_sessions: 1000,
            byzantine_threshold: 1,
            quorum_ratio_thousandths: 667,
            max_view_changes: 10,
            liveness_timeout_ms: 10000,
        }
    }
}

/// Consensus factory for creating different consensus engines
pub struct ConsensusFactory;

impl ConsensusFactory {
    /// Create a consensus engine based on type
    pub fn create_engine(
        engine_type: ConsensusEngineType,
        participant_id: ParticipantId,
        params: ConsensusParams,
    ) -> Arc<dyn ConsensusEngine> {
        match engine_type {
            ConsensusEngineType::ProofOfStakeholder => {
                Arc::new(ProofOfStakeholderConsensus::new(participant_id, params.threshold))
            }
            ConsensusEngineType::Raft => {
                Arc::new(RaftConsensus::new(participant_id, params))
            }
            ConsensusEngineType::Tendermint => {
                Arc::new(TendermintConsensus::new(participant_id, params))
            }
            ConsensusEngineType::HotStuff => {
                Arc::new(HotStuffConsensus::new(participant_id, params))
            }
            ConsensusEngineType::PracticalBFT => {
                Arc::new(PracticalBFTConsensus::new(participant_id, params))
            }
            ConsensusEngineType::Pbft => {
                Arc::new(PbftConsensus::new(participant_id, params))
            }
            ConsensusEngineType::HoneyBadgerBft => {
                Arc::new(HoneyBadgerBftConsensus::new(participant_id, params))
            }
            ConsensusEngineType::Streamlet => {
                Arc::new(StreamletConsensus::new(participant_id, params))
            }
        }
    }
}

/// Proof-of-Stakeholder consensus implementation
pub struct ProofOfStakeholderConsensus {
    participant_id: ParticipantId,
    active_proposals: HashMap<Uuid, ConsensusProposal>,
    proposal_votes: HashMap<Uuid, Vec<ConsensusVote>>,
    consensus_results: HashMap<Uuid, ConsensusResult>,
    active_commits: HashMap<TransactionId, TwoPhaseCommit>,
    consensus_threshold: f64, // Percentage of stakeholders needed for approval
    params: ConsensusParams,
}

impl ProofOfStakeholderConsensus {
    pub fn new(participant_id: ParticipantId, consensus_threshold: f64) -> Self {
        Self {
            participant_id,
            active_proposals: HashMap::new(),
            proposal_votes: HashMap::new(),
            consensus_results: HashMap::new(),
            active_commits: HashMap::new(),
            consensus_threshold,
            params: ConsensusParams::default(),
        }
    }
    
    pub fn new_with_params(participant_id: ParticipantId, params: ConsensusParams) -> Self {
        Self {
            participant_id,
            active_proposals: HashMap::new(),
            proposal_votes: HashMap::new(),
            consensus_results: HashMap::new(),
            active_commits: HashMap::new(),
            consensus_threshold: params.threshold,
            params,
        }
    }

    /// Calculate if consensus is reached for a proposal
    fn calculate_consensus(&self, proposal: &ConsensusProposal, votes: &[ConsensusVote]) -> ConsensusOutcome {
        let total_stakeholders = proposal.stakeholders.len() as f64;
        let required_votes = (total_stakeholders * self.consensus_threshold).ceil() as usize;

        let approve_votes = votes.iter().filter(|v| v.vote == VoteType::Approve).count();
        let reject_votes = votes.iter().filter(|v| v.vote == VoteType::Reject).count();

        if approve_votes >= required_votes {
            ConsensusOutcome::Approved
        } else if reject_votes >= required_votes {
            ConsensusOutcome::Rejected
        } else if Utc::now() > proposal.expires_at {
            ConsensusOutcome::Timeout
        } else if votes.len() == proposal.stakeholders.len() {
            ConsensusOutcome::InsufficientVotes
        } else {
            // Still waiting for more votes
            return ConsensusOutcome::InsufficientVotes;
        }
    }

    /// Validate that a participant is a stakeholder for the proposal
    fn validate_stakeholder(&self, proposal: &ConsensusProposal, voter: &ParticipantId) -> bool {
        proposal.stakeholders.contains(voter)
    }

    /// Process expired proposals
    pub async fn process_expired_proposals(&mut self) -> GarpResult<Vec<ConsensusResult>> {
        let now = Utc::now();
        let mut expired_results = Vec::new();

        let expired_proposals: Vec<Uuid> = self.active_proposals
            .iter()
            .filter(|(_, proposal)| now > proposal.expires_at)
            .map(|(id, _)| *id)
            .collect();

        for proposal_id in expired_proposals {
            if let Some(proposal) = self.active_proposals.remove(&proposal_id) {
                let votes = self.proposal_votes.remove(&proposal_id).unwrap_or_default();
                
                let result = ConsensusResult {
                    proposal_id,
                    outcome: ConsensusOutcome::Timeout,
                    votes,
                    finalized_at: now,
                };

                self.consensus_results.insert(proposal_id, result.clone());
                expired_results.push(result);
            }
        }

        Ok(expired_results)
    }

    /// Process two-phase commit timeouts
    pub async fn process_commit_timeouts(&mut self) -> GarpResult<Vec<TransactionId>> {
        let now = Utc::now();
        let mut timed_out = Vec::new();

        let expired_commits: Vec<TransactionId> = self.active_commits
            .iter()
            .filter(|(_, commit)| now > commit.timeout_at && commit.phase != CommitPhase::Completed)
            .map(|(id, _)| id.clone())
            .collect();

        for transaction_id in expired_commits {
            if let Some(mut commit) = self.active_commits.get_mut(&transaction_id) {
                commit.phase = CommitPhase::Abort;
                timed_out.push(transaction_id);
            }
        }

        Ok(timed_out)
    }
}

#[async_trait]
impl ConsensusEngine for ProofOfStakeholderConsensus {
    async fn submit_proposal(&self, proposal: ConsensusProposal) -> GarpResult<()> {
        // In a real implementation, this would be synchronized across nodes
        // For now, we'll just validate the proposal structure
        
        if proposal.stakeholders.is_empty() {
            return Err(ConsensusError::InvalidProposal("No stakeholders specified".to_string()).into());
        }

        if proposal.transactions.is_empty() {
            return Err(ConsensusError::InvalidProposal("No transactions specified".to_string()).into());
        }

        if proposal.expires_at <= Utc::now() {
            return Err(ConsensusError::InvalidProposal("Proposal already expired".to_string()).into());
        }

        // Store proposal (in real implementation, this would be distributed)
        tracing::info!("Consensus proposal submitted: {:?}", proposal.proposal_id);
        Ok(())
    }

    async fn cast_vote(&self, vote: ConsensusVote) -> GarpResult<()> {
        // In a real implementation, this would validate and store the vote
        // across the distributed consensus network
        
        tracing::info!(
            "Vote cast by {:?} on proposal {:?}: {:?}",
            vote.voter,
            vote.proposal_id,
            vote.vote
        );
        
        Ok(())
    }

    async fn get_consensus_result(&self, proposal_id: Uuid) -> GarpResult<Option<ConsensusResult>> {
        // In a real implementation, this would query the distributed state
        Ok(None)
    }

    async fn start_two_phase_commit(&self, commit: TwoPhaseCommit) -> GarpResult<()> {
        if commit.participants.is_empty() {
            return Err(ConsensusError::InvalidProposal("No participants in 2PC".to_string()).into());
        }

        tracing::info!(
            "Starting 2PC for transaction {:?} with {} participants",
            commit.transaction_id,
            commit.participants.len()
        );

        Ok(())
    }

    async fn vote_two_phase_commit(
        &self,
        transaction_id: TransactionId,
        participant: ParticipantId,
        vote: PhaseVote,
    ) -> GarpResult<()> {
        tracing::info!(
            "2PC vote from {:?} for transaction {:?}: {:?}",
            participant,
            transaction_id,
            vote
        );

        Ok(())
    }

    async fn get_two_phase_commit_status(&self, transaction_id: TransactionId) -> GarpResult<Option<TwoPhaseCommit>> {
        // In a real implementation, this would query the distributed state
        Ok(None)
    }
    
    fn engine_type(&self) -> ConsensusEngineType {
        ConsensusEngineType::ProofOfStakeholder
    }
    
    fn get_params(&self) -> ConsensusParams {
        self.params.clone()
    }
}

/// Byzantine Fault Tolerant consensus for Global Synchronizer
pub struct BftConsensus {
    node_id: String,
    peers: Vec<String>,
    fault_tolerance: usize, // Maximum number of Byzantine nodes tolerated
}

/// Raft consensus implementation
pub struct RaftConsensus {
    participant_id: ParticipantId,
    params: ConsensusParams,
    current_term: u64,
    voted_for: Option<ParticipantId>,
    leader_id: Option<ParticipantId>,
}

impl RaftConsensus {
    pub fn new(participant_id: ParticipantId, params: ConsensusParams) -> Self {
        Self {
            participant_id,
            params,
            current_term: 0,
            voted_for: None,
            leader_id: None,
        }
    }
}

#[async_trait]
impl ConsensusEngine for RaftConsensus {
    async fn submit_proposal(&self, proposal: ConsensusProposal) -> GarpResult<()> {
        // Raft implementation would handle proposal submission
        tracing::info!("Raft consensus proposal submitted: {:?}", proposal.proposal_id);
        Ok(())
    }

    async fn cast_vote(&self, vote: ConsensusVote) -> GarpResult<()> {
        tracing::info!(
            "Raft vote cast by {:?} on proposal {:?}: {:?}",
            vote.voter,
            vote.proposal_id,
            vote.vote
        );
        Ok(())
    }

    async fn get_consensus_result(&self, proposal_id: Uuid) -> GarpResult<Option<ConsensusResult>> {
        Ok(None)
    }

    async fn start_two_phase_commit(&self, commit: TwoPhaseCommit) -> GarpResult<()> {
        tracing::info!(
            "Starting Raft 2PC for transaction {:?} with {} participants",
            commit.transaction_id,
            commit.participants.len()
        );
        Ok(())
    }

    async fn vote_two_phase_commit(
        &self,
        transaction_id: TransactionId,
        participant: ParticipantId,
        vote: PhaseVote,
    ) -> GarpResult<()> {
        tracing::info!(
            "Raft 2PC vote from {:?} for transaction {:?}: {:?}",
            participant,
            transaction_id,
            vote
        );
        Ok(())
    }

    async fn get_two_phase_commit_status(&self, transaction_id: TransactionId) -> GarpResult<Option<TwoPhaseCommit>> {
        Ok(None)
    }
    
    fn engine_type(&self) -> ConsensusEngineType {
        ConsensusEngineType::Raft
    }
    
    fn get_params(&self) -> ConsensusParams {
        self.params.clone()
    }
}

/// Tendermint consensus implementation
pub struct TendermintConsensus {
    participant_id: ParticipantId,
    params: ConsensusParams,
    validators: HashMap<ParticipantId, u64>, // validator_id -> voting_power
    current_height: u64,
    current_round: u64,
    current_step: ConsensusStep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusStep {
    Propose,
    Prevote,
    Precommit,
    Commit,
}

impl TendermintConsensus {
    pub fn new(participant_id: ParticipantId, params: ConsensusParams) -> Self {
        Self {
            participant_id,
            params,
            validators: HashMap::new(),
            current_height: 0,
            current_round: 0,
            current_step: ConsensusStep::Propose,
        }
    }
    
    /// Add a validator
    pub fn add_validator(&mut self, validator_id: ParticipantId, voting_power: u64) {
        self.validators.insert(validator_id, voting_power);
    }
    
    /// Remove a validator
    pub fn remove_validator(&mut self, validator_id: &ParticipantId) {
        self.validators.remove(validator_id);
    }
    
    /// Calculate required votes for consensus (2/3 + 1)
    pub fn required_votes(&self) -> usize {
        let total_voting_power: u64 = self.validators.values().sum();
        let num = (self.params.quorum_ratio_thousandths as u64) * total_voting_power;
        let denom = 1000u64;
        ((num + denom - 1) / denom) as usize
    }
    
    /// Get the current proposer for this round
    pub fn get_proposer(&self) -> Option<ParticipantId> {
        let validators: Vec<&ParticipantId> = self.validators.keys().collect();
        if validators.is_empty() {
            return None;
        }
        
        let proposer_index = ((self.current_height + self.current_round) % validators.len() as u64) as usize;
        Some(validators[proposer_index].clone())
    }
    
    /// Validate a proposal according to Tendermint rules
    pub fn validate_proposal(&self, proposal: &ConsensusProposal) -> bool {
        // Check if proposal is for current height
        if proposal.metadata.get("height").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0) != self.current_height {
            return false;
        }
        
        // Check if proposal is for current round
        if proposal.metadata.get("round").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0) != self.current_round {
            return false;
        }
        
        // Check proposer validity
        if let Some(proposer) = self.get_proposer() {
            if proposal.proposer != proposer {
                return false;
            }
        } else {
            return false;
        }
        
        // Check expiration
        if proposal.expires_at < Utc::now() {
            return false;
        }
        
        true
    }
    
    /// Process a proposal in the Tendermint protocol
    pub async fn process_proposal(&mut self, proposal: ConsensusProposal) -> GarpResult<()> {
        // Validate the proposal
        if !self.validate_proposal(&proposal) {
            return Err(GarpError::InvalidProposal("Invalid proposal".to_string()));
        }
        
        // In a real implementation, we would lock the value and move to prevote step
        self.current_step = ConsensusStep::Prevote;
        Ok(())
    }
    
    /// Process a vote in the Tendermint protocol
    pub async fn process_vote(&mut self, vote: ConsensusVote) -> GarpResult<()> {
        match vote.vote {
            VoteType::Prevote => {
                // In a real implementation, we would collect prevotes
                // If we have 2/3+1 prevotes, we move to precommit step
                // For now, we'll just check if we have enough votes to move forward
                if self.has_enough_votes(VoteType::Prevote).await {
                    self.current_step = ConsensusStep::Precommit;
                }
            },
            VoteType::Precommit => {
                // In a real implementation, we would collect precommits
                // If we have 2/3+1 precommits, we commit and move to next height
                if self.has_enough_votes(VoteType::Precommit).await {
                    self.current_step = ConsensusStep::Commit;
                    self.current_height += 1;
                    self.current_round = 0;
                    self.current_step = ConsensusStep::Propose;
                }
            },
            VoteType::Nil => {
                // Nil votes can be processed in any step
                // In a real implementation, we would handle timeouts and round changes
            }
        }
        
        Ok(())
    }
    
    /// Check if we have enough votes of a specific type
    async fn has_enough_votes(&self, vote_type: VoteType) -> bool {
        // In a real implementation, we would check actual vote counts
        // For now, we'll just return true to simulate having enough votes
        true
    }
    
    /// Move to the next round
    pub fn next_round(&mut self) {
        self.current_round += 1;
        self.current_step = ConsensusStep::Propose;
    }
}

#[async_trait]
impl ConsensusEngine for TendermintConsensus {
    async fn submit_proposal(&self, proposal: ConsensusProposal) -> GarpResult<()> {
        tracing::info!("Tendermint consensus proposal submitted: {:?}", proposal.proposal_id);
        
        // In a real implementation, we would process the proposal
        // For now, we'll just log that we received it
        let mut self_clone = self.clone();
        self_clone.process_proposal(proposal).await?;
        
        Ok(())
    }

    async fn cast_vote(&self, vote: ConsensusVote) -> GarpResult<()> {
        tracing::info!(
            "Tendermint vote cast by {:?} on proposal {:?}: {:?}",
            vote.voter,
            vote.proposal_id,
            vote.vote
        );
        
        // In a real implementation, we would process the vote
        // For now, we'll just log that we received it
        let mut self_clone = self.clone();
        self_clone.process_vote(vote).await?;
        
        Ok(())
    }

    async fn get_consensus_result(&self, proposal_id: Uuid) -> GarpResult<Option<ConsensusResult>> {
        // In a real implementation, we would check if consensus was reached
        Ok(None)
    }

    async fn start_two_phase_commit(&self, commit: TwoPhaseCommit) -> GarpResult<()> {
        tracing::info!(
            "Starting Tendermint 2PC for transaction {:?} with {} participants",
            commit.transaction_id,
            commit.participants.len()
        );
        Ok(())
    }

    async fn vote_two_phase_commit(
        &self,
        transaction_id: TransactionId,
        participant: ParticipantId,
        vote: PhaseVote,
    ) -> GarpResult<()> {
        tracing::info!(
            "Tendermint 2PC vote from {:?} for transaction {:?}: {:?}",
            participant,
            transaction_id,
            vote
        );
        Ok(())
    }

    async fn get_two_phase_commit_status(&self, transaction_id: TransactionId) -> GarpResult<Option<TwoPhaseCommit>> {
        Ok(None)
    }
    
    fn engine_type(&self) -> ConsensusEngineType {
        ConsensusEngineType::Tendermint
    }
    
    fn get_params(&self) -> ConsensusParams {
        self.params.clone()
    }
}

/// HotStuff consensus implementation
pub struct HotStuffConsensus {
    participant_id: ParticipantId,
    params: ConsensusParams,
    validators: HashMap<ParticipantId, u64>, // validator_id -> voting_power
    current_view: u64,
    locked_qc: Option<QuorumCertificate>,
    high_qc: Option<QuorumCertificate>,
    block_tree: HashMap<Vec<u8>, Block>, // block_hash -> Block
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuorumCertificate {
    pub view: u64,
    pub block_hash: Vec<u8>,
    pub signatures: HashMap<ParticipantId, Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub hash: Vec<u8>,
    pub view: u64,
    pub height: u64,
    pub payload: Vec<u8>,
    pub parent_hash: Vec<u8>,
    pub justify_qc: Option<QuorumCertificate>,
}

impl HotStuffConsensus {
    pub fn new(participant_id: ParticipantId, params: ConsensusParams) -> Self {
        Self {
            participant_id,
            params,
            validators: HashMap::new(),
            current_view: 0,
            locked_qc: None,
            high_qc: None,
            block_tree: HashMap::new(),
        }
    }
    
    /// Add a validator
    pub fn add_validator(&mut self, validator_id: ParticipantId, voting_power: u64) {
        self.validators.insert(validator_id, voting_power);
    }
    
    /// Remove a validator
    pub fn remove_validator(&mut self, validator_id: &ParticipantId) {
        self.validators.remove(validator_id);
    }
    
    /// Calculate required votes for consensus (2/3 + 1)
    pub fn required_votes(&self) -> usize {
        let total_voting_power: u64 = self.validators.values().sum();
        let num = (self.params.quorum_ratio_thousandths as u64) * total_voting_power;
        let denom = 1000u64;
        ((num + denom - 1) / denom) as usize
    }
    
    /// Create a new block
    pub fn create_block(&self, payload: Vec<u8>, parent_hash: Vec<u8>) -> Block {
        let view = self.current_view;
        let height = self.get_height(&parent_hash);
        
        // In a real implementation, we would compute the actual hash
        let hash = format!("block-{}-{}", view, height).into_bytes();
        
        Block {
            hash: hash.clone(),
            view,
            height,
            payload,
            parent_hash,
            justify_qc: self.high_qc.clone(),
        }
    }
    
    /// Get the height of a block
    fn get_height(&self, block_hash: &[u8]) -> u64 {
        if let Some(block) = self.block_tree.get(block_hash) {
            block.height + 1
        } else {
            1 // Genesis block
        }
    }
    
    /// Validate a block
    pub fn validate_block(&self, block: &Block) -> bool {
        // Check if block view is valid
        if block.view <= self.current_view {
            return false;
        }
        
        // Check parent exists
        if block.height > 1 && !self.block_tree.contains_key(&block.parent_hash) {
            return false;
        }
        
        // Check QC if present
        if let Some(qc) = &block.justify_qc {
            if !self.validate_quorum_certificate(qc) {
                return false;
            }
        }
        
        true
    }
    
    /// Validate a quorum certificate
    pub fn validate_quorum_certificate(&self, qc: &QuorumCertificate) -> bool {
        // Check if QC view is valid
        if qc.view >= self.current_view {
            return false;
        }
        
        // Check if we have enough signatures
        let signature_count = qc.signatures.len();
        if signature_count < self.required_votes() {
            return false;
        }
        
        // In a real implementation, we would verify the signatures
        true
    }
    
    /// Process a proposal in the HotStuff protocol
    pub async fn process_proposal(&mut self, proposal: ConsensusProposal) -> GarpResult<()> {
        // Extract block from proposal
        // In a real implementation, we would deserialize the block from the proposal payload
        let block = self.create_block(
            proposal.transactions.iter().map(|t| t.to_string().into_bytes()).flatten().collect(),
            vec![] // For simplicity, we'll use an empty parent hash
        );
        
        // Validate the block
        if !self.validate_block(&block) {
            return Err(GarpError::InvalidProposal("Invalid block".to_string()));
        }
        
        // Add block to tree
        self.block_tree.insert(block.hash.clone(), block);
        
        // Update current view
        self.current_view += 1;
        
        Ok(())
    }
    
    /// Process a vote in the HotStuff protocol
    pub async fn process_vote(&mut self, vote: ConsensusVote) -> GarpResult<()> {
        // In a real implementation, we would collect votes to form QCs
        // For now, we'll just increment the view
        self.current_view += 1;
        Ok(())
    }
    
    /// Create a quorum certificate from votes
    pub fn create_quorum_certificate(&self, block_hash: Vec<u8>, votes: HashMap<ParticipantId, Vec<u8>>) -> QuorumCertificate {
        QuorumCertificate {
            view: self.current_view,
            block_hash,
            signatures: votes,
        }
    }
}

#[async_trait]
impl ConsensusEngine for HotStuffConsensus {
    async fn submit_proposal(&self, proposal: ConsensusProposal) -> GarpResult<()> {
        tracing::info!("HotStuff consensus proposal submitted: {:?}", proposal.proposal_id);
        
        // In a real implementation, we would process the proposal
        let mut self_clone = self.clone();
        self_clone.process_proposal(proposal).await?;
        
        Ok(())
    }

    async fn cast_vote(&self, vote: ConsensusVote) -> GarpResult<()> {
        tracing::info!(
            "HotStuff vote cast by {:?} on proposal {:?}: {:?}",
            vote.voter,
            vote.proposal_id,
            vote.vote
        );
        
        // In a real implementation, we would process the vote
        let mut self_clone = self.clone();
        self_clone.process_vote(vote).await?;
        
        Ok(())
    }

    async fn get_consensus_result(&self, proposal_id: Uuid) -> GarpResult<Option<ConsensusResult>> {
        // In a real implementation, we would check if consensus was reached
        Ok(None)
    }

    async fn start_two_phase_commit(&self, commit: TwoPhaseCommit) -> GarpResult<()> {
        tracing::info!(
            "Starting HotStuff 2PC for transaction {:?} with {} participants",
            commit.transaction_id,
            commit.participants.len()
        );
        Ok(())
    }

    async fn vote_two_phase_commit(
        &self,
        transaction_id: TransactionId,
        participant: ParticipantId,
        vote: PhaseVote,
    ) -> GarpResult<()> {
        tracing::info!(
            "HotStuff 2PC vote from {:?} for transaction {:?}: {:?}",
            participant,
            transaction_id,
            vote
        );
        Ok(())
    }

    async fn get_two_phase_commit_status(&self, transaction_id: TransactionId) -> GarpResult<Option<TwoPhaseCommit>> {
        Ok(None)
    }
    
    fn engine_type(&self) -> ConsensusEngineType {
        ConsensusEngineType::HotStuff
    }
    
    fn get_params(&self) -> ConsensusParams {
        self.params.clone()
    }
}

/// Practical BFT consensus implementation
pub struct PracticalBFTConsensus {
    participant_id: ParticipantId,
    params: ConsensusParams,
    validators: HashMap<ParticipantId, u64>, // validator_id -> voting_power
    current_sequence: u64,
    prepared_cert: Option<PrepareCertificate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareCertificate {
    pub sequence: u64,
    pub digest: Vec<u8>,
    pub signatures: HashMap<ParticipantId, Vec<u8>>,
}

impl PracticalBFTConsensus {
    pub fn new(participant_id: ParticipantId, params: ConsensusParams) -> Self {
        Self {
            participant_id,
            params,
            validators: HashMap::new(),
            current_sequence: 0,
            prepared_cert: None,
        }
    }
}

#[async_trait]
impl ConsensusEngine for PracticalBFTConsensus {
    async fn submit_proposal(&self, proposal: ConsensusProposal) -> GarpResult<()> {
        tracing::info!("PracticalBFT consensus proposal submitted: {:?}", proposal.proposal_id);
        Ok(())
    }

    async fn cast_vote(&self, vote: ConsensusVote) -> GarpResult<()> {
        tracing::info!(
            "PracticalBFT vote cast by {:?} on proposal {:?}: {:?}",
            vote.voter,
            vote.proposal_id,
            vote.vote
        );
        Ok(())
    }

    async fn get_consensus_result(&self, proposal_id: Uuid) -> GarpResult<Option<ConsensusResult>> {
        Ok(None)
    }

    async fn start_two_phase_commit(&self, commit: TwoPhaseCommit) -> GarpResult<()> {
        tracing::info!(
            "Starting PracticalBFT 2PC for transaction {:?} with {} participants",
            commit.transaction_id,
            commit.participants.len()
        );
        Ok(())
    }

    async fn vote_two_phase_commit(
        &self,
        transaction_id: TransactionId,
        participant: ParticipantId,
        vote: PhaseVote,
    ) -> GarpResult<()> {
        tracing::info!(
            "PracticalBFT 2PC vote from {:?} for transaction {:?}: {:?}",
            participant,
            transaction_id,
            vote
        );
        Ok(())
    }

    async fn get_two_phase_commit_status(&self, transaction_id: TransactionId) -> GarpResult<Option<TwoPhaseCommit>> {
        Ok(None)
    }
    
    fn engine_type(&self) -> ConsensusEngineType {
        ConsensusEngineType::PracticalBFT
    }
    
    fn get_params(&self) -> ConsensusParams {
        self.params.clone()
    }
}

/// PBFT (Practical Byzantine Fault Tolerance) consensus implementation
pub struct PbftConsensus {
    participant_id: ParticipantId,
    params: ConsensusParams,
    validators: HashMap<ParticipantId, u64>, // validator_id -> voting_power
    current_sequence: u64,
    current_view: u64,
    phase: PbftPhase,
    prepared_cert: Option<PrepareCertificate>,
    committed_cert: Option<CommitCertificate>,
    message_log: HashMap<PbftMessageId, PbftMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PbftPhase {
    PrePrepare,
    Prepare,
    Commit,
    ViewChange,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct PbftMessageId {
    pub sequence: u64,
    pub view: u64,
    pub sender: ParticipantId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbftMessage {
    pub message_id: PbftMessageId,
    pub message_type: PbftMessageType,
    pub digest: Vec<u8>,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PbftMessageType {
    PrePrepare,
    Prepare,
    Commit,
    ViewChange,
    NewView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareCertificate {
    pub sequence: u64,
    pub digest: Vec<u8>,
    pub signatures: HashMap<ParticipantId, Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitCertificate {
    pub sequence: u64,
    pub digest: Vec<u8>,
    pub signatures: HashMap<ParticipantId, Vec<u8>>,
}

impl PbftConsensus {
    pub fn new(participant_id: ParticipantId, params: ConsensusParams) -> Self {
        Self {
            participant_id,
            params,
            validators: HashMap::new(),
            current_sequence: 0,
            current_view: 0,
            phase: PbftPhase::PrePrepare,
            prepared_cert: None,
            committed_cert: None,
            message_log: HashMap::new(),
        }
    }
    
    /// Add a validator
    pub fn add_validator(&mut self, validator_id: ParticipantId, voting_power: u64) {
        self.validators.insert(validator_id, voting_power);
    }
    
    /// Remove a validator
    pub fn remove_validator(&mut self, validator_id: &ParticipantId) {
        self.validators.remove(validator_id);
    }
    
    /// Calculate required votes for consensus (2/3 + 1)
    pub fn required_votes(&self) -> usize {
        let total_voting_power: u64 = self.validators.values().sum();
        let num = (self.params.quorum_ratio_thousandths as u64) * total_voting_power;
        let denom = 1000u64;
        ((num + denom - 1) / denom) as usize
    }
    
    /// Validate a PBFT message
    pub fn validate_message(&self, message: &PbftMessage) -> bool {
        // Check if message is for current view (unless it's a view change)
        if !matches!(message.message_type, PbftMessageType::ViewChange | PbftMessageType::NewView) 
           && message.message_id.view != self.current_view {
            return false;
        }
        
        // Check if message is for current sequence (unless it's a view change)
        if !matches!(message.message_type, PbftMessageType::ViewChange | PbftMessageType::NewView) 
           && message.message_id.sequence != self.current_sequence {
            return false;
        }
        
        // In a real implementation, we would verify the signature
        true
    }
    
    /// Process a pre-prepare message
    pub async fn process_pre_prepare(&mut self, message: PbftMessage) -> GarpResult<()> {
        if self.phase != PbftPhase::PrePrepare {
            return Err(GarpError::InvalidState("Not in PrePrepare phase".to_string()));
        }
        
        if !self.validate_message(&message) {
            return Err(GarpError::InvalidMessage("Invalid pre-prepare message".to_string()));
        }
        
        // Log the message
        self.message_log.insert(message.message_id.clone(), message);
        
        // Move to Prepare phase
        self.phase = PbftPhase::Prepare;
        
        Ok(())
    }
    
    /// Process a prepare message
    pub async fn process_prepare(&mut self, message: PbftMessage) -> GarpResult<()> {
        if self.phase != PbftPhase::Prepare {
            return Err(GarpError::InvalidState("Not in Prepare phase".to_string()));
        }
        
        if !self.validate_message(&message) {
            return Err(GarpError::InvalidMessage("Invalid prepare message".to_string()));
        }
        
        // Log the message
        self.message_log.insert(message.message_id.clone(), message);
        
        // Check if we have enough prepare messages to form a prepare certificate
        if self.has_enough_prepares().await {
            // Create prepare certificate
            // In a real implementation, we would collect the signatures
            self.prepared_cert = Some(PrepareCertificate {
                sequence: self.current_sequence,
                digest: vec![], // In a real implementation, we would use the actual digest
                signatures: HashMap::new(),
            });
            
            // Move to Commit phase
            self.phase = PbftPhase::Commit;
        }
        
        Ok(())
    }
    
    /// Process a commit message
    pub async fn process_commit(&mut self, message: PbftMessage) -> GarpResult<()> {
        if self.phase != PbftPhase::Commit {
            return Err(GarpError::InvalidState("Not in Commit phase".to_string()));
        }
        
        if !self.validate_message(&message) {
            return Err(GarpError::InvalidMessage("Invalid commit message".to_string()));
        }
        
        // Log the message
        self.message_log.insert(message.message_id.clone(), message);
        
        // Check if we have enough commit messages to form a commit certificate
        if self.has_enough_commits().await {
            // Create commit certificate
            // In a real implementation, we would collect the signatures
            self.committed_cert = Some(CommitCertificate {
                sequence: self.current_sequence,
                digest: vec![], // In a real implementation, we would use the actual digest
                signatures: HashMap::new(),
            });
            
            // Move to next sequence
            self.current_sequence += 1;
            self.phase = PbftPhase::PrePrepare;
        }
        
        Ok(())
    }
    
    /// Check if we have enough prepare messages
    async fn has_enough_prepares(&self) -> bool {
        let prepare_count = self.message_log.values()
            .filter(|msg| matches!(msg.message_type, PbftMessageType::Prepare))
            .count();
        
        prepare_count >= self.required_votes()
    }
    
    /// Check if we have enough commit messages
    async fn has_enough_commits(&self) -> bool {
        let commit_count = self.message_log.values()
            .filter(|msg| matches!(msg.message_type, PbftMessageType::Commit))
            .count();
        
        commit_count >= self.required_votes()
    }
    
    /// Process a view change message
    pub async fn process_view_change(&mut self, message: PbftMessage) -> GarpResult<()> {
        if !self.validate_message(&message) {
            return Err(GarpError::InvalidMessage("Invalid view change message".to_string()));
        }
        
        // Log the message
        self.message_log.insert(message.message_id.clone(), message);
        
        // Check if we have enough view change messages to move to new view
        if self.has_enough_view_changes().await {
            // Move to new view
            self.current_view += 1;
            self.phase = PbftPhase::PrePrepare;
            
            // Clear message log for previous view
            self.message_log.retain(|id, _| id.view >= self.current_view);
        }
        
        Ok(())
    }
    
    /// Check if we have enough view change messages
    async fn has_enough_view_changes(&self) -> bool {
        let view_change_count = self.message_log.values()
            .filter(|msg| matches!(msg.message_type, PbftMessageType::ViewChange))
            .count();
        
        view_change_count >= self.required_votes()
    }
}

#[async_trait]
impl ConsensusEngine for PbftConsensus {
    async fn submit_proposal(&self, proposal: ConsensusProposal) -> GarpResult<()> {
        tracing::info!("PBFT consensus proposal submitted: {:?}", proposal.proposal_id);
        
        // In a real implementation, we would process the proposal
        let mut self_clone = self.clone();
        
        // Create a PBFT pre-prepare message
        let message = PbftMessage {
            message_id: PbftMessageId {
                sequence: self_clone.current_sequence,
                view: self_clone.current_view,
                sender: self_clone.participant_id.clone(),
            },
            message_type: PbftMessageType::PrePrepare,
            digest: proposal.transactions.iter().map(|t| t.to_string().into_bytes()).flatten().collect(),
            signature: vec![], // In a real implementation, we would sign the message
        };
        
        self_clone.process_pre_prepare(message).await?;
        
        Ok(())
    }

    async fn cast_vote(&self, vote: ConsensusVote) -> GarpResult<()> {
        tracing::info!(
            "PBFT vote cast by {:?} on proposal {:?}: {:?}",
            vote.voter,
            vote.proposal_id,
            vote.vote
        );
        
        // In a real implementation, we would process the vote
        let mut self_clone = self.clone();
        
        // Create a PBFT message based on the vote type
        let message_type = match vote.vote {
            VoteType::Prevote => PbftMessageType::Prepare,
            VoteType::Precommit => PbftMessageType::Commit,
            VoteType::Nil => PbftMessageType::ViewChange,
        };
        
        let message = PbftMessage {
            message_id: PbftMessageId {
                sequence: self_clone.current_sequence,
                view: self_clone.current_view,
                sender: vote.voter.clone(),
            },
            message_type,
            digest: vec![], // In a real implementation, we would use the actual digest
            signature: vec![], // In a real implementation, we would sign the message
        };
        
        match message_type {
            PbftMessageType::Prepare => {
                self_clone.process_prepare(message).await?;
            },
            PbftMessageType::Commit => {
                self_clone.process_commit(message).await?;
            },
            PbftMessageType::ViewChange => {
                self_clone.process_view_change(message).await?;
            },
            _ => {}
        }
        
        Ok(())
    }

    async fn get_consensus_result(&self, proposal_id: Uuid) -> GarpResult<Option<ConsensusResult>> {
        // In a real implementation, we would check if consensus was reached
        Ok(None)
    }

    async fn start_two_phase_commit(&self, commit: TwoPhaseCommit) -> GarpResult<()> {
        tracing::info!(
            "Starting PBFT 2PC for transaction {:?} with {} participants",
            commit.transaction_id,
            commit.participants.len()
        );
        Ok(())
    }

    async fn vote_two_phase_commit(
        &self,
        transaction_id: TransactionId,
        participant: ParticipantId,
        vote: PhaseVote,
    ) -> GarpResult<()> {
        tracing::info!(
            "PBFT 2PC vote from {:?} for transaction {:?}: {:?}",
            participant,
            transaction_id,
            vote
        );
        Ok(())
    }

    async fn get_two_phase_commit_status(&self, transaction_id: TransactionId) -> GarpResult<Option<TwoPhaseCommit>> {
        Ok(None)
    }
    
    fn engine_type(&self) -> ConsensusEngineType {
        ConsensusEngineType::Pbft
    }
    
    fn get_params(&self) -> ConsensusParams {
        self.params.clone()
    }
}

/// HoneyBadgerBFT consensus implementation for asynchronous networks
pub struct HoneyBadgerBftConsensus {
    participant_id: ParticipantId,
    params: ConsensusParams,
    validators: HashMap<ParticipantId, u64>, // validator_id -> voting_power
    epoch: u64,
    transaction_pool: Vec<TransactionId>,
    output_buffer: Vec<ConsensusOutput>,
    common_coin: CommonCoin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusOutput {
    pub epoch: u64,
    pub transactions: Vec<TransactionId>,
    pub proof: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonCoin {
    pub epoch: u64,
    pub shares: HashMap<ParticipantId, Vec<u8>>,
    pub threshold: usize,
}

impl HoneyBadgerBftConsensus {
    pub fn new(participant_id: ParticipantId, params: ConsensusParams) -> Self {
        Self {
            participant_id,
            params,
            validators: HashMap::new(),
            epoch: 0,
            transaction_pool: Vec::new(),
            output_buffer: Vec::new(),
            common_coin: CommonCoin {
                epoch: 0,
                shares: HashMap::new(),
                threshold: 0,
            },
        }
    }
    
    /// Add a validator
    pub fn add_validator(&mut self, validator_id: ParticipantId, voting_power: u64) {
        self.validators.insert(validator_id, voting_power);
    }
    
    /// Remove a validator
    pub fn remove_validator(&mut self, validator_id: &ParticipantId) {
        self.validators.remove(validator_id);
    }
    
    /// Calculate required votes for consensus (2/3 + 1)
    pub fn required_votes(&self) -> usize {
        let total_voting_power: u64 = self.validators.values().sum();
        let num = (self.params.quorum_ratio_thousandths as u64) * total_voting_power;
        let denom = 1000u64;
        ((num + denom - 1) / denom) as usize
    }
    
    /// Add transactions to the pool
    pub fn add_transactions(&mut self, transactions: Vec<TransactionId>) {
        self.transaction_pool.extend(transactions);
    }
    
    /// Process a proposal in the HoneyBadgerBFT protocol
    pub async fn process_proposal(&mut self, proposal: ConsensusProposal) -> GarpResult<()> {
        // In HoneyBadgerBFT, proposals are processed asynchronously
        // Add transactions to the pool
        self.add_transactions(proposal.transactions);
        
        // In a real implementation, we would:
        // 1. Create a common coin for this epoch
        // 2. Run the asynchronous consensus protocol
        // 3. Output transactions when consensus is reached
        
        // For now, we'll just simulate reaching consensus
        if !self.transaction_pool.is_empty() {
            let output = ConsensusOutput {
                epoch: self.epoch,
                transactions: self.transaction_pool.drain(..).collect(),
                proof: vec![], // In a real implementation, we would generate a proof
            };
            
            self.output_buffer.push(output);
            self.epoch += 1;
        }
        
        Ok(())
    }
    
    /// Process a vote in the HoneyBadgerBFT protocol
    pub async fn process_vote(&mut self, vote: ConsensusVote) -> GarpResult<()> {
        // In HoneyBadgerBFT, votes are part of the asynchronous protocol
        // For now, we'll just increment the epoch
        self.epoch += 1;
        Ok(())
    }
    
    /// Get the consensus output
    pub fn get_output(&mut self) -> Option<ConsensusOutput> {
        self.output_buffer.pop()
    }
}

/// Streamlet consensus implementation for simpler verification
pub struct StreamletConsensus {
    participant_id: ParticipantId,
    params: ConsensusParams,
    validators: HashMap<ParticipantId, u64>, // validator_id -> voting_power
    epoch: u64,
    notarized_blocks: HashMap<u64, NotarizedBlock>,
    finalized_blocks: HashMap<u64, FinalizedBlock>,
    pending_proposals: HashMap<Uuid, ConsensusProposal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotarizedBlock {
    pub epoch: u64,
    pub proposer: ParticipantId,
    pub transactions: Vec<TransactionId>,
    pub signatures: HashMap<ParticipantId, Vec<u8>>,
    pub notarized_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalizedBlock {
    pub epoch: u64,
    pub proposer: ParticipantId,
    pub transactions: Vec<TransactionId>,
    pub signatures: HashMap<ParticipantId, Vec<u8>>,
    pub finalized_at: DateTime<Utc>,
}

impl StreamletConsensus {
    pub fn new(participant_id: ParticipantId, params: ConsensusParams) -> Self {
        Self {
            participant_id,
            params,
            validators: HashMap::new(),
            epoch: 0,
            notarized_blocks: HashMap::new(),
            finalized_blocks: HashMap::new(),
            pending_proposals: HashMap::new(),
        }
    }
    
    /// Add a validator
    pub fn add_validator(&mut self, validator_id: ParticipantId, voting_power: u64) {
        self.validators.insert(validator_id, voting_power);
    }
    
    /// Remove a validator
    pub fn remove_validator(&mut self, validator_id: &ParticipantId) {
        self.validators.remove(validator_id);
    }
    
    /// Calculate required votes for consensus (2/3 + 1)
    pub fn required_votes(&self) -> usize {
        let total_voting_power: u64 = self.validators.values().sum();
        let num = (self.params.quorum_ratio_thousandths as u64) * total_voting_power;
        let denom = 1000u64;
        ((num + denom - 1) / denom) as usize
    }
    
    /// Process a proposal in the Streamlet protocol
    pub async fn process_proposal(&mut self, proposal: ConsensusProposal) -> GarpResult<()> {
        // Store the proposal for later processing
        self.pending_proposals.insert(proposal.proposal_id, proposal);
        
        // In a real Streamlet implementation, we would:
        // 1. Wait for the proposer's turn (based on a predetermined schedule)
        // 2. Collect votes from validators
        // 3. Notarize the block when we have enough votes
        // 4. Finalize the block after two consecutive notarized blocks
        
        // For now, we'll just simulate the process
        if let Some(proposal) = self.pending_proposals.remove(&proposal.proposal_id) {
            // Create a notarized block
            let notarized_block = NotarizedBlock {
                epoch: self.epoch,
                proposer: proposal.proposer,
                transactions: proposal.transactions,
                signatures: HashMap::new(), // In a real implementation, we would collect signatures
                notarized_at: Utc::now(),
            };
            
            self.notarized_blocks.insert(self.epoch, notarized_block);
            
            // Check if we can finalize (need two consecutive notarized blocks)
            if self.notarized_blocks.contains_key(&(self.epoch - 1)) {
                if let Some(prev_block) = self.notarized_blocks.remove(&(self.epoch - 1)) {
                    let finalized_block = FinalizedBlock {
                        epoch: self.epoch - 1,
                        proposer: prev_block.proposer,
                        transactions: prev_block.transactions,
                        signatures: prev_block.signatures,
                        finalized_at: Utc::now(),
                    };
                    
                    self.finalized_blocks.insert(self.epoch - 1, finalized_block);
                }
            }
            
            self.epoch += 1;
        }
        
        Ok(())
    }
    
    /// Process a vote in the Streamlet protocol
    pub async fn process_vote(&mut self, vote: ConsensusVote) -> GarpResult<()> {
        // In Streamlet, votes are used to notarize blocks
        // For now, we'll just increment the epoch
        self.epoch += 1;
        Ok(())
    }
    
    /// Get a finalized block
    pub fn get_finalized_block(&self, epoch: u64) -> Option<&FinalizedBlock> {
        self.finalized_blocks.get(&epoch)
    }
}

#[async_trait]
impl ConsensusEngine for StreamletConsensus {
    async fn submit_proposal(&self, proposal: ConsensusProposal) -> GarpResult<()> {
        tracing::info!("Streamlet consensus proposal submitted: {:?}", proposal.proposal_id);
        
        // In a real implementation, we would process the proposal
        let mut self_clone = self.clone();
        self_clone.process_proposal(proposal).await?;
        
        Ok(())
    }

    async fn cast_vote(&self, vote: ConsensusVote) -> GarpResult<()> {
        tracing::info!(
            "Streamlet vote cast by {:?} on proposal {:?}: {:?}",
            vote.voter,
            vote.proposal_id,
            vote.vote
        );
        
        // In a real implementation, we would process the vote
        let mut self_clone = self.clone();
        self_clone.process_vote(vote).await?;
        
        Ok(())
    }

    async fn get_consensus_result(&self, proposal_id: Uuid) -> GarpResult<Option<ConsensusResult>> {
        // In a real implementation, we would check if consensus was reached
        Ok(None)
    }

    async fn start_two_phase_commit(&self, commit: TwoPhaseCommit) -> GarpResult<()> {
        tracing::info!(
            "Starting Streamlet 2PC for transaction {:?} with {} participants",
            commit.transaction_id,
            commit.participants.len()
        );
        Ok(())
    }

    async fn vote_two_phase_commit(
        &self,
        transaction_id: TransactionId,
        participant: ParticipantId,
        vote: PhaseVote,
    ) -> GarpResult<()> {
        tracing::info!(
            "Streamlet 2PC vote from {:?} for transaction {:?}: {:?}",
            participant,
            transaction_id,
            vote
        );
        Ok(())
    }

    async fn get_two_phase_commit_status(&self, transaction_id: TransactionId) -> GarpResult<Option<TwoPhaseCommit>> {
        Ok(None)
    }
    
    fn engine_type(&self) -> ConsensusEngineType {
        ConsensusEngineType::Streamlet
    }
    
    fn get_params(&self) -> ConsensusParams {
        self.params.clone()
    }
}

/// Reference to a block used in fork-choice evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockRef {
    pub hash: Vec<u8>,
    pub parent_hash: Vec<u8>,
    pub slot: u64,
    pub weight: u64,
}

/// Simple longest-chain fork choice with weight tie-breaker by smallest hash
pub fn longest_chain_fork_choice(candidates: &[BlockRef]) -> Option<BlockRef> {
    if candidates.is_empty() { return None; }
    let mut best = candidates[0].clone();
    for b in candidates.iter().skip(1) {
        if b.slot > best.slot || (b.slot == best.slot && b.weight > best.weight) {
            best = b.clone();
        } else if b.slot == best.slot && b.weight == best.weight && b.hash < best.hash {
            best = b.clone();
        }
    }
    Some(best)
}

/// Minimal finality gadget state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalityState {
    pub justified_slot: u64,
    pub finalized_slot: u64,
}

impl FinalityState {
    pub fn new() -> Self { Self { justified_slot: 0, finalized_slot: 0 } }

    /// Process validator votes and advance finality (placeholder logic)
    pub fn on_votes(&mut self, votes_for_slot: u64, total_validators: u64, target_slot: u64) {
        if total_validators == 0 { return; }
        let quorum = (total_validators as f64 * 2.0 / 3.0).ceil() as u64;
        if votes_for_slot >= quorum {
            self.justified_slot = self.justified_slot.max(target_slot);
            // Finalize previous justified slot if consecutive
            if target_slot >= self.finalized_slot + 1 {
                self.finalized_slot = target_slot - 1;
            }
        }
    }
}

impl BftConsensus {
    pub fn new(node_id: String, peers: Vec<String>) -> Self {
        let fault_tolerance = (peers.len() - 1) / 3; // BFT can tolerate up to f = (n-1)/3 faults
        
        Self {
            node_id,
            peers,
            fault_tolerance,
        }
    }

    /// Check if we have enough nodes for BFT consensus
    pub fn is_viable(&self) -> bool {
        self.peers.len() >= 3 * self.fault_tolerance + 1
    }

    /// Calculate minimum votes needed for BFT consensus
    pub fn min_votes_needed(&self) -> usize {
        2 * self.fault_tolerance + 1
    }
}

/// Utility functions for consensus
pub mod utils {
    use super::*;

    /// Create a new consensus proposal
    pub fn create_proposal(
        proposer: ParticipantId,
        transactions: Vec<TransactionId>,
        stakeholders: Vec<ParticipantId>,
        timeout_seconds: i64,
    ) -> ConsensusProposal {
        let now = Utc::now();
        
        ConsensusProposal {
            proposal_id: Uuid::new_v4(),
            proposer,
            transactions,
            stakeholders,
            created_at: now,
            expires_at: now + chrono::Duration::seconds(timeout_seconds),
        }
    }

    /// Create a vote for a proposal
    pub fn create_vote(
        proposal_id: Uuid,
        voter: ParticipantId,
        vote_type: VoteType,
        reason: Option<String>,
    ) -> ConsensusVote {
        ConsensusVote {
            proposal_id,
            voter,
            vote: vote_type,
            reason,
            timestamp: Utc::now(),
        }
    }

    /// Create a two-phase commit
    pub fn create_two_phase_commit(
        transaction_id: TransactionId,
        coordinator: ParticipantId,
        participants: Vec<ParticipantId>,
        timeout_seconds: i64,
    ) -> TwoPhaseCommit {
        let now = Utc::now();
        
        TwoPhaseCommit {
            transaction_id,
            coordinator,
            participants,
            phase: CommitPhase::Prepare,
            votes: HashMap::new(),
            created_at: now,
            timeout_at: now + chrono::Duration::seconds(timeout_seconds),
        }
    }

    /// Determine stakeholders for a transaction
    pub fn determine_stakeholders(
        transaction: &crate::types::Transaction,
    ) -> Vec<ParticipantId> {
        let mut stakeholders = HashSet::new();
        
        // Add the submitter
        stakeholders.insert(transaction.submitter.clone());
        
        // Add stakeholders based on transaction command
        match &transaction.command {
            crate::types::TransactionCommand::Create { signatories, observers, .. } => {
                stakeholders.extend(signatories.iter().cloned());
                stakeholders.extend(observers.iter().cloned());
            },
            crate::types::TransactionCommand::Exercise { .. } => {
                // In a real implementation, we'd look up the contract to find its stakeholders
            },
            crate::types::TransactionCommand::Archive { .. } => {
                // In a real implementation, we'd look up the contract to find its stakeholders
            },
        }
        
        stakeholders.into_iter().collect()
    }
}