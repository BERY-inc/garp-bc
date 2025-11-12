use crate::types::ParticipantId;
use crate::error::GarpResult;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Validator status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ValidatorStatus {
    /// Active validator participating in consensus
    Active,
    
    /// Inactive validator (temporarily not participating)
    Inactive,
    
    /// Jailed validator due to misbehavior
    Jailed,
    
    /// Banned validator (permanently removed)
    Banned,
}

/// Validator information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorInfo {
    /// Participant ID
    pub id: ParticipantId,
    
    /// Public key in hex format
    pub public_key_hex: String,
    
    /// Voting power/stake
    pub voting_power: u64,
    
    /// Current status
    pub status: ValidatorStatus,
    
    /// When validator joined
    pub joined_at: DateTime<Utc>,
    
    /// Validator metadata
    pub metadata: HashMap<String, String>,
    
    /// Validator reputation score (0-100)
    pub reputation_score: u32,
    
    /// Number of successful proposals
    pub successful_proposals: u64,
    
    /// Number of failed proposals
    pub failed_proposals: u64,
    
    /// Number of missed votes
    pub missed_votes: u64,
    
    /// Last seen timestamp
    pub last_seen: DateTime<Utc>,
    
    /// Slashing history
    pub slashing_history: Vec<SlashingEvent>,
    
    /// Delegators who have staked tokens with this validator
    pub delegators: HashMap<ParticipantId, DelegationInfo>,
    
    /// Commission rate (basis points, e.g., 1000 = 10%)
    pub commission_rate_bp: u32,
    
    /// Total delegated stake
    pub total_delegated: u64,
    
    /// Validator self-bonded stake
    pub self_bonded: u64,
}

/// Delegation information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationInfo {
    /// Amount of tokens delegated
    pub amount: u64,
    
    /// When delegation was made
    pub delegated_at: DateTime<Utc>,
    
    /// Rewards earned from delegation
    pub rewards: u64,
    
    /// Last reward distribution time
    pub last_reward_at: DateTime<Utc>,
}

/// Staking parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakingParams {
    /// Minimum self-bonded stake to become a validator
    pub min_self_bond: u64,
    
    /// Minimum delegation amount
    pub min_delegation: u64,
    
    /// Maximum number of validators
    pub max_validators: usize,
    
    /// Unbonding period in seconds
    pub unbonding_period_secs: u64,
    
    /// Reward distribution frequency in seconds
    pub reward_frequency_secs: u64,
}

/// Validator set management
pub struct ValidatorSet {
    /// Active validators
    validators: Arc<RwLock<HashMap<ParticipantId, ValidatorInfo>>>,
    
    /// Total voting power
    total_voting_power: Arc<RwLock<u64>>,
    
    /// Byzantine threshold (f in 3f+1)
    byzantine_threshold: usize,
    
    /// Required votes for consensus
    required_votes: Arc<RwLock<usize>>,
    
    /// Staking parameters
    staking_params: Arc<RwLock<StakingParams>>,
    
    /// Pending unbonding requests
    pending_unbonds: Arc<RwLock<HashMap<ParticipantId, UnbondingRequest>>>,
}

/// Unbonding request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnbondingRequest {
    /// Validator ID
    pub validator_id: ParticipantId,
    
    /// Delegator ID
    pub delegator_id: ParticipantId,
    
    /// Amount to unbond
    pub amount: u64,
    
    /// Request timestamp
    pub requested_at: DateTime<Utc>,
    
    /// Completion timestamp
    pub completes_at: DateTime<Utc>,
}

