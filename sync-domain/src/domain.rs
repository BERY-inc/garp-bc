use crate::{
    config::SyncDomainConfig,
    storage::{Storage, StorageBackend},
    sequencer::{TransactionSequencer, SequencerFactory},
    kafka::{KafkaClient, MessageHandler, KafkaMessage},
    consensus::{ConsensusManager, ConsensusHandler},
    mediator::{TransactionMediator, MediationHandler},
    vector_clock::{ClockManager, EventType},
    api::ApiServer,
};
use garp_common::{GarpResult, GarpError, ParticipantId, TransactionId};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, oneshot};
use tokio::time::{Duration, interval};
use tracing::{info, warn, error, debug};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Main Synchronization Domain service
pub struct SyncDomain {
    /// Configuration
    config: Arc<SyncDomainConfig>,
    
    /// Storage backend
    storage: Arc<dyn StorageBackend>,
    
    /// Transaction sequencer
    sequencer: Arc<TransactionSequencer>,
    
    /// Kafka client
    kafka_client: Arc<KafkaClient>,
    
    /// Consensus manager
    consensus_manager: Arc<ConsensusManager>,
    
    /// Transaction mediator
    mediator: Arc<TransactionMediator>,
    
    /// Vector clock manager
    clock_manager: Arc<RwLock<ClockManager>>,
    
    /// API server
    api_server: Arc<ApiServer>,
    
    /// Domain state
    state: Arc<RwLock<DomainState>>,
    
    /// Shutdown signal
    shutdown_tx: Option<oneshot::Sender<()>>,
    
    /// Task handles
    task_handles: Vec<tokio::task::JoinHandle<()>>,
}

/// Domain state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainState {
    /// Domain ID
    pub domain_id: String,
    
    /// Domain status
    pub status: DomainStatus,
    
    /// Registered participants
    pub participants: HashMap<ParticipantId, ParticipantInfo>,
    
    /// Active transactions
    pub active_transactions: HashMap<TransactionId, TransactionInfo>,
    
    /// Domain statistics
    pub stats: DomainStatistics,
    
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

/// Domain status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainStatus {
    /// Domain is initializing
    Initializing,
    
    /// Domain is active and processing transactions
    Active,
    
    /// Domain is in maintenance mode
    Maintenance,
    
    /// Domain is shutting down
    ShuttingDown,
    
    /// Domain has failed
    Failed,
}

/// Participant information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantInfo {
    /// Participant ID
    pub participant_id: ParticipantId,
    
    /// Participant endpoint
    pub endpoint: String,
    
    /// Public key for verification
    pub public_key: Vec<u8>,
    
    /// Registration timestamp
    pub registered_at: DateTime<Utc>,
    
    /// Last seen timestamp
    pub last_seen: DateTime<Utc>,
    
    /// Participant status
    pub status: ParticipantStatus,
    
    /// Capabilities
    pub capabilities: Vec<String>,
}

/// Participant status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParticipantStatus {
    Active,
    Inactive,
    Suspended,
    Banned,
}

/// Transaction information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionInfo {
    /// Transaction ID
    pub transaction_id: TransactionId,
    
    /// Submitting participant
    pub submitter: ParticipantId,
    
    /// Involved participants
    pub participants: Vec<ParticipantId>,
    
    /// Transaction status
    pub status: TransactionStatus,
    
    /// Submission timestamp
    pub submitted_at: DateTime<Utc>,
    
    /// Sequence number (if sequenced)
    pub sequence_number: Option<u64>,
    
    /// Consensus status
    pub consensus_status: Option<ConsensusStatus>,
    
    /// Mediation status
    pub mediation_status: Option<MediationStatus>,
}

/// Transaction status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionStatus {
    Submitted,
    Sequenced,
    InConsensus,
    InMediation,
    Finalized,
    Rejected,
    Failed,
}

/// Consensus status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusStatus {
    Pending,
    InProgress,
    Approved,
    Rejected,
    Timeout,
}

/// Mediation status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediationStatus {
    Pending,
    InProgress,
    Consented,
    Rejected,
    Timeout,
}

/// Domain statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainStatistics {
    /// Total transactions processed
    pub total_transactions: u64,
    
    /// Transactions per second (current)
    pub current_tps: f64,
    
    /// Average transaction processing time
    pub avg_processing_time_ms: f64,
    
    /// Number of active participants
    pub active_participants: u64,
    
    /// Number of pending transactions
    pub pending_transactions: u64,
    
    /// Number of failed transactions
    pub failed_transactions: u64,
    
    /// Consensus success rate
    pub consensus_success_rate: f64,
    
    /// Mediation success rate
    pub mediation_success_rate: f64,
    
    /// Domain uptime
    pub uptime_seconds: u64,
    
    /// Last statistics update
    pub last_updated: DateTime<Utc>,
}

