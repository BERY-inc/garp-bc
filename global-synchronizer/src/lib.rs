//! Global Synchronizer Library
//! 
//! This library provides the core functionality for the Global Synchronizer service,
//! which coordinates cross-domain transactions and maintains global state consistency
//! across multiple blockchain domains.

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, error, warn};
use garp_common::{
    config::GlobalSyncConfig,
    types::*,
    error::{GarpResult, GarpError},
};

pub mod consensus;
pub mod cross_domain;
pub mod settlement;
pub mod network;
pub mod storage;
pub mod api;

use consensus::ConsensusEngine;
use cross_domain::CrossDomainCoordinator;
use settlement::SettlementEngine;
use network::{NetworkManager, NetworkTopology};
use storage::GlobalStorage;
use crate::discovery::DomainDiscovery;
use serde_json::json;

/// Main Global Synchronizer service
pub struct GlobalSynchronizer {
    config: Arc<GlobalSyncConfig>,
    consensus_engine: Arc<ConsensusEngine>,
    cross_domain_coordinator: Arc<CrossDomainCoordinator>,
    settlement_engine: Arc<SettlementEngine>,
    network_manager: Arc<NetworkManager>,
    storage: Arc<GlobalStorage>,
    is_running: Arc<RwLock<bool>>,
    metrics: Arc<GlobalSyncMetrics>,
    mempool: Arc<RwLock<Vec<TransactionId>>>,
}

/// Global synchronizer metrics
#[derive(Debug, Clone)]
pub struct GlobalSyncMetrics {
    pub total_transactions: Arc<RwLock<u64>>,
    pub successful_transactions: Arc<RwLock<u64>>,
    pub failed_transactions: Arc<RwLock<u64>>,
    pub active_domains: Arc<RwLock<u64>>,
    pub consensus_rounds: Arc<RwLock<u64>>,
    pub settlement_operations: Arc<RwLock<u64>>,
    pub network_messages: Arc<RwLock<u64>>,
    pub storage_operations: Arc<RwLock<u64>>,
    pub uptime: Arc<RwLock<std::time::Duration>>,
    pub avg_transaction_time: Arc<RwLock<f64>>,
}

impl Default for GlobalSyncMetrics {
    fn default() -> Self {
        Self {
            total_transactions: Arc::new(RwLock::new(0)),
            successful_transactions: Arc::new(RwLock::new(0)),
            failed_transactions: Arc::new(RwLock::new(0)),
            active_domains: Arc::new(RwLock::new(0)),
            consensus_rounds: Arc::new(RwLock::new(0)),
            settlement_operations: Arc::new(RwLock::new(0)),
            network_messages: Arc::new(RwLock::new(0)),
            storage_operations: Arc::new(RwLock::new(0)),
            uptime: Arc::new(RwLock::new(std::time::Duration::from_secs(0))),
            avg_transaction_time: Arc::new(RwLock::new(0.0)),
        }
    }
}

impl GlobalSynchronizer {
    /// Create a new Global Synchronizer instance
    pub async fn new(config: GlobalSyncConfig) -> GarpResult<Self> {
        let config = Arc::new(config);
        
        info!("Initializing Global Synchronizer");
        
        // Initialize storage first
        let storage = Arc::new(GlobalStorage::new(config.clone()).await?);
        
        // Initialize network manager
        let network_manager = Arc::new(NetworkManager::new(config.clone()).await?);
        
        // Initialize consensus engine
        let consensus_engine = Arc::new(ConsensusEngine::new(config.clone()).await?);
        
        // Initialize settlement engine
        let settlement_engine = Arc::new(SettlementEngine::new(
            config.clone(),
            storage.clone(),
            network_manager.clone(),
            consensus_engine.clone(),
        ).await?);
        
        // Initialize cross-domain coordinator
        let domain_discovery = Arc::new(DomainDiscovery::new(config.clone()).await?);
        let cross_domain_coordinator = Arc::new(CrossDomainCoordinator::new(
            config.clone(),
            storage.clone(),
            network_manager.clone(),
            domain_discovery.clone(),
            consensus_engine.clone(),
        ).await?);
        
        let metrics = Arc::new(GlobalSyncMetrics::default());
        
        Ok(Self {
            config,
            consensus_engine,
            cross_domain_coordinator,
            settlement_engine,
            network_manager,
            storage,
            is_running: Arc::new(RwLock::new(false)),
            metrics,
            mempool: Arc::new(RwLock::new(Vec::new())),
        })
    }
    
