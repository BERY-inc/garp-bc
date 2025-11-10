use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock, Semaphore};
use tokio::time::{Duration, Instant};
use uuid::Uuid;
use garp_common::{GarpResult, Transaction, TransactionId, ParticipantId};
use crate::storage::{StorageBackend, SequencedTransaction, TransactionMetadata, SequenceStatus};
use crate::config::PerformanceConfig;

/// Transaction sequencer that orders transactions without decrypting them
pub struct TransactionSequencer {
    /// Storage backend
    storage: Arc<dyn StorageBackend>,
    
    /// Performance configuration
    config: PerformanceConfig,
    
    /// Transaction queue
    transaction_queue: Arc<RwLock<VecDeque<PendingTransaction>>>,
    
    /// Batch processor
    batch_processor: Arc<BatchProcessor>,
    
    /// Sequence number generator
    sequence_generator: Arc<RwLock<u64>>,
    
    /// Priority queues for different transaction types
    priority_queues: Arc<RwLock<HashMap<u8, VecDeque<PendingTransaction>>>>,
    
    /// Concurrency limiter
    semaphore: Arc<Semaphore>,
    
    /// Metrics
    metrics: Arc<RwLock<SequencerMetrics>>,
    
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
}

/// Pending transaction awaiting sequencing
#[derive(Debug, Clone)]
pub struct PendingTransaction {
    /// Transaction ID
    pub transaction_id: TransactionId,
    
    /// Encrypted transaction data
    pub encrypted_data: Vec<u8>,
    
    /// Transaction metadata (extracted without decryption)
    pub metadata: TransactionMetadata,
    
    /// Received timestamp
    pub received_at: DateTime<Utc>,
    
    /// Domain ID
    pub domain_id: String,
    
    /// Priority level (0-255, higher = more priority)
    pub priority: u8,
    
    /// Estimated processing time
    pub estimated_processing_time: Duration,
}

/// Batch processor for grouping transactions
pub struct BatchProcessor {
    /// Current batch
    current_batch: Arc<RwLock<TransactionBatch>>,
    
    /// Batch configuration
    config: BatchConfig,
    
    /// Batch timer
    batch_timer: Arc<RwLock<Option<Instant>>>,
}

/// Transaction batch
#[derive(Debug, Clone)]
pub struct TransactionBatch {
    /// Batch ID
    pub batch_id: Uuid,
    
    /// Transactions in batch
    pub transactions: Vec<PendingTransaction>,
    
    /// Batch created at
    pub created_at: DateTime<Utc>,
    
    /// Total size in bytes
    pub total_size: usize,
    
    /// Batch status
    pub status: BatchStatus,
}

/// Batch configuration
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum batch size
    pub max_size: usize,
    
    /// Maximum batch timeout
    pub max_timeout: Duration,
    
    /// Minimum batch size before timeout
    pub min_size: usize,
    
    /// Enable adaptive batching
    pub adaptive: bool,
    
    /// Target batch processing time
    pub target_processing_time: Duration,
}

/// Batch status
#[derive(Debug, Clone, PartialEq)]
pub enum BatchStatus {
    /// Batch is being built
    Building,
    
    /// Batch is ready for processing
    Ready,
    
    /// Batch is being processed
    Processing,
    
    /// Batch is completed
    Completed,
    
    /// Batch failed
    Failed,
}

/// Sequencer metrics
#[derive(Debug, Clone, Default)]
pub struct SequencerMetrics {
    /// Total transactions processed
    pub total_transactions: u64,
    
    /// Transactions per second
    pub tps: f64,
    
    /// Average batch size
    pub avg_batch_size: f64,
    
    /// Average processing time per transaction
    pub avg_processing_time: Duration,
    
    /// Queue depth
    pub queue_depth: usize,
    
    /// Pending batches
    pub pending_batches: usize,
    
    /// Failed transactions
    pub failed_transactions: u64,
    
    /// Last updated
    pub last_updated: DateTime<Utc>,
}

/// Sequencing strategy
#[derive(Debug, Clone)]
pub enum SequencingStrategy {
    /// First-In-First-Out
    Fifo,
    
    /// Priority-based
    Priority,
    
    /// Shortest-Job-First
    Sjf,
    
    /// Weighted Fair Queuing
    Wfq,
    
    /// Adaptive (changes based on load)
    Adaptive,
}

/// Transaction priority calculator
pub trait PriorityCalculator: Send + Sync {
    /// Calculate priority for a transaction
    fn calculate_priority(&self, transaction: &PendingTransaction) -> u8;
}

/// Default priority calculator
pub struct DefaultPriorityCalculator;

