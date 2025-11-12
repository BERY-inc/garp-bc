use thiserror::Error;
use crate::types::{ParticipantId, ContractId, TransactionId};

/// Main error type for the GARP blockchain protocol
#[derive(Error, Debug)]
pub enum GarpError {
    #[error("Cryptographic error: {0}")]
    Crypto(#[from] CryptoError),

    #[error("Network error: {0}")]
    Network(#[from] NetworkError),

    #[error("Consensus error: {0}")]
    Consensus(#[from] ConsensusError),

    #[error("Transaction error: {0}")]
    Transaction(#[from] TransactionError),

    #[error("Contract error: {0}")]
    Contract(#[from] ContractError),

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] SerializationError),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Cryptographic operation errors
#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Invalid public key")]
    InvalidPublicKey,

    #[error("Invalid private key")]
    InvalidPrivateKey,

    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Key generation failed: {0}")]
    KeyGenerationFailed(String),

    #[error("Hash computation failed: {0}")]
    HashFailed(String),

    #[error("Unsupported algorithm: {0}")]
    UnsupportedAlgorithm(String),
}

/// Network communication errors
#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Connection failed to {peer}: {reason}")]
    ConnectionFailed { peer: String, reason: String },

    #[error("Message send failed: {0}")]
    SendFailed(String),

    #[error("Message receive failed: {0}")]
    ReceiveFailed(String),

    #[error("Invalid message format: {0}")]
    InvalidMessageFormat(String),

    #[error("Peer not found: {0}")]
    PeerNotFound(String),

    #[error("Network timeout")]
    Timeout,

    #[error("Protocol version mismatch: expected {expected}, got {actual}")]
    ProtocolMismatch { expected: String, actual: String },

    #[error("Authentication failed for peer: {0}")]
    AuthenticationFailed(String),

    #[error("Peer banned: {0}")]
    PeerBanned(String),

    #[error("Transport error: {0}")]
    TransportError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Consensus mechanism errors
#[derive(Error, Debug)]
pub enum ConsensusError {
    #[error("Proposal rejected by majority")]
    ProposalRejected,

    #[error("Insufficient votes: got {got}, needed {needed}")]
    InsufficientVotes { got: usize, needed: usize },

    #[error("Invalid proposal: {0}")]
    InvalidProposal(String),

    #[error("Consensus timeout")]
    Timeout,

    #[error("Byzantine fault detected from node: {0}")]
    ByzantineFault(String),

    #[error("Leader election failed: {0}")]
    LeaderElectionFailed(String),

    #[error("State synchronization failed: {0}")]
    SyncFailed(String),
}

/// Transaction processing errors
#[derive(Error, Debug)]
pub enum TransactionError {
    #[error("Transaction not found: {0}")]
    NotFound(TransactionId),

    #[error("Invalid transaction: {0}")]
    Invalid(String),

    #[error("Missing required signature from: {0:?}")]
    MissingSignature(ParticipantId),

    #[error("Insufficient permissions for participant: {0:?}")]
    InsufficientPermissions(ParticipantId),

    #[error("Transaction already processed: {0}")]
    AlreadyProcessed(TransactionId),

    #[error("Double spending detected for transaction: {0}")]
    DoubleSpending(TransactionId),

    #[error("Transaction validation failed: {0}")]
    ValidationFailed(String),

    #[error("Transaction execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Atomic transaction failed: some operations could not be completed")]
    AtomicityViolation,

    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: u64, available: u64 },
}

/// Smart contract errors
#[derive(Error, Debug)]
pub enum ContractError {
    #[error("Contract not found: {0}")]
    NotFound(ContractId),

    #[error("Contract already archived: {0}")]
    AlreadyArchived(ContractId),

    #[error("Invalid contract template: {0}")]
    InvalidTemplate(String),

    #[error("Contract execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Choice not available: {choice} for contract {contract_id:?}")]
    ChoiceNotAvailable { choice: String, contract_id: ContractId },

    #[error("Unauthorized access to contract: {0}")]
    UnauthorizedAccess(ContractId),

    #[error("Contract state inconsistent: {0}")]
    StateInconsistent(String),

    #[error("Template compilation failed: {0}")]
    CompilationFailed(String),
}

/// Database operation errors
#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Query failed: {0}")]
    QueryFailed(String),

    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    #[error("Migration failed: {0}")]
    MigrationFailed(String),

    #[error("Data integrity violation: {0}")]
    IntegrityViolation(String),

    #[error("Record not found: {0}")]
    RecordNotFound(String),

    #[error("Duplicate key: {0}")]
    DuplicateKey(String),
}

/// Serialization/deserialization errors
#[derive(Error, Debug)]
pub enum SerializationError {
    #[error("JSON serialization failed: {0}")]
    JsonFailed(#[from] serde_json::Error),

    #[error("Binary serialization failed: {0}")]
    BinaryFailed(#[from] bincode::Error),

    #[error("Invalid data format: {0}")]
    InvalidFormat(String),

    #[error("Version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: String, actual: String },
}

/// Result type alias for GARP operations
pub type GarpResult<T> = Result<T, GarpError>;

/// Utility functions for error handling
impl GarpError {
    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            GarpError::Network(NetworkError::Timeout) => true,
            GarpError::Network(NetworkError::ConnectionFailed { .. }) => true,
            GarpError::Database(DatabaseError::ConnectionFailed(_)) => true,
            GarpError::Consensus(ConsensusError::Timeout) => true,
            _ => false,
        }
    }

    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            GarpError::Crypto(_) => ErrorSeverity::Critical,
            GarpError::Consensus(ConsensusError::ByzantineFault(_)) => ErrorSeverity::Critical,
            GarpError::Transaction(TransactionError::DoubleSpending(_)) => ErrorSeverity::Critical,
            GarpError::Database(DatabaseError::IntegrityViolation(_)) => ErrorSeverity::High,
            GarpError::Network(_) => ErrorSeverity::Medium,
            GarpError::Config(_) => ErrorSeverity::High,
            _ => ErrorSeverity::Low,
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorSeverity {
    Critical,
    High,
    Medium,
    Low,
}

/// Error context for better debugging
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub operation: String,
    pub participant_id: Option<ParticipantId>,
    pub transaction_id: Option<TransactionId>,
    pub contract_id: Option<ContractId>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ErrorContext {
    pub fn new(operation: &str) -> Self {
        Self {
            operation: operation.to_string(),
            participant_id: None,
            transaction_id: None,
            contract_id: None,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn with_participant(mut self, participant_id: ParticipantId) -> Self {
        self.participant_id = Some(participant_id);
        self
    }

    pub fn with_transaction(mut self, transaction_id: TransactionId) -> Self {
        self.transaction_id = Some(transaction_id);
        self
    }

    pub fn with_contract(mut self, contract_id: ContractId) -> Self {
        self.contract_id = Some(contract_id);
        self
    }
}

/// Macro for creating contextual errors
#[macro_export]
macro_rules! garp_error {
    ($error:expr, $context:expr) => {
        {
            tracing::error!(
                error = ?$error,
                context = ?$context,
                "GARP error occurred"
            );
            $error
        }
    };
}

/// Macro for creating and logging errors with context
#[macro_export]
macro_rules! garp_bail {
    ($error:expr, $operation:expr) => {
        return Err(garp_error!($error, ErrorContext::new($operation)))
    };
    ($error:expr, $operation:expr, $participant:expr) => {
        return Err(garp_error!(
            $error, 
            ErrorContext::new($operation).with_participant($participant)
        ))
    };
}