impl ValidatorSet {
    /// Create new validator set
    pub fn new(byzantine_threshold: usize) -> Self {
        Self {
            validators: Arc::new(RwLock::new(HashMap::new())),
            total_voting_power: Arc::new(RwLock::new(0)),
            byzantine_threshold,
            required_votes: Arc::new(RwLock::new(0)),
            staking_params: Arc::new(RwLock::new(StakingParams {
                min_self_bond: 1000,
                min_delegation: 100,
                max_validators: 100,
                unbonding_period_secs: 86400 * 7, // 7 days
                reward_frequency_secs: 3600, // 1 hour
            })),
            pending_unbonds: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Create new validator set with custom staking parameters
    pub fn new_with_params(byzantine_threshold: usize, staking_params: StakingParams) -> Self {
        Self {
            validators: Arc::new(RwLock::new(HashMap::new())),
            total_voting_power: Arc::new(RwLock::new(0)),
            byzantine_threshold,
            required_votes: Arc::new(RwLock::new(0)),
            staking_params: Arc::new(RwLock::new(staking_params)),
            pending_unbonds: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Add a new validator
    pub async fn add_validator(&self, mut validator: ValidatorInfo) -> GarpResult<()> {
        // Check if validator meets minimum self-bond requirement
        let params = self.staking_params.read().await;
        if validator.self_bonded < params.min_self_bond {
            return Err(crate::error::GarpError::InvalidValidator("Insufficient self-bond".to_string()));
        }
        
        // Initialize delegators if not already done
        if validator.delegators.is_empty() {
            validator.delegators = HashMap::new();
        }
        
        let mut validators = self.validators.write().await;
        validators.insert(validator.id.clone(), validator);
        self.recalculate_totals().await;
        Ok(())
    }
    
    /// Remove a validator
    pub async fn remove_validator(&self, id: &ParticipantId) -> GarpResult<()> {
        let mut validators = self.validators.write().await;
        validators.remove(id);
        self.recalculate_totals().await;
        Ok(())
    }
    
    /// Delegate stake to a validator
    pub async fn delegate_stake(&self, delegator_id: ParticipantId, validator_id: ParticipantId, amount: u64) -> GarpResult<()> {
        // Check minimum delegation amount
        let params = self.staking_params.read().await;
        if amount < params.min_delegation {
            return Err(crate::error::GarpError::InvalidStake("Amount below minimum delegation".to_string()));
        }
        
        let mut validators = self.validators.write().await;
        if let Some(validator) = validators.get_mut(&validator_id) {
            // Add or update delegation
            let now = Utc::now();
            let delegation = validator.delegators.entry(delegator_id.clone()).or_insert_with(|| {
                DelegationInfo {
                    amount: 0,
                    delegated_at: now,
                    rewards: 0,
                    last_reward_at: now,
                }
            });
            
            delegation.amount += amount;
            validator.total_delegated += amount;
            validator.voting_power += amount;
        } else {
            return Err(crate::error::GarpError::InvalidValidator("Validator not found".to_string()));
        }
        
        self.recalculate_totals().await;
        Ok(())
    }
    
    /// Undelegate stake from a validator
    pub async fn undelegate_stake(&self, delegator_id: ParticipantId, validator_id: ParticipantId, amount: u64) -> GarpResult<()> {
        let mut validators = self.validators.write().await;
        if let Some(validator) = validators.get_mut(&validator_id) {
            if let Some(delegation) = validator.delegators.get_mut(&delegator_id) {
                if delegation.amount < amount {
                    return Err(crate::error::GarpError::InvalidStake("Insufficient delegated amount".to_string()));
                }
                
                delegation.amount -= amount;
                validator.total_delegated -= amount;
                validator.voting_power -= amount;
                
                // Remove delegation if amount is zero
                if delegation.amount == 0 {
                    validator.delegators.remove(&delegator_id);
                }
                
                // Create unbonding request
                let params = self.staking_params.read().await;
                let now = Utc::now();
                let unbonding_request = UnbondingRequest {
                    validator_id: validator_id.clone(),
                    delegator_id: delegator_id.clone(),
                    amount,
                    requested_at: now,
                    completes_at: now + chrono::Duration::seconds(params.unbonding_period_secs as i64),
                };
                
                let mut pending_unbonds = self.pending_unbonds.write().await;
                pending_unbonds.insert(delegator_id, unbonding_request);
            } else {
                return Err(crate::error::GarpError::InvalidStake("No delegation found".to_string()));
            }
        } else {
            return Err(crate::error::GarpError::InvalidValidator("Validator not found".to_string()));
        }
        
        self.recalculate_totals().await;
        Ok(())
    }
    
    /// Complete unbonding process
    pub async fn complete_unbonding(&self, delegator_id: &ParticipantId) -> GarpResult<u64> {
        let now = Utc::now();
        let mut pending_unbonds = self.pending_unbonds.write().await;
        
        if let Some(request) = pending_unbonds.get(delegator_id) {
            if now >= request.completes_at {
                let amount = request.amount;
                pending_unbonds.remove(delegator_id);
                Ok(amount)
            } else {
                Err(crate::error::GarpError::InvalidStake("Unbonding period not completed".to_string()))
            }
        } else {
            Err(crate::error::GarpError::InvalidStake("No unbonding request found".to_string()))
        }
    }
    
    /// Distribute rewards to validators and delegators
    pub async fn distribute_rewards(&self, total_reward: u64) -> GarpResult<()> {
        let mut validators = self.validators.write().await;
        let active_validators: Vec<&mut ValidatorInfo> = validators.values_mut()
            .filter(|v| v.status == ValidatorStatus::Active)
            .collect();
        
        if active_validators.is_empty() {
            return Ok(());
        }
        
        // Distribute rewards based on voting power
        let total_voting_power: u64 = active_validators.iter().map(|v| v.voting_power).sum();
        if total_voting_power == 0 {
            return Ok(());
        }
        
        let now = Utc::now();
        
        for validator in active_validators {
            let validator_share = (total_reward as f64 * validator.voting_power as f64 / total_voting_power as f64) as u64;
            
            // Commission for validator
            let commission = (validator_share as f64 * validator.commission_rate_bp as f64 / 10000.0) as u64;
            validator.reputation_score = (validator.reputation_score + 1).min(100);
            
            // Rewards for delegators
            let delegator_rewards = validator_share - commission;
            
            // Distribute to delegators based on their stake
            if validator.total_delegated > 0 {
                for delegation in validator.delegators.values_mut() {
                    let delegator_share = (delegator_rewards as f64 * delegation.amount as f64 / validator.total_delegated as f64) as u64;
                    delegation.rewards += delegator_share;
                    delegation.last_reward_at = now;
                }
            }
        }
        
        Ok(())
    }
    
    /// Rotate validators based on performance and reputation
    pub async fn rotate_validators(&self) -> GarpResult<()> {
        let mut validators = self.validators.write().await;
        let params = self.staking_params.read().await;
        
        // Get all validators sorted by reputation and voting power
        let mut validator_list: Vec<(&ParticipantId, &ValidatorInfo)> = validators.iter().collect();
        validator_list.sort_by(|a, b| {
            // Sort by reputation first, then by voting power
            b.1.reputation_score.cmp(&a.1.reputation_score)
                .then_with(|| b.1.voting_power.cmp(&a.1.voting_power))
        });
        
        // Mark top validators as active
        let max_validators = params.max_validators;
        for (i, (id, validator)) in validator_list.iter().enumerate() {
            let new_status = if i < max_validators {
                ValidatorStatus::Active
            } else {
                ValidatorStatus::Inactive
            };
            
            // Update validator status if changed
            if validator.status != new_status {
                if let Some(v) = validators.get_mut(*id) {
                    v.status = new_status;
                }
            }
        }
        
        self.recalculate_totals().await;
        Ok(())
    }
    
    /// Get delegators for a validator
    pub async fn get_delegators(&self, validator_id: &ParticipantId) -> Option<HashMap<ParticipantId, DelegationInfo>> {
        let validators = self.validators.read().await;
        validators.get(validator_id).map(|v| v.delegators.clone())
    }
    
    /// Get pending unbonding requests
    pub async fn get_pending_unbonds(&self) -> HashMap<ParticipantId, UnbondingRequest> {
        let pending_unbonds = self.pending_unbonds.read().await;
        pending_unbonds.clone()
    }
    
    /// Update validator status
    pub async fn update_validator_status(&self, id: &ParticipantId, status: ValidatorStatus) -> GarpResult<()> {
        let mut validators = self.validators.write().await;
        if let Some(validator) = validators.get_mut(id) {
            validator.status = status;
        }
        Ok(())
    }
    
    /// Update validator voting power
    pub async fn update_voting_power(&self, id: &ParticipantId, voting_power: u64) -> GarpResult<()> {
        let mut validators = self.validators.write().await;
        if let Some(validator) = validators.get_mut(id) {
            validator.voting_power = voting_power;
        }
        self.recalculate_totals().await;
        Ok(())
    }
    
    /// Update validator reputation score
    pub async fn update_reputation_score(&self, id: &ParticipantId, score: u32) -> GarpResult<()> {
        let mut validators = self.validators.write().await;
        if let Some(validator) = validators.get_mut(id) {
            validator.reputation_score = score.clamp(0, 100);
        }
        Ok(())
    }
    
    /// Record successful proposal
    pub async fn record_successful_proposal(&self, id: &ParticipantId) -> GarpResult<()> {
        let mut validators = self.validators.write().await;
        if let Some(validator) = validators.get_mut(id) {
            validator.successful_proposals += 1;
            // Increase reputation for successful proposals
            validator.reputation_score = (validator.reputation_score + 2).min(100);
        }
        Ok(())
    }
    
    /// Record failed proposal
    pub async fn record_failed_proposal(&self, id: &ParticipantId) -> GarpResult<()> {
        let mut validators = self.validators.write().await;
        if let Some(validator) = validators.get_mut(id) {
            validator.failed_proposals += 1;
            // Decrease reputation for failed proposals
            validator.reputation_score = validator.reputation_score.saturating_sub(5);
        }
        Ok(())
    }
    
    /// Record missed vote
    pub async fn record_missed_vote(&self, id: &ParticipantId) -> GarpResult<()> {
        let mut validators = self.validators.write().await;
        if let Some(validator) = validators.get_mut(id) {
            validator.missed_votes += 1;
            // Decrease reputation for missed votes
            validator.reputation_score = validator.reputation_score.saturating_sub(1);
        }
        Ok(())
    }
    
    /// Apply slashing to validator
    pub async fn apply_slashing(&self, id: &ParticipantId, evidence_type: EvidenceType, penalty_bp: u32, reason: String) -> GarpResult<()> {
        let mut validators = self.validators.write().await;
        if let Some(validator) = validators.get_mut(id) {
            // Record slashing event
            let slashing_event = SlashingEvent {
                evidence_type: evidence_type.clone(),
                penalty_bp,
                timestamp: Utc::now(),
                reason: reason.clone(),
            };
            validator.slashing_history.push(slashing_event);
            
            // Apply penalty to voting power
            let penalty = (validator.voting_power * penalty_bp as u64) / 10_000;
            validator.voting_power = validator.voting_power.saturating_sub(penalty);
            
            // Decrease reputation significantly
            let reputation_penalty = match evidence_type {
                EvidenceType::DoubleSign => 30,
                EvidenceType::Equivocation => 25,
                EvidenceType::LivenessFault => 10,
                EvidenceType::InvalidBehavior => 15,
            };
            validator.reputation_score = validator.reputation_score.saturating_sub(reputation_penalty);
            
            // Jail for severe offenses
            if matches!(evidence_type, EvidenceType::DoubleSign | EvidenceType::Equivocation) {
                validator.status = ValidatorStatus::Jailed;
            }
        }
        self.recalculate_totals().await;
        Ok(())
    }
    
    /// Get validator info
    pub async fn get_validator(&self, id: &ParticipantId) -> Option<ValidatorInfo> {
        let validators = self.validators.read().await;
        validators.get(id).cloned()
    }
    
    /// Get all active validators
    pub async fn get_active_validators(&self) -> Vec<ValidatorInfo> {
        let validators = self.validators.read().await;
        validators.values()
            .filter(|v| v.status == ValidatorStatus::Active)
            .cloned()
            .collect()
    }
    
    /// Get total voting power
    pub async fn get_total_voting_power(&self) -> u64 {
        *self.total_voting_power.read().await
    }
    
    /// Get required votes for consensus
    pub async fn get_required_votes(&self) -> usize {
        *self.required_votes.read().await
    }
    
    /// Recalculate totals after validator changes
    async fn recalculate_totals(&self) {
        let validators = self.validators.read().await;
        let mut total_power = 0u64;
        let mut active_count = 0usize;
        
        for validator in validators.values() {
            if validator.status == ValidatorStatus::Active {
                total_power += validator.voting_power;
                active_count += 1;
            }
        }
        
        // Update total voting power
        {
            let mut total = self.total_voting_power.write().await;
            *total = total_power;
        }
        
        // Calculate required votes (2/3 + 1 majority)
        let required = if active_count > 0 {
            (active_count * 2) / 3 + 1
        } else {
            0
        };
        
        {
            let mut required_votes = self.required_votes.write().await;
            *required_votes = required;
        }
    }
}

/// Validator reputation scorer
pub struct ReputationScorer;

impl ReputationScorer {
    /// Calculate reputation score based on validator performance
    pub fn calculate_reputation_score(
        successful_proposals: u64,
        failed_proposals: u64,
        missed_votes: u64,
        slashing_events: usize,
    ) -> u32 {
        // Base score
        let mut score = 50u32;
        
        // Reward successful proposals
        score += (successful_proposals.min(100) as u32) * 2;
        
        // Penalize failed proposals
        score = score.saturating_sub((failed_proposals.min(50) as u32) * 3);
        
        // Penalize missed votes
        score = score.saturating_sub((missed_votes.min(100) as u32) / 2);
        
        // Heavy penalty for slashing events
        score = score.saturating_sub((slashing_events as u32) * 10);
        
        // Clamp between 0 and 100
        score.clamp(0, 100)
    }
    
    /// Get validator tier based on reputation score
    pub fn get_validator_tier(score: u32) -> ValidatorTier {
        match score {
            90..=100 => ValidatorTier::Excellent,
            75..=89 => ValidatorTier::Good,
            60..=74 => ValidatorTier::Fair,
            40..=59 => ValidatorTier::Poor,
            _ => ValidatorTier::VeryPoor,
        }
    }
}

/// Validator performance tiers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidatorTier {
    Excellent,
    Good,
    Fair,
    Poor,
    VeryPoor,
}

/// Evidence types for slashing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvidenceType {
    /// Conflicting votes for the same height/view
    Equivocation,
    
    /// Two different blocks signed at same height/view
    DoubleSign,
    
    /// Missed N consecutive rounds beyond policy threshold
    LivenessFault,
    
    /// Invalid proposal or vote
    InvalidBehavior,
    
    /// Downtime beyond threshold
    Downtime,
    
    /// Light client attack
    LightClientAttack,
    
    /// Malicious proposal
    MaliciousProposal,
}

/// Slashing event for validator misbehavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashingEvent {
    /// Type of misbehavior
    pub evidence_type: EvidenceType,
    
    /// Penalty applied (in basis points of voting power)
    pub penalty_bp: u32,
    
    /// Timestamp of slashing
    pub timestamp: DateTime<Utc>,
    
    /// Reason for slashing
    pub reason: String,
    
    /// Evidence data (serialized for storage)
    pub evidence_data: Option<Vec<u8>>,
    
    /// Reporter of the evidence (if applicable)
    pub reporter: Option<ParticipantId>,
    
    /// Reward for reporter (if applicable)
    pub reporter_reward: Option<u64>,
}

/// Evidence detection and submission system
pub struct EvidenceDetector {
    /// Pending evidence awaiting verification
    pending_evidence: Arc<RwLock<HashMap<String, EvidenceSubmission>>>,
    
