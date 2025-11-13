use sha2::{Sha256, Digest};

/// Simple PoH-like ticker producing rolling hashes and ticks per slot
pub struct Poh {
    hash: [u8; 32],
    ticks_in_slot: u64,
    ticks_per_slot: u64,
}

impl Poh {
    pub fn new(seed: &[u8], ticks_per_slot: u64) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(seed);
        let h = hasher.finalize();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&h);
        Self { hash: arr, ticks_in_slot: 0, ticks_per_slot }
    }

    /// Advance the PoH and return a tick hash; resets per slot
    pub fn tick(&mut self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(&self.hash);
        let h = hasher.finalize();
        self.hash.copy_from_slice(&h);
        self.ticks_in_slot += 1;
        if self.ticks_in_slot >= self.ticks_per_slot { self.ticks_in_slot = 0; }
        self.hash
    }

    /// Get ticks per slot
    pub fn ticks_per_slot(&self) -> u64 { self.ticks_per_slot }

    /// Get current ticks in ongoing slot
    pub fn ticks_in_current_slot(&self) -> u64 { self.ticks_in_slot }
}