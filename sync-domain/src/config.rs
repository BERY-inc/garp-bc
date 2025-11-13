use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use garp_common::{GarpResult, GenesisConfig, ChainParams};
use anyhow::Context;

/// Synchronization Domain configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDomainConfig {
    /// Unique domain identifier
    pub domain_id: String,
    
    /// Domain metadata
    pub metadata: DomainMetadata,
    
    /// Kafka configuration
    pub kafka: KafkaConfig,
    
    /// Database configuration
    pub database: DatabaseConfig,
    
    /// API server configuration
    pub api: ApiConfig,
    
    /// Consensus configuration
    pub consensus: ConsensusConfig,
    
    /// Security configuration
    pub security: SecurityConfig,
    
    /// Participant management
    pub participants: ParticipantConfig,
    
    /// Performance tuning
    pub performance: PerformanceConfig,
    
    /// Monitoring configuration
    pub monitoring: MonitoringConfig,
    
    /// Genesis configuration
    #[serde(default)]
    pub genesis: Option<GenesisConfig>,

    /// Chain parameters
    #[serde(default)]
    pub chain: Option<ChainParams>,
}

/// Domain metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainMetadata {
    /// Human-readable domain name
    pub name: String,
    
    /// Domain description
    pub description: String,
    
    /// Geographic region
    pub region: String,
    
    /// Jurisdiction/regulatory domain
    pub jurisdiction: String,
    
    /// Domain operator information
    pub operator: OperatorInfo,
    
    /// Supported transaction types
    pub supported_transaction_types: Vec<String>,
    
    /// Maximum participants allowed
    pub max_participants: Option<usize>,
}

/// Domain operator information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorInfo {
    pub name: String,
    pub contact_email: String,
    pub website: Option<String>,
    pub public_key: String,
}

/// Kafka configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConfig {
    /// Kafka bootstrap servers
    pub bootstrap_servers: String,
    
    /// Security protocol (PLAINTEXT, SSL, SASL_PLAINTEXT, SASL_SSL)
    pub security_protocol: String,
    
    /// SASL mechanism
    pub sasl_mechanism: Option<String>,
    
    /// SASL username
    pub sasl_username: Option<String>,
    
    /// SASL password
    pub sasl_password: Option<String>,
    
    /// SSL configuration
    pub ssl: Option<SslConfig>,
    
    /// Topic configuration
    pub topics: TopicConfig,
    
    /// Producer configuration
    pub producer: ProducerConfig,
    
    /// Consumer configuration
    pub consumer: ConsumerConfig,
}

/// SSL configuration for Kafka
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslConfig {
    pub ca_location: Option<String>,
    pub certificate_location: Option<String>,
    pub key_location: Option<String>,
    pub key_password: Option<String>,
}

/// Kafka topic configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicConfig {
    /// Transaction sequencing topic
    pub transaction_topic: String,
    
    /// Consensus coordination topic
    pub consensus_topic: String,
    
    /// Participant coordination topic
    pub participant_topic: String,
    
    /// Event notification topic
    pub event_topic: String,
    
    /// Number of partitions for each topic
    pub partitions: u32,
    
    /// Replication factor
    pub replication_factor: u16,
    
    /// Topic retention time in milliseconds
    pub retention_ms: Option<i64>,
}

/// Kafka producer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProducerConfig {
    /// Acknowledgment level (0, 1, all)
    pub acks: String,
    
    /// Retry attempts
    pub retries: u32,
    
    /// Batch size
    pub batch_size: u32,
    
    /// Linger time in milliseconds
    pub linger_ms: u32,
    
    /// Buffer memory
    pub buffer_memory: u64,
    
    /// Compression type
    pub compression_type: String,
    
    /// Request timeout
    pub request_timeout_ms: u32,
}