/// Domain event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEvent {
    /// Event ID
    pub event_id: String,
    
    /// Event type
    pub event_type: DomainEventType,
    
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Event data
    pub data: serde_json::Value,
    
    /// Related transaction (if any)
    pub transaction_id: Option<TransactionId>,
    
    /// Related participant (if any)
    pub participant_id: Option<ParticipantId>,
}

/// Domain event type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainEventType {
    /// Domain started
    DomainStarted,
    
    /// Domain stopped
    DomainStopped,
    
    /// Participant registered
    ParticipantRegistered,
    
    /// Participant deregistered
    ParticipantDeregistered,
    
    /// Transaction submitted
    TransactionSubmitted,
    
    /// Transaction sequenced
    TransactionSequenced,
    
    /// Transaction finalized
    TransactionFinalized,
    
    /// Transaction failed
    TransactionFailed,
    
    /// Consensus started
    ConsensusStarted,
    
    /// Consensus completed
    ConsensusCompleted,
    
    /// Mediation started
    MediationStarted,
    
    /// Mediation completed
    MediationCompleted,
    
    /// Error occurred
    ErrorOccurred,
}

/// Domain message handler
pub struct DomainMessageHandler {
    domain_state: Arc<RwLock<DomainState>>,
    clock_manager: Arc<RwLock<ClockManager>>,
}

impl SyncDomain {
    /// Create new synchronization domain
    pub async fn new(config: SyncDomainConfig) -> GarpResult<Self> {
        let config = Arc::new(config);
        
        info!("Initializing Synchronization Domain: {}", config.domain.domain_id);
        
        // Initialize storage
        let storage = Storage::new(&config.database).await?;
        let storage = Arc::new(storage);
        
        // Initialize vector clock manager
        let clock_manager = Arc::new(RwLock::new(
            ClockManager::new(config.domain.domain_id.clone())
        ));
        
        // Initialize Kafka client
        let kafka_client = Arc::new(
            KafkaClient::new(&config.kafka).await?
        );
        
        // Initialize sequencer
        let sequencer = Arc::new(
            SequencerFactory::create_sequencer(&config.sequencer, storage.clone()).await?
        );
        
        // Initialize consensus manager
        let consensus_manager = Arc::new(
            ConsensusManager::new(
                config.consensus.clone(),
                storage.clone(),
                kafka_client.clone(),
            ).await?
        );
        
        // Initialize mediator
        let mediator = Arc::new(
            TransactionMediator::new(
                config.mediator.clone(),
                storage.clone(),
                kafka_client.clone(),
                consensus_manager.clone(),
            ).await?
        );
        
        // Initialize API server
        let api_server = Arc::new(
            ApiServer::new(
                config.api.clone(),
                storage.clone(),
                sequencer.clone(),
                consensus_manager.clone(),
                mediator.clone(),
                clock_manager.clone(),
            ).await?
        );
        
        // Initialize domain state
        let state = Arc::new(RwLock::new(DomainState {
            domain_id: config.domain.domain_id.clone(),
            status: DomainStatus::Initializing,
            participants: HashMap::new(),
            active_transactions: HashMap::new(),
            stats: DomainStatistics::default(),
            last_updated: Utc::now(),
        }));
        
        Ok(Self {
            config,
            storage,
            sequencer,
            kafka_client,
            consensus_manager,
            mediator,
            clock_manager,
            api_server,
            state,
            shutdown_tx: None,
            task_handles: Vec::new(),
        })
    }
    
    /// Start the synchronization domain
    pub async fn start(&mut self) -> GarpResult<()> {
        info!("Starting Synchronization Domain: {}", self.config.domain.domain_id);
        
        // Update domain status
        {
            let mut state = self.state.write().await;
            state.status = DomainStatus::Active;
            state.last_updated = Utc::now();
        }
        
        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);
        
        // Register message handlers
        self.register_message_handlers().await?;
        
        // Start Kafka client
        self.kafka_client.start().await?;
        
        // Start sequencer
        self.sequencer.start().await?;
        
        // Start consensus manager
        self.consensus_manager.start().await?;
        
        // Start mediator
        self.mediator.start().await?;
        