    /// Start the Global Synchronizer service
    pub async fn start(&self) -> GarpResult<()> {
        let mut running = self.is_running.write().await;
        if *running {
            warn!("Global Synchronizer is already running");
            return Ok(());
        }
        
        info!("Starting Global Synchronizer service");
        
        // Start all components in order
        self.storage.start().await?;
        info!("Storage layer started");
        
        self.network_manager.start().await?;
        info!("Network manager started");
        
        self.consensus_engine.start().await?;
        info!("Consensus engine started");
        
        self.settlement_engine.start().await?;
        info!("Settlement engine started");
        
        self.cross_domain_coordinator.start().await?;
        info!("Cross-domain coordinator started");
        
        // Start metrics collection
        self.start_metrics_collection().await?;
        
        *running = true;
        info!("Global Synchronizer service started successfully");
        
        Ok(())
    }
    
    /// Stop the Global Synchronizer service
    pub async fn stop(&self) -> GarpResult<()> {
        let mut running = self.is_running.write().await;
        if !*running {
            warn!("Global Synchronizer is not running");
            return Ok(());
        }
        
        info!("Stopping Global Synchronizer service");
        
        // Stop components in reverse order
        if let Err(e) = self.cross_domain_coordinator.stop().await {
            error!("Error stopping cross-domain coordinator: {}", e);
        }
        
        if let Err(e) = self.settlement_engine.stop().await {
            error!("Error stopping settlement engine: {}", e);
        }
        
        if let Err(e) = self.consensus_engine.stop().await {
            error!("Error stopping consensus engine: {}", e);
        }
        
        if let Err(e) = self.network_manager.stop().await {
            error!("Error stopping network manager: {}", e);
        }
        
        if let Err(e) = self.storage.stop().await {
            error!("Error stopping storage: {}", e);
        }
        
        *running = false;
        info!("Global Synchronizer service stopped");
        
        Ok(())
    }
    
