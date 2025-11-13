use garp_common::{
    ParticipantId, SyncDomainId, Transaction, Contract, Asset, WalletBalance,
    NetworkMessage, NetworkManager, MessageHandler, PeerInfo,
    CryptoService, GarpResult, GarpError, NetworkError,
};
use garp_common::timing::slot_at_time;
use crate::consensus::{leader_for_slot, TowerBft, ForkGraph};
use crate::mempool::{Mempool, MempoolConfig};
use crate::block_builder::BlockBuilder;
use crate::{
    config::Config,
    storage::{StorageBackend, Storage},
    ledger::{LocalLedger, ValidationResult, LedgerView, LedgerStats},
    api::ApiServer,
    wallet::WalletManager,
    contract_engine::ContractEngine,
};
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
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
    /// Transaction mempool
    mempool: Arc<Mempool>,
    /// Cryptographic service
    crypto_service: Arc<CryptoService>,
    /// Storage backend
    storage: Arc<dyn StorageBackend>,
    /// API server (moved to main; retained for future use)
    /// api_server: Option<ApiServer>,
    /// Message handlers
    message_handlers: Arc<RwLock<HashMap<String, Box<dyn MessageHandler>>>>,
    /// Shutdown broadcast tx
    shutdown_tx: Option<broadcast::Sender<()>>,
    /// Latest known global synchronizer head
    global_head: Arc<RwLock<GlobalHead>>,
    /// Highest finalized slot observed by consensus
    highest_finalized_slot: Arc<RwLock<u64>>,
    /// Sync last applied height
    sync_last_applied_height: Arc<RwLock<u64>>,
    /// Sync last applied time
    sync_last_applied_time: Arc<RwLock<Option<chrono::DateTime<Utc>>>>,
    // Consensus state
    consensus_tower: Arc<RwLock<TowerBft>>,
    fork_graph: Arc<RwLock<ForkGraph>>,
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
        info!("Initializing Participant Node with ID: {}", config.participant_config.participant_id.0);

        // Initialize crypto service
        let crypto_service = Arc::new(CryptoService::new());

        // Initialize storage
        let storage = Storage::postgres(&config.database.url, config.database.max_connections).await?;

        // Initialize ledger
        let ledger = Arc::new(LocalLedger::new(
            config.participant_config.participant_id.clone(),
            storage.clone(),
            crypto_service.clone(),
        ));

        // Initialize wallet manager
        let wallet = Arc::new(WalletManager::new(
            config.participant_config.participant_id.clone(),
            storage.clone(),
            crypto_service.clone(),
        ));

        // Initialize contract engine
        let contract_engine = Arc::new(ContractEngine::new(
            storage.clone(),
            crypto_service.clone(),
        ));

        // Initialize network manager with real network layer
        let network_layer = Arc::new(RealNetworkLayer::new(config.network.clone()));
        let network = Arc::new(NetworkManager::new(
            config.participant_config.participant_id.clone(),
            crypto_service.clone(),
            network_layer,
        ));

        // Initialize mempool
        let mempool = Arc::new(Mempool::new(MempoolConfig::default()));

        // Initialize consensus state (weights from initial balances; default 1)
        let mut voting_power: HashMap<ParticipantId, u64> = HashMap::new();
        for v in &config.genesis.initial_validators {
            let w = *config.genesis.initial_balances.get(v).unwrap_or(&1);
            voting_power.insert(v.clone(), w);
        }
        let consensus_tower = Arc::new(RwLock::new(TowerBft::new(voting_power, 0.667)));
        let fork_graph = Arc::new(RwLock::new(ForkGraph::new()));

        let node = Self {
            participant_id: config.participant_config.participant_id.clone(),
            config,
            ledger,
            wallet,
            contract_engine,
            network,
            mempool,
            crypto_service,
            storage,
            // api_server: None,
            message_handlers: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: None,
            global_head: Arc::new(RwLock::new(GlobalHead { height: 0, hash: String::new() })),
            highest_finalized_slot: Arc::new(RwLock::new(0)),
            sync_last_applied_height: Arc::new(RwLock::new(0)),
            sync_last_applied_time: Arc::new(RwLock::new(None)),
            consensus_tower,
            fork_graph,
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

        // API server is spawned from main.rs using Arc<ParticipantNode>

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

        // Load accounts snapshot if configured
        if let Ok(acc_path) = std::env::var("ACCOUNTS_SNAPSHOT_PATH") {
            if std::path::Path::new(&acc_path).exists() {
                if let Err(e) = self.ledger.load_accounts_snapshot(&acc_path) {
                    warn!("Failed to load accounts snapshot from {}: {}", acc_path, e);
                } else {
                    info!("Loaded accounts snapshot from {}", acc_path);
                }
            }
        }

        // Start background tasks
        self.start_background_tasks().await?;

        // Periodic accounts snapshot task
        if let Ok(acc_path) = std::env::var("ACCOUNTS_SNAPSHOT_PATH") {
            let ledger = self.ledger.clone();
            tokio::spawn(async move {
                let mut ticker = interval(Duration::from_secs(300));
                loop {
                    ticker.tick().await;
                    if let Err(e) = ledger.save_accounts_snapshot(&acc_path) {
                        warn!("Accounts snapshot save failed: {}", e);
                    }
                }
            });
            info!("Started periodic accounts snapshot writer");
        }

        info!("Participant Node {} started successfully", self.participant_id.0);
        Ok(())
    }

    /// Gracefully shutdown background tasks
    pub async fn shutdown(&self) -> GarpResult<()> {
        info!("Shutting down Participant Node {}", self.participant_id.0);
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(());
        }
        Ok(())
    }

    pub async fn get_global_head(&self) -> (u64, String) {
        let gh = self.global_head.read().await;
        (gh.height, gh.hash.clone())
    }

    /// Get this node's participant ID
    pub fn get_participant_id(&self) -> ParticipantId {
        self.participant_id.clone()
    }
    /// Expose storage for API usage
    pub fn get_storage(&self) -> Arc<dyn StorageBackend> {
        self.storage.clone()
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

        // Record a ledger transaction entry with current slot
        let now = Utc::now();
        let slot = garp_common::timing::slot_at_time(self.config.genesis.genesis_time, self.config.chain.slot_duration_ms, now);
        self.ledger.record_transaction_entry(transaction.id.clone(), slot);

        // Broadcast to sync domains if valid
        if result.valid {
            self.broadcast_transaction_to_sync_domains(transaction).await?;
        }

        Ok(result)
    }

    /// Submit a transaction to the local mempool with a fee for prioritization
    pub async fn submit_to_mempool(&self, transaction: Transaction, fee: u64) -> GarpResult<()> {
        self.mempool.submit(transaction, fee).await
    }

    /// Retrieve a prioritized batch of transactions for block assembly
    pub async fn get_mempool_batch(&self, max: usize) -> Vec<Transaction> {
        self.mempool.get_batch(max).await
    }

    /// Get ledger view for this participant
    pub async fn get_ledger_view(&self) -> GarpResult<LedgerView> {
        self.ledger.get_ledger_view().await
    }

    /// Simulate TxV2 via ledger interface
    pub async fn simulate_transaction_v2(&self, tx: &garp_common::TxV2) -> GarpResult<crate::ledger::SimulationResult> {
        self.ledger.simulate_v2(tx).await
    }
    /// Get genesis time and slot duration for timing calculations
    pub fn get_timing_params(&self) -> (chrono::DateTime<Utc>, u64) {
        (self.genesis_time, self.slot_duration_ms)
    }
    /// Get initial validator set from config
    pub fn get_validators(&self) -> Vec<ParticipantId> {
        self.config.genesis.initial_validators.clone()
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

        // Consensus message handler
        let consensus_handler = ConsensusMessageHandler::new(
            self.participant_id.clone(),
            self.consensus_tower.clone(),
            self.fork_graph.clone(),
            self.ledger.clone(),
            self.config.chain.slot_duration_ms,
            self.config.genesis.genesis_time,
            self.highest_finalized_slot.clone(),
        );
        handlers.insert("consensus".to_string(), Box::new(consensus_handler));

        Ok(())
    }

    /// Connect to configured sync domains
    async fn connect_to_sync_domains(&self) -> GarpResult<()> {
        for sync_domain in &self.config.sync_domains {
            info!("Connecting to sync domain: {}", sync_domain.domain_id.0);

            // Parse endpoint into NetworkAddress
            let (host, port, protocol) = parse_endpoint(&sync_domain.endpoint);
            let address = NetworkAddress::new(&host, port, protocol);
            let peer_info = PeerInfo::new(
                ParticipantId::new(&sync_domain.domain_id.0),
                address,
                sync_domain.public_key.clone(),
            );

            self.network.add_peer(peer_info).await?;
        }

        Ok(())
    }

    /// Broadcast transaction to sync domains
    async fn broadcast_transaction_to_sync_domains(&self, transaction: Transaction) -> GarpResult<()> {
        let msg = NetworkMessage::Transaction(transaction);
        let recipients: Vec<ParticipantId> = self.config
            .sync_domains
            .iter()
            .map(|sd| ParticipantId::new(&sd.domain_id.0))
            .collect();
        if let Err(e) = self.network.broadcast_secure_message(&recipients, &msg).await {
            warn!("Failed to broadcast transaction to sync domains: {}", e);
        }
        Ok(())
    }

    /// Start background tasks
    async fn start_background_tasks(&mut self) -> GarpResult<()> {
        let (shutdown_tx, _) = broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        // Ledger checkpoint task
        let ledger = self.ledger.clone();
        let checkpoint_interval = Duration::from_secs(self.config.participant.checkpoint_interval_seconds);
        tokio::spawn({
            let mut shutdown_rx = shutdown_tx.subscribe();
            async move {
            let mut interval = interval(checkpoint_interval);
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = ledger.create_checkpoint().await {
                            error!("Failed to create ledger checkpoint: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Checkpoint task shutting down");
                        break;
                    }
                }
            }
        }});

        // Sync task: poll global synchronizer for latest block and refresh local checkpoint
        let sync_interval = Duration::from_secs(30); // Sync every 30 seconds
        let synchronizer_url = std::env::var("SYNCHRONIZER_URL").unwrap_or_default();
        let global_head = self.global_head.clone();
        let ledger_for_sync = self.ledger.clone();
        let sync_last_applied_height = self.sync_last_applied_height.clone();
        let sync_last_applied_time = self.sync_last_applied_time.clone();
        tokio::spawn({
            let mut shutdown_rx = shutdown_tx.subscribe();
            async move {
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
                    _ = shutdown_rx.recv() => {
                        debug!("Sync task shutting down");
                        break;
                    }
                }
            }
        }});

        // PoH ticker task: tick at ledger-defined cadence per slot
        let ledger_for_poh = self.ledger.clone();
        let slot_duration_ms = self.config.chain.slot_duration_ms;
        let genesis_time = self.config.genesis.genesis_time;
        tokio::spawn({
            let mut shutdown_rx = shutdown_tx.subscribe();
            async move {
                let mut interval = interval(ledger_for_poh.tick_interval(slot_duration_ms));
                loop {
                    tokio::select! {
                        _ = interval.tick() => {
                            let now = Utc::now();
                            let cur_slot = garp_common::timing::slot_at_time(genesis_time, slot_duration_ms, now);
                            ledger_for_poh.record_poh_tick(cur_slot);
                        }
                        _ = shutdown_rx.recv() => {
                            debug!("PoH ticker task shutting down");
                            break;
                        }
                    }
                }
            }
        });

        // Proposer task: assemble blocks from mempool at slot cadence
        let proposer_id = self.participant_id.clone();
        let chain = self.config.chain.clone();
        let genesis = self.config.genesis.clone();
        let mempool = self.mempool.clone();
        let network = self.network.clone();
        let sync_domains = self.config.sync_domains.clone();
        let global_head = self.global_head.clone();
        let forks = self.fork_graph.clone();
        let storage = self.storage.clone();
        tokio::spawn({
            let mut shutdown_rx = shutdown_tx.subscribe();
            async move {
            let mut interval = interval(Duration::from_millis(chain.slot_duration_ms));
            let builder = BlockBuilder::new(chain.clone(), genesis.clone());
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Determine leader for current slot
                        let now = Utc::now();
                        let cur_slot = garp_common::timing::slot_at_time(genesis.genesis_time, chain.slot_duration_ms, now);
                        let leader = crate::consensus::leader_for_slot(cur_slot, &genesis.initial_validators);
                        if let Some(l) = leader {
                            if l != proposer_id { continue; }
                        } else {
                            // No leader schedule; skip proposing
                            continue;
                        }
                        // Fetch a batch from mempool; skip if empty
                        let txs = mempool.get_batch(100).await;
                        if txs.is_empty() { continue; }

                        // Select best-fork parent by cumulative weight
                        let root_hash = {
                            let gh = global_head.read().await;
                            if gh.hash.is_empty() { vec![0u8; 32] } else { gh.hash.as_bytes().to_vec() }
                        };
                        let parent_hash = forks.read().await.best_fork(&root_hash).unwrap_or(root_hash);

                        let block = builder.build_block(txs, parent_hash, proposer_id.clone());
                        info!("Proposed block: slot={} epoch={} txs={} hash={}", block.header.slot, block.header.epoch, block.transactions.len(), hex::encode(&block.hash));

                        // Persist the proposed block
                        if let Err(e) = storage.store_block(&block).await {
                            warn!("Failed to persist proposed block: {}", e);
                        }

                        // Map proposal to block hash and broadcast proposal for voting
                        let proposal_id = Uuid::new_v4();
                        forks.write().await.map_proposal(proposal_id, block.hash.clone());
                        let tx_ids: Vec<garp_common::TransactionId> = block.transactions.iter().map(|t| t.id.clone()).collect();
                        let proposal_msg = NetworkMessage::ConsensusProposal { proposal_id, transactions: tx_ids, proposer: proposer_id.clone() };

                        // Broadcast the block to sync domains (as JSON payload)
                        let payload = match serde_json::to_value(&block) {
                            Ok(v) => v,
                            Err(e) => { warn!("Failed to serialize block for gossip: {}", e); continue; }
                        };
                        let envelope = MessageEnvelope {
                            id: Uuid::new_v4().to_string(),
                            message_type: "block".to_string(),
                            sender: proposer_id.0.clone(),
                            recipient: None,
                            payload,
                            timestamp: Utc::now(),
                            signature: None,
                        };
                        let recipients: Vec<ParticipantId> = sync_domains.iter().map(|sd| ParticipantId::new(&sd.domain_id.0)).collect();
                        let block_msg = NetworkMessage::Block(block);
                        if let Err(e) = network.broadcast_secure_message(&recipients, &block_msg).await {
                            warn!("Failed to broadcast proposed block: {}", e);
                        }
                        if let Err(e) = network.broadcast_secure_message(&recipients, &proposal_msg).await {
                            warn!("Failed to broadcast consensus proposal: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Proposer task shutting down");
                        break;
                    }
                }
            }
        }});

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

    /// Public node stats used by API
    pub async fn get_node_stats(&self) -> GarpResult<PublicNodeStats> {
        let ledger_stats = self.ledger.get_stats().await?;
        let net = self.network.get_network_stats().await;
        let wallet_balances_count = match self.ledger.get_wallet_balance().await? {
            Some(wb) => wb.assets.len() as u64,
            None => 0,
        };
        let stats = PublicNodeStats {
            total_transactions: ledger_stats.total_transactions,
            total_contracts: ledger_stats.total_contracts,
            active_contracts: ledger_stats.active_contracts,
            total_assets: ledger_stats.total_assets,
            wallet_balances: wallet_balances_count,
            network_peers: net.connected_peers as u64,
            ledger_stats,
        };
        Ok(stats)
    }
}