impl PriorityCalculator for DefaultPriorityCalculator {
    fn calculate_priority(&self, transaction: &PendingTransaction) -> u8 {
        // Base priority from metadata
        let mut priority = transaction.priority;
        
        // Boost priority for time-sensitive transactions
        if let Some(expires_at) = transaction.metadata.expires_at {
            let time_to_expiry = expires_at.signed_duration_since(Utc::now());
            if time_to_expiry.num_minutes() < 5 {
                priority = priority.saturating_add(50);
            } else if time_to_expiry.num_minutes() < 15 {
                priority = priority.saturating_add(20);
            }
        }
        
        // Boost priority for smaller transactions
        if transaction.metadata.size < 1024 {
            priority = priority.saturating_add(10);
        }
        
        // Boost priority based on transaction type
        match transaction.metadata.transaction_type.as_str() {
            "TransferAsset" => priority.saturating_add(30),
            "CreateContract" => priority.saturating_add(20),
            "ExerciseContract" => priority.saturating_add(25),
            _ => priority,
        }
    }
}

impl TransactionSequencer {
    /// Create new transaction sequencer
    pub async fn new(
        storage: Arc<dyn StorageBackend>,
        config: PerformanceConfig,
    ) -> GarpResult<Self> {
        let batch_config = BatchConfig {
            max_size: config.transaction_batch_size,
            max_timeout: Duration::from_millis(config.batch_timeout_ms),
            min_size: config.transaction_batch_size / 4,
            adaptive: true,
            target_processing_time: Duration::from_millis(100),
        };
        
        let batch_processor = Arc::new(BatchProcessor::new(batch_config));
        let sequence_number = storage.get_next_sequence_number().await?;
        
        Ok(Self {
            storage,
            config,
            transaction_queue: Arc::new(RwLock::new(VecDeque::new())),
            batch_processor,
            sequence_generator: Arc::new(RwLock::new(sequence_number)),
            priority_queues: Arc::new(RwLock::new(HashMap::new())),
            semaphore: Arc::new(Semaphore::new(config.max_concurrent_transactions)),
            metrics: Arc::new(RwLock::new(SequencerMetrics::default())),
            shutdown_tx: None,
        })
    }
    
    /// Start the sequencer
    pub async fn start(&mut self) -> GarpResult<()> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);
        
        // Start transaction processing loop
        let storage = Arc::clone(&self.storage);
        let queue = Arc::clone(&self.transaction_queue);
        let batch_processor = Arc::clone(&self.batch_processor);
        let sequence_generator = Arc::clone(&self.sequence_generator);
        let priority_queues = Arc::clone(&self.priority_queues);
        let semaphore = Arc::clone(&self.semaphore);
        let metrics = Arc::clone(&self.metrics);
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(10));
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = Self::process_transactions(
                            &storage,
                            &queue,
                            &batch_processor,
                            &sequence_generator,
                            &priority_queues,
                            &semaphore,
                            &metrics,
                            &config,
                        ).await {
                            tracing::error!("Error processing transactions: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("Shutting down transaction sequencer");
                        break;
                    }
                }
            }
        });
        
        // Start batch processing loop
        let batch_processor_clone = Arc::clone(&self.batch_processor);
        let storage_clone = Arc::clone(&self.storage);
        let sequence_generator_clone = Arc::clone(&self.sequence_generator);
        let metrics_clone = Arc::clone(&self.metrics);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = batch_processor_clone.process_ready_batches(
                            &storage_clone,
                            &sequence_generator_clone,
                            &metrics_clone,
                        ).await {
                            tracing::error!("Error processing batches: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });
        
        // Start metrics update loop
        let metrics_clone = Arc::clone(&self.metrics);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            
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
    
    /// Stop the sequencer
    pub async fn stop(&mut self) -> GarpResult<()> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()).await;
        }
        Ok(())
    }
    
    /// Submit transaction for sequencing
    pub async fn submit_transaction(&self, transaction: PendingTransaction) -> GarpResult<()> {
        // Calculate priority
        let priority_calculator = DefaultPriorityCalculator;
        let priority = priority_calculator.calculate_priority(&transaction);
        
        let mut transaction = transaction;
        transaction.priority = priority;
        
        // Add to appropriate priority queue
        let mut priority_queues = self.priority_queues.write().await;
        let queue = priority_queues.entry(priority).or_insert_with(VecDeque::new);
        queue.push_back(transaction);
        
        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.queue_depth += 1;
        
        Ok(())
    }
    
    /// Get sequencer metrics
    pub async fn get_metrics(&self) -> SequencerMetrics {
        self.metrics.read().await.clone()
    }
    
    /// Process transactions from queues
    async fn process_transactions(
        storage: &Arc<dyn StorageBackend>,
        queue: &Arc<RwLock<VecDeque<PendingTransaction>>>,
        batch_processor: &Arc<BatchProcessor>,
        sequence_generator: &Arc<RwLock<u64>>,
        priority_queues: &Arc<RwLock<HashMap<u8, VecDeque<PendingTransaction>>>>,
        semaphore: &Arc<Semaphore>,
        metrics: &Arc<RwLock<SequencerMetrics>>,
        config: &PerformanceConfig,
    ) -> GarpResult<()> {
        // Get next transaction from priority queues
        let transaction = {
            let mut priority_queues = priority_queues.write().await;
            
            // Find highest priority non-empty queue
            let mut highest_priority = None;
            for (&priority, queue) in priority_queues.iter() {
                if !queue.is_empty() {
                    if highest_priority.is_none() || priority > highest_priority.unwrap() {
                        highest_priority = Some(priority);
                    }
                }
            }
            
            if let Some(priority) = highest_priority {
                priority_queues.get_mut(&priority).and_then(|queue| queue.pop_front())
            } else {
                None
            }
        };
        
        if let Some(transaction) = transaction {
            // Acquire semaphore permit
            let _permit = semaphore.acquire().await?;
            
            // Add to batch
            batch_processor.add_transaction(transaction).await?;
            
            // Update metrics
            let mut metrics = metrics.write().await;
            metrics.queue_depth = metrics.queue_depth.saturating_sub(1);
        }
        
        Ok(())
    }
}

