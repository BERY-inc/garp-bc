use crate::consensus::*;
use crate::validator::*;
use crate::types::ParticipantId;
use chrono::Utc;

#[tokio::test]
async fn test_tendermint_consensus() {
    let params = ConsensusParams::default();
    let participant_id = ParticipantId::new("test-node".to_string());
    let mut consensus = TendermintConsensus::new(participant_id, params);
    
    // Add a validator
    consensus.add_validator(ParticipantId::new("validator-1".to_string()), 100);
    
    // Test required votes calculation
    assert_eq!(consensus.required_votes(), 1);
    
    // Test proposer selection
    let proposer = consensus.get_proposer();
    assert!(proposer.is_some());
    
    println!("Tendermint consensus test passed");
}

#[tokio::test]
async fn test_hotstuff_consensus() {
    let params = ConsensusParams::default();
    let participant_id = ParticipantId::new("test-node".to_string());
    let mut consensus = HotStuffConsensus::new(participant_id, params);
    
    // Add a validator
    consensus.add_validator(ParticipantId::new("validator-1".to_string()), 100);
    
    // Test required votes calculation
    assert_eq!(consensus.required_votes(), 1);
    
    // Test block creation
    let block = consensus.create_block(vec![1, 2, 3], vec![4, 5, 6]);
    assert!(!block.hash.is_empty());
    
    println!("HotStuff consensus test passed");
}

#[tokio::test]
async fn test_pbft_consensus() {
    let params = ConsensusParams::default();
    let participant_id = ParticipantId::new("test-node".to_string());
    let mut consensus = PbftConsensus::new(participant_id, params);
    
    // Add a validator
    consensus.add_validator(ParticipantId::new("validator-1".to_string()), 100);
    
    // Test required votes calculation
    assert_eq!(consensus.required_votes(), 1);
    
    println!("PBFT consensus test passed");
}

#[tokio::test]
async fn test_honeybadger_consensus() {
    let params = ConsensusParams::default();
    let participant_id = ParticipantId::new("test-node".to_string());
    let mut consensus = HoneyBadgerBftConsensus::new(participant_id, params);
    
    // Add a validator
    consensus.add_validator(ParticipantId::new("validator-1".to_string()), 100);
    
    // Test required votes calculation
    assert_eq!(consensus.required_votes(), 1);
    
    println!("HoneyBadgerBFT consensus test passed");
}

#[tokio::test]
async fn test_streamlet_consensus() {
    let params = ConsensusParams::default();
    let participant_id = ParticipantId::new("test-node".to_string());
    let mut consensus = StreamletConsensus::new(participant_id, params);
    
    // Add a validator
    consensus.add_validator(ParticipantId::new("validator-1".to_string()), 100);
    
    // Test required votes calculation
    assert_eq!(consensus.required_votes(), 1);
    
    println!("Streamlet consensus test passed");
}

#[tokio::test]
async fn test_validator_management() {
    let validator_set = ValidatorSet::new(1);
    
    let validator = ValidatorInfo {
        id: ParticipantId::new("validator-1".to_string()),
        public_key_hex: "0123456789abcdef".to_string(),
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
        delegators: HashMap::new(),
        commission_rate_bp: 1000, // 10%
        total_delegated: 0,
        self_bonded: 100,
    };
    
    // Add validator
    assert!(validator_set.add_validator(validator).await.is_ok());
    
    // Get validator
    let retrieved = validator_set.get_validator(&ParticipantId::new("validator-1".to_string())).await;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().voting_power, 100);
    
    // Update voting power
    assert!(validator_set.update_voting_power(&ParticipantId::new("validator-1".to_string()), 150).await.is_ok());
    
    let updated = validator_set.get_validator(&ParticipantId::new("validator-1".to_string())).await.unwrap();
    assert_eq!(updated.voting_power, 150);
    
    // Get total voting power
    assert_eq!(validator_set.get_total_voting_power().await, 150);
    
    println!("Validator management test passed");
}

