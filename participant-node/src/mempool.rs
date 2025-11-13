use garp_common::GarpResult;
use garp_common::types::{NetworkMessage, ParticipantId, Transaction};
use garp_common::network::NetworkLayer;
use serde::{Serialize, Deserialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use tokio::sync::RwLock;
use std::sync::Arc;
use tokio::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolConfig {
    pub max_transactions: usize,
    pub max_bytes: usize,
    pub min_fee: u64,
    pub enable_forwarding: bool,
    pub forward_ratio_bps: u16,   // basis points of pool to forward (0-10_000)
    pub forward_batch_max: usize, // max tx per forwarding batch
    pub prefetch_hint_depth: usize, // depth to prefetch for upcoming leaders
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self { max_transactions: 100_000, max_bytes: 50 * 1024 * 1024, min_fee: 0, enable_forwarding: true, forward_ratio_bps: 1000, forward_batch_max: 1024, prefetch_hint_depth: 256 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolEntry {
    pub tx: Transaction,
    pub fee: u64,
    pub size_bytes: usize,
    pub received_at: chrono::DateTime<chrono::Utc>,
}

impl Eq for MempoolEntry {}
impl PartialEq for MempoolEntry {
    fn eq(&self, other: &Self) -> bool { self.fee == other.fee && self.received_at == other.received_at }
}

impl Ord for MempoolEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher fee first, then earlier received
        match self.fee.cmp(&other.fee) {
            Ordering::Equal => other.received_at.cmp(&self.received_at), // earlier first
            ord => ord,
        }
    }
}
impl PartialOrd for MempoolEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

pub struct Mempool {
    config: MempoolConfig,
    total_bytes: RwLock<usize>,
    heap: RwLock<BinaryHeap<MempoolEntry>>, // prioritized by fee
    // Gulf Stream-like forwarding state
    upcoming_leaders: RwLock<Vec<ParticipantId>>, // externally provided
    // Simple per-sender rate limiting and behavior scoring
    sender_buckets: RwLock<std::collections::HashMap<ParticipantId, RateBucket>>,
    sender_behavior: RwLock<std::collections::HashMap<ParticipantId, i32>>, // 0-100, drop if < 20
}

impl Mempool {
    pub fn new(config: MempoolConfig) -> Arc<Self> {
        Arc::new(Self {
            config,
            total_bytes: RwLock::new(0),
            heap: RwLock::new(BinaryHeap::new()),
            upcoming_leaders: RwLock::new(Vec::new()),
            sender_buckets: RwLock::new(std::collections::HashMap::new()),
            sender_behavior: RwLock::new(std::collections::HashMap::new()),
        })
    }

    pub async fn submit(&self, tx: Transaction, fee: u64) -> GarpResult<()> {
        // Basic admission checks
        if fee < self.config.min_fee {
            return Err(garp_common::error::GarpError::ValidationFailed("fee below minimum".into()));
        }
        let size = bincode::serialize(&tx).map_err(|e| garp_common::error::GarpError::SerializationError(e.to_string()))?.len();
        // Sender behavior gating and per-sender rate limiting
        {
            let sender = tx.submitter.clone();
            let behavior = self.sender_behavior.read().await.get(&sender).cloned().unwrap_or(100);
            if behavior < 20 { return Err(garp_common::error::GarpError::ResourceLimitExceeded("sender gated".into())); }
            let mut buckets = self.sender_buckets.write().await;
            let bucket = buckets.entry(sender.clone()).or_insert_with(|| RateBucket::new(50_000, 10_000)); // tokens ~ bytes/sec
            if !bucket.allow(size as u64).await {
                return Err(garp_common::error::GarpError::ResourceLimitExceeded("rate limited".into()));
            }
        }
        {
            let total = *self.total_bytes.read().await;
            if total + size > self.config.max_bytes { return Err(garp_common::error::GarpError::ResourceLimitExceeded("mempool size".into())); }
        }

        // TODO: stateless validation hooks (sig checks, format) and optional stateful precheck
        // For scaffolding, accept all transactions that meet fee and size constraints
        let entry = MempoolEntry { tx, fee, size_bytes: size, received_at: chrono::Utc::now() };
        {
            let mut heap = self.heap.write().await;
            if heap.len() >= self.config.max_transactions {
                // Replace-by-fee: drop lowest fee if new one is higher
                if let Some(mut lowest) = heap.peek().cloned() {
                    // BinaryHeap is max-heap; to drop lowest, we collect then rebuild (simple approach for stub)
                    let mut entries: Vec<_> = heap.drain().collect();
                    entries.sort_by(|a,b| a.fee.cmp(&b.fee));
                    lowest = entries.first().cloned().unwrap();
                    if entry.fee > lowest.fee {
                        entries.remove(0);
                        entries.push(entry);
                        *heap = entries.into_iter().collect();
                    } else {
                        // reject
                        return Err(garp_common::error::GarpError::ResourceLimitExceeded("mempool full".into()));
                    }
                }
            } else {
                heap.push(entry);
            }
        }
        {
            let mut total = self.total_bytes.write().await;
            *total += size;
        }
        Ok(())
    }

    pub async fn get_batch(&self, max: usize) -> Vec<Transaction> {
        let mut out = Vec::with_capacity(max);
        let mut removed_bytes = 0usize;
        {
            let mut heap = self.heap.write().await;
            for _ in 0..max {
                if let Some(entry) = heap.pop() {
                    removed_bytes += entry.size_bytes;
                    out.push(entry.tx);
                } else { break; }
            }
        }
        {
            let mut total = self.total_bytes.write().await;
            *total = total.saturating_sub(removed_bytes);
        }
        out
    }

    pub async fn size(&self) -> usize { self.heap.read().await.len() }
    pub async fn bytes(&self) -> usize { *self.total_bytes.read().await }

    // --- Gulf Stream-style forwarding and prefetch ---

    /// Update upcoming leaders list (ordered by slot).
    pub async fn set_upcoming_leaders(&self, leaders: Vec<ParticipantId>) {
        *self.upcoming_leaders.write().await = leaders;
    }

    /// Forward top transactions to upcoming leaders to reduce leader workload.
    pub async fn forward_to_upcoming_leaders(&self, network: Arc<dyn NetworkLayer + Send + Sync>) -> GarpResult<()> {
        if !self.config.enable_forwarding { return Ok(()); }
        let pool_size = self.size().await;
        if pool_size == 0 { return Ok(()); }
        let target_count = ((pool_size as u64 * self.config.forward_ratio_bps as u64) / 10_000).min(self.config.forward_batch_max as u64) as usize;
        let batch = self.get_batch(target_count).await;
        let leaders = self.upcoming_leaders.read().await.clone();
        if leaders.is_empty() { return Ok(()); }
        for tx in batch {
            for leader in &leaders {
                let msg = NetworkMessage::TransactionSubmission(tx.clone());
                let _ = network.send_message(leader, &msg).await; // ignore per-tx errors to continue forwarding
            }
        }
        Ok(())
    }

    /// Prefetch hinting: simulate fetching state or signatures required for upcoming leaders.
    /// Placeholder stores the count of prepared transactions; a real implementation would hydrate caches.
    pub async fn prefetch_hints(&self) -> usize {
        let prepare = self.config.prefetch_hint_depth.min(self.size().await);
        prepare
    }
}
// -----------------------------------------------------------------------------
// Mempool policy: nonce/replay protection, fee model, and prioritization
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MempoolPolicy {
    pub max_pool_size: usize,
    pub min_fee_rate: u64,      // nanounits per byte
    pub enforce_nonce: bool,
    pub allow_replace_by_fee: bool,
}

#[derive(Debug, Clone)]
pub struct TxMeta {
    pub sender: String,
    pub nonce: u64,
    pub fee_rate: u64,
    pub size_bytes: usize,
}

/// Returns whether a tx passes local mempool policy checks.
pub fn passes_policy(tx: &TxMeta, policy: &MempoolPolicy, next_expected_nonce: Option<u64>) -> bool {
    if tx.fee_rate < policy.min_fee_rate { return false; }
    if policy.enforce_nonce {
        if let Some(expected) = next_expected_nonce {
            if tx.nonce != expected { return false; }
        }
    }
    true
}

// -----------------------------------------------------------------------------
// Simple rate bucket for mempool sender-level rate limiting
// -----------------------------------------------------------------------------
#[derive(Clone)]
struct RateBucket {
    capacity: u64,
    tokens: Arc<RwLock<u64>>,
    refill_rate_per_sec: u64,
    last_refill: Arc<RwLock<Instant>>,
}

impl RateBucket {
    fn new(capacity: u64, refill_rate_per_sec: u64) -> Self {
        Self { capacity, tokens: Arc::new(RwLock::new(capacity)), refill_rate_per_sec, last_refill: Arc::new(RwLock::new(Instant::now())) }
    }
    async fn allow(&self, cost: u64) -> bool {
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
        if *t >= cost { *t -= cost; true } else { false }
    }
}