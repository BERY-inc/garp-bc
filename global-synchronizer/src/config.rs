use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use garp_common::GarpResult;

/// Global Synchronizer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSyncConfig {
    /// Node configuration
    pub node: NodeConfig,
    
    /// Consensus configuration
    pub consensus: ConsensusConfig,
    
    /// Cross-domain coordination configuration
    pub cross_domain: CrossDomainConfig,
    
    /// Settlement configuration
    pub settlement: SettlementConfig,
    
    /// Network configuration
    pub network: NetworkConfig,
    
    /// Kafka configuration
    pub kafka: KafkaConfig,
    
    /// Database configuration
    pub database: DatabaseConfig,
    
    /// API configuration
    pub api: ApiConfig,
    
    /// Security configuration
    pub security: SecurityConfig,
    
    /// Performance configuration
    pub performance: PerformanceConfig,
    
    /// Monitoring configuration
    pub monitoring: MonitoringConfig,
}

/// Node configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Unique node identifier
    pub node_id: String,
    
    /// Node type
    pub node_type: NodeType,
    
    /// Node region/zone
    pub region: String,
    
    /// Node capabilities
    pub capabilities: Vec<String>,
    
    /// Node metadata
    pub metadata: std::collections::HashMap<String, String>,
}

/// Node type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    /// Full validator node
    Validator,
    
    /// Observer node (read-only)
    Observer,
    
    /// Seed node for discovery
    Seed,
    
    /// Archive node with full history
    Archive,
}

/// Consensus configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Consensus algorithm
    pub algorithm: ConsensusAlgorithm,
    
    /// Cluster peers
    pub cluster_peers: Vec<String>,
    
    /// Consensus port
    pub port: u16,
    
    /// Minimum number of validators
    pub min_validators: usize,
    
    /// Maximum number of validators
    pub max_validators: usize,
    
    /// Block time in milliseconds
    pub block_time_ms: u64,
    
    /// Timeout for consensus rounds
    pub consensus_timeout_ms: u64,
    
    /// Maximum transactions per block
    pub max_transactions_per_block: usize,
    
    /// Byzantine fault tolerance threshold (f in 3f+1)
    pub byzantine_threshold: usize,
    
    /// Enable fast path optimization
    pub enable_fast_path: bool,
    
    /// Checkpoint interval
    pub checkpoint_interval: u64,
    /// Detailed consensus parameters
    pub params: ConsensusParams,
    /// Network limits applied to consensus gossip/vote channels
    pub network_limits: ConsensusNetworkLimits,
}

/// Consensus algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusAlgorithm {
    /// Raft consensus (for testing)
    Raft,
    
    /// PBFT (Practical Byzantine Fault Tolerance)
    PBFT,
    
    /// Tendermint BFT
    Tendermint,
    
    /// HotStuff BFT
    HotStuff,
    
    /// Custom GARP BFT
    GarpBFT,
}

/// Detailed consensus parameters to drive quorum and view-change decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusParams {
    /// Protocol descriptor (e.g., "tendermint-like")
    pub protocol: String,
    /// Quorum ratio in thousandths (e.g., 667 => 66.7%)
    pub quorum_ratio_thousandths: u32,
    /// Maximum views without progress before view-change
    pub max_views_without_progress: u32,
    /// Timeout for view changes in milliseconds
    pub view_change_timeout_ms: u64,
    /// Jail duration for validators (seconds) used by adjudication
    pub jail_duration_secs: u64,
}

/// Limits applied to consensus network channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusNetworkLimits {
    /// Max messages per second per peer on gossip channel
    pub gossip_rate_per_peer: u32,
    /// Max votes per second per peer
    pub vote_rate_per_peer: u32,
    /// Burst capacity before backpressure is applied
    pub burst_capacity: u32,
    /// Ban list of peers (IDs) that should be dropped/ignored
    pub ban_list: Vec<String>,
    /// Enable automatic temporary banning on flood
    pub enable_auto_ban: bool,
    /// Temporary ban duration in seconds
    pub temp_ban_duration_secs: u64,
}

