use async_trait::async_trait;
use rdkafka::{
    config::ClientConfig,
    consumer::{Consumer, StreamConsumer},
    message::{BorrowedMessage, Message},
    producer::{FutureProducer, FutureRecord},
    util::Timeout,
    ClientContext, TopicPartitionList,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use garp_common::{GarpResult, TransactionId, ParticipantId};
use crate::config::{KafkaConfig, TopicConfig};
use crate::storage::SequencedTransaction;

/// Kafka message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum KafkaMessage {
    /// Transaction submitted for sequencing
    TransactionSubmitted {
        transaction_id: TransactionId,
        encrypted_data: Vec<u8>,
        participants: Vec<ParticipantId>,
        domain_id: String,
        timestamp: DateTime<Utc>,
    },
    
    /// Transaction sequenced
    TransactionSequenced {
        sequence_number: u64,
        transaction_id: TransactionId,
        batch_id: Option<Uuid>,
        domain_id: String,
        timestamp: DateTime<Utc>,
    },
    
    /// Consensus vote
    ConsensusVote {
        transaction_id: TransactionId,
        participant_id: ParticipantId,
        vote: bool,
        reason: Option<String>,
        signature: String,
        timestamp: DateTime<Utc>,
    },
    
    /// Consensus result
    ConsensusResult {
        transaction_id: TransactionId,
        result: ConsensusOutcome,
        timestamp: DateTime<Utc>,
    },
    
    /// Participant joined domain
    ParticipantJoined {
        participant_id: ParticipantId,
        domain_id: String,
        public_key: String,
        endpoint: String,
        timestamp: DateTime<Utc>,
    },
    
    /// Participant left domain
    ParticipantLeft {
        participant_id: ParticipantId,
        domain_id: String,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    
    /// Domain event
    DomainEvent {
        event_id: Uuid,
        domain_id: String,
        event_type: String,
        data: serde_json::Value,
        timestamp: DateTime<Utc>,
    },
    
    /// Health check ping
    HealthPing {
        node_id: String,
        domain_id: String,
        timestamp: DateTime<Utc>,
    },
    
    /// Batch completed
    BatchCompleted {
        batch_id: Uuid,
        transaction_count: usize,
        domain_id: String,
        timestamp: DateTime<Utc>,
    },
}

/// Consensus outcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusOutcome {
    Approved,
    Rejected { reason: String },
    Timeout,
}

/// Message handler trait
#[async_trait]
pub trait MessageHandler: Send + Sync {
    /// Handle incoming message
    async fn handle_message(&self, message: KafkaMessage) -> GarpResult<()>;
    
    /// Get handler name
    fn name(&self) -> &str;
}

/// Kafka client wrapper
pub struct KafkaClient {
    /// Producer for sending messages
    producer: FutureProducer,
    
    /// Consumer for receiving messages
    consumer: StreamConsumer,
    
    /// Configuration
    config: KafkaConfig,
    
    /// Message handlers
    handlers: Arc<RwLock<HashMap<String, Arc<dyn MessageHandler>>>>,
    
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
    
    /// Metrics
    metrics: Arc<RwLock<KafkaMetrics>>,
}

/// Kafka metrics
#[derive(Debug, Clone, Default)]
pub struct KafkaMetrics {
    /// Messages sent
    pub messages_sent: u64,
    
    /// Messages received
    pub messages_received: u64,
    
    /// Messages failed
    pub messages_failed: u64,
    
    /// Average send latency
    pub avg_send_latency: Duration,
    
    /// Average receive latency
    pub avg_receive_latency: Duration,
    
    /// Consumer lag
    pub consumer_lag: i64,
    
    /// Last updated
    pub last_updated: DateTime<Utc>,
}

/// Custom client context for metrics
struct MetricsContext {
    metrics: Arc<RwLock<KafkaMetrics>>,
}

impl ClientContext for MetricsContext {}

