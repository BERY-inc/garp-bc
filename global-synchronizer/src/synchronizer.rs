use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex, mpsc, oneshot};
use tokio::time::interval;
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use garp_common::{GarpResult, GarpError};
use garp_common::types::{TransactionId, Block, ParticipantId};

use crate::config::GlobalSyncConfig;
use crate::storage::{GlobalStorage, BlockInfo, TransactionInfo, DomainId};
use crate::network::NetworkManager;
use crate::consensus::ConsensusEngine;
use crate::cross_domain::CrossDomainCoordinator;
use crate::validator::{ValidatorManager, ValidatorInfo};
use crate::bridge::{CrossChainBridge, BridgeTransaction, BridgeTransactionStatus, AssetMapping, BridgeValidator};

/// Global synchronizer for coordinating cross-domain transactions and state
pub struct GlobalSynchronizer {
    /// Configuration
    config: Arc<GlobalSyncConfig>,
    
    /// Storage layer
    storage: Arc<GlobalStorage>,
    
    /// Network manager
    network_manager: Arc<NetworkManager>,
    
    /// Consensus engine
    consensus_engine: Arc<ConsensusEngine>,
    
    /// Cross-domain coordinator
    cross_domain_coordinator: Arc<CrossDomainCoordinator>,
    
    /// Validator manager
    validator_manager: Arc<ValidatorManager>,
    
    /// Cross-chain bridge
    bridge: Arc<CrossChainBridge>,
    
    /// Active transactions
    active_transactions: Arc<RwLock<HashMap<TransactionId, ActiveTransaction>>>,
    
    /// Pending blocks
    pending_blocks: Arc<RwLock<HashMap<String, PendingBlock>>>,
    
    /// Metrics
    metrics: Arc<GlobalSyncMetrics>,
    
    /// State
    state: Arc<RwLock<GlobalSyncState>>,
    
    /// Event channels
    event_tx: mpsc::UnboundedSender<GlobalSyncEvent>,
    event_rx: Arc<Mutex<mpsc::UnboundedReceiver<GlobalSyncEvent>>>,
    
    /// Shutdown signal
    shutdown_tx: Option<oneshot::Sender<()>>,
}

/// Active transaction in the global synchronizer
#[derive(Debug, Clone)]
pub struct ActiveTransaction {
    /// Transaction ID
    pub transaction_id: TransactionId,
    
    /// Cross-domain transaction details
    pub cross_domain_tx: CrossDomainTransaction,
    
    /// Current status
    pub status: TransactionStatus,
    
    /// Participating domains
    pub participating_domains: Vec<String>,
    
    /// Consensus votes
    pub consensus_votes: HashMap<ParticipantId, ConsensusVote>,
    
    /// Settlement status
    pub settlement_status: SettlementStatus,
    
    /// Created timestamp
    pub created_at: Instant,
    
    /// Last updated timestamp
    pub updated_at: Instant,
    
    /// Timeout
    pub timeout_at: Instant,
    
    /// Retry count
    pub retry_count: usize,
}

/// Transaction status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionStatus {
    /// Transaction received and validated
    Received,
    
    /// Consensus in progress
    ConsensusInProgress,
    
    /// Consensus reached
    ConsensusReached,
    
    /// Settlement in progress
    SettlementInProgress,
    
    /// Transaction finalized
    Finalized,
    
    /// Transaction failed
    Failed(String),
    
    /// Transaction timed out
    TimedOut,
}

/// Consensus vote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusVote {
    /// Voter ID
    pub voter_id: ParticipantId,
    
    /// Vote (approve/reject)
    pub vote: bool,
    
    /// Vote reason
    pub reason: Option<String>,
    
    /// Vote signature
    pub signature: Vec<u8>,
    
    /// Vote timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Settlement status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SettlementStatus {
    /// Not started
    NotStarted,
    
    /// In progress
    InProgress,
    
    /// Completed successfully
    Completed,
    
    /// Failed
    Failed(String),
    
    /// Partially completed
    PartiallyCompleted(Vec<String>),
}

/// Pending block
#[derive(Debug, Clone)]
pub struct PendingBlock {
    /// Block ID
    pub block_id: String,
    
