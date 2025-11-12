use crate::consensus::{ConsensusEngine, ConsensusEngineType, ConsensusFactory, ConsensusParams, ValidatorSet};
use crate::validator::{ValidatorInfo, ValidatorStatus, EvidenceType};
use crate::types::{ParticipantId, TransactionId};
use crate::error::GarpResult;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

/// Consensus manager for handling pluggable consensus engines and validator management
pub struct ConsensusManager {
    /// Current consensus engine
    consensus_engine: Arc<RwLock<Arc<dyn ConsensusEngine>>>,
    
    /// Validator set
    validator_set: Arc<ValidatorSet>,
    
    /// Participant ID
    participant_id: ParticipantId,
    
    /// Consensus parameters
    params: ConsensusParams,
}

impl ConsensusManager {
    /// Create new consensus manager
    pub fn new(participant_id: ParticipantId, engine_type: ConsensusEngineType, params: ConsensusParams) -> Self {
        let consensus_engine = ConsensusFactory::create_engine(engine_type.clone(), participant_id.clone(), params.clone());
        let validator_set = Arc::new(ValidatorSet::new(params.byzantine_threshold));
        
        Self {
            consensus_engine: Arc::new(RwLock::new(consensus_engine)),
            validator_set,
            participant_id,
            params,
        }
    }
    
    /// Switch consensus engine
    pub async fn switch_consensus_engine(&self, engine_type: ConsensusEngineType) -> GarpResult<()> {
        let new_engine = ConsensusFactory::create_engine(engine_type, self.participant_id.clone(), self.params.clone());
        let mut consensus_engine = self.consensus_engine.write().await;
        *consensus_engine = new_engine;
        Ok(())
    }
    
    /// Get current consensus engine type
    pub async fn get_consensus_engine_type(&self) -> ConsensusEngineType {
        let consensus_engine = self.consensus_engine.read().await;
        consensus_engine.engine_type()
    }
    
    /// Add validator
    pub async fn add_validator(&self, validator: ValidatorInfo) -> GarpResult<()> {
        self.validator_set.add_validator(validator).await
    }
    
    /// Remove validator
    pub async fn remove_validator(&self, id: &ParticipantId) -> GarpResult<()> {
        self.validator_set.remove_validator(id).await
    }
    
    /// Update validator status
    pub async fn update_validator_status(&self, id: &ParticipantId, status: ValidatorStatus) -> GarpResult<()> {
        self.validator_set.update_validator_status(id, status).await
    }
    
    /// Update validator voting power
    pub async fn update_voting_power(&self, id: &ParticipantId, voting_power: u64) -> GarpResult<()> {
        self.validator_set.update_voting_power(id, voting_power).await
    }
    
    /// Update validator reputation score
    pub async fn update_reputation_score(&self, id: &ParticipantId, score: u32) -> GarpResult<()> {
        self.validator_set.update_reputation_score(id, score).await
    }
    
    /// Record successful proposal
    pub async fn record_successful_proposal(&self, id: &ParticipantId) -> GarpResult<()> {
        self.validator_set.record_successful_proposal(id).await
    }
    
    /// Record failed proposal
    pub async fn record_failed_proposal(&self, id: &ParticipantId) -> GarpResult<()> {
        self.validator_set.record_failed_proposal(id).await
    }
    
    /// Record missed vote
    pub async fn record_missed_vote(&self, id: &ParticipantId) -> GarpResult<()> {
        self.validator_set.record_missed_vote(id).await
    }
    
    /// Apply slashing to validator
    pub async fn apply_slashing(
        &self, 
        id: &ParticipantId, 
        evidence_type: EvidenceType, 
        penalty_bp: u32, 
        reason: String
    ) -> GarpResult<()> {
        self.validator_set.apply_slashing(id, evidence_type, penalty_bp, reason).await
    }
    
    /// Get validator info
    pub async fn get_validator(&self, id: &ParticipantId) -> Option<ValidatorInfo> {
        self.validator_set.get_validator(id).await
    }
    