impl KafkaClient {
    /// Create new Kafka client
    pub async fn new(config: KafkaConfig) -> GarpResult<Self> {
        let metrics = Arc::new(RwLock::new(KafkaMetrics::default()));
        
        // Create producer configuration
        let mut producer_config = ClientConfig::new();
        producer_config
            .set("bootstrap.servers", &config.bootstrap_servers)
            .set("security.protocol", &config.security_protocol)
            .set("acks", &config.producer.acks)
            .set("retries", &config.producer.retries.to_string())
            .set("batch.size", &config.producer.batch_size.to_string())
            .set("linger.ms", &config.producer.linger_ms.to_string())
            .set("buffer.memory", &config.producer.buffer_memory.to_string())
            .set("compression.type", &config.producer.compression_type)
            .set("request.timeout.ms", &config.producer.request_timeout_ms.to_string());
        
        // Add SASL configuration if provided
        if let Some(mechanism) = &config.sasl_mechanism {
            producer_config.set("sasl.mechanism", mechanism);
        }
        if let Some(username) = &config.sasl_username {
            producer_config.set("sasl.username", username);
        }
        if let Some(password) = &config.sasl_password {
            producer_config.set("sasl.password", password);
        }
        
        // Add SSL configuration if provided
        if let Some(ssl) = &config.ssl {
            if let Some(ca_location) = &ssl.ca_location {
                producer_config.set("ssl.ca.location", ca_location);
            }
            if let Some(cert_location) = &ssl.certificate_location {
                producer_config.set("ssl.certificate.location", cert_location);
            }
            if let Some(key_location) = &ssl.key_location {
                producer_config.set("ssl.key.location", key_location);
            }
            if let Some(key_password) = &ssl.key_password {
                producer_config.set("ssl.key.password", key_password);
            }
        }
        
        // Create producer
        let producer: FutureProducer = producer_config.create()?;
        
        // Create consumer configuration
        let mut consumer_config = ClientConfig::new();
        consumer_config
            .set("bootstrap.servers", &config.bootstrap_servers)
            .set("security.protocol", &config.security_protocol)
            .set("group.id", &config.consumer.group_id)
            .set("auto.offset.reset", &config.consumer.auto_offset_reset)
            .set("enable.auto.commit", &config.consumer.enable_auto_commit.to_string())
            .set("auto.commit.interval.ms", &config.consumer.auto_commit_interval_ms.to_string())
            .set("session.timeout.ms", &config.consumer.session_timeout_ms.to_string())
            .set("heartbeat.interval.ms", &config.consumer.heartbeat_interval_ms.to_string())
            .set("max.poll.records", &config.consumer.max_poll_records.to_string())
            .set("fetch.min.bytes", &config.consumer.fetch_min_bytes.to_string())
            .set("fetch.max.wait.ms", &config.consumer.fetch_max_wait_ms.to_string());
        
        // Add SASL and SSL configuration for consumer
        if let Some(mechanism) = &config.sasl_mechanism {
            consumer_config.set("sasl.mechanism", mechanism);
        }
        if let Some(username) = &config.sasl_username {
            consumer_config.set("sasl.username", username);
        }
        if let Some(password) = &config.sasl_password {
            consumer_config.set("sasl.password", password);
        }
        
        if let Some(ssl) = &config.ssl {
            if let Some(ca_location) = &ssl.ca_location {
                consumer_config.set("ssl.ca.location", ca_location);
            }
            if let Some(cert_location) = &ssl.certificate_location {
                consumer_config.set("ssl.certificate.location", cert_location);
            }
            if let Some(key_location) = &ssl.key_location {
                consumer_config.set("ssl.key.location", key_location);
            }
            if let Some(key_password) = &ssl.key_password {
                consumer_config.set("ssl.key.password", key_password);
            }
        }
        
        // Create consumer with metrics context
        let context = MetricsContext {
            metrics: Arc::clone(&metrics),
        };
        let consumer: StreamConsumer<MetricsContext> = consumer_config.create_with_context(context)?;
        
        Ok(Self {
            producer,
            consumer,
            config,
            handlers: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: None,
            metrics,
        })
    }
    
    /// Start the Kafka client
    pub async fn start(&mut self) -> GarpResult<()> {
        // Subscribe to topics
        let topics = vec![
            self.config.topics.transaction_topic.as_str(),
            self.config.topics.consensus_topic.as_str(),
            self.config.topics.participant_topic.as_str(),
            self.config.topics.event_topic.as_str(),
        ];
        
        self.consumer.subscribe(&topics)?;
        
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);
        