    /// Block data
    pub block: GlobalBlock,
    
    /// Consensus votes
    pub votes: HashMap<ParticipantId, bool>,
    
    /// Required votes
    pub required_votes: usize,
    
    /// Created timestamp
    pub created_at: Instant,
    
    /// Timeout
    pub timeout_at: Instant,
}

/// Global synchronizer state
#[derive(Debug, Clone)]
pub struct GlobalSyncState {
    /// Current status
    pub status: SyncStatus,
    
    /// Current block height
    pub block_height: u64,
    
    /// Last block hash
    pub last_block_hash: String,
    
    /// Active validators
    pub active_validators: Vec<ValidatorInfo>,
    
    /// Connected domains
    pub connected_domains: Vec<String>,
    
    /// Performance metrics
    pub performance_metrics: PerformanceMetrics,
    
    /// Last updated
    pub last_updated: Instant,
}

/// Synchronizer status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncStatus {
    /// Starting up
    Starting,
    
    /// Synchronizing with network
    Syncing,
    
    /// Fully operational
    Active,
    
    /// Degraded performance
    Degraded,
    
    /// Shutting down
    Stopping,
    
    /// Stopped
    Stopped,
}

/// Performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Transactions per second
    pub tps: f64,
    
    /// Average consensus time
    pub avg_consensus_time_ms: f64,
    
    /// Average settlement time
    pub avg_settlement_time_ms: f64,
    
    /// Success rate
    pub success_rate: f64,
    
    /// Network latency
    pub network_latency_ms: f64,
}

/// Global synchronizer events
#[derive(Debug, Clone)]
pub enum GlobalSyncEvent {
    /// New cross-domain transaction
    NewTransaction(CrossDomainTransaction),
    
    /// Consensus message received
    ConsensusMessage(ConsensusMessage),
    
    /// Consensus result
    ConsensusResult(ConsensusResult),
    
    /// Settlement request
    SettlementRequest(SettlementRequest),
    
    /// Settlement result
    SettlementResult(SettlementResult),
    
    /// Network event
    NetworkEvent(NetworkEvent),
    
    /// Domain discovery event
    DiscoveryEvent(DiscoveryEvent),
    
    /// Domain event
    DomainEvent(DomainEvent),
    
    /// Block proposed
    BlockProposed(GlobalBlock),
    
    /// Block finalized
    BlockFinalized(GlobalBlock),
    
    /// Validator joined
    ValidatorJoined(ValidatorInfo),
    
    /// Validator left
    ValidatorLeft(ParticipantId),
    
    /// Health check
    HealthCheck,
    
    /// Shutdown signal
    Shutdown,
}

impl GlobalSynchronizer {
    /// Create new global synchronizer
    pub async fn new(config: GlobalSyncConfig) -> GarpResult<Self> {
        let config = Arc::new(config);
        
        // Initialize storage
        let storage = Arc::new(GlobalStorage::new(config.clone()).await?);
        
        // Initialize network manager
        let network_manager = Arc::new(NetworkManager::new(config.clone()).await?);
        
        // Initialize consensus engine
        let consensus_engine = Arc::new(ConsensusEngine::new(config.clone()).await?);
        
        // Initialize cross-domain coordinator
        let cross_domain_coordinator = Arc::new(CrossDomainCoordinator::new(
            config.clone(),
            storage.clone(),
            network_manager.clone(),
            consensus_engine.clone(),
        ).await?);
        
        // Initialize validator manager
        let validator_manager = Arc::new(ValidatorManager::new(config.clone()).await?);
        
        // Initialize cross-chain bridge
        let bridge = Arc::new(CrossChainBridge::new(
            config.clone(),
            storage.clone(),
            network_manager.clone(),
        ).await?);
        
        // Create event channels
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        
        let synchronizer = Self {
            config,
            storage,
            network_manager,
            consensus_engine,
            cross_domain_coordinator,
            validator_manager,
            bridge,
            active_transactions: Arc::new(RwLock::new(HashMap::new())),
            pending_blocks: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(GlobalSyncMetrics::default()),
            state: Arc::new(RwLock::new(GlobalSyncState::default())),
            event_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
            shutdown_tx: None,
        };
        
        Ok(synchronizer)
    }
    