/// Public stats returned by ParticipantNode::get_node_stats and consumed by API
#[derive(Debug, Clone)]
pub struct PublicNodeStats {
    pub total_transactions: u64,
    pub total_contracts: u64,
    pub active_contracts: u64,
    pub total_assets: u64,
    pub wallet_balances: u64,
    pub network_peers: u64,
    pub ledger_stats: LedgerStats,
}

fn parse_endpoint(endpoint: &str) -> (String, u16, NetworkProtocol) {
    // Very simple parser: http(s)://host[:port]
    let mut e = endpoint.trim().to_string();
    let mut protocol = NetworkProtocol::Http;
    if let Some(rest) = e.strip_prefix("http://") {
        e = rest.to_string();
        protocol = NetworkProtocol::Http;
    } else if let Some(rest) = e.strip_prefix("https://") {
        e = rest.to_string();
        protocol = NetworkProtocol::Https;
    }
    let mut host = e.clone();
    let mut port: u16 = match protocol { NetworkProtocol::Http => 80, NetworkProtocol::Https => 443, _ => 80 };
    if let Some((h, p)) = e.split_once(':') {
        host = h.to_string();
        if let Ok(pp) = p.parse::<u16>() { port = pp; }
    }
    (host, port, protocol)
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

// Consensus messages handler
struct ConsensusMessageHandler {
    participant_id: ParticipantId,
    tower: Arc<RwLock<TowerBft>>,
    forks: Arc<RwLock<ForkGraph>>,
    ledger: Arc<LocalLedger>,
    slot_duration_ms: u64,
    genesis_time: chrono::DateTime<Utc>,
    highest_finalized_slot: Arc<RwLock<u64>>, 
}

impl ConsensusMessageHandler {
    fn new(
        participant_id: ParticipantId,
        tower: Arc<RwLock<TowerBft>>,
        forks: Arc<RwLock<ForkGraph>>,
        ledger: Arc<LocalLedger>,
        slot_duration_ms: u64,
        genesis_time: chrono::DateTime<Utc>,
        highest_finalized_slot: Arc<RwLock<u64>>,
    ) -> Self {
        Self { participant_id, tower, forks, ledger, slot_duration_ms, genesis_time, highest_finalized_slot }
    }
}

#[async_trait::async_trait]
impl MessageHandler for ConsensusMessageHandler {
    async fn handle_message(&self, envelope: MessageEnvelope) -> GarpResult<()> {
        if let Ok(msg) = serde_json::from_value::<NetworkMessage>(envelope.payload.clone()) {
            match msg {
                NetworkMessage::ConsensusVote { proposal_id, vote, voter } => {
                    let now = Utc::now();
                    let slot = garp_common::timing::slot_at_time(self.genesis_time, self.slot_duration_ms, now);
                    let mut tower = self.tower.write().await;
                    let voter_power = tower.voting_power_of(&voter);
                    let _ = tower.record_vote(slot, &voter, vote);

                    // Accumulate stake-weighted votes into fork graph if proposal maps to a block
                    if vote {
                        if let Some(block_hash) = self.forks.read().await.block_hash_for_proposal(&proposal_id).cloned() {
                            self.forks.write().await.add_votes(&block_hash, voter_power);
                        }
                    }

                    if tower.try_finalize(slot) {
                        debug!("Consensus finalized at slot {}", slot);
                        let mut hfs = self.highest_finalized_slot.write().await;
                        if slot > *hfs { *hfs = slot; }
                    }
                }
                NetworkMessage::ConsensusProposal { .. } => {
                    // Proposal handling can integrate with fork graph in future
                }
                _ => {}
            }
        }
        Ok(())
    }
}
use crate::network_layer::{NetworkLayer, StubNetworkLayer};
use garp_common::network::{NetworkAddress, NetworkProtocol, MessageEnvelope};

/// Real network layer implementation
pub struct RealNetworkLayer {
    config: crate::config::NetworkConfig,
}

impl RealNetworkLayer {
    pub fn new(config: crate::config::NetworkConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl NetworkLayer for RealNetworkLayer {
    async fn send_message(&self, address: &NetworkAddress, message: &MessageEnvelope) -> GarpResult<()> {
        // Implementation for sending messages over the network
        // This would use HTTP/gRPC to send messages to other nodes
        Ok(())
    }

    async fn broadcast_message(&self, addresses: &[NetworkAddress], message: &MessageEnvelope) -> GarpResult<()> {
        // Implementation for broadcasting messages to multiple nodes
        for address in addresses {
            let _ = self.send_message(address, message).await;
        }
        Ok(())
    }

    async fn listen(&self) -> GarpResult<()> {
        // Implementation for listening to incoming messages
        Ok(())
    }
}