        // Start message consumption loop
        let consumer = self.consumer.clone();
        let handlers = Arc::clone(&self.handlers);
        let metrics = Arc::clone(&self.metrics);
        
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    message_result = consumer.recv() => {
                        match message_result {
                            Ok(message) => {
                                if let Err(e) = Self::handle_received_message(&message, &handlers, &metrics).await {
                                    tracing::error!("Error handling message: {}", e);
                                }
                            }
                            Err(e) => {
                                tracing::error!("Error receiving message: {}", e);
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("Shutting down Kafka consumer");
                        break;
                    }
                }
            }
        });
        
        // Start metrics update loop
        let metrics_clone = Arc::clone(&self.metrics);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let mut metrics = metrics_clone.write().await;
                        metrics.last_updated = Utc::now();
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Stop the Kafka client
    pub async fn stop(&mut self) -> GarpResult<()> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()).await;
        }
        Ok(())
    }
    
    /// Send message to topic
    pub async fn send_message(&self, topic: &str, message: &KafkaMessage) -> GarpResult<()> {
        let start_time = std::time::Instant::now();
        
        // Serialize message
        let payload = serde_json::to_vec(message)?;
        
        // Create record
        let record = FutureRecord::to(topic)
            .payload(&payload)
            .key(&self.generate_message_key(message));
        
        // Send message
        let result = self.producer.send(record, Timeout::After(Duration::from_secs(30))).await;
        
        // Update metrics
        let send_latency = start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        
        match result {
            Ok(_) => {
                metrics.messages_sent += 1;
                metrics.avg_send_latency = (metrics.avg_send_latency + send_latency) / 2;
            }
            Err(_) => {
                metrics.messages_failed += 1;
            }
        }
        
        result.map_err(|e| anyhow::anyhow!("Failed to send message: {:?}", e))?;
        
        Ok(())
    }
    
    /// Register message handler
    pub async fn register_handler(&self, handler: Arc<dyn MessageHandler>) {
        let mut handlers = self.handlers.write().await;
        handlers.insert(handler.name().to_string(), handler);
    }
    
    /// Unregister message handler
    pub async fn unregister_handler(&self, name: &str) {
        let mut handlers = self.handlers.write().await;
        handlers.remove(name);
    }
    
    /// Get Kafka metrics
    pub async fn get_metrics(&self) -> KafkaMetrics {
        self.metrics.read().await.clone()
    }
    
    /// Generate message key for partitioning
    fn generate_message_key(&self, message: &KafkaMessage) -> String {
        match message {
            KafkaMessage::TransactionSubmitted { transaction_id, .. } => transaction_id.clone(),
            KafkaMessage::TransactionSequenced { transaction_id, .. } => transaction_id.clone(),
            KafkaMessage::ConsensusVote { transaction_id, .. } => transaction_id.clone(),
            KafkaMessage::ConsensusResult { transaction_id, .. } => transaction_id.clone(),
            KafkaMessage::ParticipantJoined { participant_id, .. } => participant_id.clone(),
            KafkaMessage::ParticipantLeft { participant_id, .. } => participant_id.clone(),
            KafkaMessage::DomainEvent { domain_id, .. } => domain_id.clone(),
            KafkaMessage::HealthPing { node_id, .. } => node_id.clone(),
            KafkaMessage::BatchCompleted { batch_id, .. } => batch_id.to_string(),
        }
    }
    
    /// Handle received message
    async fn handle_received_message(
        message: &BorrowedMessage<'_>,
        handlers: &Arc<RwLock<HashMap<String, Arc<dyn MessageHandler>>>>,
        metrics: &Arc<RwLock<KafkaMetrics>>,
    ) -> GarpResult<()> {
        let start_time = std::time::Instant::now();
        
        // Deserialize message
        let payload = message.payload().ok_or_else(|| anyhow::anyhow!("Empty message payload"))?;
        let kafka_message: KafkaMessage = serde_json::from_slice(payload)?;
        
        // Get appropriate handler
        let handlers = handlers.read().await;
        let handler_name = Self::get_handler_name(&kafka_message);
        
        if let Some(handler) = handlers.get(&handler_name) {
            // Handle message
            handler.handle_message(kafka_message).await?;
        } else {
            tracing::warn!("No handler found for message type: {}", handler_name);
        }
        
        // Update metrics
        let receive_latency = start_time.elapsed();
        let mut metrics = metrics.write().await;
        metrics.messages_received += 1;
        metrics.avg_receive_latency = (metrics.avg_receive_latency + receive_latency) / 2;
        
        Ok(())
    }
    
    /// Get handler name for message type
    fn get_handler_name(message: &KafkaMessage) -> String {
        match message {
            KafkaMessage::TransactionSubmitted { .. } => "transaction_handler".to_string(),
            KafkaMessage::TransactionSequenced { .. } => "sequencer_handler".to_string(),
            KafkaMessage::ConsensusVote { .. } => "consensus_handler".to_string(),
            KafkaMessage::ConsensusResult { .. } => "consensus_handler".to_string(),
            KafkaMessage::ParticipantJoined { .. } => "participant_handler".to_string(),
            KafkaMessage::ParticipantLeft { .. } => "participant_handler".to_string(),
            KafkaMessage::DomainEvent { .. } => "event_handler".to_string(),
            KafkaMessage::HealthPing { .. } => "health_handler".to_string(),
            KafkaMessage::BatchCompleted { .. } => "batch_handler".to_string(),
        }
    }
    
    /// Send transaction submitted message
    pub async fn send_transaction_submitted(
        &self,
        transaction_id: TransactionId,
        encrypted_data: Vec<u8>,
        participants: Vec<ParticipantId>,
        domain_id: String,
    ) -> GarpResult<()> {
        let message = KafkaMessage::TransactionSubmitted {
            transaction_id,
            encrypted_data,
            participants,
            domain_id,
            timestamp: Utc::now(),
        };
        
        self.send_message(&self.config.topics.transaction_topic, &message).await
    }
    
    /// Send transaction sequenced message
    pub async fn send_transaction_sequenced(
        &self,
        sequenced_transaction: &SequencedTransaction,
    ) -> GarpResult<()> {
        let message = KafkaMessage::TransactionSequenced {
            sequence_number: sequenced_transaction.sequence_number,
            transaction_id: sequenced_transaction.transaction_id.clone(),
            batch_id: sequenced_transaction.batch_id,
            domain_id: sequenced_transaction.domain_id.clone(),
            timestamp: Utc::now(),
        };
        
        self.send_message(&self.config.topics.transaction_topic, &message).await
    }
    
    /// Send consensus vote message
    pub async fn send_consensus_vote(
        &self,
        transaction_id: TransactionId,
        participant_id: ParticipantId,
        vote: bool,
        reason: Option<String>,
        signature: String,
    ) -> GarpResult<()> {
        let message = KafkaMessage::ConsensusVote {
            transaction_id,
            participant_id,
            vote,
            reason,
            signature,
            timestamp: Utc::now(),
        };
        
        self.send_message(&self.config.topics.consensus_topic, &message).await
    }
    
    /// Send consensus result message
    pub async fn send_consensus_result(
        &self,
        transaction_id: TransactionId,
        result: ConsensusOutcome,
    ) -> GarpResult<()> {
        let message = KafkaMessage::ConsensusResult {
            transaction_id,
            result,
            timestamp: Utc::now(),
        };
        
        self.send_message(&self.config.topics.consensus_topic, &message).await
    }
    
    /// Send participant joined message
    pub async fn send_participant_joined(
        &self,
        participant_id: ParticipantId,
        domain_id: String,
        public_key: String,
        endpoint: String,
    ) -> GarpResult<()> {
        let message = KafkaMessage::ParticipantJoined {
            participant_id,
            domain_id,
            public_key,
            endpoint,
            timestamp: Utc::now(),
        };
        
        self.send_message(&self.config.topics.participant_topic, &message).await
    }
    
    /// Send participant left message
    pub async fn send_participant_left(
        &self,
        participant_id: ParticipantId,
        domain_id: String,
        reason: String,
    ) -> GarpResult<()> {
        let message = KafkaMessage::ParticipantLeft {
            participant_id,
            domain_id,
            reason,
            timestamp: Utc::now(),
        };
        
        self.send_message(&self.config.topics.participant_topic, &message).await
    }
    
    /// Send domain event message
    pub async fn send_domain_event(
        &self,
        domain_id: String,
        event_type: String,
        data: serde_json::Value,
    ) -> GarpResult<()> {
        let message = KafkaMessage::DomainEvent {
            event_id: Uuid::new_v4(),
            domain_id,
            event_type,
            data,
            timestamp: Utc::now(),
        };
        
        self.send_message(&self.config.topics.event_topic, &message).await
    }
    
    /// Send health ping message
    pub async fn send_health_ping(&self, node_id: String, domain_id: String) -> GarpResult<()> {
        let message = KafkaMessage::HealthPing {
            node_id,
            domain_id,
            timestamp: Utc::now(),
        };
        
        self.send_message(&self.config.topics.event_topic, &message).await
    }
    
    /// Send batch completed message
    pub async fn send_batch_completed(
        &self,
        batch_id: Uuid,
        transaction_count: usize,
        domain_id: String,
    ) -> GarpResult<()> {
        let message = KafkaMessage::BatchCompleted {
            batch_id,
            transaction_count,
            domain_id,
            timestamp: Utc::now(),
        };
        
        self.send_message(&self.config.topics.event_topic, &message).await
    }
}