    /// Start the global synchronizer
    pub async fn start(&mut self) -> GarpResult<()> {
        info!("Starting Global Synchronizer");
        
        // Update state
        {
            let mut state = self.state.write().await;
            state.status = SyncStatus::Starting;
            state.last_updated = Instant::now();
        }
        
        // Start components
        self.consensus_engine.start().await?;
        self.cross_domain_coordinator.start().await?;
        self.network_manager.start().await?;
        self.validator_manager.start().await?;
        self.bridge.start().await?;
        
        // Start event processing
        let event_processor = self.start_event_processor().await?;
        
        // Start background tasks
        let metrics_updater = self.start_metrics_updater().await?;
        let health_checker = self.start_health_checker().await?;
        let transaction_monitor = self.start_transaction_monitor().await?;
        let block_processor = self.start_block_processor().await?;
        
        // Update state to active
        {
            let mut state = self.state.write().await;
            state.status = SyncStatus::Active;
            state.last_updated = Instant::now();
        }
        
        info!("Global Synchronizer started successfully");
        Ok(())
    }
    
    /// Stop the global synchronizer
    pub async fn stop(&mut self) -> GarpResult<()> {
        info!("Stopping Global Synchronizer");
        
        // Update state
        {
            let mut state = self.state.write().await;
            state.status = SyncStatus::Stopping;
            state.last_updated = Instant::now();
        }
        
        // Send shutdown signal
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        
        // Stop components
        self.bridge.stop().await?;
        self.validator_manager.stop().await?;
        self.network_manager.stop().await?;
        self.cross_domain_coordinator.stop().await?;
        self.consensus_engine.stop().await?;
        
        // Update state to stopped
        {
            let mut state = self.state.write().await;
            state.status = SyncStatus::Stopped;
            state.last_updated = Instant::now();
        }
        
        info!("Global Synchronizer stopped");
        Ok(())
    }
    
    /// Submit cross-domain transaction
    pub async fn submit_transaction(
        &self,
        transaction: CrossDomainTransaction,
    ) -> GarpResult<TransactionId> {
        let transaction_id = TransactionId::new();
        
        debug!("Submitting cross-domain transaction: {}", transaction_id);
        
        // Create active transaction
        let active_tx = ActiveTransaction {
            transaction_id: transaction_id.clone(),
            cross_domain_tx: transaction.clone(),
            status: TransactionStatus::Received,
            participating_domains: transaction.participating_domains.clone(),
            consensus_votes: HashMap::new(),
            settlement_status: SettlementStatus::NotStarted,
            created_at: Instant::now(),
            updated_at: Instant::now(),
            timeout_at: Instant::now() + self.config.transaction_timeout(),
            retry_count: 0,
        };
        
        // Store active transaction
        {
            let mut active_transactions = self.active_transactions.write().await;
            active_transactions.insert(transaction_id.clone(), active_tx);
        }
        // Add to mempool
        {
            let mut mem = self.mempool.write().await;
            mem.push(transaction_id.clone());
        }
        
        // Send event
        self.event_tx.send(GlobalSyncEvent::NewTransaction(transaction))?;
        
        // Update metrics
        self.metrics.increment_transactions_submitted().await;
        
        Ok(transaction_id)
    }
    
    /// Get transaction status
    pub async fn get_transaction_status(
        &self,
        transaction_id: &TransactionId,
    ) -> GarpResult<Option<TransactionStatus>> {
        let active_transactions = self.active_transactions.read().await;
        Ok(active_transactions.get(transaction_id).map(|tx| tx.status.clone()))
    }
    
    /// Get current state
    pub async fn get_state(&self) -> GlobalSyncState {
        self.state.read().await.clone()
    }
    
    /// Get metrics
    pub async fn get_metrics(&self) -> GlobalSyncMetrics {
        self.metrics.clone()
    }
    
