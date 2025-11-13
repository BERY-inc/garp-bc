use sha2::{Digest, Sha256};

pub fn hash_leaf(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

pub fn merkle_root(leaves: &[Vec<u8>]) -> Vec<u8> {
    if leaves.is_empty() {
        return vec![0u8; 32];
    }
    let mut level: Vec<[u8; 32]> = leaves.iter().map(|l| hash_leaf(l)).collect();
    while level.len() > 1 {
        let mut next = Vec::with_capacity((level.len() + 1) / 2);
        let mut i = 0;
        while i < level.len() {
            let left = level[i];
            let right = if i + 1 < level.len() { level[i + 1] } else { left };
            let mut hasher = Sha256::new();
            hasher.update(left);
            hasher.update(right);
            next.push(hasher.finalize().into());
            i += 2;
        }
        level = next;
    }
    level[0].to_vec()
}

#[derive(Clone, Debug)]
pub struct MerkleProof {
    pub leaf: Vec<u8>,
    pub root: Vec<u8>,
    pub path: Vec<Vec<u8>>, // sibling hashes from leaf to root
    pub directions: Vec<bool>, // true = right sibling, false = left sibling
}

pub fn merkle_proof(leaves: &[Vec<u8>], index: usize) -> Option<MerkleProof> {
    if leaves.is_empty() || index >= leaves.len() {
        return None;
    }
    let mut level: Vec<[u8; 32]> = leaves.iter().map(|l| hash_leaf(l)).collect();
    let leaf_hash = level[index].to_vec();
    let mut idx = index;
    let mut path = Vec::new();
    let mut dirs = Vec::new();

    while level.len() > 1 {
        let is_right = idx % 2 == 1;
        let sibling_idx = if is_right { idx - 1 } else { idx + 1 };
        let sibling = if sibling_idx < level.len() { level[sibling_idx].to_vec() } else { level[idx].to_vec() };
        path.push(sibling);
        dirs.push(is_right);

        // compute parent level
        let mut next = Vec::with_capacity((level.len() + 1) / 2);
        let mut i = 0;
        while i < level.len() {
            let left = level[i];
            let right = if i + 1 < level.len() { level[i + 1] } else { left };
            let mut hasher = Sha256::new();
            hasher.update(left);
            hasher.update(right);
            next.push(hasher.finalize().into());
            i += 2;
        }
        level = next;
        idx /= 2;
    }

    Some(MerkleProof { leaf: leaf_hash, root: level[0].to_vec(), path, directions: dirs })
}

pub fn verify_proof(proof: &MerkleProof) -> bool {
    let mut current = proof.leaf.clone();
    for (i, sibling) in proof.path.iter().enumerate() {
        let mut hasher = Sha256::new();
        if proof.directions[i] {
            // current is right child
            hasher.update(sibling);
            hasher.update(&current);
        } else {
            // current is left child
            hasher.update(&current);
            hasher.update(sibling);
        }
        current = hasher.finalize().to_vec();
    }
    current == proof.root
}