impl BatchProcessor {
    /// Create new batch processor
    pub fn new(config: BatchConfig) -> Self {
        Self {
            current_batch: Arc::new(RwLock::new(TransactionBatch {
                batch_id: Uuid::new_v4(),
                transactions: Vec::new(),
                created_at: Utc::now(),
                total_size: 0,
                status: BatchStatus::Building,
            })),
            config,
            batch_timer: Arc::new(RwLock::new(None)),
        }
    }
    
    /// Add transaction to current batch
    pub async fn add_transaction(&self, transaction: PendingTransaction) -> GarpResult<()> {
        let mut batch = self.current_batch.write().await;
        
        // Start timer if this is the first transaction
        if batch.transactions.is_empty() {
            let mut timer = self.batch_timer.write().await;
            *timer = Some(Instant::now());
        }
        
        batch.transactions.push(transaction.clone());
        batch.total_size += transaction.metadata.size;
        
        // Check if batch is ready
        let should_finalize = batch.transactions.len() >= self.config.max_size
            || batch.total_size >= self.config.max_size * 1024
            || (batch.transactions.len() >= self.config.min_size && self.is_batch_timeout().await);
        
        if should_finalize {
            batch.status = BatchStatus::Ready;
        }
        
        Ok(())
    }
    
    /// Check if batch has timed out
    async fn is_batch_timeout(&self) -> bool {
        if let Some(timer) = *self.batch_timer.read().await {
            timer.elapsed() >= self.config.max_timeout
        } else {
            false
        }
    }
    
    /// Process ready batches
    pub async fn process_ready_batches(
        &self,
        storage: &Arc<dyn StorageBackend>,
        sequence_generator: &Arc<RwLock<u64>>,
        metrics: &Arc<RwLock<SequencerMetrics>>,
    ) -> GarpResult<()> {
        let should_process = {
            let batch = self.current_batch.read().await;
            batch.status == BatchStatus::Ready || 
            (batch.status == BatchStatus::Building && 
             !batch.transactions.is_empty() && 
             self.is_batch_timeout().await)
        };
        
        if should_process {
            let batch = {
                let mut current_batch = self.current_batch.write().await;
                current_batch.status = BatchStatus::Processing;
                
                // Create new batch for next transactions
                let old_batch = current_batch.clone();
                *current_batch = TransactionBatch {
                    batch_id: Uuid::new_v4(),
                    transactions: Vec::new(),
                    created_at: Utc::now(),
                    total_size: 0,
                    status: BatchStatus::Building,
                };
                
                // Reset timer
                let mut timer = self.batch_timer.write().await;
                *timer = None;
                
                old_batch
            };
            
            // Process the batch
            self.sequence_batch(batch, storage, sequence_generator, metrics).await?;
        }
        
        Ok(())
    }
    