    /// Start event processor
    async fn start_event_processor(&self) -> GarpResult<tokio::task::JoinHandle<()>> {
        let event_rx = self.event_rx.clone();
        let consensus_engine = self.consensus_engine.clone();
        let cross_domain_coordinator = self.cross_domain_coordinator.clone();
        let settlement_engine = self.settlement_engine.clone();
        let active_transactions = self.active_transactions.clone();
        let pending_blocks = self.pending_blocks.clone();
        let metrics = self.metrics.clone();
        let state = self.state.clone();
        
        let storage = self.storage.clone();
        let handle = tokio::spawn(async move {
            let mut event_rx = event_rx.lock().await;
            
            while let Some(event) = event_rx.recv().await {
                match event {
                    GlobalSyncEvent::NewTransaction(transaction) => {
                        // Start consensus for the transaction
                        if let Err(e) = consensus_engine.start_consensus(transaction.clone()).await {
                            error!("Failed to start consensus for transaction: {}", e);
                        }
                        // Gossip stub: broadcast proposal to peers (placeholder)
                        info!("Broadcasting proposal for transaction: {:?}", transaction.transaction_id);
                    }
                    
                    GlobalSyncEvent::ConsensusResult(result) => {
                        // Handle consensus result
                        Self::handle_consensus_result(
                            result,
                            &active_transactions,
                            &settlement_engine,
                            &metrics,
                        ).await;
                    }
                    
                    GlobalSyncEvent::SettlementResult(result) => {
                        // Handle settlement result
                        Self::handle_settlement_result(
                            result,
                            &active_transactions,
                            &metrics,
                        ).await;
                    }
                    
                    GlobalSyncEvent::BlockProposed(block) => {
                        // Handle block proposal
                        Self::handle_block_proposal(
                            block,
                            &pending_blocks,
                            &consensus_engine,
                        ).await;
                    }
                    
                    GlobalSyncEvent::BlockFinalized(block) => {
                        // Handle block finalization
                        Self::handle_block_finalization(
                            block,
                            &state,
                            &metrics,
                            &storage,
                        ).await;
                    }
                    
                    GlobalSyncEvent::Shutdown => {
                        info!("Received shutdown signal in event processor");
                        break;
                    }
                    
                    _ => {
                        // Handle other events
                        debug!("Received event: {:?}", event);
                    }
                }
            }
        });
        
        Ok(handle)
    }
    
    /// Handle consensus result
    async fn handle_consensus_result(
        result: ConsensusResult,
        active_transactions: &Arc<RwLock<HashMap<TransactionId, ActiveTransaction>>>,
        settlement_engine: &Arc<SettlementEngine>,
        metrics: &Arc<GlobalSyncMetrics>,
    ) {
        let mut active_txs = active_transactions.write().await;
        
        if let Some(active_tx) = active_txs.get_mut(&result.transaction_id) {
            if result.approved {
                active_tx.status = TransactionStatus::ConsensusReached;
                active_tx.updated_at = Instant::now();
                
                // Start settlement
                let settlement_request = SettlementRequest {
                    transaction_id: result.transaction_id.clone(),
                    cross_domain_tx: active_tx.cross_domain_tx.clone(),
                    consensus_proof: result.proof,
                };
                
                if let Err(e) = settlement_engine.start_settlement(settlement_request).await {
                    error!("Failed to start settlement: {}", e);
                    active_tx.status = TransactionStatus::Failed(e.to_string());
                } else {
                    active_tx.status = TransactionStatus::SettlementInProgress;
                    active_tx.settlement_status = SettlementStatus::InProgress;
                }
                
                metrics.increment_consensus_reached().await;
            } else {
                active_tx.status = TransactionStatus::Failed("Consensus rejected".to_string());
                metrics.increment_consensus_rejected().await;
            }
        }
    }
    
    /// Handle settlement result
    async fn handle_settlement_result(
        result: SettlementResult,
        active_transactions: &Arc<RwLock<HashMap<TransactionId, ActiveTransaction>>>,
        metrics: &Arc<GlobalSyncMetrics>,
    ) {
        let mut active_txs = active_transactions.write().await;
        
        if let Some(active_tx) = active_txs.get_mut(&result.transaction_id) {
            if result.success {
                active_tx.status = TransactionStatus::Finalized;
                active_tx.settlement_status = SettlementStatus::Completed;
                metrics.increment_transactions_finalized().await;
            } else {
                active_tx.status = TransactionStatus::Failed(result.error.unwrap_or_default());
                active_tx.settlement_status = SettlementStatus::Failed(
                    result.error.unwrap_or_default()
                );
                metrics.increment_settlement_failed().await;
            }
            
            active_tx.updated_at = Instant::now();
        }
    }
    