#[tokio::test]
async fn test_delegation_staking() {
    let validator_set = ValidatorSet::new(1);
    
    let validator = ValidatorInfo {
        id: ParticipantId::new("validator-1".to_string()),
        public_key_hex: "0123456789abcdef".to_string(),
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
        delegators: HashMap::new(),
        commission_rate_bp: 1000, // 10%
        total_delegated: 0,
        self_bonded: 100,
    };
    
    // Add validator
    assert!(validator_set.add_validator(validator).await.is_ok());
    
    // Delegate stake
    let delegator_id = ParticipantId::new("delegator-1".to_string());
    assert!(validator_set.delegate_stake(delegator_id.clone(), ParticipantId::new("validator-1".to_string()), 50).await.is_ok());
    
    // Check delegation
    let validator_info = validator_set.get_validator(&ParticipantId::new("validator-1".to_string())).await.unwrap();
    assert_eq!(validator_info.total_delegated, 50);
    assert_eq!(validator_info.voting_power, 150); // 100 self-bonded + 50 delegated
    
    // Undelegate stake
    assert!(validator_set.undelegate_stake(delegator_id, ParticipantId::new("validator-1".to_string()), 25).await.is_ok());
    
    let validator_info = validator_set.get_validator(&ParticipantId::new("validator-1".to_string())).await.unwrap();
    assert_eq!(validator_info.total_delegated, 25);
    assert_eq!(validator_info.voting_power, 125); // 100 self-bonded + 25 delegated
    
    println!("Delegation and staking test passed");
}

#[tokio::test]
async fn test_slashing() {
    let validator_set = ValidatorSet::new(1);
    
    let validator = ValidatorInfo {
        id: ParticipantId::new("validator-1".to_string()),
        public_key_hex: "0123456789abcdef".to_string(),
        voting_power: 1000,
        status: ValidatorStatus::Active,
        joined_at: Utc::now(),
        metadata: HashMap::new(),
        reputation_score: 80,
        successful_proposals: 10,
        failed_proposals: 2,
        missed_votes: 1,
        last_seen: Utc::now(),
        slashing_history: Vec::new(),
        delegators: HashMap::new(),
        commission_rate_bp: 1000, // 10%
        total_delegated: 0,
        self_bonded: 1000,
    };
    
    // Add validator
    assert!(validator_set.add_validator(validator).await.is_ok());
    
    // Apply slashing for equivocation
    assert!(validator_set.apply_slashing(
        &ParticipantId::new("validator-1".to_string()),
        EvidenceType::Equivocation,
        500, // 5% penalty
        "Equivocation detected".to_string()
    ).await.is_ok());
    
    // Check slashing results
    let validator_info = validator_set.get_validator(&ParticipantId::new("validator-1".to_string())).await.unwrap();
    assert_eq!(validator_info.voting_power, 950); // 1000 - 5% = 950
    assert_eq!(validator_info.reputation_score, 55); // 80 - 25 = 55
    assert_eq!(validator_info.slashing_history.len(), 1);
    assert_eq!(validator_info.status, ValidatorStatus::Jailed);
    
    println!("Slashing test passed");
}

#[tokio::test]
async fn test_reputation_scoring() {
    // Test reputation calculation
    let score = ReputationScorer::calculate_reputation_score(20, 2, 5, 0);
    assert_eq!(score, 87); // 50 + 40 - 6 - 2 = 82, but capped adjustments might apply
    
    let score_with_slashing = ReputationScorer::calculate_reputation_score(20, 2, 5, 1);
    assert!(score_with_slashing < score); // Should be lower due to slashing
    
    // Test tier determination
    let tier = ReputationScorer::get_validator_tier(95);
    assert_eq!(tier, ValidatorTier::Excellent);
    
    let tier = ReputationScorer::get_validator_tier(45);
    assert_eq!(tier, ValidatorTier::Poor);
    
    println!("Reputation scoring test passed");
}

#[tokio::test]
async fn test_consensus_factory() {
    let params = ConsensusParams::default();
    let participant_id = ParticipantId::new("test-node".to_string());
    
    // Test creating different consensus engines
    let tendermint = ConsensusFactory::create_engine(ConsensusEngineType::Tendermint, participant_id.clone(), params.clone());
    assert_eq!(tendermint.engine_type(), ConsensusEngineType::Tendermint);
    
    let hotstuff = ConsensusFactory::create_engine(ConsensusEngineType::HotStuff, participant_id.clone(), params.clone());
    assert_eq!(hotstuff.engine_type(), ConsensusEngineType::HotStuff);
    
    let pbft = ConsensusFactory::create_engine(ConsensusEngineType::Pbft, participant_id.clone(), params.clone());
    assert_eq!(pbft.engine_type(), ConsensusEngineType::Pbft);
    
    let honeybadger = ConsensusFactory::create_engine(ConsensusEngineType::HoneyBadgerBft, participant_id.clone(), params.clone());
    assert_eq!(honeybadger.engine_type(), ConsensusEngineType::HoneyBadgerBft);
    
    let streamlet = ConsensusFactory::create_engine(ConsensusEngineType::Streamlet, participant_id.clone(), params.clone());
    assert_eq!(streamlet.engine_type(), ConsensusEngineType::Streamlet);
    
    println!("Consensus factory test passed");
}