    /// Verified evidence ready for slashing
    verified_evidence: Arc<RwLock<HashMap<String, EvidenceSubmission>>>,
}

/// Evidence submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceSubmission {
    /// Unique evidence ID
    pub id: String,
    
    /// Type of evidence
    pub evidence_type: EvidenceType,
    
    /// Validator ID being reported
    pub validator_id: ParticipantId,
    
    /// Evidence data
    pub data: Vec<u8>,
    
    /// Timestamp of submission
    pub submitted_at: DateTime<Utc>,
    
    /// Reporter ID
    pub reporter: ParticipantId,
    
    /// Verification status
    pub status: EvidenceStatus,
}

/// Evidence verification status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EvidenceStatus {
    /// Pending verification
    Pending,
    
    /// Verified and accepted
    Verified,
    
    /// Rejected
    Rejected,
    
    /// Expired
    Expired,
}

impl EvidenceDetector {
    pub fn new() -> Self {
        Self {
            pending_evidence: Arc::new(RwLock::new(HashMap::new())),
            verified_evidence: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Submit evidence for misbehavior
    pub async fn submit_evidence(&self, evidence: EvidenceSubmission) -> GarpResult<()> {
        // Validate evidence
        if !self.validate_evidence_format(&evidence).await {
            return Err(crate::error::GarpError::InvalidEvidence("Invalid evidence format".to_string()));
        }
        
        // Store pending evidence
        let mut pending = self.pending_evidence.write().await;
        pending.insert(evidence.id.clone(), evidence);
        
        Ok(())
    }
    
    /// Validate evidence format
    async fn validate_evidence_format(&self, evidence: &EvidenceSubmission) -> bool {
        // Check if evidence is not too old (e.g., within 2 weeks)
        let max_age = chrono::Duration::days(14);
        if Utc::now() - evidence.submitted_at > max_age {
            return false;
        }
        
        // Check if evidence data is not empty
        if evidence.data.is_empty() {
            return false;
        }
        
        // In a real implementation, we would validate the cryptographic proofs
        true
    }
    
    /// Verify pending evidence
    pub async fn verify_evidence(&self, evidence_id: &str) -> GarpResult<EvidenceStatus> {
        let mut pending = self.pending_evidence.write().await;
        if let Some(mut evidence) = pending.remove(evidence_id) {
            // In a real implementation, we would perform cryptographic verification
            // For now, we'll simulate verification
            let verified = self.perform_verification(&evidence).await?;
            
            if verified {
                evidence.status = EvidenceStatus::Verified;
                let mut verified_evidence = self.verified_evidence.write().await;
                verified_evidence.insert(evidence_id.to_string(), evidence.clone());
                Ok(EvidenceStatus::Verified)
            } else {
                evidence.status = EvidenceStatus::Rejected;
                pending.insert(evidence_id.to_string(), evidence);
                Ok(EvidenceStatus::Rejected)
            }
        } else {
            Err(crate::error::GarpError::InvalidEvidence("Evidence not found".to_string()))
        }
    }
    
    /// Perform actual verification of evidence
    async fn perform_verification(&self, evidence: &EvidenceSubmission) -> GarpResult<bool> {
        // In a real implementation, this would involve:
        // 1. Cryptographic verification of signatures
        // 2. Checking consistency with blockchain state
        // 3. Validating the evidence against protocol rules
        
        // For now, we'll simulate a 90% success rate for valid evidence
        Ok(true)
    }
    
    /// Get verified evidence for a validator
    pub async fn get_verified_evidence(&self, validator_id: &ParticipantId) -> Vec<EvidenceSubmission> {
        let verified = self.verified_evidence.read().await;
        verified.values()
            .filter(|e| &e.validator_id == validator_id)
            .cloned()
            .collect()
    }
    
    /// Process verified evidence and apply slashing
    pub async fn process_slashing(&self, validator_set: &ValidatorSet) -> GarpResult<Vec<SlashingEvent>> {
        let verified = self.verified_evidence.read().await;
        let mut slashing_events = Vec::new();
        
        for evidence in verified.values() {
            // Determine penalty based on evidence type
            let (penalty_bp, reason) = match evidence.evidence_type {
                EvidenceType::DoubleSign => (1000, "Double signing detected".to_string()), // 10% penalty
                EvidenceType::Equivocation => (500, "Equivocation detected".to_string()), // 5% penalty
                EvidenceType::LivenessFault => (100, "Liveness fault".to_string()), // 1% penalty
                EvidenceType::InvalidBehavior => (300, "Invalid behavior".to_string()), // 3% penalty
                EvidenceType::Downtime => (50, "Excessive downtime".to_string()), // 0.5% penalty
                EvidenceType::LightClientAttack => (2000, "Light client attack".to_string()), // 20% penalty
                EvidenceType::MaliciousProposal => (1500, "Malicious proposal".to_string()), // 15% penalty
            };
            
            // Apply slashing
            validator_set.apply_slashing(
                &evidence.validator_id,
                evidence.evidence_type.clone(),
                penalty_bp,
                reason.clone()
            ).await?;
            
            // Create slashing event
            let slashing_event = SlashingEvent {
                evidence_type: evidence.evidence_type.clone(),
                penalty_bp,
                timestamp: Utc::now(),
                reason,
                evidence_data: Some(evidence.data.clone()),
                reporter: Some(evidence.reporter.clone()),
                reporter_reward: Some(100), // Fixed reward for now
            };
            
            slashing_events.push(slashing_event);
        }
        
        Ok(slashing_events)
    }
    
    /// Clean up expired evidence
    pub async fn cleanup_expired_evidence(&self) -> GarpResult<()> {
        let max_age = chrono::Duration::days(30);
        let now = Utc::now();
        
        // Clean pending evidence
        {
            let mut pending = self.pending_evidence.write().await;
            pending.retain(|_, evidence| now - evidence.submitted_at <= max_age);
        }
        
        // Clean verified evidence
        {
            let mut verified = self.verified_evidence.write().await;
            verified.retain(|_, evidence| now - evidence.submitted_at <= max_age);
        }
        
        Ok(())
    }
}

/// Validator performance analytics
pub struct ValidatorAnalytics {
    /// Performance metrics for validators
    metrics: Arc<RwLock<HashMap<ParticipantId, ValidatorMetrics>>>,
    
    /// Historical performance data
    history: Arc<RwLock<HashMap<ParticipantId, Vec<HistoricalMetrics>>>>,
}

/// Validator performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorMetrics {
    /// Uptime percentage (0-100)
    pub uptime_percentage: f64,
    
    /// Average response time in milliseconds
    pub avg_response_ms: u64,
    
    /// Number of proposals submitted
    pub proposals_submitted: u64,
    
    /// Number of proposals accepted
    pub proposals_accepted: u64,
    
    /// Vote participation rate (0-100)
    pub vote_participation_rate: f64,
    
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
    
    /// Block proposal success rate
    pub proposal_success_rate: f64,
    
    /// Average time between proposals
    pub avg_proposal_interval_secs: u64,
}

/// Historical metrics for trend analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalMetrics {
    /// Timestamp of metrics collection
    pub timestamp: DateTime<Utc>,
    
