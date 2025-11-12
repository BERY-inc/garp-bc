use crate::types::{NetworkMessage, ParticipantId, EncryptedData};
use crate::types::{Transaction, Block};
use crate::error::{NetworkError, GarpResult};
use crate::crypto::CryptoService;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use async_trait::async_trait;
use tracing::{info, warn, error};

/// Network address for participants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkAddress {
    pub host: String,
    pub port: u16,
    pub protocol: NetworkProtocol,
}

/// Supported network protocols
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkProtocol {
    Http,
    Https,
    Grpc,
    WebSocket,
}

/// Network peer information
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub participant_id: ParticipantId,
    pub address: NetworkAddress,
    pub public_key: Vec<u8>,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub connection_status: ConnectionStatus,
}

/// Connection status with peers
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Connecting,
    Failed,
}

/// Network message envelope with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    pub message_id: uuid::Uuid,
    pub sender: ParticipantId,
    pub recipient: ParticipantId,
    pub message_type: String,
    pub payload: EncryptedData,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub signature: crate::types::Signature,
}

/// Network layer trait for different transport implementations
#[async_trait]
pub trait NetworkLayer: Send + Sync {
    /// Send a message to a specific peer
    async fn send_message(
        &self,
        recipient: &ParticipantId,
        message: &NetworkMessage,
    ) -> GarpResult<()>;

    /// Broadcast a message to multiple peers
    async fn broadcast_message(
        &self,
        recipients: &[ParticipantId],
        message: &NetworkMessage,
    ) -> GarpResult<()>;

    /// Start listening for incoming messages
    async fn start_listening(&self) -> GarpResult<mpsc::Receiver<MessageEnvelope>>;

    /// Connect to a peer
    async fn connect_peer(&self, peer: &PeerInfo) -> GarpResult<()>;

    /// Disconnect from a peer
    async fn disconnect_peer(&self, participant_id: &ParticipantId) -> GarpResult<()>;

    /// Get connection status with a peer
    async fn get_peer_status(&self, participant_id: &ParticipantId) -> Option<ConnectionStatus>;

    /// Get list of connected peers
    async fn get_connected_peers(&self) -> Vec<ParticipantId>;
}

/// Network manager for handling peer connections and message routing
pub struct NetworkManager {
    participant_id: ParticipantId,
    crypto_service: Arc<CryptoService>,
    peers: Arc<RwLock<HashMap<ParticipantId, PeerInfo>>>,
    network_layer: Arc<dyn NetworkLayer>,
    message_handlers: Arc<RwLock<HashMap<String, Box<dyn MessageHandler>>>>,
}

/// Gossip topics for P2P pub/sub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GossipTopic {
    Transactions,
    Blocks,
    Status,
}

/// Gossip service trait for pub/sub dissemination of transactions and blocks
#[async_trait]
pub trait GossipService: Send + Sync {
    async fn publish_transaction(&self, tx: Transaction) -> GarpResult<()>;
    async fn publish_block(&self, block: Block) -> GarpResult<()>;
    async fn subscribe_transactions(&self) -> GarpResult<mpsc::Receiver<Transaction>>;
    async fn subscribe_blocks(&self) -> GarpResult<mpsc::Receiver<Block>>;
}

/// Simple in-memory gossip router (stub for libp2p or other transports)
pub struct GossipRouter {
    tx_sender: mpsc::Sender<Transaction>,
    tx_receiver: Option<mpsc::Receiver<Transaction>>,
    block_sender: mpsc::Sender<Block>,
    block_receiver: Option<mpsc::Receiver<Block>>,
}

impl GossipRouter {
    pub fn new(buffer: usize) -> Self {
        let (tx_s, tx_r) = mpsc::channel(buffer);
        let (bk_s, bk_r) = mpsc::channel(buffer);
        Self { tx_sender: tx_s, tx_receiver: Some(tx_r), block_sender: bk_s, block_receiver: Some(bk_r) }
    }
}

#[async_trait]
impl GossipService for GossipRouter {
    async fn publish_transaction(&self, tx: Transaction) -> GarpResult<()> {
        self.tx_sender.send(tx).await.map_err(|e| NetworkError::TransportError(e.to_string()))?;
        Ok(())
    }

    async fn publish_block(&self, block: Block) -> GarpResult<()> {
        self.block_sender.send(block).await.map_err(|e| NetworkError::TransportError(e.to_string()))?;
        Ok(())
    }

    async fn subscribe_transactions(&self) -> GarpResult<mpsc::Receiver<Transaction>> {
        self.tx_receiver
            .as_ref()
            .ok_or_else(|| NetworkError::Internal("transactions receiver already taken".into()))?;
        // Clone by replacing with a new channel; for stub we transfer ownership once
        let mut me = self as *const _ as *mut GossipRouter; // unsafe: only for stub to move out Option
        let recv = unsafe { &mut *me }.tx_receiver.take().unwrap();
        Ok(recv)
    }