        // Start API server
        let api_handle = {
            let api_server = self.api_server.clone();
            tokio::spawn(async move {
                if let Err(e) = api_server.start().await {
                    error!("API server error: {}", e);
                }
            })
        };
        self.task_handles.push(api_handle);
        
        // Start statistics updater
        let stats_handle = {
            let state = self.state.clone();
            let storage = self.storage.clone();
            let sequencer = self.sequencer.clone();
            let consensus_manager = self.consensus_manager.clone();
            let mediator = self.mediator.clone();
            
            tokio::spawn(async move {
                Self::statistics_updater(
                    state,
                    storage,
                    sequencer,
                    consensus_manager,
                    mediator,
                ).await;
            })
        };
        self.task_handles.push(stats_handle);
        
        // Start health checker
        let health_handle = {
            let state = self.state.clone();
            let kafka_client = self.kafka_client.clone();
            let clock_manager = self.clock_manager.clone();
            
            tokio::spawn(async move {
                Self::health_checker(state, kafka_client, clock_manager).await;
            })
        };
        self.task_handles.push(health_handle);
        
        // Emit domain started event
        self.emit_domain_event(DomainEventType::DomainStarted, serde_json::json!({})).await?;
        
        info!("Synchronization Domain started successfully");
        
        // Wait for shutdown signal
        tokio::select! {
            _ = shutdown_rx => {
                info!("Received shutdown signal");
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Received Ctrl+C signal");
            }
        }
        