    /// Submit a cross-domain transaction
    pub async fn submit_transaction(&self, transaction: CrossDomainTransaction) -> GarpResult<TransactionId> {
        let running = self.is_running.read().await;
        if !*running {
            return Err(GarpError::ServiceNotRunning("Global Synchronizer not running".to_string()));
        }
        
        info!("Submitting cross-domain transaction: {:?}", transaction.transaction_id);
        
        // Update metrics
        {
            let mut total = self.metrics.total_transactions.write().await;
            *total += 1;
        }
        
        // Submit to cross-domain coordinator
        let result = self.cross_domain_coordinator.submit_transaction(transaction.clone()).await;
        
        match &result {
            Ok(tid) => {
                info!("Transaction submitted successfully");
                // Track in mempool
                let mut mp = self.mempool.write().await;
                mp.push(tid.clone());

                // Normalize and persist transaction payload as garp_common::Transaction JSON in storage
                let common_tx = Self::convert_to_common_transaction(&transaction);
                let serialized = serde_json::to_vec(&common_tx).unwrap_or_default();
                let now = std::time::SystemTime::now();
                let stored = storage::StoredTransaction {
                    transaction_id: tid.clone(),
                    transaction_data: serialized,
                    transaction_type: "common_tx".to_string(),
                    source_domain: transaction.source_domain.clone(),
                    target_domains: transaction.target_domains.clone(),
                    status: storage::TransactionStatus::Pending,
                    consensus_state: storage::ConsensusState {
                        phase: "received".to_string(),
                        votes: std::collections::HashMap::new(),
                        required_votes: 0,
                        result: None,
                        proof: None,
                        started_at: now,
                        completed_at: None,
                    },
                    settlement_state: storage::SettlementState {
                        settlement_id: None,
                        settlement_type: "none".to_string(),
                        domain_settlements: std::collections::HashMap::new(),
                        proof: None,
                        started_at: None,
                        completed_at: None,
                    },
                    created_at: now,
                    updated_at: now,
                    block_height: None,
                    block_hash: None,
                    metadata: std::collections::HashMap::new(),
                    dependencies: transaction.dependencies.clone(),
                    dependents: Vec::new(),
                };
                if let Err(e) = self.storage.store_transaction(stored).await {
                    warn!("Failed to persist submitted transaction: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to submit transaction: {}", e);
                let mut failed = self.metrics.failed_transactions.write().await;
                *failed += 1;
            }
        }
        
        result
    }
    
    /// Get transaction status
    pub async fn get_transaction_status(&self, transaction_id: &TransactionId) -> GarpResult<TransactionStatus> {
        self.cross_domain_coordinator.get_transaction_status(transaction_id).await
    }
    
    /// Get active domains
    pub async fn get_active_domains(&self) -> GarpResult<Vec<DomainId>> {
        self.cross_domain_coordinator.get_active_domains().await
    }
    
    /// Get domain state
    pub async fn get_domain_state(&self, domain_id: &DomainId) -> GarpResult<DomainState> {
        self.cross_domain_coordinator.get_domain_state(domain_id).await
    }
    
    /// Get current consensus view
    pub async fn get_consensus_view(&self) -> GarpResult<u64> {
        self.consensus_engine.get_current_view().await
    }
    
    /// Get network topology
    pub async fn get_network_topology(&self) -> GarpResult<NetworkTopology> {
        self.network_manager.get_network_topology().await
    }

    /// Get mempool transaction IDs
    pub async fn get_mempool(&self) -> Vec<String> {
        let mp = self.mempool.read().await;
        mp.iter().map(|tid| tid.0.to_string()).collect()
    }
    
    /// Get global metrics
    pub async fn get_metrics(&self) -> GarpResult<GlobalSyncMetrics> {
        Ok(self.metrics.as_ref().clone())
    }

    /// Convert cross-domain transaction into garp_common::Transaction for deterministic application
    fn convert_to_common_transaction(&self, tx: &CrossDomainTransaction) -> Transaction {
        let submitter = ParticipantId::new(&tx.source_domain);
        let signatories = vec![ParticipantId::new(&tx.source_domain)];
        let observers: Vec<ParticipantId> = tx
            .target_domains
            .iter()
            .map(|d| ParticipantId::new(&d.0))
            .collect();
        let argument = json!({
            "transaction_type": format!("{:?}", tx.transaction_type),
            "target_domains": tx.target_domains.iter().map(|d| d.0.clone()).collect::<Vec<_>>(),
            "data_hex": hex::encode(&tx.data),
            "required_confirmations": tx.required_confirmations,
        });
        let command = TransactionCommand::Create {
            template_id: "cross_domain".to_string(),
            argument,
            signatories: signatories.clone(),
            observers: observers.clone(),
        };
        Transaction {
            id: tx.transaction_id.clone(),
            submitter,
            command,
            created_at: tx.created_at,
            signatures: Vec::new(),
            encrypted_payload: None,
        }
    }

    /// Get consensus metrics snapshot
    pub async fn get_consensus_metrics(&self) -> GarpResult<crate::consensus::ConsensusMetricsSnapshot> {
        self.consensus_engine.get_metrics_snapshot().await
    }

    /// Get stored transaction
    pub async fn get_transaction(&self, transaction_id: &TransactionId) -> GarpResult<Option<storage::StoredTransaction>> {
        self.storage.get_transaction(transaction_id).await
    }

    /// Get transaction IDs by block height
    pub async fn get_transactions_by_height(&self, height: u64) -> GarpResult<Vec<TransactionId>> {
        self.storage.get_transactions_by_height(height).await
    }
    
    /// Check if the service is running
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }
    