    async fn subscribe_blocks(&self) -> GarpResult<mpsc::Receiver<Block>> {
        self.block_receiver
            .as_ref()
            .ok_or_else(|| NetworkError::Internal("blocks receiver already taken".into()))?;
        let mut me = self as *const _ as *mut GossipRouter;
        let recv = unsafe { &mut *me }.block_receiver.take().unwrap();
        Ok(recv)
    }
}

/// Peer manager trait for connection lifecycle and policies
#[async_trait]
pub trait PeerManager: Send + Sync {
    async fn connect(&self, peer: PeerInfo) -> GarpResult<()>;
    async fn disconnect(&self, participant_id: &ParticipantId) -> GarpResult<()>;
    async fn list_peers(&self) -> Vec<PeerInfo>;
    async fn ban_peer(&self, participant_id: &ParticipantId) -> GarpResult<()>;
    async fn unban_peer(&self, participant_id: &ParticipantId) -> GarpResult<()>;
}

/// Simple in-memory peer manager (placeholder)
pub struct SimplePeerManager {
    peers: Arc<RwLock<HashMap<ParticipantId, PeerInfo>>>,
    banned: Arc<RwLock<HashMap<ParticipantId, chrono::DateTime<chrono::Utc>>>>,
}

impl SimplePeerManager {
    pub fn new() -> Self {
        Self { peers: Arc::new(RwLock::new(HashMap::new())), banned: Arc::new(RwLock::new(HashMap::new())) }
    }
}

#[async_trait]
impl PeerManager for SimplePeerManager {
    async fn connect(&self, peer: PeerInfo) -> GarpResult<()> {
        let banned = self.banned.read().await;
        if banned.contains_key(&peer.participant_id) {
            return Err(NetworkError::PeerBanned(peer.participant_id.0.clone()).into());
        }
        drop(banned);
        self.peers.write().await.insert(peer.participant_id.clone(), peer);
        Ok(())
    }

    async fn disconnect(&self, participant_id: &ParticipantId) -> GarpResult<()> {
        self.peers.write().await.remove(participant_id);
        Ok(())
    }

    async fn list_peers(&self) -> Vec<PeerInfo> {
        self.peers.read().await.values().cloned().collect()
    }

    async fn ban_peer(&self, participant_id: &ParticipantId) -> GarpResult<()> {
        self.banned.write().await.insert(participant_id.clone(), chrono::Utc::now());
        self.peers.write().await.remove(participant_id);
        Ok(())
    }

    async fn unban_peer(&self, participant_id: &ParticipantId) -> GarpResult<()> {
        self.banned.write().await.remove(participant_id);
        Ok(())
    }
}

/// Message handler trait for processing different message types
#[async_trait]
pub trait MessageHandler: Send + Sync {
    async fn handle_message(
        &self,
        sender: &ParticipantId,
        message: &NetworkMessage,
    ) -> GarpResult<Option<NetworkMessage>>;
}