/// Kafka consumer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerConfig {
    /// Consumer group ID
    pub group_id: String,
    
    /// Auto offset reset policy
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
    
    /// Fetch min bytes
    pub fetch_min_bytes: u32,
    
    /// Fetch max wait
    pub fetch_max_wait_ms: u32,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database connection URL
    pub url: String,
    
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    
    /// Minimum number of connections in the pool
    pub min_connections: u32,
    
    /// Connection timeout in seconds
    pub connect_timeout: u64,
    
    /// Idle timeout in seconds
    pub idle_timeout: u64,
    
    /// Maximum lifetime of a connection in seconds
    pub max_lifetime: u64,
    
    /// Enable SQL logging
    pub enable_logging: bool,
    
    /// Redis configuration for caching
    pub redis: Option<RedisConfig>,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis connection URL
    pub url: String,
    
    /// Connection pool size
    pub pool_size: u32,
    
    /// Connection timeout in seconds
    pub timeout: u64,
    
    /// Default TTL for cached items in seconds
    pub default_ttl: u64,
}

/// API server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Server host
    pub host: String,
    
    /// Server port
    pub port: u16,
    
    /// Enable TLS
    pub tls_enabled: bool,
    
    /// TLS certificate file path
    pub tls_cert_path: Option<String>,
    
    /// TLS private key file path
    pub tls_key_path: Option<String>,
    
    /// Request timeout in seconds
    pub request_timeout: u64,
    
    /// Maximum request body size in bytes
    pub max_body_size: usize,
    
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
    
    /// CORS configuration
    pub cors: CorsConfig,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Enable rate limiting
    pub enabled: bool,
    
    /// Requests per minute per IP
    pub requests_per_minute: u32,
    
    /// Burst size
    pub burst_size: u32,
}

/// CORS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    /// Allowed origins
    pub allowed_origins: Vec<String>,
    
    /// Allowed methods
    pub allowed_methods: Vec<String>,
    
    /// Allowed headers
    pub allowed_headers: Vec<String>,
    
    /// Max age for preflight requests
    pub max_age: u64,
}

/// Consensus configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Consensus algorithm (raft, pbft)
    pub algorithm: String,
    
    /// Node ID for consensus
    pub node_id: String,
    
    /// Cluster peers
    pub peers: Vec<String>,
    
    /// Election timeout in milliseconds
    pub election_timeout_ms: u64,
    
    /// Heartbeat interval in milliseconds
    pub heartbeat_interval_ms: u64,
    
    /// Log compaction threshold
    pub log_compaction_threshold: u64,
    
    /// Snapshot interval
    pub snapshot_interval: u64,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Private key for domain signing
    pub private_key_path: String,
    
    /// Public key for verification
    pub public_key_path: String,
    
    /// Trusted participant public keys
    pub trusted_participants: HashMap<String, String>,
    
    /// Enable message encryption
    pub enable_encryption: bool,
    
    /// Encryption algorithm
    pub encryption_algorithm: String,
    
    /// Key rotation interval in hours
    pub key_rotation_interval: u64,
    
    /// Enable audit logging
    pub enable_audit_log: bool,
    
    /// Audit log retention days
    pub audit_log_retention_days: u32,
}

/// Participant management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantConfig {
    /// Auto-approve new participants
    pub auto_approve: bool,
    
    /// Require participant verification
    pub require_verification: bool,
    
    /// Maximum participants per domain
    pub max_participants: Option<usize>,
    
    /// Participant onboarding timeout in hours
    pub onboarding_timeout_hours: u64,
    
    /// Participant health check interval in seconds
    pub health_check_interval: u64,
    
    /// Participant timeout threshold in seconds
    pub timeout_threshold: u64,
}

