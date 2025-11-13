//! Example usage of the pluggable consensus system with validator management
//!
//! This example demonstrates how to:
//! 1. Create a consensus manager with different consensus engines
//! 2. Manage validators dynamically
//! 3. Apply slashing conditions
//! 4. Use reputation scoring

use garp_common::{
    ConsensusManager, 
    ConsensusEngineType, 
    ConsensusParams, 
    ValidatorInfo, 
    ValidatorStatus, 
    EvidenceType,
    ParticipantId
};
use std::collections::HashMap;
use chrono::Utc;

/// Example of setting up and using the consensus manager
pub async fn consensus_manager_example() {
    println!("=== Consensus Manager Example ===");
    
    // Create consensus parameters
    let params = ConsensusParams {
        timeout_seconds: 30,
        threshold: 0.67,
        max_validators: 100,
        min_validators: 4,
        weighted_voting: true,
        require_unanimous: false,
        allow_abstain: false,
        max_concurrent_sessions: 1000,
        byzantine_threshold: 1,
        quorum_ratio_thousandths: 667,
        max_view_changes: 10,
        liveness_timeout_ms: 10000,
    };
    
    // Create consensus manager with Tendermint consensus
    let manager = ConsensusManager::new(
        ParticipantId::new("node-1".to_string()),
        ConsensusEngineType::Tendermint,
        params.clone()
    );
    
    println!("Created consensus manager with Tendermint consensus");
    
    // Add validators
    let validators = create_sample_validators();
    for validator in validators {
        if let Err(e) = manager.add_validator(validator).await {
            println!("Error adding validator: {}", e);
        }
    }
    
    println!("Added {} validators", manager.get_active_validators().await.len());
    
    // Switch to different consensus engine
    if let Err(e) = manager.switch_consensus_engine(ConsensusEngineType::HotStuff).await {
        println!("Error switching consensus engine: {}", e);
    } else {
        println!("Switched to HotStuff consensus");
    }
    
    // Update validator voting power
    let validator_id = ParticipantId::new("validator-1".to_string());
    if let Err(e) = manager.update_voting_power(&validator_id, 200).await {
        println!("Error updating voting power: {}", e);
    } else {
        println!("Updated voting power for validator-1");
    }
    
    // Apply slashing for misbehavior
    let evidence_type = EvidenceType::DoubleSign;
    if let Err(e) = manager.apply_slashing(
        &validator_id, 
        evidence_type, 
        1000, // 10% penalty in basis points
        "Double signing detected".to_string()
    ).await {
        println!("Error applying slashing: {}", e);
    } else {
        println!("Applied slashing to validator-1 for double signing");
    }
    
    // Update reputation score
    if let Err(e) = manager.update_reputation_score(&validator_id, 75).await {
        println!("Error updating reputation score: {}", e);
    } else {
        println!("Updated reputation score for validator-1");
    }
    
    // Get validator info
    if let Some(validator) = manager.get_validator(&validator_id).await {
        println!("Validator info:");
        println!("  ID: {}", validator.id.0);
        println!("  Voting Power: {}", validator.voting_power);
        println!("  Status: {:?}", validator.status);
        println!("  Reputation Score: {}", validator.reputation_score);
        println!("  Successful Proposals: {}", validator.successful_proposals);
        println!("  Failed Proposals: {}", validator.failed_proposals);
        println!("  Missed Votes: {}", validator.missed_votes);
        println!("  Slashing Events: {}", validator.slashing_history.len());
    }
    
    // Get consensus parameters
    let current_params = manager.get_params().await;
    println!("Current consensus parameters:");
    println!("  Timeout: {} seconds", current_params.timeout_seconds);
    println!("  Threshold: {}", current_params.threshold);
    println!("  Byzantine Threshold: {}", current_params.byzantine_threshold);
}

/// Create sample validators for testing
fn create_sample_validators() -> Vec<ValidatorInfo> {
    let mut validators = Vec::new();
    
    for i in 1..=5 {
        let validator = ValidatorInfo {
            id: ParticipantId::new(format!("validator-{}", i)),
            public_key_hex: format!("pubkey-{}", i),
            voting_power: 100,
            status: ValidatorStatus::Active,
            joined_at: Utc::now(),
            metadata: HashMap::new(),
            reputation_score: 50,
            successful_proposals: 0,
            failed_proposals: 0,
            missed_votes: 0,
            last_seen: Utc::now(),
            slashing_history: Vec::new(),
        };
        validators.push(validator);
    }
    
    validators
}

/// Example of different consensus engines
pub async fn consensus_engine_examples() {
    println!("\n=== Consensus Engine Examples ===");
    
    let params = ConsensusParams::default();
    let participant_id = ParticipantId::new("test-node".to_string());
    
    // Proof of Stakeholder consensus
    let pos_manager = ConsensusManager::new(
        participant_id.clone(),
        ConsensusEngineType::ProofOfStakeholder,
        params.clone()
    );
    println!("Created Proof of Stakeholder consensus manager");
    
    // Raft consensus
    let raft_manager = ConsensusManager::new(
        participant_id.clone(),
        ConsensusEngineType::Raft,
        params.clone()
    );
    println!("Created Raft consensus manager");
    
    // Tendermint consensus
    let tendermint_manager = ConsensusManager::new(
        participant_id.clone(),
        ConsensusEngineType::Tendermint,
        params.clone()
    );
    println!("Created Tendermint consensus manager");
    
    // HotStuff consensus
    let hotstuff_manager = ConsensusManager::new(
        participant_id.clone(),
        ConsensusEngineType::HotStuff,
        params.clone()
    );
    println!("Created HotStuff consensus manager");
    
    // Practical BFT consensus
    let pbft_manager = ConsensusManager::new(
        participant_id,
        ConsensusEngineType::PracticalBFT,
        params
    );
    println!("Created Practical BFT consensus manager");
}

/// Example of validator reputation scoring
pub fn reputation_scoring_example() {
    println!("\n=== Reputation Scoring Example ===");
    
    // Example validator performance metrics
    let successful_proposals = 50;
    let failed_proposals = 5;
    let missed_votes = 10;
    let slashing_events = 1;
    
    // Calculate reputation score
    let score = garp_common::validator::ReputationScorer::calculate_reputation_score(
        successful_proposals,
        failed_proposals,
        missed_votes,
        slashing_events
    );
    
    println!("Validator Performance Metrics:");
    println!("  Successful Proposals: {}", successful_proposals);
    println!("  Failed Proposals: {}", failed_proposals);
    println!("  Missed Votes: {}", missed_votes);
    println!("  Slashing Events: {}", slashing_events);
    println!("  Calculated Reputation Score: {}", score);
    
    // Get validator tier
    let tier = garp_common::validator::ReputationScorer::get_validator_tier(score);
    println!("  Validator Tier: {:?}", tier);
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_consensus_manager_example() {
        consensus_manager_example().await;
    }
    
    #[tokio::test]
    async fn test_consensus_engine_examples() {
        consensus_engine_examples().await;
    }
    
    #[test]
    fn test_reputation_scoring_example() {
        reputation_scoring_example();
    }
}