/// Cross-domain coordination configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossDomainConfig {
    /// Known sync domains
    pub known_domains: Vec<DomainInfo>,
    
    /// Domain discovery settings
    pub discovery: DomainDiscoveryConfig,
    
    /// Cross-domain transaction timeout
    pub transaction_timeout_ms: u64,
    
    /// Maximum concurrent cross-domain transactions
    pub max_concurrent_transactions: usize,
    
    /// Retry configuration
    pub retry_config: RetryConfig,
    
    /// Enable domain health monitoring
    pub enable_health_monitoring: bool,
    
    /// Health check interval
    pub health_check_interval_ms: u64,
}

/// Domain information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainInfo {
    /// Domain ID
    pub domain_id: String,
    
    /// Domain endpoints
    pub endpoints: Vec<String>,
    
    /// Domain public key
    pub public_key: Vec<u8>,
    
    /// Domain capabilities
    pub capabilities: Vec<String>,
    
    /// Trust level
    pub trust_level: TrustLevel,
}

/// Trust level for domains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrustLevel {
    /// Fully trusted domain
    Trusted,
    
    /// Partially trusted domain
    Partial,
    
    /// Untrusted domain (requires additional verification)
    Untrusted,
    
    /// Blacklisted domain
    Blacklisted,
}

/// Domain discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainDiscoveryConfig {
    /// Enable automatic domain discovery
    pub enable_auto_discovery: bool,
    
    /// Discovery protocol
    pub protocol: DiscoveryProtocol,
    
    /// Discovery interval
    pub discovery_interval_ms: u64,
    
    /// Bootstrap nodes for discovery
    pub bootstrap_nodes: Vec<String>,
    
    /// Maximum domains to discover
    pub max_discovered_domains: usize,
}

/// Discovery protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscoveryProtocol {
    /// mDNS discovery
    MDNS,
    
    /// Kademlia DHT
    Kademlia,
    
    /// Gossip-based discovery
    Gossip,
    
    /// Static configuration
    Static,
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum retry attempts
    pub max_attempts: usize,
    
    /// Initial retry delay in milliseconds
    pub initial_delay_ms: u64,
    
    /// Maximum retry delay in milliseconds
    pub max_delay_ms: u64,
    
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    
    /// Enable jitter
    pub enable_jitter: bool,
}

/// Settlement configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementConfig {
    /// Settlement mode
    pub mode: SettlementMode,
    
    /// Batch size for settlement
    pub batch_size: usize,
    
    /// Settlement timeout
    pub settlement_timeout_ms: u64,
    
    /// Enable atomic settlement
    pub enable_atomic_settlement: bool,
    
    /// Finality confirmation blocks
    pub finality_blocks: u64,
    
    /// Enable settlement compression
    pub enable_compression: bool,
    
    /// Settlement verification settings
    pub verification: VerificationConfig,
}

/// Settlement mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SettlementMode {
    /// Immediate settlement
    Immediate,
    
    /// Batched settlement
    Batched,
    
    /// Scheduled settlement
    Scheduled,
    
    /// On-demand settlement
    OnDemand,
}

/// Verification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationConfig {
    /// Enable cryptographic verification
    pub enable_crypto_verification: bool,
    
    /// Enable state verification
    pub enable_state_verification: bool,
    
    /// Verification timeout
    pub verification_timeout_ms: u64,
    
    /// Required verification signatures
    pub required_signatures: usize,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Listen address
    pub listen_address: String,
    
    /// External address (for NAT traversal)
    pub external_address: Option<String>,
    
    /// Maximum connections
    pub max_connections: usize,
    
    /// Connection timeout
    pub connection_timeout_ms: u64,
    
    /// Keep-alive interval
    pub keep_alive_interval_ms: u64,
    
    /// Enable TLS
    pub enable_tls: bool,
    
    /// TLS certificate path
    pub tls_cert_path: Option<String>,
    
    /// TLS private key path
    pub tls_key_path: Option<String>,
    
    /// Enable compression
    pub enable_compression: bool,
    
    /// Compression algorithm
    pub compression_algorithm: CompressionAlgorithm,
}

/// Compression algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    None,
    LZ4,
    Zstd,
    Gzip,
}

/// Kafka configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConfig {
    /// Bootstrap servers
    pub bootstrap_servers: Vec<String>,
    
    /// Consumer group ID
    pub consumer_group_id: String,
    
    /// Producer configuration
    pub producer: KafkaProducerConfig,
    
    /// Consumer configuration
    pub consumer: KafkaConsumerConfig,
    
    /// Topic configuration
    pub topics: KafkaTopicConfig,
    
    /// Security configuration
    pub security: KafkaSecurityConfig,
}