    /// Handle block proposal
    async fn handle_block_proposal(
        block: GlobalBlock,
        pending_blocks: &Arc<RwLock<HashMap<String, PendingBlock>>>,
        consensus_engine: &Arc<ConsensusEngine>,
    ) {
        // Use canonical block hash (hex-encoded) as the block identifier
        let block_id = hex::encode(&block.hash);
        let required_votes = consensus_engine.get_required_votes().await;
        
        let pending_block = PendingBlock {
            block_id: block_id.clone(),
            block: block.clone(),
            votes: HashMap::new(),
            required_votes,
            created_at: Instant::now(),
            timeout_at: Instant::now() + Duration::from_secs(30), // 30 second timeout
        };
        
        let mut pending = pending_blocks.write().await;
        pending.insert(block_id, pending_block);
        
        // Vote on the block
        if let Err(e) = consensus_engine.vote_on_block(block).await {
            error!("Failed to vote on block: {}", e);
        }
    }
    
    /// Handle block finalization
    async fn handle_block_finalization(
        block: GlobalBlock,
        state: &Arc<RwLock<GlobalSyncState>>, 
        metrics: &Arc<GlobalSyncMetrics>,
        storage: &Arc<GlobalStorage>,
    ) {
        let mut state = state.write().await;
        // Align with canonical fields: slot as height, hex-encoded hash for display/state
        state.block_height = block.header.slot;
        state.last_block_hash = hex::encode(&block.hash);
        state.last_updated = Instant::now();
        
        metrics.increment_blocks_finalized().await;
        metrics.update_block_height(block.header.slot).await;

        // Persist finalized block into storage
        let block_hash = block.hash.clone();
        // Build BlockInfo from the finalized block
        // Convert block timestamp (chrono) to SystemTime
        let ts = block.timestamp;
        let timestamp = std::time::UNIX_EPOCH
            + std::time::Duration::from_secs(ts.timestamp() as u64);

        // Use canonical header fields
        let state_root_bytes = block.header.state_root.clone();

        let info = crate::storage::BlockInfo {
            block_hash: block_hash.clone(),
            height: block.header.slot,
            parent_hash: block.header.parent_hash.clone(),
            transaction_count: block.transactions.len() as u32,
            size: bincode::serialize(&block).map(|b| b.len()).unwrap_or(0),
            timestamp,
            difficulty: 0,
            nonce: 0,
            merkle_root: block.header.tx_root.clone(),
            state_root: state_root_bytes,
            metadata: std::collections::HashMap::new(),
        };
        if let Err(e) = storage.store_block(block_hash.clone(), info).await {
            error!("Failed to store finalized block: {}", e);
        }

        // Try to load and log the finality certificate for this block
        let block_hash_hex = hex::encode(&block_hash);
        match storage.get_finality_certificate_by_height(block.header.slot).await {
            Ok(Some(cert)) => {
                info!(
                    "Finality certificate found for block {} (height {}), validators: {}",
                    block_hash_hex,
                    cert.height,
                    cert.signatures.len()
                );
            }
            Ok(None) => {
                match storage.get_finality_certificate_by_hash(block_hash_hex.clone()).await {
                    Ok(Some(cert)) => {
                        info!(
                            "Finality certificate found by hash for block {} (height {}), validators: {}",
                            block_hash_hex,
                            cert.height,
                            cert.signatures.len()
                        );
                    }
                    Ok(None) => {
                        debug!(
                            "No finality certificate recorded yet for block {}",
                            block_hash_hex
                        );
                    }
                    Err(e) => {
                        error!(
                            "Error fetching finality certificate by hash for {}: {}",
                            block_hash_hex,
                            e
                        );
                    }
                }
            }
            Err(e) => {
                error!(
                    "Error fetching finality certificate by height {}: {}",
                    block.header.slot,
                    e
                );
            }
        }

        // Tag transactions in this block with height/hash and record index
        let tx_ids: Vec<TransactionId> = block
            .transactions
            .iter()
            .map(|t| t.id.clone())
            .collect();
        if let Err(e) = storage.assign_block_transactions(block.header.slot, block_hash.clone(), &tx_ids).await {
            error!("Failed to assign transactions to block {}: {}", block.header.slot, e);
        }
    }
    
