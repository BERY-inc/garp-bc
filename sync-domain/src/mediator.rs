use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;
use garp_common::{GarpResult, TransactionId, ParticipantId, ContractId};
use crate::config::MediatorConfig;
use crate::storage::{StorageBackend, SequencedTransaction};
use crate::kafka::{KafkaClient, MessageHandler, KafkaMessage};
use crate::consensus::{ConsensusManager, ConsensusSession};

/// Transaction mediator for ensuring consent and coordination
pub struct TransactionMediator {
    /// Configuration
    config: MediatorConfig,
    
    /// Storage backend
    storage: Arc<dyn StorageBackend>,
    
    /// Kafka client
    kafka: Arc<KafkaClient>,
    
    /// Consensus manager
    consensus: Arc<ConsensusManager>,
    
    /// Active mediation sessions
    sessions: Arc<RwLock<HashMap<TransactionId, MediationSession>>>,
    
    /// Participant registry
    participants: Arc<RwLock<HashMap<ParticipantId, ParticipantInfo>>>,
    
    /// Contract registry
    contracts: Arc<RwLock<HashMap<ContractId, ContractInfo>>>,
    
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
    
    /// Metrics
    metrics: Arc<RwLock<MediatorMetrics>>,
}

/// Mediation session for a transaction
#[derive(Debug, Clone)]
pub struct MediationSession {
    /// Transaction ID
    pub transaction_id: TransactionId,
    
    /// Transaction data (encrypted)
    pub encrypted_data: Vec<u8>,
    
    /// Required participants
    pub required_participants: HashSet<ParticipantId>,
    
    /// Received consents
    pub consents: HashMap<ParticipantId, ConsentInfo>,
    
    /// Affected contracts
    pub affected_contracts: HashSet<ContractId>,
    
    /// Session status
    pub status: MediationStatus,
    
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    
    /// Timeout
    pub timeout: DateTime<Utc>,
    
    /// Domain ID
    pub domain_id: String,
    
    /// Mediation result
    pub result: Option<MediationResult>,
    
    /// Dependencies (other transactions that must complete first)
    pub dependencies: HashSet<TransactionId>,
    
    /// Priority level
    pub priority: MediationPriority,
}

/// Consent information from a participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentInfo {
    /// Participant ID
    pub participant_id: ParticipantId,
    
    /// Consent given
    pub consent: bool,
    
    /// Reason for consent/rejection
    pub reason: Option<String>,
    
    /// Digital signature
    pub signature: String,
    
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Conditions (if any)
    pub conditions: Vec<ConsentCondition>,
}

/// Consent condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentCondition {
    /// Condition type
    pub condition_type: ConditionType,
    
    /// Condition value
    pub value: String,
    
    /// Description
    pub description: String,
}

/// Condition type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionType {
    /// Time-based condition
    TimeWindow { start: DateTime<Utc>, end: DateTime<Utc> },
    
    /// Amount-based condition
    MaxAmount { amount: u64, currency: String },
    
    /// Dependency condition
    DependsOn { transaction_id: TransactionId },
    
    /// Custom condition
    Custom { key: String, value: String },
}

/// Mediation status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MediationStatus {
    /// Waiting for consents
    WaitingForConsent,
    
    /// All consents received, validating
    Validating,
    
    /// Mediation successful
    Approved,
    
    /// Mediation failed
    Rejected,
    
    /// Mediation timed out
    TimedOut,
    
    /// Mediation cancelled
    Cancelled,
}

/// Mediation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediationResult {
    /// Transaction approved for execution
    Approved {
        approved_at: DateTime<Utc>,
        conditions_met: Vec<String>,
    },
    
    /// Transaction rejected
    Rejected {
        rejected_at: DateTime<Utc>,
        reasons: Vec<String>,
        rejecting_participants: Vec<ParticipantId>,
    },
    
    /// Mediation timed out
    TimedOut {
        timed_out_at: DateTime<Utc>,
        missing_consents: Vec<ParticipantId>,
    },
    
    /// Mediation cancelled
    Cancelled {
        cancelled_at: DateTime<Utc>,
        reason: String,
    },
}

/// Mediation priority
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MediationPriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

