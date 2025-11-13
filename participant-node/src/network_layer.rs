use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{Duration, Instant};

use garp_common::error::GarpResult;
use garp_common::network::{ConnectionStatus, MessageEnvelope, NetworkAddress, NetworkLayer, PeerInfo};
use garp_common::types::{NetworkMessage, ParticipantId};

/// Simple per-peer token bucket for rate limiting.
#[derive(Clone)]
struct RateBucket {
    capacity: u64,
    tokens: Arc<RwLock<u64>>, // current tokens
    refill_rate_per_sec: u64,
    last_refill: Arc<RwLock<Instant>>,
}

impl RateBucket {
    fn new(capacity: u64, refill_rate_per_sec: u64) -> Self {
        Self { capacity, tokens: Arc::new(RwLock::new(capacity)), refill_rate_per_sec, last_refill: Arc::new(RwLock::new(Instant::now())) }
    }
    async fn allow(&self, cost: u64) -> bool {
        // Refill first
        {
            let mut last = self.last_refill.write().await;
            let elapsed = last.elapsed();
            if elapsed.as_secs() > 0 {
                let add = elapsed.as_secs() as u64 * self.refill_rate_per_sec;
                let mut t = self.tokens.write().await;
                *t = (*t + add).min(self.capacity);
                *last = Instant::now();
            }
        }
        let mut t = self.tokens.write().await;
        if *t >= cost {
            *t -= cost;
            true
        } else {
            false
        }
    }
}

/// QUIC-based network layer with stake-weighted QoS, Turbine-like fanout, and gating.
pub struct QuicNetworkLayer {
    /// Connected peer handles
    connections: Arc<RwLock<HashMap<ParticipantId, ()>>>, // placeholder for quinn::Connection
    /// Known peer addresses
    addresses: Arc<RwLock<HashMap<ParticipantId, NetworkAddress>>>,
    /// Current connection status
    status: Arc<RwLock<HashMap<ParticipantId, ConnectionStatus>>>,
    /// Stake weights for QoS scheduling
    stake_weights: Arc<RwLock<HashMap<ParticipantId, u64>>>,
    /// Behavior scores for DoS gating (lower is worse)
    behavior_scores: Arc<RwLock<HashMap<ParticipantId, i32>>>,
    /// Per-peer rate limiter
    rate_buckets: Arc<RwLock<HashMap<ParticipantId, RateBucket>>>,
    /// Inbound envelope channel
    inbound_tx: mpsc::Sender<MessageEnvelope>,
    inbound_rx: Arc<RwLock<Option<mpsc::Receiver<MessageEnvelope>>>>,
}

impl QuicNetworkLayer {
    pub fn new(buffer: usize) -> Arc<Self> {
        let (tx, rx) = mpsc::channel(buffer);
        Arc::new(Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            addresses: Arc::new(RwLock::new(HashMap::new())),
            status: Arc::new(RwLock::new(HashMap::new())),
            stake_weights: Arc::new(RwLock::new(HashMap::new())),
            behavior_scores: Arc::new(RwLock::new(HashMap::new())),
            rate_buckets: Arc::new(RwLock::new(HashMap::new())),
            inbound_tx: tx,
            inbound_rx: Arc::new(RwLock::new(Some(rx))),
        })
    }

    /// Update stake weight for a peer.
    pub async fn set_stake_weight(&self, pid: ParticipantId, weight: u64) {
        self.stake_weights.write().await.insert(pid, weight);
    }

    /// Update behavior score for a peer.
    pub async fn set_behavior_score(&self, pid: ParticipantId, score: i32) {
        self.behavior_scores.write().await.insert(pid, score);
    }

    /// Simple gate: drop if score too low.
    async fn allowed(&self, pid: &ParticipantId) -> bool {
        let score = self.behavior_scores.read().await.get(pid).cloned().unwrap_or(100);
        score >= 20
    }

    /// Get a rate bucket for peer; create if missing.
    async fn bucket_for(&self, pid: &ParticipantId) -> RateBucket {
        let mut buckets = self.rate_buckets.write().await;
        if let Some(b) = buckets.get(pid) { return b.clone(); }
        let b = RateBucket::new(10_000, 2_000); // tokens represent bytes cost per second
        buckets.insert(pid.clone(), b.clone());
        b
    }

    /// Turbine-like fanout that shards payload and distributes across recipients.
    async fn turbine_fanout(&self, recipients: &[ParticipantId], payload: Bytes) -> Vec<(ParticipantId, Bytes)> {
        // Simple chunking: small packets of ~1KB
        let chunk_size = 1024usize;
        let mut out = Vec::new();
        if recipients.is_empty() || payload.is_empty() { return out; }
        let mut offset = 0usize;
        let mut idx = 0usize;
        while offset < payload.len() {
            let end = (offset + chunk_size).min(payload.len());
            let chunk = payload.slice(offset..end);
            let peer = &recipients[idx % recipients.len()];
            out.push((peer.clone(), chunk));
            offset = end;
            idx += 1;
        }
        // Simple parity shard (XOR of all chunks) to first peer for resilience
        // Note: lightweight placeholder; replace with Reed-Solomon later
        if !out.is_empty() {
            let mut parity = vec![0u8; chunk_size];
            for (_, c) in &out {
                for (i, b) in c.iter().enumerate() {
                    if i < parity.len() { parity[i] ^= *b; }
                }
            }
            out.push((recipients[0].clone(), Bytes::from(parity)));
        }
        out
    }
}