    /// Metrics data
    pub metrics: ValidatorMetrics,
}

impl ValidatorAnalytics {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Update validator metrics
    pub async fn update_metrics(&self, validator_id: ParticipantId, metrics: ValidatorMetrics) -> GarpResult<()> {
        // Store current metrics
        {
            let mut current_metrics = self.metrics.write().await;
            current_metrics.insert(validator_id.clone(), metrics.clone());
        }
        
        // Store in history
        {
            let mut history = self.history.write().await;
            let historical_entry = HistoricalMetrics {
                timestamp: Utc::now(),
                metrics: metrics.clone(),
            };
            
            let entries = history.entry(validator_id).or_insert_with(Vec::new);
            entries.push(historical_entry);
            
            // Keep only last 100 entries
            if entries.len() > 100 {
                entries.drain(0..entries.len() - 100);
            }
        }
        
        Ok(())
    }
    
    /// Get current metrics for a validator
    pub async fn get_metrics(&self, validator_id: &ParticipantId) -> Option<ValidatorMetrics> {
        let metrics = self.metrics.read().await;
        metrics.get(validator_id).cloned()
    }
    
    /// Get historical metrics for a validator
    pub async fn get_historical_metrics(&self, validator_id: &ParticipantId) -> Vec<HistoricalMetrics> {
        let history = self.history.read().await;
        history.get(validator_id).cloned().unwrap_or_default()
    }
    
