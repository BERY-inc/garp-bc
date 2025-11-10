use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;
use garp_common::{GarpResult, TransactionId, ParticipantId};
use crate::config::ConsensusConfig;
use crate::storage::{StorageBackend, ConsensusState, ConsensusPhase, ConsensusVote, ConsensusResult};
use crate::kafka::{KafkaClient, MessageHandler, KafkaMessage, ConsensusOutcome};

/// Consensus manager for handling transaction validation
pub struct ConsensusManager {
    /// Configuration
    config: ConsensusConfig,
    
    /// Storage backend
    storage: Arc<dyn StorageBackend>,
    
    /// Kafka client for messaging
    kafka: Arc<KafkaClient>,
    
    /// Active consensus sessions
    sessions: Arc<RwLock<HashMap<TransactionId, ConsensusSession>>>,
    
    /// Registered validators
    validators: Arc<RwLock<HashMap<ParticipantId, ValidatorInfo>>>,
    
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
    
    /// Metrics
    metrics: Arc<RwLock<ConsensusMetrics>>,
}

/// Consensus session for a single transaction
#[derive(Debug, Clone)]
pub struct ConsensusSession {
    /// Transaction ID
    pub transaction_id: TransactionId,
    
    /// Required participants
    pub required_participants: HashSet<ParticipantId>,
    
    /// Received votes
    pub votes: HashMap<ParticipantId, ConsensusVote>,
    
    /// Session phase
    pub phase: ConsensusPhase,
    
    /// Session timeout
    pub timeout: DateTime<Utc>,
    
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    
    /// Domain ID
    pub domain_id: String,
    
    /// Encrypted transaction data
    pub encrypted_data: Vec<u8>,
    
    /// Consensus result
    pub result: Option<ConsensusResult>,
}

/// Validator information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorInfo {
    /// Participant ID
    pub participant_id: ParticipantId,
    
    /// Public key for signature verification
    pub public_key: String,
    
    /// Endpoint for communication
    pub endpoint: String,
    
    /// Validator weight (for weighted voting)
    pub weight: u64,
    
    /// Last seen timestamp
    pub last_seen: DateTime<Utc>,
    
    /// Status
    pub status: ValidatorStatus,
}

/// Validator status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidatorStatus {
    Active,
    Inactive,
    Suspended,
    Banned,
}

/// Consensus metrics
#[derive(Debug, Clone, Default)]
pub struct ConsensusMetrics {
    /// Total consensus sessions
    pub total_sessions: u64,
    
    /// Successful consensus
    pub successful_consensus: u64,
    
    /// Failed consensus
    pub failed_consensus: u64,
    
    /// Timed out consensus
    pub timed_out_consensus: u64,
    
    /// Average consensus time
    pub avg_consensus_time: Duration,
    
    /// Active sessions
    pub active_sessions: u64,
    
    /// Total votes received
    pub total_votes: u64,
    
    /// Last updated
    pub last_updated: DateTime<Utc>,
}

/// Consensus handler for Kafka messages
pub struct ConsensusHandler {
    manager: Arc<ConsensusManager>,
}

impl ConsensusManager {
    /// Create new consensus manager
    pub async fn new(
        config: ConsensusConfig,
        storage: Arc<dyn StorageBackend>,
        kafka: Arc<KafkaClient>,
    ) -> GarpResult<Self> {
        Ok(Self {
            config,
            storage,
            kafka,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            validators: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: None,
            metrics: Arc::new(RwLock::new(ConsensusMetrics::default())),
        })
    }
    
    /// Start the consensus manager
    pub async fn start(&mut self) -> GarpResult<()> {
        // Register consensus handler with Kafka
        let handler = Arc::new(ConsensusHandler {
            manager: Arc::new(self.clone()),
        });
        self.kafka.register_handler(handler).await;
        
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);
        