    /// Get all active validators
    pub async fn get_active_validators(&self) -> Vec<ValidatorInfo> {
        self.validator_set.get_active_validators().await
    }
    
    /// Get total voting power
    pub async fn get_total_voting_power(&self) -> u64 {
        self.validator_set.get_total_voting_power().await
    }
    
    /// Get required votes for consensus
    pub async fn get_required_votes(&self) -> usize {
        self.validator_set.get_required_votes().await
    }
    
    /// Submit a proposal for consensus
    pub async fn submit_proposal(&self, proposal_data: Vec<u8>) -> GarpResult<()> {
        // This would create and submit a proper consensus proposal
        // For now, we'll just delegate to the consensus engine
        let consensus_engine = self.consensus_engine.read().await;
        // In a real implementation, we would create a proper ConsensusProposal here
        Ok(())
    }
    
    /// Cast a vote on a proposal
    pub async fn cast_vote(&self, proposal_id: String, vote: bool) -> GarpResult<()> {
        // This would create and submit a proper consensus vote
        // For now, we'll just delegate to the consensus engine
        let consensus_engine = self.consensus_engine.read().await;
        // In a real implementation, we would create a proper ConsensusVote here
        Ok(())
    }
    
    /// Get consensus parameters
    pub async fn get_params(&self) -> ConsensusParams {
        let consensus_engine = self.consensus_engine.read().await;
        consensus_engine.get_params()
    }
    
    /// Update consensus parameters
    pub async fn update_params(&mut self, new_params: ConsensusParams) -> GarpResult<()> {
        self.params = new_params.clone();
        // If we need to recreate the consensus engine with new params, we can do so
        let engine_type = self.get_consensus_engine_type().await;
        let new_engine = ConsensusFactory::create_engine(engine_type, self.participant_id.clone(), new_params);
        let mut consensus_engine = self.consensus_engine.write().await;
        *consensus_engine = new_engine;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::ConsensusEngineType;
    use crate::types::ParticipantId;
    
    #[tokio::test]
    async fn test_consensus_manager_creation() {
        let manager = ConsensusManager::new(
            ParticipantId::new("test-node".to_string()),
            ConsensusEngineType::ProofOfStakeholder,
            ConsensusParams::default()
        );
        
        assert_eq!(manager.get_consensus_engine_type().await, ConsensusEngineType::ProofOfStakeholder);
    }
    
    #[tokio::test]
    async fn test_consensus_engine_switching() {
        let manager = ConsensusManager::new(
            ParticipantId::new("test-node".to_string()),
            ConsensusEngineType::ProofOfStakeholder,
            ConsensusParams::default()
        );
        
        assert_eq!(manager.get_consensus_engine_type().await, ConsensusEngineType::ProofOfStakeholder);
        
        // Switch to Raft consensus
        assert!(manager.switch_consensus_engine(ConsensusEngineType::Raft).await.is_ok());
        assert_eq!(manager.get_consensus_engine_type().await, ConsensusEngineType::Raft);
    }
    
    #[tokio::test]
    async fn test_validator_management() {
        let manager = ConsensusManager::new(
            ParticipantId::new("test-node".to_string()),
            ConsensusEngineType::ProofOfStakeholder,
            ConsensusParams::default()
        );
        
        let validator = ValidatorInfo {
            id: ParticipantId::new("validator-1".to_string()),
            public_key_hex: "0123456789abcdef".to_string(),
            voting_power: 100,
            status: ValidatorStatus::Active,
            joined_at: chrono::Utc::now(),
            metadata: std::collections::HashMap::new(),
            reputation_score: 50,
            successful_proposals: 0,
            failed_proposals: 0,
            missed_votes: 0,
            last_seen: chrono::Utc::now(),
            slashing_history: Vec::new(),
        };
        
        // Add validator
        assert!(manager.add_validator(validator).await.is_ok());
        
        // Get validator
        let retrieved = manager.get_validator(&ParticipantId::new("validator-1".to_string())).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().voting_power, 100);
    }
}