    /// Start metrics updater
    async fn start_metrics_updater(&self) -> GarpResult<tokio::task::JoinHandle<()>> {
        let metrics = self.metrics.clone();
        let active_transactions = self.active_transactions.clone();
        let state = self.state.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));
            
            loop {
                interval.tick().await;
                
                // Update performance metrics
                let active_count = active_transactions.read().await.len();
                metrics.update_active_transactions(active_count).await;
                
                // Calculate TPS and other metrics
                let tps = metrics.calculate_tps().await;
                let avg_consensus_time = metrics.get_avg_consensus_time().await;
                let avg_settlement_time = metrics.get_avg_settlement_time().await;
                let success_rate = metrics.get_success_rate().await;
                
                // Update state metrics
                {
                    let mut state = state.write().await;
                    state.performance_metrics.tps = tps;
                    state.performance_metrics.avg_consensus_time_ms = avg_consensus_time;
                    state.performance_metrics.avg_settlement_time_ms = avg_settlement_time;
                    state.performance_metrics.success_rate = success_rate;
                    state.last_updated = Instant::now();
                }
            }
        });
        
        Ok(handle)
    }
    
    /// Start health checker
    async fn start_health_checker(&self) -> GarpResult<tokio::task::JoinHandle<()>> {
        let event_tx = self.event_tx.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                if let Err(e) = event_tx.send(GlobalSyncEvent::HealthCheck) {
                    error!("Failed to send health check event: {}", e);
                    break;
                }
            }
        });
        
        Ok(handle)
    }
    
    /// Start transaction monitor
    async fn start_transaction_monitor(&self) -> GarpResult<tokio::task::JoinHandle<()>> {
        let active_transactions = self.active_transactions.clone();
        let metrics = self.metrics.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                let now = Instant::now();
                let mut timed_out_transactions = Vec::new();
                
                // Check for timed out transactions
                {
                    let mut active_txs = active_transactions.write().await;
                    
                    for (tx_id, active_tx) in active_txs.iter_mut() {
                        if now > active_tx.timeout_at {
                            active_tx.status = TransactionStatus::TimedOut;
                            active_tx.updated_at = now;
                            timed_out_transactions.push(tx_id.clone());
                        }
                    }
                }
                
                // Update metrics for timed out transactions
                for _ in timed_out_transactions {
                    metrics.increment_transactions_timed_out().await;
                }
            }
        });
        
        Ok(handle)
    }
    
    /// Start block processor
    async fn start_block_processor(&self) -> GarpResult<tokio::task::JoinHandle<()>> {
        let pending_blocks = self.pending_blocks.clone();
        let event_tx = self.event_tx.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(1));
            
            loop {
                interval.tick().await;
                
                let now = Instant::now();
                let mut finalized_blocks = Vec::new();
                let mut timed_out_blocks = Vec::new();
                
                // Check pending blocks
                {
                    let mut pending = pending_blocks.write().await;
                    let mut to_remove = Vec::new();
                    
                    for (block_id, pending_block) in pending.iter() {
                        if pending_block.votes.len() >= pending_block.required_votes {
                            // Block has enough votes
                            finalized_blocks.push(pending_block.block.clone());
                            to_remove.push(block_id.clone());
                        } else if now > pending_block.timeout_at {
                            // Block timed out
                            timed_out_blocks.push(block_id.clone());
                            to_remove.push(block_id.clone());
                        }
                    }
                    
                    // Remove processed blocks
                    for block_id in to_remove {
                        pending.remove(&block_id);
                    }
                }
                
                // Send finalization events
                for block in finalized_blocks {
                    if let Err(e) = event_tx.send(GlobalSyncEvent::BlockFinalized(block)) {
                        error!("Failed to send block finalized event: {}", e);
                    }
                }
                
                // Log timed out blocks
                for block_id in timed_out_blocks {
                    warn!("Block timed out: {}", block_id);
                }
            }
        });
        
        Ok(handle)
    }
    
    /// Initiate a cross-chain bridge transfer
    pub async fn initiate_bridge_transfer(
        &self,
        source_chain: String,
        source_tx_id: String,
        target_chain: String,
        amount: u64,
        source_address: String,
        target_address: String,
        asset_id: String,
    ) -> GarpResult<String> {
        self.bridge.initiate_asset_transfer(
            source_chain,
            source_tx_id,
            target_chain,
            amount,
            source_address,
            target_address,
            asset_id,
        ).await
    }
    
    /// Get bridge transaction
    pub async fn get_bridge_transaction(&self, bridge_tx_id: &str) -> GarpResult<Option<BridgeTransaction>> {
        let transactions = self.bridge.bridge_transactions.read().await;
        Ok(transactions.get(bridge_tx_id).cloned())
    }
    
    /// Get bridge transaction status
    pub async fn get_bridge_transaction_status(&self, bridge_tx_id: &str) -> GarpResult<BridgeTransactionStatus> {
        self.bridge.get_bridge_transaction_status(bridge_tx_id).await
    }
    
    /// Add asset mapping
    pub async fn add_asset_mapping(&self, mapping: AssetMapping) -> GarpResult<()> {
        self.bridge.add_asset_mapping(mapping).await
    }
    
    /// Get asset mapping
    pub async fn get_asset_mapping(&self, source_chain: &str, source_asset_id: &str, target_chain: &str) -> GarpResult<Option<AssetMapping>> {
        self.bridge.get_asset_mapping(source_chain, source_asset_id, target_chain).await
    }
    
    /// Add bridge validator
    pub async fn add_bridge_validator(&self, validator: BridgeValidator) -> GarpResult<()> {
        self.bridge.add_validator(validator).await
    }
    
    /// Get bridge validator
    pub async fn get_bridge_validator(&self, validator_id: &str) -> GarpResult<Option<BridgeValidator>> {
        self.bridge.get_validator(validator_id).await
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            tps: 0.0,
            avg_consensus_time_ms: 0.0,
            avg_settlement_time_ms: 0.0,
            success_rate: 0.0,
            network_latency_ms: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::GlobalStorage;
    use crate::config::GlobalSyncConfig;
    
    #[tokio::test]
    async fn test_global_synchronizer_creation() {
        let config = GlobalSyncConfig::default();
        let storage = Arc::new(GlobalStorage::new(Arc::new(config.clone())).await.unwrap());
        
        let synchronizer = GlobalSynchronizer::new(config, storage).await;
        assert!(synchronizer.is_ok());
    }
    
    #[tokio::test]
    async fn test_transaction_submission() {
        let config = GlobalSyncConfig::default();
        let storage = Arc::new(GlobalStorage::new(Arc::new(config.clone())).await.unwrap());
        
        let synchronizer = GlobalSynchronizer::new(config, storage).await.unwrap();
        
        let transaction = CrossDomainTransaction {
            transaction_id: TransactionId::new(),
            source_domain: "domain1".to_string(),
            target_domains: vec!["domain2".to_string()],
            transaction_type: crate::cross_domain::CrossDomainTransactionType::AssetTransfer { asset_id: "asset".into(), amount: 1, from_address: "a".into(), to_address: "b".into() },
            data: vec![1, 2, 3, 4],
            dependencies: vec![],
            required_confirmations: 1,
            confirmations: HashMap::new(),
            status: crate::cross_domain::TransactionStatus::Pending,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            timeout_at: chrono::Utc::now(),
            metadata: HashMap::new(),
        };
        
        let result = synchronizer.submit_transaction(transaction).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_state_management() {
        let config = GlobalSyncConfig::default();
        let storage = Arc::new(GlobalStorage::new(Arc::new(config.clone())).await.unwrap());
        
        let synchronizer = GlobalSynchronizer::new(config, storage).await.unwrap();
        
        let state = synchronizer.get_state().await;
        assert_eq!(state.status, SyncStatus::Starting);
        assert_eq!(state.block_height, 0);
    }
}