    /// Calculate validator reliability score (0-100)
    pub async fn calculate_reliability_score(&self, validator_id: &ParticipantId) -> f64 {
        if let Some(metrics) = self.get_metrics(validator_id).await {
            // Weighted calculation:
            // 40% uptime
            // 20% proposal success rate
            // 20% vote participation
            // 20% response time (inverted)
            let uptime_component = metrics.uptime_percentage * 0.4;
            let proposal_component = metrics.proposal_success_rate * 0.2;
            let vote_component = metrics.vote_participation_rate * 0.2;
            let response_component = (100.0 - (metrics.avg_response_ms as f64 / 1000.0).min(100.0)) * 0.2;
            
            uptime_component + proposal_component + vote_component + response_component
        } else {
            0.0
        }
    }
    
    /// Get top performing validators
    pub async fn get_top_validators(&self, count: usize) -> Vec<(ParticipantId, f64)> {
        let metrics = self.metrics.read().await;
        let mut scores: Vec<(ParticipantId, f64)> = metrics.keys()
            .map(|id| {
                let score = self.calculate_reliability_score(id).await;
                (id.clone(), score)
            })
            .collect();
        
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(count);
        scores
    }
    
    /// Generate performance report for a validator
    pub async fn generate_performance_report(&self, validator_id: &ParticipantId) -> Option<ValidatorPerformanceReport> {
        let metrics = self.get_metrics(validator_id).await?;
        let history = self.get_historical_metrics(validator_id).await;
        let reliability_score = self.calculate_reliability_score(validator_id).await;
        
        // Calculate trends
        let uptime_trend = self.calculate_trend(&history, |m| m.uptime_percentage).await;
        let proposal_trend = self.calculate_trend(&history, |m| m.proposal_success_rate).await;
        let response_trend = self.calculate_trend(&history, |m| 1000.0 - m.avg_response_ms as f64).await;
        
        Some(ValidatorPerformanceReport {
            validator_id: validator_id.clone(),
            current_metrics: metrics,
            reliability_score,
            uptime_trend,
            proposal_trend,
            response_trend,
            last_updated: Utc::now(),
        })
    }
    