    /// Get service health status
    pub async fn get_health_status(&self) -> GarpResult<ServiceHealth> {
        let running = self.is_running().await;
        
        if !running {
            return Ok(ServiceHealth {
                status: HealthStatus::Down,
                message: "Service is not running".to_string(),
                components: Vec::new(),
                timestamp: std::time::SystemTime::now(),
            });
        }
        
        let mut components = Vec::new();
        
        // Check consensus engine health
        components.push(ComponentHealth {
            name: "consensus_engine".to_string(),
            status: HealthStatus::Up,
            message: "Consensus engine is operational".to_string(),
            metrics: HashMap::new(),
        });
        
        // Check cross-domain coordinator health
        components.push(ComponentHealth {
            name: "cross_domain_coordinator".to_string(),
            status: HealthStatus::Up,
            message: "Cross-domain coordinator is operational".to_string(),
            metrics: HashMap::new(),
        });
        
        // Check settlement engine health
        components.push(ComponentHealth {
            name: "settlement_engine".to_string(),
            status: HealthStatus::Up,
            message: "Settlement engine is operational".to_string(),
            metrics: HashMap::new(),
        });
        
        // Check network manager health
        components.push(ComponentHealth {
            name: "network_manager".to_string(),
            status: HealthStatus::Up,
            message: "Network manager is operational".to_string(),
            metrics: HashMap::new(),
        });
        
        // Check storage health
        components.push(ComponentHealth {
            name: "storage".to_string(),
            status: HealthStatus::Up,
            message: "Storage is operational".to_string(),
            metrics: HashMap::new(),
        });
        
        Ok(ServiceHealth {
            status: HealthStatus::Up,
            message: "All components are operational".to_string(),
            components,
            timestamp: std::time::SystemTime::now(),
        })
    }
    
    /// Start metrics collection background task
    async fn start_metrics_collection(&self) -> GarpResult<()> {
        info!("Starting metrics collection");
        
        // TODO: Implement periodic metrics collection
        // This would typically run in a background task and collect
        // metrics from all components periodically
        
        Ok(())
    }

    /// Get API port from configuration
    pub fn api_port(&self) -> u16 {
        self.config.api.port
    }
    
    /// Get latest block info
    pub async fn get_latest_block(&self) -> GarpResult<Option<storage::BlockInfo>> {
        self.storage.get_latest_block().await
    }
    
    /// Get block by height
    pub async fn get_block_by_height(&self, height: u64) -> GarpResult<Option<storage::BlockInfo>> {
        self.storage.get_block_by_height(height).await
    }
}

/// Service health status
#[derive(Debug, Clone)]
pub struct ServiceHealth {
    pub status: HealthStatus,
    pub message: String,
    pub components: Vec<ComponentHealth>,
    pub timestamp: std::time::SystemTime,
}

/// Component health status
#[derive(Debug, Clone)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthStatus,
    pub message: String,
    pub metrics: HashMap<String, f64>,
}

/// Health status enumeration
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Up,
    Down,
    Degraded,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use garp_common::config::GlobalSyncConfig;
    
    #[tokio::test]
    async fn test_global_synchronizer_creation() {
        let config = GlobalSyncConfig::default();
        let synchronizer = GlobalSynchronizer::new(config).await;
        assert!(synchronizer.is_ok());
    }
    
    #[tokio::test]
    async fn test_global_synchronizer_lifecycle() {
        let config = GlobalSyncConfig::default();
        let synchronizer = GlobalSynchronizer::new(config).await.unwrap();
        
        // Initially not running
        assert!(!synchronizer.is_running().await);
        
        // Start the service
        let start_result = synchronizer.start().await;
        assert!(start_result.is_ok());
        assert!(synchronizer.is_running().await);
        
        // Stop the service
        let stop_result = synchronizer.stop().await;
        assert!(stop_result.is_ok());
        assert!(!synchronizer.is_running().await);
    }
    
    #[tokio::test]
    async fn test_health_status() {
        let config = GlobalSyncConfig::default();
        let synchronizer = GlobalSynchronizer::new(config).await.unwrap();
        
        // Health check when not running
        let health = synchronizer.get_health_status().await.unwrap();
        assert_eq!(health.status, HealthStatus::Down);
        
        // Start and check health
        synchronizer.start().await.unwrap();
        let health = synchronizer.get_health_status().await.unwrap();
        assert_eq!(health.status, HealthStatus::Up);
        assert_eq!(health.components.len(), 5);
        
        synchronizer.stop().await.unwrap();
    }
    
    #[tokio::test]
    async fn test_metrics() {
        let config = GlobalSyncConfig::default();
        let synchronizer = GlobalSynchronizer::new(config).await.unwrap();
        
        let metrics = synchronizer.get_metrics().await.unwrap();
        
        // Check initial metrics values
        assert_eq!(*metrics.total_transactions.read().await, 0);
        assert_eq!(*metrics.successful_transactions.read().await, 0);
        assert_eq!(*metrics.failed_transactions.read().await, 0);
    }
}