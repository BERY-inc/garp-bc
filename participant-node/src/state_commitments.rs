use garp_common::Transaction;

/// A single state change item captured for commitment purposes.
/// Policy: coalesce by key to the latest value within the block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateChangeItem {
    pub key: String,
    pub value_hash: Vec<u8>,
}

/// Derive coalesced state changes from a list of transactions.
///
/// Current scaffolding policy:
/// - Treat each transaction as changing a synthetic key `tx:<id>`.
/// - Value hash = SHA256 over the transaction ID bytes.
/// - If multiple transactions modify the same key, later ones win (coalesced).
pub fn derive_state_changes(transactions: &Vec<Transaction>) -> Vec<StateChangeItem> {
    use sha2::{Digest, Sha256};
    use std::collections::HashMap;

    let mut latest_by_key: HashMap<String, Vec<u8>> = HashMap::new();
    for tx in transactions {
        let key = format!("tx:{}", tx.id.0);
        let mut hasher = Sha256::new();
        hasher.update(tx.id.0.as_bytes());
        let value_hash = hasher.finalize().to_vec();
        latest_by_key.insert(key, value_hash);
    }

    // Deterministic order by key for Merkle leaf ordering
    let mut items: Vec<StateChangeItem> = latest_by_key
        .into_iter()
        .map(|(key, value_hash)| StateChangeItem { key, value_hash })
        .collect();
    items.sort_by(|a, b| a.key.cmp(&b.key));
    items
}

/// Produce Merkle leaves from coalesced state changes.
/// Leaf bytes = key bytes concatenated with value_hash bytes.
pub fn leaves_for_changes(changes: &Vec<StateChangeItem>) -> Vec<Vec<u8>> {
    let mut leaves: Vec<Vec<u8>> = Vec::with_capacity(changes.len());
    for c in changes {
        let mut leaf = Vec::with_capacity(c.key.as_bytes().len() + c.value_hash.len());
        leaf.extend_from_slice(c.key.as_bytes());
        leaf.extend_from_slice(&c.value_hash);
        leaves.push(leaf);
    }
    leaves
}

/// Compute the Merkle root for coalesced state changes.
pub fn state_root_from_changes(changes: &Vec<StateChangeItem>) -> Vec<u8> {
    let leaves = leaves_for_changes(changes);
    crate::merkle::merkle_root(&leaves)
}