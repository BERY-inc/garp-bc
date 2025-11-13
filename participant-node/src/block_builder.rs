use chrono::Utc;
use garp_common::{
    Block, BlockHeader, Transaction, ParticipantId, ChainParams, GenesisConfig,
    timing::{slot_at_time, epoch_for_slot},
};

/// Simple block builder that assembles blocks from a batch of transactions
/// using chain timing parameters. This is an in-memory, non-consensus builder
/// intended for local testing and scaffolding.
pub struct BlockBuilder {
    chain: ChainParams,
    genesis: GenesisConfig,
}

impl BlockBuilder {
    pub fn new(chain: ChainParams, genesis: GenesisConfig) -> Self {
        Self { chain, genesis }
    }

    /// Build a block from transactions and parent state.
    /// Note: This computes simple roots by hashing concatenated transaction IDs; 
    /// it is NOT a Merkle tree and is suitable only for scaffolding.
    pub fn build_block(
        &self,
        transactions: Vec<Transaction>,
        parent_hash: Vec<u8>,
        proposer: ParticipantId,
    ) -> Block {
        use sha2::{Digest, Sha256};
        use crate::merkle::merkle_root;
        use crate::state_commitments::{derive_state_changes, state_root_from_changes};

        let now = Utc::now();
        let slot = slot_at_time(self.genesis.genesis_time, self.chain.slot_duration_ms, now);
        let epoch = epoch_for_slot(slot, self.chain.epoch_length);

        // Compute tx_root using Merkle tree over transaction IDs
        let leaves: Vec<Vec<u8>> = transactions.iter().map(|tx| tx.id.0.as_bytes().to_vec()).collect();
        let tx_root = merkle_root(&leaves);

        // Derive state changes and compute the state_root Merkle root
        let state_changes = derive_state_changes(&transactions);
        let state_root = state_root_from_changes(&state_changes);
        let receipt_root = vec![0u8; 32];

        let header = BlockHeader {
            parent_hash,
            slot,
            epoch,
            proposer,
            state_root: state_root.clone(),
            tx_root: tx_root.clone(),
            receipt_root: receipt_root.clone(),
        };

        // Compute a synthetic block hash as hash(header.slot || header.epoch || tx_root)
        let mut bh = Sha256::new();
        bh.update(header.slot.to_le_bytes());
        bh.update(header.epoch.to_le_bytes());
        bh.update(&header.tx_root);
        let hash = bh.finalize().to_vec();

        Block {
            header,
            hash,
            timestamp: now,
            transactions,
        }
    }
}
// -----------------------------------------------------------------------------
// Utilities
// -----------------------------------------------------------------------------

pub fn compute_tx_merkle_root(tx_hashes: &[Vec<u8>]) -> Vec<u8> {
    if tx_hashes.is_empty() { return vec![0u8; 32]; }
    // Simple pairwise blake3 over leaves for a placeholder merkle calculation.
    let mut layer = tx_hashes.to_vec();
    while layer.len() > 1 {
        let mut next = Vec::new();
        for chunk in layer.chunks(2) {
            let combined = if chunk.len() == 2 {
                [chunk[0].as_slice(), chunk[1].as_slice()].concat()
            } else { chunk[0].clone() };
            let h = blake3::hash(&combined);
            next.push(h.as_bytes().to_vec());
        }
        layer = next;
    }
    layer[0].clone()
}