impl NetworkManager {
    pub fn new(
        participant_id: ParticipantId,
        crypto_service: Arc<CryptoService>,
        network_layer: Arc<dyn NetworkLayer>,
    ) -> Self {
        Self {
            participant_id,
            crypto_service,
            peers: Arc::new(RwLock::new(HashMap::new())),
            network_layer,
            message_handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a message handler for a specific message type
    pub async fn register_handler<H>(&self, message_type: String, handler: H)
    where
        H: MessageHandler + 'static,
    {
        let mut handlers = self.message_handlers.write().await;
        handlers.insert(message_type, Box::new(handler));
    }

    /// Add a peer to the network
    pub async fn add_peer(&self, peer: PeerInfo) -> GarpResult<()> {
        info!("Adding peer: {:?}", peer.participant_id);
        
        // Connect to the peer
        self.network_layer.connect_peer(&peer).await?;
        
        // Store peer information
        let mut peers = self.peers.write().await;
        peers.insert(peer.participant_id.clone(), peer);
        
        Ok(())
    }

    /// Remove a peer from the network
    pub async fn remove_peer(&self, participant_id: &ParticipantId) -> GarpResult<()> {
        info!("Removing peer: {:?}", participant_id);
        
        // Disconnect from the peer
        self.network_layer.disconnect_peer(participant_id).await?;
        
        // Remove peer information
        let mut peers = self.peers.write().await;
        peers.remove(participant_id);
        
        Ok(())
    }

    /// Send a secure message to a peer
    pub async fn send_secure_message(
        &self,
        recipient: &ParticipantId,
        message: &NetworkMessage,
    ) -> GarpResult<()> {
        let peers = self.peers.read().await;
        let peer = peers.get(recipient)
            .ok_or_else(|| NetworkError::PeerNotFound(recipient.0.clone()))?;

        // Encrypt the message for the recipient
        let encrypted_message = self.encrypt_message_for_peer(message, peer).await?;
        
        // Send through network layer
        self.network_layer.send_message(recipient, &encrypted_message).await?;
        
        info!("Sent secure message to peer: {:?}", recipient);
        Ok(())
    }

    /// Broadcast a message to multiple peers
    pub async fn broadcast_secure_message(
        &self,
        recipients: &[ParticipantId],
        message: &NetworkMessage,
    ) -> GarpResult<()> {
        let peers = self.peers.read().await;
        let mut encrypted_messages = Vec::new();
        
        for recipient in recipients {
            if let Some(peer) = peers.get(recipient) {
                let encrypted = self.encrypt_message_for_peer(message, peer).await?;
                encrypted_messages.push((recipient.clone(), encrypted));
            } else {
                warn!("Peer not found for broadcast: {:?}", recipient);
            }
        }
        
        drop(peers);
        
        // Send to all recipients
        for (recipient, encrypted_message) in encrypted_messages {
            self.network_layer.send_message(&recipient, &encrypted_message).await?;
        }
        
        info!("Broadcast secure message to {} peers", recipients.len());
        Ok(())
    }

    /// Start the network manager and begin processing messages
    pub async fn start(&self) -> GarpResult<()> {
        info!("Starting network manager for participant: {:?}", self.participant_id);
        
        let mut message_receiver = self.network_layer.start_listening().await?;
        
        // Spawn message processing task
        let crypto_service = Arc::clone(&self.crypto_service);
        let message_handlers = Arc::clone(&self.message_handlers);
        let participant_id = self.participant_id.clone();
        
        tokio::spawn(async move {
            while let Some(envelope) = message_receiver.recv().await {
                if let Err(e) = Self::process_message_envelope(
                    &envelope,
                    &crypto_service,
                    &message_handlers,
                    &participant_id,
                ).await {
                    error!("Failed to process message: {:?}", e);
                }
            }
        });
        
        Ok(())
    }

    /// Process an incoming message envelope
    async fn process_message_envelope(
        envelope: &MessageEnvelope,
        crypto_service: &CryptoService,
        message_handlers: &Arc<RwLock<HashMap<String, Box<dyn MessageHandler>>>>,
        participant_id: &ParticipantId,
    ) -> GarpResult<()> {
        // Verify message signature
        let envelope_bytes = bincode::serialize(envelope)
            .map_err(|e| NetworkError::InvalidMessageFormat(e.to_string()))?;
        
        if !crypto_service.verify(&envelope_bytes, &envelope.signature)? {
            return Err(NetworkError::AuthenticationFailed(envelope.sender.0.clone()).into());
        }

        // Decrypt message payload (implementation would need shared secret)
        // For now, we'll assume the message is already decrypted
        let message: NetworkMessage = serde_json::from_slice(&envelope.payload.ciphertext)
            .map_err(|e| NetworkError::InvalidMessageFormat(e.to_string()))?;

        // Find and execute message handler
        let handlers = message_handlers.read().await;
        if let Some(handler) = handlers.get(&envelope.message_type) {
            if let Some(response) = handler.handle_message(&envelope.sender, &message).await? {
                // Send response back to sender (implementation needed)
                info!("Generated response message for: {:?}", envelope.sender);
            }
        } else {
            warn!("No handler found for message type: {}", envelope.message_type);
        }

        Ok(())
    }

    /// Encrypt a message for a specific peer
    async fn encrypt_message_for_peer(
        &self,
        message: &NetworkMessage,
        peer: &PeerInfo,
    ) -> GarpResult<NetworkMessage> {
        // This is a simplified implementation
        // In practice, you'd use the peer's public key for encryption
        let serialized = serde_json::to_vec(message)
            .map_err(|e| NetworkError::InvalidMessageFormat(e.to_string()))?;
        
        // For now, return the original message
        // Real implementation would encrypt using peer's public key
        Ok(message.clone())
    }

    /// Get network statistics
    pub async fn get_network_stats(&self) -> NetworkStats {
        let peers = self.peers.read().await;
        let connected_count = peers.values()
            .filter(|p| p.connection_status == ConnectionStatus::Connected)
            .count();
        
        NetworkStats {
            total_peers: peers.len(),
            connected_peers: connected_count,
            disconnected_peers: peers.len() - connected_count,
        }
    }
}

/// Network statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    pub total_peers: usize,
    pub connected_peers: usize,
    pub disconnected_peers: usize,
}

impl NetworkAddress {
    pub fn new(host: &str, port: u16, protocol: NetworkProtocol) -> Self {
        Self {
            host: host.to_string(),
            port,
            protocol,
        }
    }

    pub fn to_url(&self) -> String {
        match self.protocol {
            NetworkProtocol::Http => format!("http://{}:{}", self.host, self.port),
            NetworkProtocol::Https => format!("https://{}:{}", self.host, self.port),
            NetworkProtocol::Grpc => format!("grpc://{}:{}", self.host, self.port),
            NetworkProtocol::WebSocket => format!("ws://{}:{}", self.host, self.port),
        }
    }
}

impl PeerInfo {
    pub fn new(
        participant_id: ParticipantId,
        address: NetworkAddress,
        public_key: Vec<u8>,
    ) -> Self {
        Self {
            participant_id,
            address,
            public_key,
            last_seen: chrono::Utc::now(),
            connection_status: ConnectionStatus::Disconnected,
        }
    }

    pub fn update_last_seen(&mut self) {
        self.last_seen = chrono::Utc::now();
    }

    pub fn set_connection_status(&mut self, status: ConnectionStatus) {
        self.connection_status = status;
    }
}