/// Performance tuning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Transaction batch size
    pub transaction_batch_size: usize,
    
    /// Batch timeout in milliseconds
    pub batch_timeout_ms: u64,
    
    /// Maximum concurrent transactions
    pub max_concurrent_transactions: usize,
    
    /// Sequencer buffer size
    pub sequencer_buffer_size: usize,
    
    /// Enable transaction compression
    pub enable_compression: bool,
    
    /// Compression algorithm
    pub compression_algorithm: String,
    
    /// Memory pool size for transactions
    pub transaction_pool_size: usize,
    
    /// Enable parallel processing
    pub enable_parallel_processing: bool,
    
    /// Worker thread count
    pub worker_threads: Option<usize>,
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable Prometheus metrics
    pub enable_metrics: bool,
    
    /// Metrics endpoint path
    pub metrics_path: String,
    
    /// Metrics port
    pub metrics_port: u16,
    
    /// Enable health checks
    pub enable_health_checks: bool,
    
    /// Health check interval in seconds
    pub health_check_interval: u64,
    
    /// Enable distributed tracing
    pub enable_tracing: bool,
    
    /// Tracing endpoint
    pub tracing_endpoint: Option<String>,
    
    /// Log level
    pub log_level: String,
    
    /// Enable structured logging
    pub structured_logging: bool,
}

impl SyncDomainConfig {
    /// Load configuration from file
    pub fn load(path: &str) -> GarpResult<Self> {
        let config_str = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path))?;
        
        let config: SyncDomainConfig = toml::from_str(&config_str)
            .with_context(|| format!("Failed to parse config file: {}", path))?;
        
        Ok(config)
    }

    /// Save configuration to file
    pub fn save(&self, path: &str) -> GarpResult<()> {
        let config_str = toml::to_string_pretty(self)
            .context("Failed to serialize configuration")?;
        
        std::fs::write(path, config_str)
            .with_context(|| format!("Failed to write config file: {}", path))?;
        
        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> GarpResult<()> {
        // Validate domain ID
        if self.domain_id.is_empty() {
            return Err(anyhow::anyhow!("Domain ID cannot be empty").into());
        }

        // Validate Kafka configuration
        if self.kafka.bootstrap_servers.is_empty() {
            return Err(anyhow::anyhow!("Kafka bootstrap servers cannot be empty").into());
        }

        // Validate database URL
        if self.database.url.is_empty() {
            return Err(anyhow::anyhow!("Database URL cannot be empty").into());
        }

        // Validate API configuration
        if self.api.port == 0 {
            return Err(anyhow::anyhow!("API port must be greater than 0").into());
        }

        // Validate security configuration
        if self.security.private_key_path.is_empty() {
            return Err(anyhow::anyhow!("Private key path cannot be empty").into());
        }

        if self.security.public_key_path.is_empty() {
            return Err(anyhow::anyhow!("Public key path cannot be empty").into());
        }

        // Validate performance settings
        if self.performance.transaction_batch_size == 0 {
            return Err(anyhow::anyhow!("Transaction batch size must be greater than 0").into());
        }

        if self.performance.max_concurrent_transactions == 0 {
            return Err(anyhow::anyhow!("Max concurrent transactions must be greater than 0").into());
        }

        Ok(())
    }
}