    /// Sequence a batch of transactions
    async fn sequence_batch(
        &self,
        mut batch: TransactionBatch,
        storage: &Arc<dyn StorageBackend>,
        sequence_generator: &Arc<RwLock<u64>>,
        metrics: &Arc<RwLock<SequencerMetrics>>,
    ) -> GarpResult<()> {
        let start_time = Instant::now();
        
        // Sort transactions by priority within the batch
        batch.transactions.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        let mut sequenced_count = 0;
        
        for transaction in &batch.transactions {
            // Get next sequence number
            let sequence_number = {
                let mut seq_gen = sequence_generator.write().await;
                let current = *seq_gen;
                *seq_gen += 1;
                current
            };
            
            // Create sequenced transaction
            let sequenced_transaction = SequencedTransaction {
                sequence_number,
                transaction_id: transaction.transaction_id.clone(),
                encrypted_data: transaction.encrypted_data.clone(),
                metadata: transaction.metadata.clone(),
                sequenced_at: Utc::now(),
                domain_id: transaction.domain_id.clone(),
                batch_id: Some(batch.batch_id),
                status: SequenceStatus::Sequenced,
            };
            
            // Store in database
            storage.store_sequenced_transaction(&sequenced_transaction).await?;
            storage.increment_transaction_count().await?;
            
            sequenced_count += 1;
        }
        
        // Update batch status
        batch.status = BatchStatus::Completed;
        
        // Update metrics
        let processing_time = start_time.elapsed();
        let mut metrics = metrics.write().await;
        metrics.total_transactions += sequenced_count;
        metrics.avg_batch_size = (metrics.avg_batch_size + sequenced_count as f64) / 2.0;
        metrics.avg_processing_time = (metrics.avg_processing_time + processing_time) / 2;
        
        // Calculate TPS
        if processing_time.as_secs_f64() > 0.0 {
            let batch_tps = sequenced_count as f64 / processing_time.as_secs_f64();
            metrics.tps = (metrics.tps + batch_tps) / 2.0;
        }
        
        tracing::info!(
            "Sequenced batch {} with {} transactions in {:?}",
            batch.batch_id,
            sequenced_count,
            processing_time
        );
        
        Ok(())
    }
}

/// Sequencer factory
pub struct SequencerFactory;

impl SequencerFactory {
    /// Create transaction sequencer
    pub async fn create_sequencer(
        storage: Arc<dyn StorageBackend>,
        config: PerformanceConfig,
    ) -> GarpResult<TransactionSequencer> {
        TransactionSequencer::new(storage, config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemoryStorage;
    
    #[tokio::test]
    async fn test_transaction_sequencing() {
        let storage = Arc::new(MemoryStorage::new()) as Arc<dyn StorageBackend>;
        let config = PerformanceConfig {
            transaction_batch_size: 10,
            batch_timeout_ms: 1000,
            max_concurrent_transactions: 100,
            sequencer_buffer_size: 1000,
            enable_compression: false,
            compression_algorithm: "none".to_string(),
            transaction_pool_size: 1000,
            enable_parallel_processing: true,
            worker_threads: None,
        };
        
        let mut sequencer = TransactionSequencer::new(storage, config).await.unwrap();
        sequencer.start().await.unwrap();
        
        // Submit test transaction
        let transaction = PendingTransaction {
            transaction_id: "test-tx-1".to_string(),
            encrypted_data: vec![1, 2, 3, 4],
            metadata: TransactionMetadata {
                participants: vec!["participant-1".to_string()],
                transaction_type: "TransferAsset".to_string(),
                priority: 100,
                size: 4,
                hash: "test-hash".to_string(),
                dependencies: vec![],
                expires_at: None,
            },
            received_at: Utc::now(),
            domain_id: "test-domain".to_string(),
            priority: 100,
            estimated_processing_time: Duration::from_millis(10),
        };
        
        sequencer.submit_transaction(transaction).await.unwrap();
        
        // Wait for processing
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let metrics = sequencer.get_metrics().await;
        assert!(metrics.total_transactions > 0);
        
        sequencer.stop().await.unwrap();
    }
    
    #[tokio::test]
    async fn test_priority_calculation() {
        let calculator = DefaultPriorityCalculator;
        
        let transaction = PendingTransaction {
            transaction_id: "test-tx-1".to_string(),
            encrypted_data: vec![1, 2, 3, 4],
            metadata: TransactionMetadata {
                participants: vec!["participant-1".to_string()],
                transaction_type: "TransferAsset".to_string(),
                priority: 50,
                size: 512,
                hash: "test-hash".to_string(),
                dependencies: vec![],
                expires_at: Some(Utc::now() + chrono::Duration::minutes(3)),
            },
            received_at: Utc::now(),
            domain_id: "test-domain".to_string(),
            priority: 50,
            estimated_processing_time: Duration::from_millis(10),
        };
        
        let priority = calculator.calculate_priority(&transaction);
        
        // Should boost priority for urgent expiry, small size, and TransferAsset type
        assert!(priority > 50);
    }
}