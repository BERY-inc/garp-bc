use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use chrono::Utc;
use garp_common::types::ParticipantId;

/// Validator status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ValidatorStatus {
    Active,
    Inactive,
    Jailed,
}

/// Validator information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorInfo {
    pub id: ParticipantId,
    pub public_key_hex: String,
    pub voting_power: u64,
    pub status: ValidatorStatus,
    pub joined_at: chrono::DateTime<chrono::Utc>,
    pub metadata: HashMap<String, String>,
}

impl ValidatorInfo {
    pub fn new(id: ParticipantId, public_key_hex: String, voting_power: u64) -> Self {
        Self {
            id,
            public_key_hex,
            voting_power,
            status: ValidatorStatus::Active,
            joined_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }
}