impl Default for SyncDomainConfig {
    fn default() -> Self {
        Self {
            domain_id: "default-domain".to_string(),
            metadata: DomainMetadata {
                name: "Default Sync Domain".to_string(),
                description: "Default synchronization domain".to_string(),
                region: "global".to_string(),
                jurisdiction: "international".to_string(),
                operator: OperatorInfo {
                    name: "GARP Network".to_string(),
                    contact_email: "admin@garp.network".to_string(),
                    website: Some("https://garp.network".to_string()),
                    public_key: "".to_string(),
                },
                supported_transaction_types: vec![
                    "CreateContract".to_string(),
                    "ExerciseContract".to_string(),
                    "TransferAsset".to_string(),
                ],
                max_participants: Some(1000),
            },
            kafka: KafkaConfig {
                bootstrap_servers: "localhost:9092".to_string(),
                security_protocol: "PLAINTEXT".to_string(),
                sasl_mechanism: None,
                sasl_username: None,
                sasl_password: None,
                ssl: None,
                topics: TopicConfig {
                    transaction_topic: "garp-transactions".to_string(),
                    consensus_topic: "garp-consensus".to_string(),
                    participant_topic: "garp-participants".to_string(),
                    event_topic: "garp-events".to_string(),
                    partitions: 3,
                    replication_factor: 1,
                    retention_ms: Some(604800000), // 7 days
                },
                producer: ProducerConfig {
                    acks: "all".to_string(),
                    retries: 3,
                    batch_size: 16384,
                    linger_ms: 5,
                    buffer_memory: 33554432,
                    compression_type: "snappy".to_string(),
                    request_timeout_ms: 30000,
                },
                consumer: ConsumerConfig {
                    group_id: "sync-domain-consumer".to_string(),
                    auto_offset_reset: "earliest".to_string(),
                    enable_auto_commit: false,
                    auto_commit_interval_ms: 5000,
                    session_timeout_ms: 30000,
                    heartbeat_interval_ms: 3000,
                    max_poll_records: 500,
                    fetch_min_bytes: 1,
                    fetch_max_wait_ms: 500,
                },
            },
            database: DatabaseConfig {
                url: "postgresql://localhost/garp_sync_domain".to_string(),
                max_connections: 10,
                min_connections: 1,
                connect_timeout: 30,
                idle_timeout: 600,
                max_lifetime: 3600,
                enable_logging: false,
                redis: Some(RedisConfig {
                    url: "redis://localhost:6379".to_string(),
                    pool_size: 10,
                    timeout: 5,
                    default_ttl: 3600,
                }),
            },
            api: ApiConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
                tls_enabled: false,
                tls_cert_path: None,
                tls_key_path: None,
                request_timeout: 30,
                max_body_size: 1048576, // 1MB
                rate_limit: RateLimitConfig {
                    enabled: true,
                    requests_per_minute: 100,
                    burst_size: 10,
                },
                cors: CorsConfig {
                    allowed_origins: vec!["*".to_string()],
                    allowed_methods: vec!["GET".to_string(), "POST".to_string(), "PUT".to_string(), "DELETE".to_string()],
                    allowed_headers: vec!["*".to_string()],
                    max_age: 3600,
                },
            },
            consensus: ConsensusConfig {
                algorithm: "raft".to_string(),
                node_id: "node-1".to_string(),
                peers: vec![],
                election_timeout_ms: 5000,
                heartbeat_interval_ms: 1000,
                log_compaction_threshold: 1000,
                snapshot_interval: 100,
            },
            security: SecurityConfig {
                private_key_path: "keys/domain_private.pem".to_string(),
                public_key_path: "keys/domain_public.pem".to_string(),
                trusted_participants: HashMap::new(),
                enable_encryption: true,
                encryption_algorithm: "AES-256-GCM".to_string(),
                key_rotation_interval: 24,
                enable_audit_log: true,
                audit_log_retention_days: 90,
            },
            participants: ParticipantConfig {
                auto_approve: false,
                require_verification: true,
                max_participants: Some(1000),
                onboarding_timeout_hours: 24,
                health_check_interval: 30,
                timeout_threshold: 300,
            },
            performance: PerformanceConfig {
                transaction_batch_size: 100,
                batch_timeout_ms: 1000,
                max_concurrent_transactions: 1000,
                sequencer_buffer_size: 10000,
                enable_compression: true,
                compression_algorithm: "snappy".to_string(),
                transaction_pool_size: 100000,
                enable_parallel_processing: true,
                worker_threads: None,
            },
            monitoring: MonitoringConfig {
                enable_metrics: true,
                metrics_path: "/metrics".to_string(),
                metrics_port: 9090,
                enable_health_checks: true,
                health_check_interval: 30,
                enable_tracing: true,
                tracing_endpoint: None,
                log_level: "info".to_string(),
                structured_logging: true,
            },
        }
    }
}