/// Participant information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantInfo {
    /// Participant ID
    pub participant_id: ParticipantId,
    
    /// Public key
    pub public_key: String,
    
    /// Endpoint
    pub endpoint: String,
    
    /// Participant status
    pub status: ParticipantStatus,
    
    /// Last seen
    pub last_seen: DateTime<Utc>,
    
    /// Consent preferences
    pub consent_preferences: ConsentPreferences,
}

/// Participant status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParticipantStatus {
    Active,
    Inactive,
    Suspended,
    Banned,
}

/// Consent preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentPreferences {
    /// Auto-consent for trusted participants
    pub auto_consent_trusted: bool,
    
    /// Auto-consent threshold amount
    pub auto_consent_threshold: Option<u64>,
    
    /// Consent timeout (seconds)
    pub consent_timeout: u64,
    
    /// Require explicit consent for high-value transactions
    pub require_explicit_high_value: bool,
    
    /// Trusted participants
    pub trusted_participants: HashSet<ParticipantId>,
}

/// Contract information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractInfo {
    /// Contract ID
    pub contract_id: ContractId,
    
    /// Contract template
    pub template: String,
    
    /// Signatories (required for consent)
    pub signatories: HashSet<ParticipantId>,
    
    /// Observers (can view but don't need to consent)
    pub observers: HashSet<ParticipantId>,
    
    /// Contract status
    pub status: ContractStatus,
    
    /// Created timestamp
    pub created_at: DateTime<Utc>,
}

/// Contract status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContractStatus {
    Active,
    Archived,
    Suspended,
}

/// Mediator metrics
#[derive(Debug, Clone, Default)]
pub struct MediatorMetrics {
    /// Total mediation sessions
    pub total_sessions: u64,
    
    /// Successful mediations
    pub successful_mediations: u64,
    
    /// Failed mediations
    pub failed_mediations: u64,
    
    /// Timed out mediations
    pub timed_out_mediations: u64,
    
    /// Average mediation time
    pub avg_mediation_time: Duration,
    
    /// Active sessions
    pub active_sessions: u64,
    
    /// Total consents received
    pub total_consents: u64,
    
    /// Auto-consents given
    pub auto_consents: u64,
    
    /// Last updated
    pub last_updated: DateTime<Utc>,
}

/// Mediation handler for Kafka messages
pub struct MediationHandler {
    mediator: Arc<TransactionMediator>,
}

impl TransactionMediator {
    /// Create new transaction mediator
    pub async fn new(
        config: MediatorConfig,
        storage: Arc<dyn StorageBackend>,
        kafka: Arc<KafkaClient>,
        consensus: Arc<ConsensusManager>,
    ) -> GarpResult<Self> {
        Ok(Self {
            config,
            storage,
            kafka,
            consensus,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            participants: Arc::new(RwLock::new(HashMap::new())),
            contracts: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: None,
            metrics: Arc::new(RwLock::new(MediatorMetrics::default())),
        })
    }
    
    /// Start the mediator
    pub async fn start(&mut self) -> GarpResult<()> {
        // Register mediation handler with Kafka
        let handler = Arc::new(MediationHandler {
            mediator: Arc::new(self.clone()),
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
        
        // Start auto-consent processor
        let sessions_clone = Arc::clone(&self.sessions);
        let participants_clone = Arc::clone(&self.participants);
        let metrics_clone = Arc::clone(&self.metrics);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        Self::process_auto_consents(&sessions_clone, &participants_clone, &metrics_clone).await;
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Stop the mediator
    pub async fn stop(&mut self) -> GarpResult<()> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()).await;
        }
        Ok(())
    }
    
