use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::fmt;

/// Unique identifier for participants in the network
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParticipantId(pub String);

/// Unique identifier for synchronization domains
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SyncDomainId(pub String);

/// Unique identifier for contracts
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContractId(pub Uuid);

/// Unique identifier for transactions
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionId(pub Uuid);

/// Digital signature for cryptographic verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub algorithm: String,
    pub signature: Vec<u8>,
    pub public_key: Vec<u8>,
}

/// Encrypted data with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub algorithm: String,
}

/// Contract template defining signatories and observers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contract {
    pub id: ContractId,
    pub template_id: String,
    pub signatories: Vec<ParticipantId>,
    pub observers: Vec<ParticipantId>,
    pub argument: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub archived: bool,
}

/// Transaction affecting one or more contracts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: TransactionId,
    pub submitter: ParticipantId,
    pub command: TransactionCommand,
    pub created_at: DateTime<Utc>,
    pub signatures: Vec<Signature>,
    pub encrypted_payload: Option<EncryptedData>,
}

/// Commands that can be executed in a transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionCommand {
    Create {
        template_id: String,
        argument: serde_json::Value,
        signatories: Vec<ParticipantId>,
        observers: Vec<ParticipantId>,
    },
    Exercise {
        contract_id: ContractId,
        choice: String,
        argument: serde_json::Value,
    },
    Archive {
        contract_id: ContractId,
    },
}

/// Asset representation for e-commerce
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    pub asset_type: AssetType,
    pub amount: u64,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Types of assets supported
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssetType {
    Currency { symbol: String, decimals: u8 },
    Product { sku: String, name: String },
    LoyaltyPoints { program: String },
    NFT { collection: String, token_id: String },
}

/// Canonical block header structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub parent_hash: Vec<u8>,
    pub slot: u64,
    pub epoch: u64,
    pub proposer: ParticipantId,
    pub state_root: Vec<u8>,
    pub tx_root: Vec<u8>,
    pub receipt_root: Vec<u8>,
}

/// Canonical block structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub hash: Vec<u8>,
    pub timestamp: DateTime<Utc>,
    pub transactions: Vec<Transaction>,
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Block(epoch={}, slot={}, txs={})",
            self.header.epoch, self.header.slot, self.transactions.len()
        )
    }
}

/// Transaction execution receipt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub transaction_id: TransactionId,
    pub status: bool,
    pub gas_used: u64,
    pub logs: Vec<String>,
    pub bloom: Vec<u8>,
}

/// Wallet balance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletBalance {
    pub participant_id: ParticipantId,
    pub assets: Vec<Asset>,
    pub last_updated: DateTime<Utc>,
}

/// E-commerce transaction types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ECommerceTransaction {
    Purchase {
        buyer: ParticipantId,
        seller: ParticipantId,
        product: Asset,
        payment: Asset,
        escrow: Option<ParticipantId>,
    },
    Transfer {
        from: ParticipantId,
        to: ParticipantId,
        asset: Asset,
    },
    Escrow {
        buyer: ParticipantId,
        seller: ParticipantId,
        escrow_agent: ParticipantId,
        asset: Asset,
        conditions: Vec<String>,
    },
}

/// Network message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    TransactionSubmission(Transaction),
    TransactionValidation {
        transaction_id: TransactionId,
        validation_result: ValidationResult,
    },
    ConsensusProposal {
        proposal_id: Uuid,
        transactions: Vec<TransactionId>,
        proposer: ParticipantId,
    },
    ConsensusVote {
        proposal_id: Uuid,
        vote: bool,
        voter: ParticipantId,
    },
    SyncRequest {
        domain_id: SyncDomainId,
        from_sequence: u64,
    },
    SyncResponse {
        domain_id: SyncDomainId,
        transactions: Vec<Transaction>,
        sequence_range: (u64, u64),
    },
}

/// Result of transaction validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationResult {
    Valid,
    Invalid { reason: String },
    Pending { missing_signatures: Vec<ParticipantId> },
}

/// Configuration for participant nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantConfig {
    pub participant_id: ParticipantId,
    pub private_key: Vec<u8>,
    pub sync_domains: Vec<SyncDomainId>,
    pub database_url: String,
    pub api_port: u16,
}

/// Configuration for sync domains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDomainConfig {
    pub domain_id: SyncDomainId,
    pub kafka_brokers: Vec<String>,
    pub participants: Vec<ParticipantId>,
    pub sequencer_port: u16,
}

/// Configuration for global synchronizer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSyncConfig {
    pub node_id: String,
    pub peers: Vec<String>,
    pub consensus_port: u16,
    pub api_port: u16,
}

/// Genesis configuration describing initial chain state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisConfig {
    pub chain_id: String,
    pub genesis_time: DateTime<Utc>,
    pub initial_validators: Vec<ParticipantId>,
    pub initial_balances: HashMap<ParticipantId, u64>,
}

/// Chain parameters for timing and validator rotation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainParams {
    pub slot_duration_ms: u64,
    pub epoch_length: u64,
    pub randomness_beacon: Option<String>,
    pub rotation_interval_slots: u64,
}

impl ParticipantId {
    pub fn new(id: &str) -> Self {
        Self(id.to_string())
    }
}

impl SyncDomainId {
    pub fn new(id: &str) -> Self {
        Self(id.to_string())
    }
}

impl ContractId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl TransactionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

// Display implementations for identifier types used in error messages
impl fmt::Display for ContractId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for TransactionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}