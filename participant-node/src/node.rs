use garp_common::{
    ParticipantId, SyncDomainId, Transaction, Contract, Asset, WalletBalance,
    NetworkMessage, MessageEnvelope, NetworkManager, MessageHandler, PeerInfo,
    CryptoService, GarpResult, GarpError, NetworkError, ParticipantNodeConfig
};
use crate::{
    config::Config,
    storage::{StorageBackend, Storage},
    ledger::{LocalLedger, ValidationResult, LedgerView, LedgerStats},
    api::ApiServer,
    wallet::WalletManager,
    contract_engine::ContractEngine,
};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, oneshot};
use tokio::time::{interval, Duration};
use tracing::{info, warn, error, debug};
use chrono::Utc;
use uuid::Uuid;
use std::collections::HashMap;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Default)]
pub struct GlobalHead {
    pub height: u64,
    pub hash: String,
}

/// Participant Node - the main component of the GARP network
pub struct ParticipantNode {
    /// Node configuration
    config: Config,
    /// Unique participant identifier
    participant_id: ParticipantId,
    /// Local ledger for this participant
    ledger: Arc<LocalLedger>,
    /// Wallet manager
    wallet: Arc<WalletManager>,
    /// Contract execution engine
    contract_engine: Arc<ContractEngine>,
    /// Network manager for peer communication
    network: Arc<NetworkManager>,
    /// Cryptographic service
    crypto_service: Arc<CryptoService>,
    /// Storage backend
    storage: Arc<dyn StorageBackend>,
    /// API server
    api_server: Option<ApiServer>,
    /// Message handlers
    message_handlers: Arc<RwLock<HashMap<String, Box<dyn MessageHandler>>>>,
    /// Shutdown signal
    shutdown_tx: Option<oneshot::Sender<()>>,
    /// Latest known global synchronizer head
    global_head: Arc<RwLock<GlobalHead>>,
    /// Sync last applied height
    sync_last_applied_height: Arc<RwLock<u64>>,
    /// Sync last applied time
    sync_last_applied_time: Arc<RwLock<Option<chrono::DateTime<Utc>>>>,
}

/// Node status
#[derive(Debug, Clone)]
pub enum NodeStatus {
    Starting,
    Running,
    Syncing,
    Error(String),
    Shutdown,
}

/// Node statistics
#[derive(Debug, Clone)]
pub struct NodeStats {
    pub status: NodeStatus,
    pub uptime: Duration,
    pub connected_peers: u32,
    pub sync_domains: Vec<SyncDomainId>,
    pub ledger_stats: LedgerStats,
    pub last_sync_time: Option<chrono::DateTime<Utc>>,
}

impl ParticipantNode {
    /// Create a new participant node
    pub async fn new(config: Config) -> GarpResult<Self> {
        info!("Initializing Participant Node with ID: {}", config.participant.participant_id.0);

        // Initialize crypto service
        let crypto_service = Arc::new(CryptoService::new());

        // Initialize storage
        let storage = if let Some(db_config) = &config.database {
            Storage::postgres(&db_config.url, db_config.max_connections).await?
        } else {
            Storage::memory()
        };

        // Initialize ledger
        let ledger = Arc::new(LocalLedger::new(
            config.participant.participant_id.clone(),
            storage.clone(),
            crypto_service.clone(),
        ));

        // Initialize wallet manager
        let wallet = Arc::new(WalletManager::new(
            config.participant.participant_id.clone(),
            storage.clone(),
            crypto_service.clone(),
        ));

        // Initialize contract engine
        let contract_engine = Arc::new(ContractEngine::new(
            storage.clone(),
            crypto_service.clone(),
        ));

        // Initialize network manager
        let network = Arc::new(NetworkManager::new(
            config.participant.participant_id.clone(),
            config.participant.network_address.clone(),
        ));

        let node = Self {
            participant_id: config.participant.participant_id.clone(),
            config,
            ledger,
            wallet,
            contract_engine,
            network,
            crypto_service,
            storage,
            api_server: None,
            message_handlers: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: None,
            global_head: Arc::new(RwLock::new(GlobalHead { height: 0, hash: String::new() })),
            sync_last_applied_height: Arc::new(RwLock::new(0)),
            sync_last_applied_time: Arc::new(RwLock::new(None)),
        };

        Ok(node)
    }