    /// Start mediation for a transaction
    pub async fn start_mediation(
        &self,
        transaction_id: TransactionId,
        encrypted_data: Vec<u8>,
        required_participants: HashSet<ParticipantId>,
        affected_contracts: HashSet<ContractId>,
        domain_id: String,
        priority: MediationPriority,
    ) -> GarpResult<()> {
        let timeout = Utc::now() + chrono::Duration::seconds(self.config.mediation_timeout_seconds as i64);
        
        let session = MediationSession {
            transaction_id: transaction_id.clone(),
            encrypted_data,
            required_participants: required_participants.clone(),
            consents: HashMap::new(),
            affected_contracts,
            status: MediationStatus::WaitingForConsent,
            created_at: Utc::now(),
            timeout,
            domain_id: domain_id.clone(),
            result: None,
            dependencies: HashSet::new(),
            priority,
        };
        
        // Store session
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(transaction_id.clone(), session);
        }
        
        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_sessions += 1;
            metrics.active_sessions += 1;
        }
        
        // Send consent requests to participants
        for participant_id in required_participants {
            self.send_consent_request(&transaction_id, &participant_id, &domain_id).await?;
        }
        
        tracing::info!(
            "Started mediation for transaction {} with {} participants",
            transaction_id,
            session.required_participants.len()
        );
        
        Ok(())
    }
    
    /// Handle consent from participant
    pub async fn handle_consent(
        &self,
        transaction_id: &TransactionId,
        consent_info: ConsentInfo,
    ) -> GarpResult<()> {
        // Verify signature
        if !self.verify_consent_signature(&consent_info).await? {
            return Err(anyhow::anyhow!("Invalid consent signature"));
        }
        
        let mut session_updated = false;
        let mut mediation_complete = false;
        let mut mediation_result = None;
        
        // Update session
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(transaction_id) {
                // Check if participant is required
                if !session.required_participants.contains(&consent_info.participant_id) {
                    return Err(anyhow::anyhow!("Participant not required for this mediation"));
                }
                
                // Check if already consented
                if session.consents.contains_key(&consent_info.participant_id) {
                    return Err(anyhow::anyhow!("Participant already provided consent"));
                }
                
                // Add consent
                session.consents.insert(consent_info.participant_id.clone(), consent_info.clone());
                session_updated = true;
                
                // Check if all consents received
                if session.consents.len() == session.required_participants.len() {
                    let all_consented = session.consents.values().all(|c| c.consent);
                    
                    if all_consented && self.validate_consent_conditions(session).await? {
                        session.status = MediationStatus::Approved;
                        mediation_result = Some(MediationResult::Approved {
                            approved_at: Utc::now(),
                            conditions_met: self.get_met_conditions(session).await,
                        });
                    } else {
                        session.status = MediationStatus::Rejected;
                        let rejecting_participants: Vec<ParticipantId> = session.consents.iter()
                            .filter(|(_, consent)| !consent.consent)
                            .map(|(id, _)| id.clone())
                            .collect();
                        
                        let reasons: Vec<String> = session.consents.values()
                            .filter(|c| !c.consent)
                            .filter_map(|c| c.reason.clone())
                            .collect();
                        
                        mediation_result = Some(MediationResult::Rejected {
                            rejected_at: Utc::now(),
                            reasons,
                            rejecting_participants,
                        });
                    }
                    
                    session.result = mediation_result.clone();
                    mediation_complete = true;
                }
            }
        }
        
        if session_updated {
            // Update metrics
            {
                let mut metrics = self.metrics.write().await;
                metrics.total_consents += 1;
                
                if !consent_info.consent {
                    // This was a rejection
                }
            }
            
            // If mediation complete, proceed to consensus or finalization
            if mediation_complete {
                if let Some(result) = mediation_result {
                    match result {
                        MediationResult::Approved { .. } => {
                            // Start consensus phase
                            let session = self.sessions.read().await.get(transaction_id).cloned();
                            if let Some(session) = session {
                                self.consensus.start_consensus(
                                    transaction_id.clone(),
                                    session.required_participants,
                                    session.domain_id,
                                    session.encrypted_data,
                                ).await?;
                            }
                            
                            // Update metrics
                            {
                                let mut metrics = self.metrics.write().await;
                                metrics.successful_mediations += 1;
                                metrics.active_sessions -= 1;
                            }
                        }
                        MediationResult::Rejected { .. } => {
                            // Send rejection notification
                            self.send_mediation_result(transaction_id, &result).await?;
                            
                            // Update metrics
                            {
                                let mut metrics = self.metrics.write().await;
                                metrics.failed_mediations += 1;
                                metrics.active_sessions -= 1;
                            }
                        }
                        _ => {}
                    }
                    
                    tracing::info!(
                        "Mediation completed for transaction {}: {:?}",
                        transaction_id,
                        result
                    );
                }
            }
        }
        
        Ok(())
    }
    
    /// Register participant
    pub async fn register_participant(&self, participant: ParticipantInfo) -> GarpResult<()> {
        let mut participants = self.participants.write().await;
        participants.insert(participant.participant_id.clone(), participant);
        Ok(())
    }
    
    /// Register contract
    pub async fn register_contract(&self, contract: ContractInfo) -> GarpResult<()> {
        let mut contracts = self.contracts.write().await;
        contracts.insert(contract.contract_id.clone(), contract);
        Ok(())
    }
    
    /// Get mediation session
    pub async fn get_session(&self, transaction_id: &TransactionId) -> Option<MediationSession> {
        self.sessions.read().await.get(transaction_id).cloned()
    }
    
    /// Get mediator metrics
    pub async fn get_metrics(&self) -> MediatorMetrics {
        self.metrics.read().await.clone()
    }
    
    /// Send consent request to participant
    async fn send_consent_request(
        &self,
        transaction_id: &TransactionId,
        participant_id: &ParticipantId,
        domain_id: &str,
    ) -> GarpResult<()> {
        // Send consent request message via Kafka
        let event_data = serde_json::json!({
            "type": "consent_request",
            "transaction_id": transaction_id,
            "participant_id": participant_id,
            "domain_id": domain_id,
            "timestamp": Utc::now()
        });
        
        self.kafka.send_domain_event(
            domain_id.to_string(),
            "consent_request".to_string(),
            event_data,
        ).await
    }
    
    /// Send mediation result
    async fn send_mediation_result(
        &self,
        transaction_id: &TransactionId,
        result: &MediationResult,
    ) -> GarpResult<()> {
        let event_data = serde_json::json!({
            "type": "mediation_result",
            "transaction_id": transaction_id,
            "result": result,
            "timestamp": Utc::now()
        });
        
        // Get domain ID from session
        let domain_id = {
            let sessions = self.sessions.read().await;
            sessions.get(transaction_id)
                .map(|s| s.domain_id.clone())
                .unwrap_or_else(|| "unknown".to_string())
        };
        
        self.kafka.send_domain_event(
            domain_id,
            "mediation_result".to_string(),
            event_data,
        ).await
    }
    
    /// Verify consent signature
    async fn verify_consent_signature(&self, consent: &ConsentInfo) -> GarpResult<bool> {
        // Get participant info
        let participants = self.participants.read().await;
        let participant = participants.get(&consent.participant_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown participant"))?;
        
        // Create message to verify
        let message = format!("{}:{}:{}", consent.participant_id, consent.consent, consent.timestamp);
        
        // Verify signature (simplified - in production use proper crypto)
        let expected_signature = format!("consent_sig_{}_{}", participant.public_key, message);
        
        Ok(consent.signature == expected_signature)
    }
    
    /// Validate consent conditions
    async fn validate_consent_conditions(&self, session: &MediationSession) -> GarpResult<bool> {
        for consent in session.consents.values() {
            for condition in &consent.conditions {
                if !self.validate_condition(condition, session).await? {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
    
    /// Validate individual condition
    async fn validate_condition(
        &self,
        condition: &ConsentCondition,
        session: &MediationSession,
    ) -> GarpResult<bool> {
        match &condition.condition_type {
            ConditionType::TimeWindow { start, end } => {
                let now = Utc::now();
                Ok(now >= *start && now <= *end)
            }
            ConditionType::MaxAmount { amount: _, currency: _ } => {
                // Would validate against transaction amount
                Ok(true) // Simplified
            }
            ConditionType::DependsOn { transaction_id } => {
                // Check if dependency transaction is complete
                // This would query the storage for transaction status
                Ok(true) // Simplified
            }
            ConditionType::Custom { key: _, value: _ } => {
                // Custom validation logic
                Ok(true) // Simplified
            }
        }
    }
    
    /// Get met conditions
    async fn get_met_conditions(&self, session: &MediationSession) -> Vec<String> {
        let mut met_conditions = Vec::new();
        
        for consent in session.consents.values() {
            for condition in &consent.conditions {
                if self.validate_condition(condition, session).await.unwrap_or(false) {
                    met_conditions.push(condition.description.clone());
                }
            }
        }
        
        met_conditions
    }
    
    /// Check session timeouts
    async fn check_session_timeouts(
        sessions: &Arc<RwLock<HashMap<TransactionId, MediationSession>>>,
        kafka: &Arc<KafkaClient>,
        metrics: &Arc<RwLock<MediatorMetrics>>,
    ) {
        let now = Utc::now();
        let mut timed_out_sessions = Vec::new();
        
        // Find timed out sessions
        {
            let sessions_read = sessions.read().await;
            for (transaction_id, session) in sessions_read.iter() {
                if now > session.timeout && session.status == MediationStatus::WaitingForConsent {
                    timed_out_sessions.push((transaction_id.clone(), session.clone()));
                }
            }
        }
        
        // Handle timed out sessions
        for (transaction_id, mut session) in timed_out_sessions {
            session.status = MediationStatus::TimedOut;
            
            let missing_consents: Vec<ParticipantId> = session.required_participants
                .difference(&session.consents.keys().cloned().collect())
                .cloned()
                .collect();
            
            let result = MediationResult::TimedOut {
                timed_out_at: now,
                missing_consents,
            };
            
            session.result = Some(result.clone());
            
            // Update session
            {
                let mut sessions_write = sessions.write().await;
                sessions_write.insert(transaction_id.clone(), session);
            }
            
            // Send timeout result
            let event_data = serde_json::json!({
                "type": "mediation_timeout",
                "transaction_id": transaction_id,
                "result": result,
                "timestamp": now
            });
            
            if let Err(e) = kafka.send_domain_event(
                "unknown".to_string(), // Would get from session
                "mediation_timeout".to_string(),
                event_data,
            ).await {
                tracing::error!("Failed to send timeout result for {}: {}", transaction_id, e);
            }
            
            // Update metrics
            {
                let mut metrics = metrics.write().await;
                metrics.active_sessions -= 1;
                metrics.timed_out_mediations += 1;
            }
            
            tracing::warn!("Mediation timed out for transaction {}", transaction_id);
        }
    }
    
    /// Process auto-consents
    async fn process_auto_consents(
        sessions: &Arc<RwLock<HashMap<TransactionId, MediationSession>>>,
        participants: &Arc<RwLock<HashMap<ParticipantId, ParticipantInfo>>>,
        metrics: &Arc<RwLock<MediatorMetrics>>,
    ) {
        // This would implement auto-consent logic based on participant preferences
        // Simplified for now
    }
}

impl Clone for TransactionMediator {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            storage: Arc::clone(&self.storage),
            kafka: Arc::clone(&self.kafka),
            consensus: Arc::clone(&self.consensus),
            sessions: Arc::clone(&self.sessions),
            participants: Arc::clone(&self.participants),
            contracts: Arc::clone(&self.contracts),
            shutdown_tx: None, // Don't clone shutdown channel
            metrics: Arc::clone(&self.metrics),
        }
    }
}

#[async_trait]
impl MessageHandler for MediationHandler {
    async fn handle_message(&self, message: KafkaMessage) -> GarpResult<()> {
        match message {
            KafkaMessage::DomainEvent { event_type, data, .. } => {
                if event_type == "consent_response" {
                    // Handle consent response
                    if let Ok(consent_info) = serde_json::from_value::<ConsentInfo>(data) {
                        if let Some(transaction_id) = consent_info.participant_id.split(':').next() {
                            self.mediator.handle_consent(&transaction_id.to_string(), consent_info).await?;
                        }
                    }
                }
            }
            KafkaMessage::TransactionSubmitted {
                transaction_id,
                participants,
                domain_id,
                encrypted_data,
                ..
            } => {
                // Start mediation for new transaction
                let required_participants: HashSet<ParticipantId> = participants.into_iter().collect();
                let affected_contracts = HashSet::new(); // Would be determined from transaction
                
                self.mediator.start_mediation(
                    transaction_id,
                    encrypted_data,
                    required_participants,
                    affected_contracts,
                    domain_id,
                    MediationPriority::Normal,
                ).await?;
            }
            _ => {
                // Ignore other message types
            }
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "mediation_handler"
    }
}

impl Default for ConsentPreferences {
    fn default() -> Self {
        Self {
            auto_consent_trusted: false,
            auto_consent_threshold: None,
            consent_timeout: 300, // 5 minutes
            require_explicit_high_value: true,
            trusted_participants: HashSet::new(),
        }
    }
}

impl Default for MediatorConfig {
    fn default() -> Self {
        Self {
            mediation_timeout_seconds: 300, // 5 minutes
            max_concurrent_sessions: 1000,
            enable_auto_consent: true,
            auto_consent_threshold: Some(1000), // Auto-consent for amounts <= 1000
            require_all_signatories: true,
            allow_partial_consent: false,
            consent_cache_ttl_seconds: 3600, // 1 hour
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemoryStorage;
    use crate::config::KafkaConfig;
    use crate::consensus::ConsensusManager;
    
    #[tokio::test]
    async fn test_mediation_session_creation() {
        let config = MediatorConfig::default();
        let storage = Arc::new(MemoryStorage::new());
        let kafka_config = KafkaConfig::default();
        let kafka = Arc::new(KafkaClient::new(kafka_config).await.unwrap());
        let consensus_config = crate::config::ConsensusConfig::default();
        let consensus = Arc::new(ConsensusManager::new(consensus_config, Arc::clone(&storage), Arc::clone(&kafka)).await.unwrap());
        
        let mediator = TransactionMediator::new(config, storage, kafka, consensus).await.unwrap();
        
        let transaction_id = "test-tx-1".to_string();
        let participants = vec!["participant-1".to_string(), "participant-2".to_string()]
            .into_iter().collect();
        let domain_id = "test-domain".to_string();
        let encrypted_data = vec![1, 2, 3, 4];
        let affected_contracts = HashSet::new();
        
        mediator.start_mediation(
            transaction_id.clone(),
            encrypted_data,
            participants,
            affected_contracts,
            domain_id,
            MediationPriority::Normal,
        ).await.unwrap();
        
        let session = mediator.get_session(&transaction_id).await;
        assert!(session.is_some());
        
        let session = session.unwrap();
        assert_eq!(session.transaction_id, transaction_id);
        assert_eq!(session.required_participants.len(), 2);
        assert_eq!(session.status, MediationStatus::WaitingForConsent);
    }
    
    #[tokio::test]
    async fn test_consent_handling() {
        let config = MediatorConfig::default();
        let storage = Arc::new(MemoryStorage::new());
        let kafka_config = KafkaConfig::default();
        let kafka = Arc::new(KafkaClient::new(kafka_config).await.unwrap());
        let consensus_config = crate::config::ConsensusConfig::default();
        let consensus = Arc::new(ConsensusManager::new(consensus_config, Arc::clone(&storage), Arc::clone(&kafka)).await.unwrap());
        
        let mediator = TransactionMediator::new(config, storage, kafka, consensus).await.unwrap();
        
        // Register participant
        let participant = ParticipantInfo {
            participant_id: "participant-1".to_string(),
            public_key: "test-key-1".to_string(),
            endpoint: "http://localhost:8001".to_string(),
            status: ParticipantStatus::Active,
            last_seen: Utc::now(),
            consent_preferences: ConsentPreferences::default(),
        };
        mediator.register_participant(participant).await.unwrap();
        
        // Start mediation
        let transaction_id = "test-tx-1".to_string();
        let participants = vec!["participant-1".to_string()].into_iter().collect();
        let domain_id = "test-domain".to_string();
        let encrypted_data = vec![1, 2, 3, 4];
        let affected_contracts = HashSet::new();
        
        mediator.start_mediation(
            transaction_id.clone(),
            encrypted_data,
            participants,
            affected_contracts,
            domain_id,
            MediationPriority::Normal,
        ).await.unwrap();
        
        // Submit consent
        let consent = ConsentInfo {
            participant_id: "participant-1".to_string(),
            consent: true,
            reason: None,
            signature: "consent_sig_test-key-1_participant-1:true:2024-01-01T00:00:00Z".to_string(),
            timestamp: Utc::now(),
            conditions: vec![],
        };
        
        mediator.handle_consent(&transaction_id, consent).await.unwrap();
        
        let session = mediator.get_session(&transaction_id).await.unwrap();
        assert_eq!(session.consents.len(), 1);
        assert_eq!(session.status, MediationStatus::Approved);
    }
}