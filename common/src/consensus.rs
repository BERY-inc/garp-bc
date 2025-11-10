use crate::types::{ParticipantId, TransactionId, ValidationResult};
use crate::error::{ConsensusError, GarpResult};
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use async_trait::async_trait;

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
}

/// Proof-of-Stakeholder consensus implementation
pub struct ProofOfStakeholderConsensus {
    participant_id: ParticipantId,
    active_proposals: HashMap<Uuid, ConsensusProposal>,
    proposal_votes: HashMap<Uuid, Vec<ConsensusVote>>,
    consensus_results: HashMap<Uuid, ConsensusResult>,
    active_commits: HashMap<TransactionId, TwoPhaseCommit>,
    consensus_threshold: f64, // Percentage of stakeholders needed for approval
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
}

/// Byzantine Fault Tolerant consensus for Global Synchronizer
pub struct BftConsensus {
    node_id: String,
    peers: Vec<String>,
    fault_tolerance: usize, // Maximum number of Byzantine nodes tolerated
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