        // Start session timeout monitor
        let sessions = Arc::clone(&self.sessions);
        let kafka = Arc::clone(&self.kafka);
        let metrics = Arc::clone(&self.metrics);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        Self::check_session_timeouts(&sessions, &kafka, &metrics).await;
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });
        
        // Start metrics update loop
        let metrics_clone = Arc::clone(&self.metrics);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let mut metrics = metrics_clone.write().await;
                        metrics.last_updated = Utc::now();
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Stop the consensus manager
    pub async fn stop(&mut self) -> GarpResult<()> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()).await;
        }
        Ok(())
    }
    
    /// Start consensus for a transaction
    pub async fn start_consensus(
        &self,
        transaction_id: TransactionId,
        required_participants: HashSet<ParticipantId>,
        domain_id: String,
        encrypted_data: Vec<u8>,
    ) -> GarpResult<()> {
        let timeout = Utc::now() + chrono::Duration::seconds(self.config.consensus_timeout_seconds as i64);
        
        let session = ConsensusSession {
            transaction_id: transaction_id.clone(),
            required_participants: required_participants.clone(),
            votes: HashMap::new(),
            phase: ConsensusPhase::Voting,
            timeout,
            created_at: Utc::now(),
            domain_id: domain_id.clone(),
            encrypted_data,
            result: None,
        };
        
        // Store session
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(transaction_id.clone(), session.clone());
        }
        
        // Store in database
        let consensus_state = ConsensusState {
            transaction_id: transaction_id.clone(),
            phase: ConsensusPhase::Voting,
            required_participants: required_participants.clone(),
            votes: HashMap::new(),
            result: None,
            timeout,
            created_at: Utc::now(),
            domain_id: domain_id.clone(),
        };
        
        self.storage.store_consensus_state(&consensus_state).await?;
        
        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_sessions += 1;
            metrics.active_sessions += 1;
        }
        
        tracing::info!(
            "Started consensus for transaction {} with {} participants",
            transaction_id,
            required_participants.len()
        );
        
        Ok(())
    }
    
    /// Handle consensus vote
    pub async fn handle_vote(
        &self,
        transaction_id: &TransactionId,
        participant_id: &ParticipantId,
        vote: bool,
        reason: Option<String>,
        signature: String,
    ) -> GarpResult<()> {
        // Verify signature
        if !self.verify_vote_signature(participant_id, transaction_id, vote, &signature).await? {
            return Err(anyhow::anyhow!("Invalid vote signature"));
        }
        
        let mut session_updated = false;
        let mut consensus_reached = false;
        let mut consensus_result = None;
        
        // Update session
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(transaction_id) {
                // Check if participant is required
                if !session.required_participants.contains(participant_id) {
                    return Err(anyhow::anyhow!("Participant not required for this consensus"));
                }
                
                // Check if already voted
                if session.votes.contains_key(participant_id) {
                    return Err(anyhow::anyhow!("Participant already voted"));
                }
                
                // Add vote
                let consensus_vote = ConsensusVote {
                    participant_id: participant_id.clone(),
                    vote,
                    reason: reason.clone(),
                    signature: signature.clone(),
                    timestamp: Utc::now(),
                };
                
                session.votes.insert(participant_id.clone(), consensus_vote);
                session_updated = true;
                
                // Check if consensus reached
                if session.votes.len() == session.required_participants.len() {
                    let all_approved = session.votes.values().all(|v| v.vote);
                    
                    if all_approved {
                        session.phase = ConsensusPhase::Committed;
                        consensus_result = Some(ConsensusResult::Approved);
                    } else {
                        session.phase = ConsensusPhase::Aborted;
                        let rejection_reasons: Vec<String> = session.votes.values()
                            .filter(|v| !v.vote)
                            .filter_map(|v| v.reason.clone())
                            .collect();
                        consensus_result = Some(ConsensusResult::Rejected {
                            reason: rejection_reasons.join("; "),
                        });
                    }
                    
                    session.result = consensus_result.clone();
                    consensus_reached = true;
                }
            }
        }
        
        if session_updated {
            // Update database
            if let Some(session) = self.sessions.read().await.get(transaction_id) {
                let consensus_state = ConsensusState {
                    transaction_id: transaction_id.clone(),
                    phase: session.phase.clone(),
                    required_participants: session.required_participants.clone(),
                    votes: session.votes.clone(),
                    result: session.result.clone(),
                    timeout: session.timeout,
                    created_at: session.created_at,
                    domain_id: session.domain_id.clone(),
                };
                
                self.storage.update_consensus_state(&consensus_state).await?;
            }
            
            // Update metrics
            {
                let mut metrics = self.metrics.write().await;
                metrics.total_votes += 1;
            }
            
            // If consensus reached, send result
            if consensus_reached {
                if let Some(result) = consensus_result {
                    let outcome = match result {
                        ConsensusResult::Approved => ConsensusOutcome::Approved,
                        ConsensusResult::Rejected { reason } => ConsensusOutcome::Rejected { reason },
                        ConsensusResult::Timeout => ConsensusOutcome::Timeout,
                    };
                    
                    self.kafka.send_consensus_result(transaction_id.clone(), outcome).await?;
                    
                    // Update metrics
                    {
                        let mut metrics = self.metrics.write().await;
                        metrics.active_sessions -= 1;
                        
                        match result {
                            ConsensusResult::Approved => metrics.successful_consensus += 1,
                            ConsensusResult::Rejected { .. } => metrics.failed_consensus += 1,
                            ConsensusResult::Timeout => metrics.timed_out_consensus += 1,
                        }
                    }
                    
                    tracing::info!(
                        "Consensus reached for transaction {}: {:?}",
                        transaction_id,
                        result
                    );
                }
            }
        }
        
        Ok(())
    }
    
    /// Register validator
    pub async fn register_validator(&self, validator: ValidatorInfo) -> GarpResult<()> {
        let mut validators = self.validators.write().await;
        validators.insert(validator.participant_id.clone(), validator);
        Ok(())
    }
    
    /// Unregister validator
    pub async fn unregister_validator(&self, participant_id: &ParticipantId) -> GarpResult<()> {
        let mut validators = self.validators.write().await;
        validators.remove(participant_id);
        Ok(())
    }
    
    /// Get consensus session
    pub async fn get_session(&self, transaction_id: &TransactionId) -> Option<ConsensusSession> {
        self.sessions.read().await.get(transaction_id).cloned()
    }
    
    /// Get consensus metrics
    pub async fn get_metrics(&self) -> ConsensusMetrics {
        self.metrics.read().await.clone()
    }
    
    /// Verify vote signature
    async fn verify_vote_signature(
        &self,
        participant_id: &ParticipantId,
        transaction_id: &TransactionId,
        vote: bool,
        signature: &str,
    ) -> GarpResult<bool> {
        // Get validator info
        let validators = self.validators.read().await;
        let validator = validators.get(participant_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown validator"))?;
        
        // Create message to verify
        let message = format!("{}:{}:{}", transaction_id, participant_id, vote);
        
        // Verify signature (simplified - in production use proper crypto)
        // This would use the validator's public key to verify the signature
        let expected_signature = format!("sig_{}_{}", validator.public_key, message);
        
        Ok(signature == expected_signature)
    }
    
    /// Check session timeouts
    async fn check_session_timeouts(
        sessions: &Arc<RwLock<HashMap<TransactionId, ConsensusSession>>>,
        kafka: &Arc<KafkaClient>,
        metrics: &Arc<RwLock<ConsensusMetrics>>,
    ) {
        let now = Utc::now();
        let mut timed_out_sessions = Vec::new();
        
        // Find timed out sessions
        {
            let sessions_read = sessions.read().await;
            for (transaction_id, session) in sessions_read.iter() {
                if now > session.timeout && session.result.is_none() {
                    timed_out_sessions.push(transaction_id.clone());
                }
            }
        }
        
        // Handle timed out sessions
        for transaction_id in timed_out_sessions {
            {
                let mut sessions_write = sessions.write().await;
                if let Some(session) = sessions_write.get_mut(&transaction_id) {
                    session.phase = ConsensusPhase::Aborted;
                    session.result = Some(ConsensusResult::Timeout);
                }
            }
            
            // Send timeout result
            if let Err(e) = kafka.send_consensus_result(transaction_id.clone(), ConsensusOutcome::Timeout).await {
                tracing::error!("Failed to send timeout result for {}: {}", transaction_id, e);
            }
            
            // Update metrics
            {
                let mut metrics = metrics.write().await;
                metrics.active_sessions -= 1;
                metrics.timed_out_consensus += 1;
            }
            
            tracing::warn!("Consensus timed out for transaction {}", transaction_id);
        }
    }
}