    /// Calculate trend for a metric over time
    async fn calculate_trend<F>(&self, history: &[HistoricalMetrics], extractor: F) -> MetricTrend
    where
        F: Fn(&ValidatorMetrics) -> f64,
    {
        if history.len() < 2 {
            return MetricTrend::Stable;
        }
        
        // Compare first third with last third
        let segment_size = history.len() / 3;
        if segment_size == 0 {
            return MetricTrend::Stable;
        }
        
        let first_segment: f64 = history[0..segment_size]
            .iter()
            .map(|h| extractor(&h.metrics))
            .sum::<f64>() / segment_size as f64;
        
        let last_segment: f64 = history[history.len() - segment_size..]
            .iter()
            .map(|h| extractor(&h.metrics))
            .sum::<f64>() / segment_size as f64;
        
        let change = last_segment - first_segment;
        let threshold = first_segment * 0.05; // 5% threshold
        
        if change > threshold {
            MetricTrend::Improving
        } else if change < -threshold {
            MetricTrend::Deteriorating
        } else {
            MetricTrend::Stable
        }
    }
}

/// Validator performance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorPerformanceReport {
    /// Validator ID
    pub validator_id: ParticipantId,
    
    /// Current metrics
    pub current_metrics: ValidatorMetrics,
    
    /// Overall reliability score (0-100)
    pub reliability_score: f64,
    
    /// Uptime trend
    pub uptime_trend: MetricTrend,
    
    /// Proposal success trend
    pub proposal_trend: MetricTrend,
    
    /// Response time trend
    pub response_trend: MetricTrend,
    
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

