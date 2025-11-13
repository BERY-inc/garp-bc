use std::collections::{HashMap, HashSet, VecDeque};
use chrono::{DateTime, Utc};
use garp_common::{ParticipantId, Block, BlockHeader};
use uuid::Uuid;

/// Leader rotation using round-robin over the validator set
pub fn leader_for_slot(slot: u64, validators: &[ParticipantId]) -> Option<ParticipantId> {
    if validators.is_empty() { return None; }
    let idx = (slot as usize) % validators.len();
    validators.get(idx).cloned()
}

/// Tower BFT-like vote lockouts and weighted voting
pub struct TowerBft {
    /// Lockout durations per voter: (slot, lockout_until_slot)
    lockouts: HashMap<ParticipantId, Vec<(u64, u64)>>,
    /// Voting power per validator
    voting_power: HashMap<ParticipantId, u64>,
    /// Votes by slot: voter -> approve
    votes_by_slot: HashMap<u64, HashMap<ParticipantId, bool>>, 
    /// Finalized slots
    finalized_slots: HashSet<u64>,
    /// Supermajority threshold (e.g., 0.667)
    threshold: f64,
}

impl TowerBft {
    pub fn new(voting_power: HashMap<ParticipantId, u64>, threshold: f64) -> Self {
        Self { lockouts: HashMap::new(), voting_power, votes_by_slot: HashMap::new(), finalized_slots: HashSet::new(), threshold }
    }

    /// Record a vote if voter is not locked out
    pub fn record_vote(&mut self, slot: u64, voter: &ParticipantId, approve: bool) -> bool {
        if self.is_locked_out(voter, slot) { return false; }
        let entry = self.votes_by_slot.entry(slot).or_default();
        entry.insert(voter.clone(), approve);
        // Extend lockout if approved (simplified exponential lockout)
        if approve { self.extend_lockout(voter, slot); }
        true
    }

    fn is_locked_out(&self, voter: &ParticipantId, current_slot: u64) -> bool {
        if let Some(lockouts) = self.lockouts.get(voter) {
            return lockouts.iter().any(|(_, until)| current_slot < *until);
        }
        false
    }

    fn extend_lockout(&mut self, voter: &ParticipantId, slot: u64) {
        let l = self.lockouts.entry(voter.clone()).or_default();
        let base = 2u64.pow(l.len() as u32).max(1);
        let until = slot + base; // simplified exponential backoff
        l.push((slot, until));
    }

    /// Compute weighted approvals at a slot
    pub fn approval_weight(&self, slot: u64) -> u64 {
        let mut acc = 0u64;
        if let Some(votes) = self.votes_by_slot.get(&slot) {
            for (voter, approve) in votes.iter() {
                if *approve { acc += *self.voting_power.get(voter).unwrap_or(&0) };
            }
        }
        acc
    }

    /// Determine finality at a slot based on threshold
    pub fn try_finalize(&mut self, slot: u64) -> bool {
        if self.finalized_slots.contains(&slot) { return true; }
        let total_power: u64 = self.voting_power.values().sum();
        if total_power == 0 { return false; }
        let approvals = self.approval_weight(slot);
        let ratio = (approvals as f64) / (total_power as f64);
        if ratio >= self.threshold {
            self.finalized_slots.insert(slot);
            return true;
        }
        false
    }

    /// Expose voting power for QoS/stake-weighted operations
    pub fn voting_power_of(&self, id: &ParticipantId) -> u64 {
        *self.voting_power.get(id).unwrap_or(&0)
    }
}

/// Fork graph with parent-child and descendant/ancestor indexing
pub struct ForkGraph {
    /// block_hash -> header
    headers: HashMap<Vec<u8>, BlockHeader>,
    /// parent_hash -> children hashes
    children: HashMap<Vec<u8>, Vec<Vec<u8>>>,
    /// block_hash -> cumulative vote weight
    cumulative_weight: HashMap<Vec<u8>, u64>,
    /// replay protection: seen tx ids
    seen_transactions: HashSet<uuid::Uuid>,
    /// consensus proposal id -> block hash mapping (local proposals)
    proposal_to_block: HashMap<Uuid, Vec<u8>>, 
}

impl ForkGraph {
    pub fn new() -> Self {
        Self { headers: HashMap::new(), children: HashMap::new(), cumulative_weight: HashMap::new(), seen_transactions: HashSet::new(), proposal_to_block: HashMap::new() }
    }

    pub fn insert_block(&mut self, block: &Block) {
        self.headers.insert(block.hash.clone(), block.header.clone());
        self.children.entry(block.header.parent_hash.clone()).or_default().push(block.hash.clone());
        self.cumulative_weight.entry(block.hash.clone()).or_insert(0);
        for tx in &block.transactions {
            self.seen_transactions.insert(tx.id.0);
        }
    }

    pub fn add_votes(&mut self, block_hash: &[u8], weight: u64) {
        let entry = self.cumulative_weight.entry(block_hash.to_vec()).or_insert(0);
        *entry += weight;
    }

    /// Map a proposal id to a produced block hash
    pub fn map_proposal(&mut self, proposal_id: Uuid, block_hash: Vec<u8>) {
        self.proposal_to_block.insert(proposal_id, block_hash);
    }

    /// Lookup block hash for a proposal id
    pub fn block_hash_for_proposal(&self, proposal_id: &Uuid) -> Option<&Vec<u8>> {
        self.proposal_to_block.get(proposal_id)
    }

    /// Select best fork by cumulative weight using BFS
    pub fn best_fork(&self, root_hash: &[u8]) -> Option<Vec<u8>> {
        let mut queue = VecDeque::new();
        queue.push_back(root_hash.to_vec());
        let mut best = (root_hash.to_vec(), *self.cumulative_weight.get(root_hash).unwrap_or(&0));
        while let Some(cur) = queue.pop_front() {
            let w = *self.cumulative_weight.get(&cur).unwrap_or(&0);
            if w > best.1 { best = (cur.clone(), w); }
            if let Some(ch) = self.children.get(&cur) {
                for c in ch { queue.push_back(c.clone()); }
            }
        }
        Some(best.0)
    }

    /// Check replay (tx already seen)
    pub fn is_replay(&self, tx_id: &uuid::Uuid) -> bool { self.seen_transactions.contains(tx_id) }

    /// Check if we have seen a block by hash
    pub fn has_block(&self, hash: &[u8]) -> bool { self.headers.contains_key(hash) }
}

/// Explicit rollback procedure to a target slot
pub fn rollback_to_slot(blocks: &mut Vec<Block>, target_slot: u64) {
    while let Some(last) = blocks.last() {
        if last.header.slot > target_slot { blocks.pop(); } else { break; }
    }
}