#[async_trait]
impl NetworkLayer for QuicNetworkLayer {
    async fn send_message(&self, recipient: &ParticipantId, message: &NetworkMessage) -> GarpResult<()> {
        // Gate based on behavior
        if !self.allowed(recipient).await { return Err(garp_common::error::GarpError::TransportError("peer gated".into())); }
        // QoS cost by size; stake grants priority (lower effective cost)
        let bytes = bincode::serialize(message).map_err(|e| garp_common::error::GarpError::SerializationError(e.to_string()))?;
        let size = bytes.len() as u64;
        let stake = self.stake_weights.read().await.get(recipient).cloned().unwrap_or(1).max(1);
        let effective_cost = size.saturating_div(stake.max(1));
        let bucket = self.bucket_for(recipient).await;
        if !bucket.allow(effective_cost).await {
            return Err(garp_common::error::GarpError::ResourceLimitExceeded("rate limited".into()));
        }
        // TODO: Use quinn connection; placeholder drops
        // let conn = self.connections.read().await.get(recipient).cloned().ok_or_else(|| garp_common::error::GarpError::TransportError("no connection".into()))?;
        // conn.send_datagram(Bytes::from(bytes)).map_err(|e| garp_common::error::GarpError::TransportError(e.to_string()))?;
        Ok(())
    }

    async fn broadcast_message(&self, recipients: &[ParticipantId], message: &NetworkMessage) -> GarpResult<()> {
        if recipients.is_empty() { return Ok(()); }
        let bytes = bincode::serialize(message).map_err(|e| garp_common::error::GarpError::SerializationError(e.to_string()))?;
        let shards = self.turbine_fanout(recipients, Bytes::from(bytes)).await;
        for (peer, shard) in shards {
            if !self.allowed(&peer).await { continue; }
            let stake = self.stake_weights.read().await.get(&peer).cloned().unwrap_or(1).max(1);
            let effective_cost = (shard.len() as u64).saturating_div(stake);
            let bucket = self.bucket_for(&peer).await;
            if !bucket.allow(effective_cost).await { continue; }
            // TODO: send over quinn; placeholder drops
            let _ = shard;
        }
        Ok(())
    }

    async fn start_listening(&self) -> GarpResult<mpsc::Receiver<MessageEnvelope>> {
        // In a real implementation, accept quinn inbound and decode envelopes.
        // Here we provide the receiver side tied to a background task placeholder.
        let mut rx_opt = self.inbound_rx.write().await;
        let rx = rx_opt.take().ok_or_else(|| garp_common::error::GarpError::TransportError("listener already started".into()))?;
        Ok(rx)
    }

    async fn connect_peer(&self, peer: &PeerInfo) -> GarpResult<()> {
        self.addresses.write().await.insert(peer.participant_id.clone(), peer.address.clone());
        self.status.write().await.insert(peer.participant_id.clone(), ConnectionStatus::Connecting);
        // TODO: establish quinn connection based on NetworkAddress
        self.status.write().await.insert(peer.participant_id.clone(), ConnectionStatus::Connected);
        Ok(())
    }

    async fn disconnect_peer(&self, participant_id: &ParticipantId) -> GarpResult<()> {
        self.status.write().await.insert(participant_id.clone(), ConnectionStatus::Disconnected);
        // TODO: close quinn connection if present
        Ok(())
    }

    async fn get_peer_status(&self, participant_id: &ParticipantId) -> Option<ConnectionStatus> {
        self.status.read().await.get(participant_id).cloned()
    }

    async fn get_connected_peers(&self) -> Vec<ParticipantId> {
        self.status
            .read()
            .await
            .iter()
            .filter_map(|(pid, st)| if *st == ConnectionStatus::Connected { Some(pid.clone()) } else { None })
            .collect()
    }
}