/// Metric trend direction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricTrend {
    Improving,
    Stable,
    Deteriorating,
}

/// Automated validator onboarding system
pub struct ValidatorOnboarding {
    /// Pending validator applications
    pending_applications: Arc<RwLock<HashMap<ParticipantId, ValidatorApplication>>>,
    
    /// Approved validators awaiting activation
    approved_validators: Arc<RwLock<HashMap<ParticipantId, ValidatorInfo>>>,
}

/// Validator application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorApplication {
    /// Applicant ID
    pub applicant_id: ParticipantId,
    
    /// Public key
    pub public_key_hex: String,
    
    /// Self-bonded stake
    pub self_bonded: u64,
    
    /// Commission rate (basis points)
    pub commission_rate_bp: u32,
    
    /// Description
    pub description: String,
    
    /// Website
    pub website: String,
    
    /// Application timestamp
    pub applied_at: DateTime<Utc>,
    
    /// Status
    pub status: ApplicationStatus,
    
    /// Review notes
    pub review_notes: Option<String>,
}

/// Application status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ApplicationStatus {
    /// Pending review
    Pending,
    
    /// Approved but awaiting activation
    Approved,
    
    /// Rejected
    Rejected,
    
    /// Active validator
    Active,
}

impl ValidatorOnboarding {
    pub fn new() -> Self {
        Self {
            pending_applications: Arc::new(RwLock::new(HashMap::new())),
            approved_validators: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Submit a validator application
    pub async fn submit_application(&self, application: ValidatorApplication) -> GarpResult<()> {
        // Validate application
        if application.self_bonded < 1000 {
            return Err(crate::error::GarpError::InvalidValidator("Insufficient self-bonded stake".to_string()));
        }
        
        if application.commission_rate_bp > 10000 {
            return Err(crate::error::GarpError::InvalidValidator("Invalid commission rate".to_string()));
        }
        
        let mut applications = self.pending_applications.write().await;
        applications.insert(application.applicant_id.clone(), application);
        
        Ok(())
    }
    
    /// Review a validator application
    pub async fn review_application(&self, applicant_id: &ParticipantId, approved: bool, notes: Option<String>) -> GarpResult<()> {
        let mut applications = self.pending_applications.write().await;
        if let Some(mut application) = applications.get_mut(applicant_id) {
            if approved {
                application.status = ApplicationStatus::Approved;
                application.review_notes = notes;
                
                // Move to approved validators
                if let Some(approved_app) = applications.remove(applicant_id) {
                    let validator_info = ValidatorInfo {
                        id: approved_app.applicant_id.clone(),
                        public_key_hex: approved_app.public_key_hex,
                        voting_power: approved_app.self_bonded,
                        status: ValidatorStatus::Inactive, // Start as inactive
                        joined_at: Utc::now(),
                        metadata: {
                            let mut meta = HashMap::new();
                            meta.insert("description".to_string(), approved_app.description);
                            meta.insert("website".to_string(), approved_app.website);
                            meta
                        },
                        reputation_score: 50, // Starting reputation
                        successful_proposals: 0,
                        failed_proposals: 0,
                        missed_votes: 0,
                        last_seen: Utc::now(),
                        slashing_history: Vec::new(),
                        delegators: HashMap::new(),
                        commission_rate_bp: approved_app.commission_rate_bp,
                        total_delegated: 0,
                        self_bonded: approved_app.self_bonded,
                    };
                    
                    let mut approved_validators = self.approved_validators.write().await;
                    approved_validators.insert(applicant_id.clone(), validator_info);
                }
            } else {
                application.status = ApplicationStatus::Rejected;
                application.review_notes = notes;
            }
        } else {
            return Err(crate::error::GarpError::InvalidValidator("Application not found".to_string()));
        }
        
        Ok(())
    }
    
    /// Activate an approved validator
    pub async fn activate_validator(&self, validator_id: &ParticipantId, validator_set: &ValidatorSet) -> GarpResult<()> {
        let mut approved_validators = self.approved_validators.write().await;
        if let Some(validator) = approved_validators.remove(validator_id) {
            // Add to active validator set
            validator_set.add_validator(validator).await?;
            Ok(())
        } else {
            Err(crate::error::GarpError::InvalidValidator("Approved validator not found".to_string()))
        }
    }
    
    /// Get pending applications
    pub async fn get_pending_applications(&self) -> Vec<ValidatorApplication> {
        let applications = self.pending_applications.read().await;
        applications.values()
            .filter(|app| app.status == ApplicationStatus::Pending)
            .cloned()
            .collect()
    }
    
    /// Get application by ID
    pub async fn get_application(&self, applicant_id: &ParticipantId) -> Option<ValidatorApplication> {
        let applications = self.pending_applications.read().await;
        applications.get(applicant_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ParticipantId;
    
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
    }
    
    #[tokio::test]
    async fn test_reputation_scoring() {
        let score = ReputationScorer::calculate_reputation_score(10, 2, 5, 0);
        assert_eq!(score, 62); // 50 + 20 - 6 - 2 = 62
        
        let score_with_slashing = ReputationScorer::calculate_reputation_score(10, 2, 5, 1);
        assert_eq!(score_with_slashing, 52); // 62 - 10 = 52
        
        let tier = ReputationScorer::get_validator_tier(85);
        assert_eq!(tier, ValidatorTier::Good);
    }
}