/// Topic manager for creating and managing Kafka topics
pub struct TopicManager {
    config: KafkaConfig,
}

impl TopicManager {
    /// Create new topic manager
    pub fn new(config: KafkaConfig) -> Self {
        Self { config }
    }
    
    /// Create all required topics
    pub async fn create_topics(&self) -> GarpResult<()> {
        // This would typically use Kafka Admin API to create topics
        // For now, we'll assume topics are created externally
        tracing::info!("Topics should be created externally:");
        tracing::info!("  - {}", self.config.topics.transaction_topic);
        tracing::info!("  - {}", self.config.topics.consensus_topic);
        tracing::info!("  - {}", self.config.topics.participant_topic);
        tracing::info!("  - {}", self.config.topics.event_topic);
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    struct TestHandler {
        name: String,
        message_count: Arc<AtomicUsize>,
    }
    
    impl TestHandler {
        fn new(name: String) -> Self {
            Self {
                name,
                message_count: Arc::new(AtomicUsize::new(0)),
            }
        }
        
        fn get_message_count(&self) -> usize {
            self.message_count.load(Ordering::Relaxed)
        }
    }
    
    #[async_trait]
    impl MessageHandler for TestHandler {
        async fn handle_message(&self, _message: KafkaMessage) -> GarpResult<()> {
            self.message_count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        
        fn name(&self) -> &str {
            &self.name
        }
    }
    
    #[tokio::test]
    async fn test_message_key_generation() {
        let config = KafkaConfig::default();
        let client = KafkaClient::new(config).await.unwrap();
        
        let message = KafkaMessage::TransactionSubmitted {
            transaction_id: "test-tx-1".to_string(),
            encrypted_data: vec![1, 2, 3],
            participants: vec!["participant-1".to_string()],
            domain_id: "test-domain".to_string(),
            timestamp: Utc::now(),
        };
        
        let key = client.generate_message_key(&message);
        assert_eq!(key, "test-tx-1");
    }
    
    #[test]
    fn test_handler_name_mapping() {
        let message = KafkaMessage::ConsensusVote {
            transaction_id: "test-tx-1".to_string(),
            participant_id: "participant-1".to_string(),
            vote: true,
            reason: None,
            signature: "signature".to_string(),
            timestamp: Utc::now(),
        };
        
        let handler_name = KafkaClient::get_handler_name(&message);
        assert_eq!(handler_name, "consensus_handler");
    }
}

impl Default for KafkaConfig {
    fn default() -> Self {
        Self {
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
            producer: crate::config::ProducerConfig {
                acks: "all".to_string(),
                retries: 3,
                batch_size: 16384,
                linger_ms: 5,
                buffer_memory: 33554432,
                compression_type: "snappy".to_string(),
                request_timeout_ms: 30000,
            },
            consumer: crate::config::ConsumerConfig {
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
        }
    }
}