impl Clone for ConsensusManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            storage: Arc::clone(&self.storage),
            kafka: Arc::clone(&self.kafka),
            sessions: Arc::clone(&self.sessions),
            validators: Arc::clone(&self.validators),
            shutdown_tx: None, // Don't clone shutdown channel
            metrics: Arc::clone(&self.metrics),
        }
    }
}

impl ConsensusHandler {
    /// Create new consensus handler
    pub fn new(manager: Arc<ConsensusManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl MessageHandler for ConsensusHandler {
    async fn handle_message(&self, message: KafkaMessage) -> GarpResult<()> {
        match message {
            KafkaMessage::ConsensusVote {
                transaction_id,
                participant_id,
                vote,
                reason,
                signature,
                ..
            } => {
                self.manager.handle_vote(&transaction_id, &participant_id, vote, reason, signature).await?;
            }
            KafkaMessage::TransactionSubmitted {
                transaction_id,
                participants,
                domain_id,
                encrypted_data,
                ..
            } => {
                // Start consensus for new transaction
                let required_participants: HashSet<ParticipantId> = participants.into_iter().collect();
                self.manager.start_consensus(transaction_id, required_participants, domain_id, encrypted_data).await?;
            }
            _ => {
                // Ignore other message types
            }
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "consensus_handler"
    }
}

/// Consensus coordinator for managing multiple consensus sessions
pub struct ConsensusCoordinator {
    /// Consensus manager
    manager: Arc<ConsensusManager>,
    
    /// Configuration
    config: ConsensusConfig,
}

impl ConsensusCoordinator {
    /// Create new consensus coordinator
    pub fn new(manager: Arc<ConsensusManager>, config: ConsensusConfig) -> Self {
        Self { manager, config }
    }
    
    /// Coordinate consensus for multiple transactions
    pub async fn coordinate_batch_consensus(
        &self,
        transactions: Vec<(TransactionId, HashSet<ParticipantId>, String, Vec<u8>)>,
    ) -> GarpResult<Vec<ConsensusResult>> {
        let mut results = Vec::new();
        
        // Start consensus for all transactions
        for (transaction_id, participants, domain_id, encrypted_data) in transactions {
            self.manager.start_consensus(transaction_id.clone(), participants, domain_id, encrypted_data).await?;
        }
        
        // Wait for all consensus to complete (simplified)
        // In production, this would be more sophisticated
        tokio::time::sleep(Duration::from_secs(self.config.consensus_timeout_seconds)).await;
        
        Ok(results)
    }
    
    /// Get coordinator statistics
    pub async fn get_stats(&self) -> GarpResult<ConsensusCoordinatorStats> {
        let metrics = self.manager.get_metrics().await;
        
        Ok(ConsensusCoordinatorStats {
            total_sessions: metrics.total_sessions,
            active_sessions: metrics.active_sessions,
            success_rate: if metrics.total_sessions > 0 {
                (metrics.successful_consensus as f64 / metrics.total_sessions as f64) * 100.0
            } else {
                0.0
            },
            avg_consensus_time: metrics.avg_consensus_time,
            last_updated: metrics.last_updated,
        })
    }
}

/// Consensus coordinator statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusCoordinatorStats {
    /// Total consensus sessions
    pub total_sessions: u64,
    
    /// Active sessions
    pub active_sessions: u64,
    
    /// Success rate percentage
    pub success_rate: f64,
    
    /// Average consensus time
    pub avg_consensus_time: Duration,
    
    /// Last updated
    pub last_updated: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemoryStorage;
    use crate::config::KafkaConfig;
    
    #[tokio::test]
    async fn test_consensus_session_creation() {
        let config = ConsensusConfig::default();
        let storage = Arc::new(MemoryStorage::new());
        let kafka_config = KafkaConfig::default();
        let kafka = Arc::new(KafkaClient::new(kafka_config).await.unwrap());
        
        let manager = ConsensusManager::new(config, storage, kafka).await.unwrap();
        
        let transaction_id = "test-tx-1".to_string();
        let participants = vec!["participant-1".to_string(), "participant-2".to_string()]
            .into_iter().collect();
        let domain_id = "test-domain".to_string();
        let encrypted_data = vec![1, 2, 3, 4];
        
        manager.start_consensus(transaction_id.clone(), participants, domain_id, encrypted_data).await.unwrap();
        
        let session = manager.get_session(&transaction_id).await;
        assert!(session.is_some());
        
        let session = session.unwrap();
        assert_eq!(session.transaction_id, transaction_id);
        assert_eq!(session.required_participants.len(), 2);
        assert_eq!(session.phase, ConsensusPhase::Voting);
    }
    
    #[tokio::test]
    async fn test_vote_handling() {
        let config = ConsensusConfig::default();
        let storage = Arc::new(MemoryStorage::new());
        let kafka_config = KafkaConfig::default();
        let kafka = Arc::new(KafkaClient::new(kafka_config).await.unwrap());
        
        let manager = ConsensusManager::new(config, storage, kafka).await.unwrap();
        
        // Register validator
        let validator = ValidatorInfo {
            participant_id: "participant-1".to_string(),
            public_key: "test-key-1".to_string(),
            endpoint: "http://localhost:8001".to_string(),
            weight: 1,
            last_seen: Utc::now(),
            status: ValidatorStatus::Active,
        };
        manager.register_validator(validator).await.unwrap();
        
        // Start consensus
        let transaction_id = "test-tx-1".to_string();
        let participants = vec!["participant-1".to_string()].into_iter().collect();
        let domain_id = "test-domain".to_string();
        let encrypted_data = vec![1, 2, 3, 4];
        
        manager.start_consensus(transaction_id.clone(), participants, domain_id, encrypted_data).await.unwrap();
        
        // Submit vote
        let signature = "sig_test-key-1_test-tx-1:participant-1:true".to_string();
        manager.handle_vote(&transaction_id, &"participant-1".to_string(), true, None, signature).await.unwrap();
        
        let session = manager.get_session(&transaction_id).await.unwrap();
        assert_eq!(session.votes.len(), 1);
        assert_eq!(session.phase, ConsensusPhase::Committed);
    }
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            consensus_timeout_seconds: 30,
            min_validators: 1,
            max_validators: 100,
            vote_threshold: 0.67,
            enable_weighted_voting: false,
            require_unanimous: false,
            allow_abstain: false,
            max_concurrent_sessions: 1000,
        }
    }
}