    /// Start the participant node
    pub async fn start(&mut self) -> GarpResult<()> {
        info!("Starting Participant Node {}", self.participant_id.0);

        // Register message handlers
        self.register_message_handlers().await?;

        // Start network manager
        self.network.start().await?;

        // Connect to sync domains
        self.connect_to_sync_domains().await?;

        // Start API server if configured
        if let Some(api_config) = &self.config.api {
            let api_server = ApiServer::new(
                api_config.clone(),
                self.ledger.clone(),
                self.wallet.clone(),
                self.contract_engine.clone(),
                self.crypto_service.clone(),
            );
            
            api_server.start().await?;
            self.api_server = Some(api_server);
        }

        // Initialize local head height from file if present
        if let Ok(path) = std::env::var("LOCAL_HEAD_PATH") {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                if let Ok(h) = contents.trim().parse::<u64>() {
                    *self.sync_last_applied_height.write().await = h;
                    *self.sync_last_applied_time.write().await = Some(Utc::now());
                    info!("Loaded local head height {} from {}", h, path);
                }
            }
        }

        // Start background tasks
        self.start_background_tasks().await?;

        info!("Participant Node {} started successfully", self.participant_id.0);
        Ok(())
    }

    /// Stop the participant node
    pub async fn stop(&mut self) -> GarpResult<()> {
        info!("Stopping Participant Node {}", self.participant_id.0);

        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        // Stop API server
        if let Some(api_server) = &mut self.api_server {
            api_server.stop().await?;
        }

        // Stop network manager
        // Note: NetworkManager would need a stop method in a real implementation

        info!("Participant Node {} stopped", self.participant_id.0);
        Ok(())
    }

    pub async fn get_global_head(&self) -> (u64, String) {
        let gh = self.global_head.read().await;
        (gh.height, gh.hash.clone())
    }
    pub fn get_sync_domain_ids(&self) -> Vec<String> {
        self.config.sync_domains.iter().map(|sd| sd.domain_id.0.clone()).collect()
    }

    /// Submit a transaction to the network
    pub async fn submit_transaction(&self, transaction: Transaction) -> GarpResult<ValidationResult> {
        debug!("Submitting transaction {}", transaction.id.0);

        // Validate transaction locally first
        let validation = self.ledger.validate_transaction(&transaction).await?;
        if !validation.valid {
            warn!("Transaction validation failed locally: {:?}", validation.errors);
            return Ok(validation);
        }

        // Submit to local ledger
        let result = self.ledger.submit_transaction(transaction.clone()).await?;

        // Broadcast to sync domains if valid
        if result.valid {
            self.broadcast_transaction_to_sync_domains(transaction).await?;
        }

        Ok(result)
    }

    /// Get ledger view for this participant
    pub async fn get_ledger_view(&self) -> GarpResult<LedgerView> {
        self.ledger.get_ledger_view().await
    }

    /// Get node statistics
    pub async fn get_stats(&self) -> GarpResult<NodeStats> {
        let ledger_stats = self.ledger.get_stats().await?;
        let network_stats = self.network.get_stats().await;

        Ok(NodeStats {
            status: NodeStatus::Running, // Simplified
            uptime: Duration::from_secs(0), // Would track actual uptime
            connected_peers: network_stats.connected_peers,
            sync_domains: self.config.sync_domains.iter().map(|sd| sd.domain_id.clone()).collect(),
            ledger_stats,
            last_sync_time: Some(Utc::now()), // Simplified
        })
    }

    /// Get ledger checkpoint/state snapshot
    pub async fn get_ledger_state(&self) -> GarpResult<crate::storage::LedgerState> {
        self.ledger.create_checkpoint().await
    }

    /// Register message handlers for different message types
    async fn register_message_handlers(&self) -> GarpResult<()> {
        let mut handlers = self.message_handlers.write().await;

        // Transaction message handler
        let transaction_handler = TransactionMessageHandler::new(
            self.ledger.clone(),
            self.crypto_service.clone(),
        );
        handlers.insert("transaction".to_string(), Box::new(transaction_handler));

        // Sync message handler
        let sync_handler = SyncMessageHandler::new(
            self.participant_id.clone(),
            self.ledger.clone(),
        );
        handlers.insert("sync".to_string(), Box::new(sync_handler));

        // Contract message handler
        let contract_handler = ContractMessageHandler::new(
            self.contract_engine.clone(),
            self.ledger.clone(),
        );
        handlers.insert("contract".to_string(), Box::new(contract_handler));

        Ok(())
    }

    /// Connect to configured sync domains
    async fn connect_to_sync_domains(&self) -> GarpResult<()> {
        for sync_domain in &self.config.sync_domains {
            info!("Connecting to sync domain: {}", sync_domain.domain_id.0);
            
            let peer_info = PeerInfo {
                id: sync_domain.domain_id.0.clone(),
                address: sync_domain.endpoint.clone(),
                public_key: sync_domain.public_key.clone(),
                last_seen: Utc::now(),
            };

            self.network.add_peer(peer_info).await?;
        }

        Ok(())
    }

    /// Broadcast transaction to sync domains
    async fn broadcast_transaction_to_sync_domains(&self, transaction: Transaction) -> GarpResult<()> {
        let message = NetworkMessage::Transaction(transaction);
        let envelope = MessageEnvelope {
            id: Uuid::new_v4().to_string(),
            message_type: "transaction".to_string(),
            sender: self.participant_id.0.clone(),
            recipient: None, // Broadcast
            payload: serde_json::to_value(&message)
                .map_err(|e| NetworkError::SerializationFailed(e.to_string()))?,
            timestamp: Utc::now(),
            signature: None, // Would be added by network layer
        };

        // Send to all sync domains
        for sync_domain in &self.config.sync_domains {
            if let Err(e) = self.network.send_message(&sync_domain.domain_id.0, envelope.clone()).await {
                warn!("Failed to send transaction to sync domain {}: {}", sync_domain.domain_id.0, e);
            }
        }

        Ok(())
    }

    /// Start background tasks
    async fn start_background_tasks(&self) -> GarpResult<()> {
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        // Ledger checkpoint task
        let ledger = self.ledger.clone();
        let checkpoint_interval = Duration::from_secs(self.config.participant.checkpoint_interval_seconds);
        tokio::spawn(async move {
            let mut interval = interval(checkpoint_interval);
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = ledger.create_checkpoint().await {
                            error!("Failed to create ledger checkpoint: {}", e);
                        }
                    }
                    _ = &mut shutdown_rx => {
                        debug!("Checkpoint task shutting down");
                        break;
                    }
                }
            }
        });

        // Sync task: poll global synchronizer for latest block and refresh local checkpoint
        let sync_interval = Duration::from_secs(30); // Sync every 30 seconds
        let synchronizer_url = std::env::var("SYNCHRONIZER_URL").unwrap_or_default();
        let global_head = self.global_head.clone();
        let ledger_for_sync = self.ledger.clone();
        let sync_last_applied_height = self.sync_last_applied_height.clone();
        let sync_last_applied_time = self.sync_last_applied_time.clone();
        tokio::spawn(async move {
            let mut interval = interval(sync_interval);
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if !synchronizer_url.is_empty() {
                            if let Err(e) = poll_latest_block(&synchronizer_url, global_head.clone()).await {
                                warn!("Sync poll error: {}", e);
                            } else {
                                if let Err(e) = ledger_for_sync.create_checkpoint().await {
                                    error!("Failed to refresh ledger checkpoint after sync: {}", e);
                                }
                                // Apply any new finalized blocks to local ledger
                                if let Err(e) = apply_finalized_blocks(&synchronizer_url, global_head.clone(), ledger_for_sync.clone(), sync_last_applied_height.clone()).await {
                                    warn!("Apply finalized blocks error: {}", e);
                                }
                                {
                                    let h = global_head.read().await.height;
                                    *sync_last_applied_height.write().await = h;
                                    *sync_last_applied_time.write().await = Some(Utc::now());
                                }
                            }
                        } else {
                            debug!("SYNCHRONIZER_URL not set; skipping block sync poll");
                        }
                    }
                    _ = &mut shutdown_rx => {
                        debug!("Sync task shutting down");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

async fn poll_latest_block(base: &str, head: Arc<RwLock<GlobalHead>>) -> Result<(), String> {
    use hyper::{Client, Request};
    use hyper::body::to_bytes;
    use hyper::http::Uri;
    let url = format!("{}/api/v1/blocks/latest", base);
    let uri: Uri = url.parse().map_err(|e| e.to_string())?;
    let client = Client::new();
    let req = Request::builder().method("GET").uri(uri).body(hyper::Body::empty()).map_err(|e| e.to_string())?;
    let resp = client.request(req).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() { return Err(format!("status {}", resp.status())); }
    let body = to_bytes(resp.into_body()).await.map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    // Log block info if present
    if let Some(data) = v.get("data") {
        let height = data.get("height").or_else(|| data.get("number")).and_then(|h| h.as_u64()).unwrap_or(0);
        let hash = data.get("block_hash").or_else(|| data.get("hash")).and_then(|h| h.as_str()).unwrap_or("");
        info!("Synced latest global block height={} hash={}", height, hash);
        let mut gh = head.write().await;
        gh.height = height;
        gh.hash = hash.to_string();
    }
    Ok(())
}

async fn apply_finalized_blocks(base: &str, head: Arc<RwLock<GlobalHead>>, ledger: Arc<LocalLedger>, last_applied: Arc<RwLock<u64>>) -> Result<(), String> {
    use hyper::{Client, Request};
    use hyper::body::to_bytes;
    use hyper::http::Uri;
    let client = Client::new();
    let target_height = head.read().await.height;
    let local_applied = *last_applied.read().await;
    for h in (local_applied + 1)..=target_height {
        let url = format!("{}/api/v1/blocks/{}/details", base, h);
        let uri: Uri = url.parse().map_err(|e| e.to_string())?;
        let req = Request::builder().method("GET").uri(uri).body(hyper::Body::empty()).map_err(|e| e.to_string())?;
        let resp = client.request(req).await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() { continue; }
        let body = to_bytes(resp.into_body()).await.map_err(|e| e.to_string())?;
        let v: serde_json::Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
        let data = match v.get("data") { Some(d) => d, None => continue };
        // Get transaction IDs
        let txids: Vec<String> = data.get("transaction_ids").and_then(|a| a.as_array()).map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
        for txid in txids {
            // Fetch transaction details
            let turl = format!("{}/api/v1/transactions/{}/details", base, txid);
            let turi: Uri = turl.parse().map_err(|e| e.to_string())?;
            let treq = Request::builder().method("GET").uri(turi).body(hyper::Body::empty()).map_err(|e| e.to_string())?;
            let tresp = client.request(treq).await.map_err(|e| e.to_string())?;
            if !tresp.status().is_success() { continue; }
            let tbody = to_bytes(tresp.into_body()).await.map_err(|e| e.to_string())?;
            let tv: serde_json::Value = serde_json::from_slice(&tbody).map_err(|e| e.to_string())?;
            let tdata = match tv.get("data") { Some(d) => d, None => continue };
            // If typed transaction JSON present, try to decode into garp_common::Transaction
            if let Some(tj) = tdata.get("transaction") {
                let tx: Result<garp_common::types::Transaction, _> = serde_json::from_value(tj.clone());
                if let Ok(tx) = tx {
                    // Apply transaction to local ledger without broadcasting
                    if let Err(e) = ledger.submit_transaction(tx).await {
                        warn!("Failed to apply transaction to local ledger: {}", e);
                    }
                }
            }
        }
        // Persist updated local head height to file if configured
        if let Ok(path) = std::env::var("LOCAL_HEAD_PATH") {
            let _ = std::fs::write(&path, h.to_string());
        }
        *last_applied.write().await = h;
    }
    Ok(())
}

    /// Handle incoming network message
    pub async fn handle_message(&self, envelope: MessageEnvelope) -> GarpResult<()> {
        let handlers = self.message_handlers.read().await;
        
        if let Some(handler) = handlers.get(&envelope.message_type) {
            handler.handle_message(envelope).await?;
        } else {
            warn!("No handler found for message type: {}", envelope.message_type);
        }

        Ok(())
    }

    /// Get contract by ID
    pub async fn get_contract(&self, contract_id: &garp_common::ContractId) -> GarpResult<Option<Contract>> {
        self.ledger.get_contract(contract_id).await
    }

    /// Get wallet balance
    pub async fn get_wallet_balance(&self) -> GarpResult<Option<WalletBalance>> {
        self.wallet.get_balance().await
    }

    /// Transfer asset to another participant
    pub async fn transfer_asset(&self, to: ParticipantId, asset: Asset) -> GarpResult<garp_common::TransactionId> {
        self.wallet.transfer_asset(to, asset).await
    }

    /// Create a new asset
    pub async fn create_asset(&self, asset: Asset) -> GarpResult<garp_common::TransactionId> {
        self.wallet.create_asset(asset).await
    }

    pub async fn get_global_head(&self) -> (u64, String) {
        let gh = self.global_head.read().await;
        (gh.height, gh.hash.clone())
    }

    pub fn get_sync_domain_ids(&self) -> Vec<String> {
        self.config.sync_domains.iter().map(|sd| sd.domain_id.0.clone()).collect()
    }

    pub async fn get_sync_last_applied(&self) -> (u64, Option<chrono::DateTime<Utc>>) {
        (*self.sync_last_applied_height.read().await, self.sync_last_applied_time.read().await.clone())
    }
}

/// Message handler for transaction messages
struct TransactionMessageHandler {
    ledger: Arc<LocalLedger>,
    crypto_service: Arc<CryptoService>,
}

impl TransactionMessageHandler {
    fn new(ledger: Arc<LocalLedger>, crypto_service: Arc<CryptoService>) -> Self {
        Self { ledger, crypto_service }
    }
}

#[async_trait::async_trait]
impl MessageHandler for TransactionMessageHandler {
    async fn handle_message(&self, envelope: MessageEnvelope) -> GarpResult<()> {
        if let Ok(NetworkMessage::Transaction(transaction)) = serde_json::from_value(envelope.payload) {
            debug!("Received transaction message: {}", transaction.id.0);
            
            // Validate and apply transaction
            match self.ledger.submit_transaction(transaction).await {
                Ok(result) => {
                    if result.valid {
                        info!("Transaction applied successfully");
                    } else {
                        warn!("Transaction validation failed: {:?}", result.errors);
                    }
                }
                Err(e) => {
                    error!("Failed to process transaction: {}", e);
                }
            }
        }

        Ok(())
    }
}

/// Message handler for sync messages
struct SyncMessageHandler {
    participant_id: ParticipantId,
    ledger: Arc<LocalLedger>,
}

impl SyncMessageHandler {
    fn new(participant_id: ParticipantId, ledger: Arc<LocalLedger>) -> Self {
        Self { participant_id, ledger }
    }
}

#[async_trait::async_trait]
impl MessageHandler for SyncMessageHandler {
    async fn handle_message(&self, envelope: MessageEnvelope) -> GarpResult<()> {
        debug!("Received sync message from {}", envelope.sender);
        
        // Handle sync-related messages
        // This would include ledger state synchronization, checkpoint sharing, etc.
        
        Ok(())
    }
}

/// Message handler for contract messages
struct ContractMessageHandler {
    contract_engine: Arc<ContractEngine>,
    ledger: Arc<LocalLedger>,
}

impl ContractMessageHandler {
    fn new(contract_engine: Arc<ContractEngine>, ledger: Arc<LocalLedger>) -> Self {
        Self { contract_engine, ledger }
    }
}

#[async_trait::async_trait]
impl MessageHandler for ContractMessageHandler {
    async fn handle_message(&self, envelope: MessageEnvelope) -> GarpResult<()> {
        debug!("Received contract message from {}", envelope.sender);
        
        // Handle contract-related messages
        // This would include contract execution requests, results, etc.
        
        Ok(())
    }
}