        Ok(())
    }
    
    /// Stop the synchronization domain
    pub async fn stop(&mut self) -> GarpResult<()> {
        info!("Stopping Synchronization Domain: {}", self.config.domain.domain_id);
        
        // Update domain status
        {
            let mut state = self.state.write().await;
            state.status = DomainStatus::ShuttingDown;
            state.last_updated = Utc::now();
        }
        
        // Emit domain stopped event
        if let Err(e) = self.emit_domain_event(DomainEventType::DomainStopped, serde_json::json!({})).await {
            warn!("Failed to emit domain stopped event: {}", e);
        }
        
        // Stop components in reverse order
        if let Err(e) = self.mediator.stop().await {
            warn!("Error stopping mediator: {}", e);
        }
        
        if let Err(e) = self.consensus_manager.stop().await {
            warn!("Error stopping consensus manager: {}", e);
        }
        
        if let Err(e) = self.sequencer.stop().await {
            warn!("Error stopping sequencer: {}", e);
        }
        
        if let Err(e) = self.kafka_client.stop().await {
            warn!("Error stopping Kafka client: {}", e);
        }
        
        // Cancel all tasks
        for handle in self.task_handles.drain(..) {
            handle.abort();
        }
        
        // Send shutdown signal
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        
        info!("Synchronization Domain stopped");
        Ok(())
    }
    
    /// Register participant
    pub async fn register_participant(
        &self,
        participant_id: ParticipantId,
        endpoint: String,
        public_key: Vec<u8>,
        capabilities: Vec<String>,
    ) -> GarpResult<()> {
        info!("Registering participant: {}", participant_id);
        
        let participant_info = ParticipantInfo {
            participant_id: participant_id.clone(),
            endpoint,
            public_key,
            registered_at: Utc::now(),
            last_seen: Utc::now(),
            status: ParticipantStatus::Active,
            capabilities,
        };
        
        // Store in database
        self.storage.register_participant(
            &participant_id,
            &participant_info.endpoint,
            &participant_info.public_key,
            &participant_info.capabilities,
        ).await?;
        
        // Update domain state
        {
            let mut state = self.state.write().await;
            state.participants.insert(participant_id.clone(), participant_info.clone());
            state.last_updated = Utc::now();
        }
        
        // Add to clock manager
        {
            let mut clock_manager = self.clock_manager.write().await;
            clock_manager.add_node(participant_id.clone());
        }
        
        // Register with mediator
        self.mediator.register_participant(
            participant_id.clone(),
            participant_info.public_key.clone(),
            participant_info.endpoint.clone(),
        ).await?;
        
        // Emit event
        self.emit_domain_event(
            DomainEventType::ParticipantRegistered,
            serde_json::json!({
                "participant_id": participant_id,
                "endpoint": participant_info.endpoint,
                "capabilities": participant_info.capabilities
            })
        ).await?;
        
        info!("Participant registered successfully: {}", participant_id);
        Ok(())
    }
    
    /// Deregister participant
    pub async fn deregister_participant(&self, participant_id: &ParticipantId) -> GarpResult<()> {
        info!("Deregistering participant: {}", participant_id);
        
        // Remove from database
        self.storage.deregister_participant(participant_id).await?;
        
        // Update domain state
        {
            let mut state = self.state.write().await;
            state.participants.remove(participant_id);
            state.last_updated = Utc::now();
        }
        
        // Remove from clock manager
        {
            let mut clock_manager = self.clock_manager.write().await;
            clock_manager.remove_node(participant_id);
        }
        
        // Emit event
        self.emit_domain_event(
            DomainEventType::ParticipantDeregistered,
            serde_json::json!({
                "participant_id": participant_id
            })
        ).await?;
        
        info!("Participant deregistered successfully: {}", participant_id);
        Ok(())
    }
    
    /// Get domain state
    pub async fn get_state(&self) -> DomainState {
        self.state.read().await.clone()
    }
    
    /// Get domain statistics
    pub async fn get_statistics(&self) -> DomainStatistics {
        self.state.read().await.stats.clone()
    }
    
    /// Register message handlers
    async fn register_message_handlers(&self) -> GarpResult<()> {
        // Register domain message handler
        let domain_handler = Arc::new(DomainMessageHandler {
            domain_state: self.state.clone(),
            clock_manager: self.clock_manager.clone(),
        });
        
        self.kafka_client.register_handler("domain_handler", domain_handler).await?;
        
        // Register consensus handler
        let consensus_handler = Arc::new(ConsensusHandler::new(
            self.consensus_manager.clone()
        ));
        
        self.kafka_client.register_handler("consensus_handler", consensus_handler).await?;
        
        // Register mediation handler
        let mediation_handler = Arc::new(MediationHandler::new(
            self.mediator.clone()
        ));
        
        self.kafka_client.register_handler("mediation_handler", mediation_handler).await?;
        
        Ok(())
    }
    
    /// Emit domain event
    async fn emit_domain_event(
        &self,
        event_type: DomainEventType,
        data: serde_json::Value,
    ) -> GarpResult<()> {
        let event = DomainEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            event_type,
            timestamp: Utc::now(),
            data,
            transaction_id: None,
            participant_id: None,
        };
        
        // Generate clock event
        {
            let mut clock_manager = self.clock_manager.write().await;
            let _clock_event = clock_manager.generate_event(
                EventType::DomainEvent,
                serde_json::to_value(&event)?
            );
        }
        
        // Send to Kafka
        self.kafka_client.send_domain_event(
            &self.config.domain.domain_id,
            &event.event_id,
            &event
        ).await?;
        
        Ok(())
    }
    
    /// Statistics updater task
    async fn statistics_updater(
        state: Arc<RwLock<DomainState>>,
        storage: Arc<dyn StorageBackend>,
        sequencer: Arc<TransactionSequencer>,
        consensus_manager: Arc<ConsensusManager>,
        mediator: Arc<TransactionMediator>,
    ) {
        let mut interval = interval(Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            
            if let Err(e) = Self::update_statistics(
                &state,
                &storage,
                &sequencer,
                &consensus_manager,
                &mediator,
            ).await {
                error!("Failed to update statistics: {}", e);
            }
        }
    }
    
    /// Update domain statistics
    async fn update_statistics(
        state: &Arc<RwLock<DomainState>>,
        storage: &Arc<dyn StorageBackend>,
        sequencer: &Arc<TransactionSequencer>,
        consensus_manager: &Arc<ConsensusManager>,
        mediator: &Arc<TransactionMediator>,
    ) -> GarpResult<()> {
        let mut state_guard = state.write().await;
        
        // Get statistics from storage
        let domain_stats = storage.get_domain_stats().await?;
        
        // Get metrics from components
        let sequencer_metrics = sequencer.get_metrics().await;
        let consensus_metrics = consensus_manager.get_metrics().await;
        let mediator_metrics = mediator.get_metrics().await;
        
        // Update statistics
        state_guard.stats = DomainStatistics {
            total_transactions: domain_stats.total_transactions,
            current_tps: sequencer_metrics.transactions_per_second,
            avg_processing_time_ms: sequencer_metrics.avg_processing_time_ms,
            active_participants: state_guard.participants.len() as u64,
            pending_transactions: sequencer_metrics.pending_transactions,
            failed_transactions: domain_stats.failed_transactions,
            consensus_success_rate: consensus_metrics.success_rate,
            mediation_success_rate: mediator_metrics.success_rate,
            uptime_seconds: domain_stats.uptime_seconds,
            last_updated: Utc::now(),
        };
        
        state_guard.last_updated = Utc::now();
        
        debug!("Updated domain statistics: TPS={:.2}, Participants={}, Pending={}",
               state_guard.stats.current_tps,
               state_guard.stats.active_participants,
               state_guard.stats.pending_transactions);
        
        Ok(())
    }
    
    /// Health checker task
    async fn health_checker(
        state: Arc<RwLock<DomainState>>,
        kafka_client: Arc<KafkaClient>,
        clock_manager: Arc<RwLock<ClockManager>>,
    ) {
        let mut interval = interval(Duration::from_secs(60));
        
        loop {
            interval.tick().await;
            
            // Check for suspected nodes
            {
                let mut clock_manager = clock_manager.write().await;
                clock_manager.check_suspected_nodes(chrono::Duration::minutes(5));
            }
            
            // Send health ping
            if let Err(e) = Self::send_health_ping(&state, &kafka_client, &clock_manager).await {
                error!("Failed to send health ping: {}", e);
            }
        }
    }
    
    /// Send health ping
    async fn send_health_ping(
        state: &Arc<RwLock<DomainState>>,
        kafka_client: &Arc<KafkaClient>,
        clock_manager: &Arc<RwLock<ClockManager>>,
    ) -> GarpResult<()> {
        let domain_id = {
            let state_guard = state.read().await;
            state_guard.domain_id.clone()
        };
        
        let clock = {
            let clock_manager = clock_manager.read().await;
            clock_manager.get_clock_for_sending()
        };
        
        kafka_client.send_health_ping(&domain_id, &clock).await?;
        
        debug!("Sent health ping for domain: {}", domain_id);
        Ok(())
    }
}