/// Kafka producer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaProducerConfig {
    /// Acknowledgment level
    pub acks: String,
    
    /// Retries
    pub retries: u32,
    
    /// Batch size
    pub batch_size: u32,
    
    /// Linger time in milliseconds
    pub linger_ms: u32,
    
    /// Buffer memory
    pub buffer_memory: u64,
    
    /// Compression type
    pub compression_type: String,
}

/// Kafka consumer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConsumerConfig {
    /// Auto offset reset
    pub auto_offset_reset: String,
    
    /// Enable auto commit
    pub enable_auto_commit: bool,
    
    /// Auto commit interval
    pub auto_commit_interval_ms: u32,
    
    /// Session timeout
    pub session_timeout_ms: u32,
    
    /// Heartbeat interval
    pub heartbeat_interval_ms: u32,
    
    /// Max poll records
    pub max_poll_records: u32,
}

/// Kafka topic configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaTopicConfig {
    /// Cross-domain transactions topic
    pub cross_domain_transactions: String,
    
    /// Settlement events topic
    pub settlement_events: String,
    
    /// Consensus events topic
    pub consensus_events: String,
    
    /// Domain events topic
    pub domain_events: String,
    
    /// Health events topic
    pub health_events: String,
}

/// Kafka security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaSecurityConfig {
    /// Security protocol
    pub security_protocol: String,
    
    /// SASL mechanism
    pub sasl_mechanism: Option<String>,
    
    /// SASL username
    pub sasl_username: Option<String>,
    
    /// SASL password
    pub sasl_password: Option<String>,
    
    /// SSL CA location
    pub ssl_ca_location: Option<String>,
    
    /// SSL certificate location
    pub ssl_certificate_location: Option<String>,
    
    /// SSL key location
    pub ssl_key_location: Option<String>,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database URL
    pub url: String,
    
    /// Maximum connections
    pub max_connections: u32,
    
    /// Minimum connections
    pub min_connections: u32,
    
    /// Connection timeout
    pub connect_timeout_ms: u64,
    
    /// Idle timeout
    pub idle_timeout_ms: u64,
    
    /// Enable migrations
    pub enable_migrations: bool,
    
    /// Enable connection pooling
    pub enable_pooling: bool,
}

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// API server port
    pub port: u16,
    
    /// Bind address
    pub bind_address: String,
    
    /// Enable CORS
    pub enable_cors: bool,
    
    /// CORS allowed origins
    pub cors_allowed_origins: Vec<String>,
    
    /// Request timeout
    pub request_timeout_ms: u64,
    
    /// Request body limit
    pub request_body_limit: usize,
    
    /// Enable rate limiting
    pub enable_rate_limiting: bool,
    
    /// Rate limit requests per minute
    pub rate_limit_rpm: u32,
    
    /// Enable authentication
    pub enable_auth: bool,
    
    /// JWT secret
    pub jwt_secret: Option<String>,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Private key path
    pub private_key_path: String,
    
    /// Public key path
    pub public_key_path: String,
    
    /// Enable signature verification
    pub enable_signature_verification: bool,
    
    /// Enable encryption
    pub enable_encryption: bool,
    
    /// Encryption algorithm
    pub encryption_algorithm: EncryptionAlgorithm,
    
    /// Key rotation interval in hours
    pub key_rotation_interval_hours: u64,
    
    /// Enable audit logging
    pub enable_audit_logging: bool,
}

/// Encryption algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EncryptionAlgorithm {
    AES256GCM,
    ChaCha20Poly1305,
    XChaCha20Poly1305,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Worker thread count
    pub worker_threads: Option<usize>,
    
    /// Enable thread pinning
    pub enable_thread_pinning: bool,
    
    /// Memory pool size
    pub memory_pool_size: usize,
    
    /// Enable memory compression
    pub enable_memory_compression: bool,
    
    /// Cache configuration
    pub cache: CacheConfig,
    
    /// Batch processing configuration
    pub batch_processing: BatchProcessingConfig,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable caching
    pub enable_cache: bool,
    
    /// Cache size in MB
    pub cache_size_mb: usize,
    
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
    
    /// Cache eviction policy
    pub eviction_policy: EvictionPolicy,
}

