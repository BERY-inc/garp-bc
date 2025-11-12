use chrono::{DateTime, Utc};

/// Compute the slot number at a given time since genesis.
pub fn slot_at_time(genesis_time: DateTime<Utc>, slot_duration_ms: u64, now: DateTime<Utc>) -> u64 {
    let elapsed_ms = (now - genesis_time).num_milliseconds().max(0) as u64;
    elapsed_ms / slot_duration_ms
}

/// Compute the epoch number for a given slot.
pub fn epoch_for_slot(slot: u64, epoch_length: u64) -> u64 {
    if epoch_length == 0 { return 0; }
    slot / epoch_length
}

/// Determine validator rotation index at a given slot.
pub fn validator_rotation_index(slot: u64, rotation_interval_slots: u64) -> u64 {
    if rotation_interval_slots == 0 { return 0; }
    slot / rotation_interval_slots
}