impl MessageHandler for DomainMessageHandler {
    async fn handle_message(&self, message: &KafkaMessage) -> GarpResult<()> {
        match message {
            KafkaMessage::ParticipantJoined { participant_id, .. } => {
                debug!("Participant joined: {}", participant_id);
                
                // Add to clock manager
                {
                    let mut clock_manager = self.clock_manager.write().await;
                    clock_manager.add_node(participant_id.clone());
                }
            }
            
            KafkaMessage::ParticipantLeft { participant_id, .. } => {
                debug!("Participant left: {}", participant_id);
                
                // Remove from clock manager
                {
                    let mut clock_manager = self.clock_manager.write().await;
                    clock_manager.remove_node(participant_id);
                }
            }
            
            KafkaMessage::HealthPing { domain_id, vector_clock, .. } => {
                debug!("Received health ping from domain: {}", domain_id);
                
                // Update clock manager
                {
                    let mut clock_manager = self.clock_manager.write().await;
                    clock_manager.process_event(crate::vector_clock::ClockEvent {
                        event_id: uuid::Uuid::new_v4().to_string(),
                        node_id: domain_id.clone(),
                        vector_clock: vector_clock.clone(),
                        event_type: EventType::Heartbeat,
                        data: serde_json::json!({}),
                    })?;
                }
            }
            
            _ => {
                // Other messages are handled by specific handlers
            }
        }
        
        Ok(())
    }
}

impl Default for DomainStatistics {
    fn default() -> Self {
        Self {
            total_transactions: 0,
            current_tps: 0.0,
            avg_processing_time_ms: 0.0,
            active_participants: 0,
            pending_transactions: 0,
            failed_transactions: 0,
            consensus_success_rate: 0.0,
            mediation_success_rate: 0.0,
            uptime_seconds: 0,
            last_updated: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SyncDomainConfig;
    
    #[tokio::test]
    async fn test_domain_creation() {
        let config = SyncDomainConfig::default();
        let domain = SyncDomain::new(config).await;
        assert!(domain.is_ok());
    }
    
    #[tokio::test]
    async fn test_participant_registration() {
        let config = SyncDomainConfig::default();
        let domain = SyncDomain::new(config).await.unwrap();
        
        let result = domain.register_participant(
            "participant1".to_string(),
            "http://localhost:8080".to_string(),
            vec![1, 2, 3, 4],
            vec!["wallet".to_string(), "contracts".to_string()],
        ).await;
        
        assert!(result.is_ok());
        
        let state = domain.get_state().await;
        assert_eq!(state.participants.len(), 1);
        assert!(state.participants.contains_key("participant1"));
    }
    
    #[tokio::test]
    async fn test_domain_statistics() {
        let config = SyncDomainConfig::default();
        let domain = SyncDomain::new(config).await.unwrap();
        
        let stats = domain.get_statistics().await;
        assert_eq!(stats.total_transactions, 0);
        assert_eq!(stats.active_participants, 0);
    }
}