/// Cache eviction policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvictionPolicy {
    LRU,
    LFU,
    FIFO,
    Random,
}

/// Batch processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProcessingConfig {
    /// Enable batch processing
    pub enable_batch_processing: bool,
    
    /// Batch size
    pub batch_size: usize,
    
    /// Batch timeout in milliseconds
    pub batch_timeout_ms: u64,
    
    /// Maximum concurrent batches
    pub max_concurrent_batches: usize,
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable metrics
    pub enable_metrics: bool,
    
    /// Metrics port
    pub metrics_port: u16,
    
    /// Metrics path
    pub metrics_path: String,
    
    /// Enable health checks
    pub enable_health_checks: bool,
    
    /// Health check port
    pub health_check_port: u16,
    
    /// Health check path
    pub health_check_path: String,
    
    /// Enable tracing
    pub enable_tracing: bool,
    
    /// Tracing endpoint
    pub tracing_endpoint: Option<String>,
    
    /// Log configuration
    pub logging: LoggingConfig,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    pub level: String,
    
    /// Log format
    pub format: LogFormat,
    
    /// Enable file logging
    pub enable_file_logging: bool,
    
    /// Log file path
    pub log_file_path: Option<String>,
    
    /// Log rotation
    pub rotation: LogRotationConfig,
}

/// Log format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogFormat {
    Json,
    Pretty,
    Compact,
}

/// Log rotation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRotationConfig {
    /// Enable rotation
    pub enable_rotation: bool,
    
    /// Max file size in MB
    pub max_file_size_mb: usize,
    
    /// Max files to keep
    pub max_files: usize,
    
    /// Rotation interval in hours
    pub rotation_interval_hours: u64,
}

impl GlobalSyncConfig {
    /// Load configuration from file
    pub fn load<P: AsRef<Path>>(path: P) -> GarpResult<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
    
    /// Save configuration to file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> GarpResult<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
    
    /// Validate configuration
    pub fn validate(&self) -> GarpResult<()> {
        // Validate node configuration
        if self.node.node_id.is_empty() {
            return Err(garp_common::GarpError::ConfigError("Node ID cannot be empty".to_string()));
        }
        
        // Validate consensus configuration
        if self.consensus.cluster_peers.is_empty() {
            return Err(garp_common::GarpError::ConfigError("Cluster peers cannot be empty".to_string()));
        }
        
        if self.consensus.min_validators == 0 {
            return Err(garp_common::GarpError::ConfigError("Minimum validators must be greater than 0".to_string()));
        }
        
        if self.consensus.max_validators < self.consensus.min_validators {
            return Err(garp_common::GarpError::ConfigError("Maximum validators must be >= minimum validators".to_string()));
        }
        
        // Validate Byzantine threshold (3f+1 rule)
        let min_nodes_for_bft = 3 * self.consensus.byzantine_threshold + 1;
        if self.consensus.min_validators < min_nodes_for_bft {
            return Err(garp_common::GarpError::ConfigError(
                format!("Minimum validators ({}) must be at least {} for Byzantine threshold of {}",
                        self.consensus.min_validators, min_nodes_for_bft, self.consensus.byzantine_threshold)
            ));
        }

        // Validate extended consensus params
        if self.consensus.params.quorum_ratio_thousandths == 0 || self.consensus.params.quorum_ratio_thousandths > 1000 {
            return Err(garp_common::GarpError::ConfigError("quorum_ratio_thousandths must be in (0, 1000]".to_string()));
        }
        if self.consensus.params.view_change_timeout_ms == 0 {
            return Err(garp_common::GarpError::ConfigError("view_change_timeout_ms must be > 0".to_string()));
        }
        if self.consensus.params.jail_duration_secs == 0 {
            return Err(garp_common::GarpError::ConfigError("jail_duration_secs must be > 0".to_string()));
        }

        // Validate network limits for consensus channels
        if self.consensus.network_limits.gossip_rate_per_peer == 0 {
            return Err(garp_common::GarpError::ConfigError("gossip_rate_per_peer must be > 0".to_string()));
        }
        if self.consensus.network_limits.vote_rate_per_peer == 0 {
            return Err(garp_common::GarpError::ConfigError("vote_rate_per_peer must be > 0".to_string()));
        }
        if self.consensus.network_limits.burst_capacity == 0 {
            return Err(garp_common::GarpError::ConfigError("burst_capacity must be > 0".to_string()));
        }
        
        // Validate database URL
        if self.database.url.is_empty() {
            return Err(garp_common::GarpError::ConfigError("Database URL cannot be empty".to_string()));
        }
        
        // Validate Kafka configuration
        if self.kafka.bootstrap_servers.is_empty() {
            return Err(garp_common::GarpError::ConfigError("Kafka bootstrap servers cannot be empty".to_string()));
        }
        
        // Validate security configuration
        if self.security.private_key_path.is_empty() {
            return Err(garp_common::GarpError::ConfigError("Private key path cannot be empty".to_string()));
        }
        
        if self.security.public_key_path.is_empty() {
            return Err(garp_common::GarpError::ConfigError("Public key path cannot be empty".to_string()));
        }
        
        Ok(())
    }
    
    /// Get consensus timeout as Duration
    pub fn consensus_timeout(&self) -> Duration {
        Duration::from_millis(self.consensus.consensus_timeout_ms)
    }
    
    /// Get block time as Duration
    pub fn block_time(&self) -> Duration {
        Duration::from_millis(self.consensus.block_time_ms)
    }
    
    /// Get transaction timeout as Duration
    pub fn transaction_timeout(&self) -> Duration {
        Duration::from_millis(self.cross_domain.transaction_timeout_ms)
    }
    
    /// Get settlement timeout as Duration
    pub fn settlement_timeout(&self) -> Duration {
        Duration::from_millis(self.settlement.settlement_timeout_ms)
    }
}

impl Default for GlobalSyncConfig {
    fn default() -> Self {
        Self {
            node: NodeConfig {
                node_id: "global-sync-1".to_string(),
                node_type: NodeType::Validator,
                region: "us-east-1".to_string(),
                capabilities: vec!["consensus".to_string(), "settlement".to_string()],
                metadata: std::collections::HashMap::new(),
            },
            consensus: ConsensusConfig {
                algorithm: ConsensusAlgorithm::GarpBFT,
                cluster_peers: vec![
                    "localhost:7000".to_string(),
                    "localhost:7001".to_string(),
                    "localhost:7002".to_string(),
                ],
                port: 7000,
                min_validators: 4,
                max_validators: 100,
                block_time_ms: 1000,
                consensus_timeout_ms: 5000,
                max_transactions_per_block: 1000,
                byzantine_threshold: 1,
                enable_fast_path: true,
                checkpoint_interval: 100,
                params: ConsensusParams {
                    protocol: "tendermint-like".to_string(),
                    quorum_ratio_thousandths: 667,
                    max_views_without_progress: 3,
                    view_change_timeout_ms: 5000,
                    jail_duration_secs: 3600,
                },
                network_limits: ConsensusNetworkLimits {
                    gossip_rate_per_peer: 50,
                    vote_rate_per_peer: 20,
                    burst_capacity: 100,
                    ban_list: vec![],
                    enable_auto_ban: true,
                    temp_ban_duration_secs: 600,
                },
            },
            cross_domain: CrossDomainConfig {
                known_domains: Vec::new(),
                discovery: DomainDiscoveryConfig {
                    enable_auto_discovery: true,
                    protocol: DiscoveryProtocol::Gossip,
                    discovery_interval_ms: 30000,
                    bootstrap_nodes: Vec::new(),
                    max_discovered_domains: 100,
                },
                transaction_timeout_ms: 30000,
                max_concurrent_transactions: 1000,
                retry_config: RetryConfig {
                    max_attempts: 3,
                    initial_delay_ms: 1000,
                    max_delay_ms: 10000,
                    backoff_multiplier: 2.0,
                    enable_jitter: true,
                },
                enable_health_monitoring: true,
                health_check_interval_ms: 10000,
            },
            settlement: SettlementConfig {
                mode: SettlementMode::Batched,
                batch_size: 100,
                settlement_timeout_ms: 10000,
                enable_atomic_settlement: true,
                finality_blocks: 6,
                enable_compression: true,
                verification: VerificationConfig {
                    enable_crypto_verification: true,
                    enable_state_verification: true,
                    verification_timeout_ms: 5000,
                    required_signatures: 2,
                },
            },
            network: NetworkConfig {
                listen_address: "0.0.0.0:8000".to_string(),
                external_address: None,
                max_connections: 1000,
                connection_timeout_ms: 10000,
                keep_alive_interval_ms: 30000,
                enable_tls: false,
                tls_cert_path: None,
                tls_key_path: None,
                enable_compression: true,
                compression_algorithm: CompressionAlgorithm::LZ4,
            },
            kafka: KafkaConfig {
                bootstrap_servers: vec!["localhost:9092".to_string()],
                consumer_group_id: "global-synchronizer".to_string(),
                producer: KafkaProducerConfig {
                    acks: "all".to_string(),
                    retries: 3,
                    batch_size: 16384,
                    linger_ms: 5,
                    buffer_memory: 33554432,
                    compression_type: "lz4".to_string(),
                },
                consumer: KafkaConsumerConfig {
                    auto_offset_reset: "earliest".to_string(),
                    enable_auto_commit: false,
                    auto_commit_interval_ms: 5000,
                    session_timeout_ms: 30000,
                    heartbeat_interval_ms: 3000,
                    max_poll_records: 500,
                },
                topics: KafkaTopicConfig {
                    cross_domain_transactions: "cross-domain-transactions".to_string(),
                    settlement_events: "settlement-events".to_string(),
                    consensus_events: "consensus-events".to_string(),
                    domain_events: "domain-events".to_string(),
                    health_events: "health-events".to_string(),
                },
                security: KafkaSecurityConfig {
                    security_protocol: "PLAINTEXT".to_string(),
                    sasl_mechanism: None,
                    sasl_username: None,
                    sasl_password: None,
                    ssl_ca_location: None,
                    ssl_certificate_location: None,
                    ssl_key_location: None,
                },
            },
            database: DatabaseConfig {
                url: "postgresql://garp:garp@localhost:5432/global_sync".to_string(),
                max_connections: 20,
                min_connections: 5,
                connect_timeout_ms: 10000,
                idle_timeout_ms: 600000,
                enable_migrations: true,
                enable_pooling: true,
            },
            api: ApiConfig {
                port: 8000,
                bind_address: "0.0.0.0".to_string(),
                enable_cors: true,
                cors_allowed_origins: vec!["*".to_string()],
                request_timeout_ms: 30000,
                request_body_limit: 1048576, // 1MB
                enable_rate_limiting: true,
                rate_limit_rpm: 1000,
                enable_auth: false,
                jwt_secret: None,
            },
            security: SecurityConfig {
                private_key_path: "keys/global-sync-private.pem".to_string(),
                public_key_path: "keys/global-sync-public.pem".to_string(),
                enable_signature_verification: true,
                enable_encryption: true,
                encryption_algorithm: EncryptionAlgorithm::ChaCha20Poly1305,
                key_rotation_interval_hours: 24,
                enable_audit_logging: true,
            },
            performance: PerformanceConfig {
                worker_threads: None,
                enable_thread_pinning: false,
                memory_pool_size: 1073741824, // 1GB
                enable_memory_compression: false,
                cache: CacheConfig {
                    enable_cache: true,
                    cache_size_mb: 256,
                    cache_ttl_seconds: 3600,
                    eviction_policy: EvictionPolicy::LRU,
                },
                batch_processing: BatchProcessingConfig {
                    enable_batch_processing: true,
                    batch_size: 100,
                    batch_timeout_ms: 1000,
                    max_concurrent_batches: 10,
                },
            },
            monitoring: MonitoringConfig {
                enable_metrics: true,
                metrics_port: 9090,
                metrics_path: "/metrics".to_string(),
                enable_health_checks: true,
                health_check_port: 8080,
                health_check_path: "/health".to_string(),
                enable_tracing: true,
                tracing_endpoint: None,
                logging: LoggingConfig {
                    level: "info".to_string(),
                    format: LogFormat::Pretty,
                    enable_file_logging: false,
                    log_file_path: None,
                    rotation: LogRotationConfig {
                        enable_rotation: false,
                        max_file_size_mb: 100,
                        max_files: 10,
                        rotation_interval_hours: 24,
                    },
                },
            },
        }
    }
}