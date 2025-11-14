use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use axum::middleware;
use tower::limit::ConcurrencyLimitLayer;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
    limit::RequestBodyLimitLayer,
};
use tracing::{info, warn, error};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use garp_common::{
    ParticipantId, ContractId, TransactionId, AssetId,
    Transaction, Contract, Asset, WalletBalance,
    TransactionCommand, CreateContractCommand, ExerciseContractCommand,
    ArchiveContractCommand, TransferAssetCommand, CreateAssetCommand,
    GarpResult, GarpError,
};
use crate::{
    node::ParticipantNode,
    config::ApiConfig,
    eth_compatibility::EthCompatibilityLayer,
};
use crate::merkle::{merkle_proof, merkle_root, MerkleProof};

/// API server for participant node
pub struct ApiServer {
    node: Arc<ParticipantNode>,
    config: ApiConfig,
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Transaction submission request
#[derive(Debug, Deserialize)]
pub struct SubmitTransactionRequest {
    pub command: TransactionCommandDto,
    pub metadata: Option<serde_json::Value>,
}

/// Transaction command DTO
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum TransactionCommandDto {
    CreateContract {
        template_id: String,
        signatories: Vec<String>,
        observers: Vec<String>,
        argument: serde_json::Value,
    },
    ExerciseContract {
        contract_id: String,
        choice: String,
        argument: serde_json::Value,
    },
    ArchiveContract {
        contract_id: String,
    },
    TransferAsset {
        from: String,
        to: String,
        asset_id: String,
        amount: f64,
    },
    CreateAsset {
        asset_type: String,
        initial_owner: String,
        metadata: serde_json::Value,
    },
}

/// Contract creation request
#[derive(Debug, Deserialize)]
pub struct CreateContractRequest {
    pub template_id: String,
    pub signatories: Vec<String>,
    pub observers: Vec<String>,
    pub argument: serde_json::Value,
}

/// Contract exercise request
#[derive(Debug, Deserialize)]
pub struct ExerciseContractRequest {
    pub choice: String,
    pub argument: serde_json::Value,
}

/// Asset transfer request
#[derive(Debug, Deserialize)]
pub struct TransferAssetRequest {
    pub to: String,
    pub amount: f64,
}

/// Asset creation request
#[derive(Debug, Deserialize)]
pub struct CreateAssetRequest {
    pub asset_type: String,
    pub initial_owner: String,
    pub metadata: serde_json::Value,
}

/// Query parameters for listing transactions
#[derive(Debug, Deserialize)]
pub struct TransactionQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub participant_id: Option<String>,
    pub contract_id: Option<String>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
}

/// Query parameters for listing contracts
#[derive(Debug, Deserialize)]
pub struct ContractQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub template_id: Option<String>,
    pub signatory: Option<String>,
    pub observer: Option<String>,
    pub archived: Option<bool>,
}

/// Query parameters for wallet balances
#[derive(Debug, Deserialize)]
pub struct WalletQuery {
    pub asset_type: Option<String>,
}

/// Transaction response DTO
#[derive(Debug, Serialize)]
pub struct TransactionDto {
    pub id: String,
    pub command: TransactionCommandDto,
    pub submitter: String,
    pub timestamp: DateTime<Utc>,
    pub status: String,
    pub metadata: Option<serde_json::Value>,
}

/// Contract response DTO
#[derive(Debug, Serialize)]
pub struct ContractDto {
    pub id: String,
    pub template_id: String,
    pub signatories: Vec<String>,
    pub observers: Vec<String>,
    pub argument: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub archived: bool,
}

/// Asset response DTO
#[derive(Debug, Serialize)]
pub struct AssetDto {
    pub id: String,
    pub asset_type: String,
    pub owner: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Wallet balance response DTO
#[derive(Debug, Serialize)]
pub struct WalletBalanceDto {
    pub participant_id: String,
    pub asset_id: String,
    pub balance: f64,
    pub last_updated: DateTime<Utc>,
}

/// Node status response
#[derive(Debug, Serialize)]
pub struct NodeStatusDto {
    pub participant_id: String,
    pub status: String,
    pub version: String,
    pub uptime: u64,
    pub connected_peers: usize,
    pub sync_domains: Vec<String>,
    pub global_head_height: u64,
    pub global_head_hash: String,
    pub in_sync: bool,
    pub sync_lag: u64,
    pub sync_last_applied_height: u64,
    pub sync_last_applied_time: Option<DateTime<Utc>>,
}

/// Block info DTO (synthetic for participant view)
#[derive(Debug, Serialize)]
pub struct BlockInfoDto {
    pub number: u64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: DateTime<Utc>,
    pub transaction_count: u32,
    pub size: u64,
    pub gas_used: u64,
    pub gas_limit: u64,
}

/// Block details DTO
#[derive(Debug, Serialize)]
pub struct BlockDetailsDto {
    pub info: BlockInfoDto,
    pub transactions: Vec<TransactionDto>,
}

/// Block summary DTO
#[derive(Debug, Serialize)]
pub struct BlockSummaryDto {
    pub number: u64,
    pub hash: String,
    pub epoch: u64,
    pub proposer: String,
    pub timestamp: DateTime<Utc>,
    pub transaction_count: u32,
}

/// Query parameters for listing blocks
#[derive(Debug, Deserialize)]
pub struct BlockListQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub epoch: Option<u64>,
    pub proposer: Option<String>,
}

/// Merkle proof DTO for transaction inclusion
#[derive(Debug, Serialize)]
pub struct MerkleProofDto {
    pub block_hash: String,
    pub tx_id: String,
    pub leaf_hash: String,
    pub root: String,
    pub path: Vec<String>,
    pub directions: Vec<String>, // "left" or "right"
    pub valid: bool,
}

/// Node statistics response
#[derive(Debug, Serialize)]
pub struct NodeStatsDto {
    pub total_transactions: u64,
    pub total_contracts: u64,
    pub active_contracts: u64,
    pub total_assets: u64,
    pub wallet_balances: u64,
    pub network_peers: u64,
}

/// Contract event DTO
#[derive(Debug, Serialize)]
pub struct ContractEventDto {
    pub id: String,
    pub contract_id: String,
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub emitter: String,
}

/// Query parameters for listing events
#[derive(Debug, Deserialize)]
pub struct EventQueryParams {
    pub contract_id: Option<String>,
    pub event_type: Option<String>,
    pub participant_id: Option<String>,
    pub from_timestamp: Option<DateTime<Utc>>,
    pub to_timestamp: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
}

impl ApiServer {
    /// Create new API server
    pub fn new(node: Arc<ParticipantNode>, config: ApiConfig) -> Self {
        Self { node, config }
    }

    /// Create router with all endpoints
    pub fn create_router(&self) -> Router {
        let eth_layer = EthCompatibilityLayer::new(self.node.clone());
        
        Router::new()
            // JSON-RPC
            .route("/rpc", post(json_rpc))
            // Ethereum compatibility JSON-RPC
            .route("/eth", post(eth_json_rpc))
            // Transaction endpoints
            .route("/api/v1/transactions", post(submit_transaction))
            .route("/api/v1/transactions", get(list_transactions))
            .route("/api/v1/transactions/:id", get(get_transaction))
            .route("/api/v1/transactions/simulate", post(simulate_transaction_v2))
            // Block endpoints (synthetic)
            .route("/api/v1/blocks/latest", get(get_latest_block))
            .route("/api/v1/blocks/:number", get(get_block_by_number))
            .route("/api/v1/blocks/hash/:hash", get(get_block_by_hash))
            .route("/api/v1/blocks", get(list_blocks))
            .route("/api/v1/blocks/:number/summary", get(get_block_summary))
            .route("/api/v1/blocks/:number/tx/:tx_id/proof", get(get_tx_inclusion_proof))
            .route("/api/v1/blocks/:number/state/:state_key/proof", get(get_state_change_proof))
            
            // Contract endpoints
            .route("/api/v1/contracts", post(create_contract))
            .route("/api/v1/contracts", get(list_contracts))
            .route("/api/v1/contracts/:id", get(get_contract))
            .route("/api/v1/contracts/:id/exercise", post(exercise_contract))
            .route("/api/v1/contracts/:id/archive", delete(archive_contract))
            
            // Asset endpoints
            .route("/api/v1/assets", post(create_asset))
            .route("/api/v1/assets", get(list_assets))
            .route("/api/v1/assets/:id", get(get_asset))
            .route("/api/v1/assets/:id/transfer", post(transfer_asset))
            
            // Wallet endpoints
            .route("/api/v1/wallet/balances", get(get_wallet_balances))
            .route("/api/v1/wallet/history", get(get_wallet_history))
            
            // Event endpoints
            .route("/api/v1/events", get(list_events))
            .route("/api/v1/events/contract/:contract_id", get(get_contract_events))
            .route("/api/v1/events/participant/:participant_id", get(get_participant_events))
            
            // Node endpoints
            .route("/api/v1/node/status", get(get_node_status))
            .route("/api/v1/node/stats", get(get_node_stats))
            .route("/api/v1/node/peers", get(get_node_peers))
            // Ledger checkpoint endpoint
            .route("/api/v1/ledger/checkpoint", get(get_ledger_checkpoint))
            // Mempool endpoints
            .route("/api/v1/mempool/submit", post(submit_mempool))
            .route("/api/v1/mempool/stats", get(get_mempool_stats))
            
            // Template endpoints
            .route("/api/v1/templates", get(list_templates))
            .route("/api/v1/templates/:id", get(get_template))
            
            // Health check
            .route("/health", get(health_check))
            
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(CorsLayer::permissive())
                    .layer(RequestBodyLimitLayer::new(1024 * 1024)) // 1MB limit
            )
            .layer(middleware::from_fn(auth_middleware))
            .layer(ConcurrencyLimitLayer::new(64))
            .with_state(self.node.clone())
    }

    /// Start the API server
    pub async fn start(&self) -> GarpResult<()> {
        let app = self.create_router();
        let addr = format!("{}:{}", self.config.host, self.config.port);
        
        info!("Starting API server on {}", addr);
        
        let listener = tokio::net::TcpListener::bind(&addr).await
            .map_err(|e| GarpError::NetworkError(format!("Failed to bind to {}: {}", addr, e)))?;
        
        axum::serve(listener, app).await
            .map_err(|e| GarpError::NetworkError(format!("Server error: {}", e)))?;
        
        Ok(())
    }
}

// API handlers

/// Submit a transaction
async fn submit_transaction(
    State(node): State<Arc<ParticipantNode>>,
    Json(request): Json<SubmitTransactionRequest>,
) -> Result<Json<ApiResponse<TransactionDto>>, StatusCode> {
    info!("Submitting transaction: {:?}", request.command);

    let command = match convert_transaction_command(request.command) {
        Ok(cmd) => cmd,
        Err(e) => {
            return Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }));
        }
    };

    let transaction = Transaction {
        id: TransactionId(Uuid::new_v4()),
        submitter: node.get_participant_id().clone(),
        command,
        created_at: Utc::now(),
        signatures: Vec::new(),
        encrypted_payload: None,
    };

    match node.submit_transaction(transaction.clone()).await {
        Ok(_) => {
            let dto = convert_transaction_to_dto(&transaction);
            Ok(Json(ApiResponse {
                success: true,
                data: Some(dto),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to submit transaction: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// List transactions
async fn list_transactions(
    State(node): State<Arc<ParticipantNode>>,
    Query(query): Query<TransactionQuery>,
) -> Result<Json<ApiResponse<Vec<TransactionDto>>>, StatusCode> {
    let limit = query.limit.unwrap_or(50).min(1000);
    let offset = query.offset.unwrap_or(0);

    match node.get_ledger_view().await {
        Ok(view) => {
            let transactions: Vec<TransactionDto> = view.transactions
                .into_iter()
                .skip(offset)
                .take(limit)
                .map(|tx| convert_transaction_to_dto(&tx))
                .collect();

            Ok(Json(ApiResponse {
                success: true,
                data: Some(transactions),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to list transactions: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Get a specific transaction
async fn get_transaction(
    State(node): State<Arc<ParticipantNode>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<TransactionDto>>, StatusCode> {
    let transaction_id = match Uuid::parse_str(&id) {
        Ok(uuid) => TransactionId(uuid),
        Err(_) => {
            return Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Invalid transaction ID".to_string()),
                timestamp: Utc::now(),
            }));
        }
    };

    match node.get_ledger_view().await {
        Ok(view) => {
            if let Some(transaction) = view.transactions.iter().find(|tx| tx.id == transaction_id) {
                let dto = convert_transaction_to_dto(transaction);
                Ok(Json(ApiResponse {
                    success: true,
                    data: Some(dto),
                    error: None,
                    timestamp: Utc::now(),
                }))
            } else {
                Ok(Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some("Transaction not found".to_string()),
                    timestamp: Utc::now(),
                }))
            }
        }
        Err(e) => {
            error!("Failed to get transaction: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Get latest block (synthetic based on ledger view)
async fn get_latest_block(
    State(node): State<Arc<ParticipantNode>>,
) -> Result<Json<ApiResponse<BlockInfoDto>>, StatusCode> {
    let storage = node.get_storage();
    match storage.get_latest_block().await {
        Ok(Some(block)) => {
            let info = BlockInfoDto {
                number: block.header.slot,
                hash: hex::encode(&block.hash),
                parent_hash: hex::encode(&block.header.parent_hash),
                timestamp: block.timestamp,
                transaction_count: block.transactions.len() as u32,
                size: 1024,
                gas_used: 0,
                gas_limit: 0,
            };
            Ok(Json(ApiResponse { success: true, data: Some(info), error: None, timestamp: Utc::now() }))
        }
        Ok(None) => {
            Ok(Json(ApiResponse { success: true, data: None, error: None, timestamp: Utc::now() }))
        }
        Err(e) => {
            error!("Failed to get latest block: {}", e);
            Ok(Json(ApiResponse { success: false, data: None, error: Some(e.to_string()), timestamp: Utc::now() }))
        }
    }
}

/// Get block by number (synthetic)
async fn get_block_by_number(
    State(node): State<Arc<ParticipantNode>>,
    Path(number): Path<u64>,
) -> Result<Json<ApiResponse<BlockDetailsDto>>, StatusCode> {
    let storage = node.get_storage();
    match storage.get_block_by_slot(number).await {
        Ok(Some(block)) => {
            let transactions: Vec<TransactionDto> = block.transactions.iter().map(convert_transaction_to_dto).collect();
            let info = BlockInfoDto {
                number: block.header.slot,
                hash: hex::encode(&block.hash),
                parent_hash: hex::encode(&block.header.parent_hash),
                timestamp: block.timestamp,
                transaction_count: transactions.len() as u32,
                size: 1024,
                gas_used: 0,
                gas_limit: 0,
            };
            Ok(Json(ApiResponse { success: true, data: Some(BlockDetailsDto { info, transactions }), error: None, timestamp: Utc::now() }))
        }
        Ok(None) => {
            Ok(Json(ApiResponse { success: true, data: None, error: None, timestamp: Utc::now() }))
        }
        Err(e) => {
            error!("Failed to get block {}: {}", number, e);
            Ok(Json(ApiResponse { success: false, data: None, error: Some(e.to_string()), timestamp: Utc::now() }))
        }
    }
}

/// Get block by hash (synthetic)
async fn get_block_by_hash(
    State(node): State<Arc<ParticipantNode>>,
    Path(hash): Path<String>,
) -> Result<Json<ApiResponse<BlockDetailsDto>>, StatusCode> {
    let storage = node.get_storage();
    match storage.get_block_by_hash_hex(&hash).await {
        Ok(Some(block)) => {
            let transactions: Vec<TransactionDto> = block.transactions.iter().map(convert_transaction_to_dto).collect();
            let info = BlockInfoDto {
                number: block.header.slot,
                hash: hex::encode(&block.hash),
                parent_hash: hex::encode(&block.header.parent_hash),
                timestamp: block.timestamp,
                transaction_count: transactions.len() as u32,
                size: 1024,
                gas_used: 0,
                gas_limit: 0,
            };
            Ok(Json(ApiResponse { success: true, data: Some(BlockDetailsDto { info, transactions }), error: None, timestamp: Utc::now() }))
        }
        Ok(None) => {
            Ok(Json(ApiResponse { success: true, data: None, error: None, timestamp: Utc::now() }))
        }
        Err(e) => {
            error!("Failed to get block by hash {}: {}", hash, e);
            Ok(Json(ApiResponse { success: false, data: None, error: Some(e.to_string()), timestamp: Utc::now() }))
        }
    }
}

/// List blocks with pagination
async fn list_blocks(
    State(node): State<Arc<ParticipantNode>>,
    Query(query): Query<BlockListQuery>,
) -> Result<Json<ApiResponse<Vec<BlockSummaryDto>>>, StatusCode> {
    let storage = node.get_storage();
    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);
    match storage.list_blocks_filtered(query.epoch, query.proposer.clone(), limit as u32, offset as u32).await {
        Ok(blocks) => {
            let items = blocks.into_iter().map(|b| BlockSummaryDto {
                number: b.header.slot,
                hash: hex::encode(&b.hash),
                epoch: b.header.epoch,
                proposer: b.header.proposer.0.clone(),
                timestamp: b.timestamp,
                transaction_count: b.transactions.len() as u32,
            }).collect();
            Ok(Json(ApiResponse { success: true, data: Some(items), error: None, timestamp: Utc::now() }))
        }
        Err(e) => {
            error!("Failed to list blocks: {}", e);
            Ok(Json(ApiResponse { success: false, data: None, error: Some(e.to_string()), timestamp: Utc::now() }))
        }
    }
}

/// Get block summary by number
async fn get_block_summary(
    State(node): State<Arc<ParticipantNode>>,
    Path(number): Path<u64>,
) -> Result<Json<ApiResponse<BlockSummaryDto>>, StatusCode> {
    let storage = node.get_storage();
    match storage.get_block_by_slot(number).await {
        Ok(Some(block)) => {
            let dto = BlockSummaryDto {
                number: block.header.slot,
                hash: hex::encode(&block.hash),
                epoch: block.header.epoch,
                proposer: block.header.proposer.0.clone(),
                timestamp: block.timestamp,
                transaction_count: block.transactions.len() as u32,
            };
            Ok(Json(ApiResponse { success: true, data: Some(dto), error: None, timestamp: Utc::now() }))
        }
        Ok(None) => Ok(Json(ApiResponse { success: true, data: None, error: None, timestamp: Utc::now() })),
        Err(e) => {
            error!("Failed to get block summary {}: {}", number, e);
            Ok(Json(ApiResponse { success: false, data: None, error: Some(e.to_string()), timestamp: Utc::now() }))
        }
    }
}

/// Get Merkle proof for transaction inclusion in a block
async fn get_tx_inclusion_proof(
    State(node): State<Arc<ParticipantNode>>,
    Path((number, tx_id)): Path<(u64, String)>,
) -> Result<Json<ApiResponse<MerkleProofDto>>, StatusCode> {
    let storage = node.get_storage();
    match storage.get_block_by_slot(number).await {
        Ok(Some(block)) => {
            let leaves: Vec<Vec<u8>> = block.transactions.iter().map(|tx| tx.id.0.as_bytes().to_vec()).collect();
            let index = block.transactions.iter().position(|tx| tx.id.0 == tx_id);
            if let Some(idx) = index {
                if let Some(proof) = merkle_proof(&leaves, idx) {
                    let dto = MerkleProofDto {
                        block_hash: hex::encode(&block.hash),
                        tx_id: tx_id.clone(),
                        leaf_hash: hex::encode(&proof.leaf),
                        root: hex::encode(&proof.root),
                        path: proof.path.iter().map(|p| hex::encode(p)).collect(),
                        directions: proof.directions.iter().map(|d| if *d { "right".to_string() } else { "left".to_string() }).collect(),
                        valid: crate::merkle::verify_proof(&proof),
                    };
                    Ok(Json(ApiResponse { success: true, data: Some(dto), error: None, timestamp: Utc::now() }))
                } else {
                    Ok(Json(ApiResponse { success: false, data: None, error: Some("Proof generation failed".to_string()), timestamp: Utc::now() }))
                }
            } else {
                Ok(Json(ApiResponse { success: false, data: None, error: Some("Transaction not in block".to_string()), timestamp: Utc::now() }))
            }
        }
        Ok(None) => Ok(Json(ApiResponse { success: false, data: None, error: Some("Block not found".to_string()), timestamp: Utc::now() })),
        Err(e) => {
            error!("Failed to get tx proof in block {}: {}", number, e);
            Ok(Json(ApiResponse { success: false, data: None, error: Some(e.to_string()), timestamp: Utc::now() }))
        }
    }
}

/// Get Merkle proof for a state key change in a block
async fn get_state_change_proof(
    State(node): State<Arc<ParticipantNode>>,
    Path((number, state_key)): Path<(u64, String)>,
) -> Result<Json<ApiResponse<MerkleProofDto>>, StatusCode> {
    use crate::state_commitments::leaves_for_changes;
    let storage = node.get_storage();
    match storage.get_block_by_slot(number).await {
        Ok(Some(block)) => {
            let changes = match storage.get_block_state_changes(number).await {
                Ok(items) => items,
                Err(e) => {
                    error!("Failed to load state changes for slot {}: {}", number, e);
                    return Ok(Json(ApiResponse { success: false, data: None, error: Some(e.to_string()), timestamp: Utc::now() }));
                }
            };
            // Find the first change matching the requested key
            let index = changes.iter().position(|c| c.key == state_key);
            if let Some(idx) = index {
                let leaves = leaves_for_changes(&changes);
                if let Some(proof) = merkle_proof(&leaves, idx) {
                    // Verify computed root matches block header's state_root and proof validity
                    let root_matches = proof.root == block.header.state_root;
                    let valid = crate::merkle::verify_proof(&proof) && root_matches;
                    let dto = MerkleProofDto {
                        block_hash: hex::encode(&block.hash),
                        tx_id: state_key.clone(), // reuse field to carry state_key
                        leaf_hash: hex::encode(&proof.leaf),
                        root: hex::encode(&proof.root),
                        path: proof.path.iter().map(|p| hex::encode(p)).collect(),
                        directions: proof.directions.iter().map(|d| if *d { "right".to_string() } else { "left".to_string() }).collect(),
                        valid,
                    };
                    Ok(Json(ApiResponse { success: true, data: Some(dto), error: None, timestamp: Utc::now() }))
                } else {
                    Ok(Json(ApiResponse { success: false, data: None, error: Some("Proof generation failed".to_string()), timestamp: Utc::now() }))
                }
            } else {
                Ok(Json(ApiResponse { success: false, data: None, error: Some("State key not changed in block".to_string()), timestamp: Utc::now() }))
            }
        }
        Ok(None) => Ok(Json(ApiResponse { success: false, data: None, error: Some("Block not found".to_string()), timestamp: Utc::now() })),
        Err(e) => {
            error!("Failed to get state proof in block {}: {}", number, e);
            Ok(Json(ApiResponse { success: false, data: None, error: Some(e.to_string()), timestamp: Utc::now() }))
        }
    }
}

/// Create a contract
async fn create_contract(
    State(node): State<Arc<ParticipantNode>>,
    Json(request): Json<CreateContractRequest>,
) -> Result<Json<ApiResponse<ContractDto>>, StatusCode> {
    let signatories: Vec<ParticipantId> = request.signatories
        .into_iter()
        .map(ParticipantId)
        .collect();
    
    let observers: Vec<ParticipantId> = request.observers
        .into_iter()
        .map(ParticipantId)
        .collect();

    let command = TransactionCommand::CreateContract(CreateContractCommand {
        template_id: request.template_id,
        signatories,
        observers,
        argument: request.argument,
    });

    let transaction = Transaction {
        id: TransactionId(Uuid::new_v4()),
        submitter: node.get_participant_id().clone(),
        command,
        created_at: Utc::now(),
        signatures: Vec::new(),
        encrypted_payload: None,
    };

    match node.submit_transaction(transaction).await {
        Ok(_) => {
            // In a real implementation, we would return the created contract
            Ok(Json(ApiResponse {
                success: true,
                data: None, // Would contain the created contract
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to create contract: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// List contracts
async fn list_contracts(
    State(node): State<Arc<ParticipantNode>>,
    Query(query): Query<ContractQuery>,
) -> Result<Json<ApiResponse<Vec<ContractDto>>>, StatusCode> {
    let limit = query.limit.unwrap_or(50).min(1000);
    let offset = query.offset.unwrap_or(0);

    match node.get_ledger_view(&node.get_participant_id()).await {
        Ok(view) => {
            let contracts: Vec<ContractDto> = view.contracts
                .into_iter()
                .skip(offset)
                .take(limit)
                .map(|contract| convert_contract_to_dto(&contract))
                .collect();

            Ok(Json(ApiResponse {
                success: true,
                data: Some(contracts),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to list contracts: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Get a specific contract
async fn get_contract(
    State(node): State<Arc<ParticipantNode>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ContractDto>>, StatusCode> {
    let contract_id = match Uuid::parse_str(&id) {
        Ok(uuid) => ContractId(uuid),
        Err(_) => {
            return Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Invalid contract ID".to_string()),
                timestamp: Utc::now(),
            }));
        }
    };

    match node.get_ledger_view(&node.get_participant_id()).await {
        Ok(view) => {
            if let Some(contract) = view.contracts.iter().find(|c| c.id == contract_id) {
                let dto = convert_contract_to_dto(contract);
                Ok(Json(ApiResponse {
                    success: true,
                    data: Some(dto),
                    error: None,
                    timestamp: Utc::now(),
                }))
            } else {
                Ok(Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some("Contract not found".to_string()),
                    timestamp: Utc::now(),
                }))
            }
        }
        Err(e) => {
            error!("Failed to get contract: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Exercise a contract choice
async fn exercise_contract(
    State(node): State<Arc<ParticipantNode>>,
    Path(id): Path<String>,
    Json(request): Json<ExerciseContractRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let contract_id = match Uuid::parse_str(&id) {
        Ok(uuid) => ContractId(uuid),
        Err(_) => {
            return Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Invalid contract ID".to_string()),
                timestamp: Utc::now(),
            }));
        }
    };

    let command = TransactionCommand::ExerciseContract(ExerciseContractCommand {
        contract_id,
        choice: request.choice,
        argument: request.argument,
    });

    let transaction = Transaction {
        id: TransactionId(Uuid::new_v4()),
        submitter: node.get_participant_id().clone(),
        command,
        created_at: Utc::now(),
        signatures: Vec::new(),
        encrypted_payload: None,
    };

    match node.submit_transaction(transaction).await {
        Ok(_) => {
            Ok(Json(ApiResponse {
                success: true,
                data: Some(()),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to exercise contract: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Archive a contract
async fn archive_contract(
    State(node): State<Arc<ParticipantNode>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let contract_id = match Uuid::parse_str(&id) {
        Ok(uuid) => ContractId(uuid),
        Err(_) => {
            return Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Invalid contract ID".to_string()),
                timestamp: Utc::now(),
            }));
        }
    };

    let command = TransactionCommand::ArchiveContract(ArchiveContractCommand {
        contract_id,
    });

    let transaction = Transaction {
        id: TransactionId(Uuid::new_v4()),
        submitter: node.get_participant_id().clone(),
        command,
        created_at: Utc::now(),
        signatures: Vec::new(),
        encrypted_payload: None,
    };

    match node.submit_transaction(transaction).await {
        Ok(_) => {
            Ok(Json(ApiResponse {
                success: true,
                data: Some(()),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to archive contract: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Create an asset
async fn create_asset(
    State(node): State<Arc<ParticipantNode>>,
    Json(request): Json<CreateAssetRequest>,
) -> Result<Json<ApiResponse<AssetDto>>, StatusCode> {
    let command = TransactionCommand::CreateAsset(CreateAssetCommand {
        asset_type: request.asset_type,
        initial_owner: ParticipantId(request.initial_owner),
        metadata: request.metadata,
    });

    let transaction = Transaction {
        id: TransactionId(Uuid::new_v4()),
        submitter: node.get_participant_id().clone(),
        command,
        created_at: Utc::now(),
        signatures: Vec::new(),
        encrypted_payload: None,
    };

    match node.submit_transaction(transaction).await {
        Ok(_) => {
            Ok(Json(ApiResponse {
                success: true,
                data: None, // Would contain the created asset
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to create asset: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// List assets
async fn list_assets(
    State(node): State<Arc<ParticipantNode>>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<Vec<AssetDto>>>, StatusCode> {
    match node.get_ledger_view(&node.get_participant_id()).await {
        Ok(view) => {
            let assets: Vec<AssetDto> = view.assets
                .into_iter()
                .map(|asset| convert_asset_to_dto(&asset))
                .collect();

            Ok(Json(ApiResponse {
                success: true,
                data: Some(assets),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to list assets: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Get a specific asset
async fn get_asset(
    State(node): State<Arc<ParticipantNode>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<AssetDto>>, StatusCode> {
    let asset_id = AssetId(id);

    match node.get_ledger_view(&node.get_participant_id()).await {
        Ok(view) => {
            if let Some(asset) = view.assets.iter().find(|a| a.id == asset_id) {
                let dto = convert_asset_to_dto(asset);
                Ok(Json(ApiResponse {
                    success: true,
                    data: Some(dto),
                    error: None,
                    timestamp: Utc::now(),
                }))
            } else {
                Ok(Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some("Asset not found".to_string()),
                    timestamp: Utc::now(),
                }))
            }
        }
        Err(e) => {
            error!("Failed to get asset: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Transfer an asset
async fn transfer_asset(
    State(node): State<Arc<ParticipantNode>>,
    Path(id): Path<String>,
    Json(request): Json<TransferAssetRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let asset_id = AssetId(id);

    let command = TransactionCommand::TransferAsset(TransferAssetCommand {
        from: node.get_participant_id().clone(),
        to: ParticipantId(request.to),
        asset_id,
        amount: request.amount,
    });

    let transaction = Transaction {
        id: TransactionId(Uuid::new_v4()),
        submitter: node.get_participant_id().clone(),
        command,
        created_at: Utc::now(),
        signatures: Vec::new(),
        encrypted_payload: None,
    };

    match node.submit_transaction(transaction).await {
        Ok(_) => {
            Ok(Json(ApiResponse {
                success: true,
                data: Some(()),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to transfer asset: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Get wallet balances
async fn get_wallet_balances(
    State(node): State<Arc<ParticipantNode>>,
    Query(query): Query<WalletQuery>,
) -> Result<Json<ApiResponse<Vec<WalletBalanceDto>>>, StatusCode> {
    match node.get_ledger_view(&node.get_participant_id()).await {
        Ok(view) => {
            let balances: Vec<WalletBalanceDto> = view.wallet_balances
                .into_iter()
                .map(|balance| convert_wallet_balance_to_dto(&balance))
                .collect();

            Ok(Json(ApiResponse {
                success: true,
                data: Some(balances),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to get wallet balances: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Get wallet transaction history
async fn get_wallet_history(
    State(node): State<Arc<ParticipantNode>>,
    Query(query): Query<TransactionQuery>,
) -> Result<Json<ApiResponse<Vec<TransactionDto>>>, StatusCode> {
    // This would filter transactions related to wallet operations
    list_transactions(State(node), Query(query)).await
}

/// Get node status
async fn get_node_status(
    State(node): State<Arc<ParticipantNode>>,
) -> Result<Json<ApiResponse<NodeStatusDto>>, StatusCode> {
    match node.get_node_stats().await {
        Ok(stats) => {
            let (gh_height, gh_hash) = node.get_global_head().await;
            let local_tx = stats.ledger_stats.total_transactions;
            let (last_h, last_t) = node.get_sync_last_applied().await;
            let status = NodeStatusDto {
                participant_id: node.get_participant_id().0.clone(),
                status: "running".to_string(),
                version: "1.0.0".to_string(),
                uptime: 0, // Would be calculated from start time
                connected_peers: stats.network_peers as usize,
                sync_domains: node.get_sync_domain_ids(),
                global_head_height: gh_height,
                global_head_hash: gh_hash,
                in_sync: local_tx >= gh_height,
                sync_lag: if local_tx >= gh_height { 0 } else { gh_height - local_tx },
                sync_last_applied_height: last_h,
                sync_last_applied_time: last_t,
            };

            Ok(Json(ApiResponse {
                success: true,
                data: Some(status),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to get node status: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Get node statistics
async fn get_node_stats(
    State(node): State<Arc<ParticipantNode>>,
) -> Result<Json<ApiResponse<NodeStatsDto>>, StatusCode> {
    match node.get_node_stats().await {
        Ok(stats) => {
            let dto = NodeStatsDto {
                total_transactions: stats.total_transactions,
                total_contracts: stats.total_contracts,
                active_contracts: stats.active_contracts,
                total_assets: stats.total_assets,
                wallet_balances: stats.wallet_balances,
                network_peers: stats.network_peers,
            };

            Ok(Json(ApiResponse {
                success: true,
                data: Some(dto),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to get node stats: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Get connected peers
async fn get_node_peers(
    State(_node): State<Arc<ParticipantNode>>,
) -> Result<Json<ApiResponse<Vec<String>>>, StatusCode> {
    // This would return the list of connected peers
    Ok(Json(ApiResponse {
        success: true,
        data: Some(vec![]), // Placeholder
        error: None,
        timestamp: Utc::now(),
    }))
}

/// List contract templates
async fn list_templates(
    State(node): State<Arc<ParticipantNode>>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, StatusCode> {
    // This would return available contract templates
    Ok(Json(ApiResponse {
        success: true,
        data: Some(vec![]), // Placeholder
        error: None,
        timestamp: Utc::now(),
    }))
}

/// Get a specific template
async fn get_template(
    State(_node): State<Arc<ParticipantNode>>,
    Path(_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    // This would return a specific template
    Ok(Json(ApiResponse {
        success: true,
        data: None, // Placeholder
        error: None,
        timestamp: Utc::now(),
    }))
}

/// Health check endpoint
async fn health_check() -> Result<Json<ApiResponse<String>>, StatusCode> {
    Ok(Json(ApiResponse {
        success: true,
        data: Some("OK".to_string()),
        error: None,
        timestamp: Utc::now(),
    }))
}

// ------------ JSON-RPC (Solana-like) ------------
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub id: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Option<serde_json::Value>,
}

const RPC_INVALID_REQUEST: i32 = -32600;
const RPC_METHOD_NOT_FOUND: i32 = -32601;
const RPC_INVALID_PARAMS: i32 = -32602;
const RPC_INTERNAL_ERROR: i32 = -32603;
const RPC_SERVER_ERROR: i32 = -32000;

fn rpc_error(code: i32, message: impl Into<String>, id: Option<serde_json::Value>) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: None,
        error: Some(JsonRpcError { code, message: message.into(), data: None }),
        id,
    }
}

async fn handle_single_rpc(node: Arc<ParticipantNode>, req: JsonRpcRequest) -> JsonRpcResponse {
    let mut error: Option<JsonRpcError> = None;
    let mut result: Option<serde_json::Value> = None;

    match req.method.as_str() {
        // Timing and consensus
        "getSlot" => {
            let (genesis, slot_duration_ms) = node.get_timing_params();
            let now = Utc::now();
            let slot = garp_common::timing::slot_at_time(genesis, slot_duration_ms, now);
            result = Some(serde_json::json!(slot));
        }
        "getSlotLeader" => {
            let (genesis, slot_duration_ms) = node.get_timing_params();
            let now = Utc::now();
            // Optional param: slot
            let slot = if let Some(p) = &req.params {
                p.get("slot").and_then(|v| v.as_u64()).unwrap_or_else(|| garp_common::timing::slot_at_time(genesis, slot_duration_ms, now))
            } else {
                garp_common::timing::slot_at_time(genesis, slot_duration_ms, now)
            };
            let validators = node.get_validators();
            if let Some(leader) = crate::consensus::leader_for_slot(slot, &validators) {
                result = Some(serde_json::json!(leader.0));
            } else {
                error = Some(JsonRpcError { code: RPC_SERVER_ERROR, message: "No validators configured".to_string(), data: None });
            }
        }
        // Blocks
        "getBlock" => {
            let storage = node.get_storage();
            if let Some(params) = &req.params {
                if let Some(number) = params.get("slot").and_then(|v| v.as_u64()) {
                    match storage.get_block_by_slot(number).await {
                        Ok(Some(block)) => {
                            let txs: Vec<serde_json::Value> = block.transactions.iter().map(|t| serde_json::json!({
                                "id": t.id.0.to_string(),
                                "submitter": t.submitter.0,
                                "timestamp": t.created_at,
                            })).collect();
                            result = Some(serde_json::json!({
                                "slot": block.header.slot,
                                "hash": hex::encode(&block.hash),
                                "parentHash": hex::encode(&block.header.parent_hash),
                                "timestamp": block.timestamp,
                                "transactions": txs,
                            }));
                        }
                        Ok(None) => { error = Some(JsonRpcError { code: RPC_SERVER_ERROR, message: "Block not found".to_string(), data: None }); }
                        Err(e) => { error = Some(JsonRpcError { code: RPC_SERVER_ERROR, message: e.to_string(), data: None }); }
                    }
                } else if let Some(hash_hex) = params.get("hash").and_then(|v| v.as_str()) {
                    match storage.get_block_by_hash_hex(hash_hex).await {
                        Ok(Some(block)) => {
                            let txs: Vec<serde_json::Value> = block.transactions.iter().map(|t| serde_json::json!({
                                "id": t.id.0.to_string(),
                                "submitter": t.submitter.0,
                                "timestamp": t.created_at,
                            })).collect();
                            result = Some(serde_json::json!({
                                "slot": block.header.slot,
                                "hash": hex::encode(&block.hash),
                                "parentHash": hex::encode(&block.header.parent_hash),
                                "timestamp": block.timestamp,
                                "transactions": txs,
                            }));
                        }
                        Ok(None) => { error = Some(JsonRpcError { code: RPC_SERVER_ERROR, message: "Block not found".to_string(), data: None }); }
                        Err(e) => { error = Some(JsonRpcError { code: RPC_SERVER_ERROR, message: e.to_string(), data: None }); }
                    }
                } else {
                    error = Some(JsonRpcError { code: RPC_INVALID_PARAMS, message: "Missing parameter: slot or hash".to_string(), data: None });
                }
            } else {
                error = Some(JsonRpcError { code: RPC_INVALID_PARAMS, message: "Missing params".to_string(), data: None });
            }
        }
        // Transactions
        "getTransaction" => {
            let storage = node.get_storage();
            if let Some(params) = &req.params {
                if let Some(id_str) = params.get("signature").and_then(|v| v.as_str()) {
                    match uuid::Uuid::parse_str(id_str) {
                        Ok(uuid) => {
                            match storage.get_transaction(&garp_common::TransactionId(uuid)).await {
                                Ok(Some(tx)) => {
                                    result = Some(serde_json::json!({
                                        "id": tx.id.0.to_string(),
                                        "submitter": tx.submitter.0,
                                        "timestamp": tx.created_at,
                                        "command": tx.command,
                                    }));
                                }
                                Ok(None) => { error = Some(JsonRpcError { code: RPC_SERVER_ERROR, message: "Transaction not found".to_string(), data: None }); }
                                Err(e) => { error = Some(JsonRpcError { code: RPC_SERVER_ERROR, message: e.to_string(), data: None }); }
                            }
                        }
                        Err(_) => error = Some(JsonRpcError { code: RPC_INVALID_PARAMS, message: "Invalid signature".to_string(), data: None }),
                    }
                } else {
                    error = Some(JsonRpcError { code: RPC_INVALID_PARAMS, message: "Missing parameter: signature".to_string(), data: None });
                }
            } else {
                error = Some(JsonRpcError { code: RPC_INVALID_PARAMS, message: "Missing params".to_string(), data: None });
            }
        }
        "getBalance" => {
            // Params: participantId, optional assetId
            if let Some(params) = &req.params {
                let participant_id = params.get("participantId").and_then(|v| v.as_str()).map(|s| garp_common::ParticipantId(s.to_string()));
                if let Some(_pid) = participant_id {
                    match node.get_wallet_balance().await {
                        Ok(Some(balance)) => {
                            if let Some(asset_id) = params.get("assetId").and_then(|v| v.as_str()) {
                                let amount = balance.assets.iter().find(|a| a.id == asset_id).map(|a| a.amount as f64).unwrap_or(0.0);
                                result = Some(serde_json::json!({"balance": amount, "assetId": asset_id}));
                            } else {
                                let total: f64 = balance.assets.iter().map(|a| a.amount as f64).sum();
                                result = Some(serde_json::json!({"balance": total}));
                            }
                        }
                        Ok(None) => { result = Some(serde_json::json!({"balance": 0.0})); }
                        Err(e) => { error = Some(JsonRpcError { code: RPC_SERVER_ERROR, message: e.to_string(), data: None }); }
                    }
                } else {
                    error = Some(JsonRpcError { code: RPC_INVALID_PARAMS, message: "Missing parameter: participantId".to_string(), data: None });
                }
            } else {
                error = Some(JsonRpcError { code: RPC_INVALID_PARAMS, message: "Missing params".to_string(), data: None });
            }
        }
        // Node info
        "getVersion" => {
            result = Some(serde_json::json!({"version": env!("CARGO_PKG_VERSION")}));
        }
        "getHealth" => {
            result = Some(serde_json::json!("ok"));
        }
        // Transaction submission and simulation
        "sendTransaction" => {
            if let Some(params) = &req.params {
                // Expect { command: TransactionCommandDto }
                match serde_json::from_value::<TransactionCommandDto>(params.get("command").cloned().unwrap_or(serde_json::Value::Null)) {
                    Ok(cmd_dto) => {
                        match convert_transaction_command(cmd_dto) {
                            Ok(command) => {
                                let tx = garp_common::Transaction {
                                    id: garp_common::TransactionId(uuid::Uuid::new_v4()),
                                    submitter: node.get_participant_id(),
                                    command,
                                    created_at: Utc::now(),
                                    signatures: vec![],
                                    encrypted_payload: None,
                                };
                                match node.submit_transaction(tx).await {
                                    Ok(vr) => {
                                        result = Some(serde_json::json!({"accepted": true, "status": format!("{:?}", vr)}));
                                    }
                                    Err(e) => { error = Some(JsonRpcError { code: RPC_SERVER_ERROR, message: e.to_string(), data: None }); }
                                }
                            }
                            Err(e) => error = Some(JsonRpcError { code: RPC_INVALID_PARAMS, message: e.to_string(), data: None }),
                        }
                    }
                    Err(_) => error = Some(JsonRpcError { code: RPC_INVALID_PARAMS, message: "Invalid command".to_string(), data: None }),
                }
            } else {
                error = Some(JsonRpcError { code: RPC_INVALID_PARAMS, message: "Missing params".to_string(), data: None });
            }
        }
        "simulateTransaction" => {
            if let Some(params) = &req.params {
                match serde_json::from_value::<SimulationRequestDto>(params.clone()) {
                    Ok(sim) => {
                        match convert_simulation_request_to_tx_v2(sim) {
                            Ok(txv2) => {
                                match node.simulate_transaction_v2(&txv2).await {
                                    Ok(res) => {
                                        result = Some(serde_json::json!({
                                            "accepted": res.accepted,
                                            "estimatedFeeLamports": res.estimated_fee_lamports,
                                            "logs": res.logs,
                                        }));
                                    }
                                    Err(e) => error = Some(JsonRpcError { code: RPC_SERVER_ERROR, message: e.to_string(), data: None }),
                                }
                            }
                            Err(e) => error = Some(JsonRpcError { code: RPC_INVALID_PARAMS, message: e.to_string(), data: None }),
                        }
                    }
                    Err(_) => error = Some(JsonRpcError { code: RPC_INVALID_PARAMS, message: "Invalid simulation params".to_string(), data: None }),
                }
            } else {
                error = Some(JsonRpcError { code: RPC_INVALID_PARAMS, message: "Missing params".to_string(), data: None });
            }
        }
        _ => {
            error = Some(JsonRpcError { code: RPC_METHOD_NOT_FOUND, message: format!("Unknown method: {}", req.method), data: None });
        }
    }

    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result,
        error,
        id: req.id,
    }
}

/// JSON-RPC entrypoint supporting single and batch requests
async fn json_rpc(
    State(node): State<Arc<ParticipantNode>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if body.is_array() {
        let mut responses: Vec<serde_json::Value> = Vec::new();
        for item in body.as_array().unwrap() {
            let req: Result<JsonRpcRequest, _> = serde_json::from_value(item.clone());
            match req {
                Ok(r) => {
                    let resp = handle_single_rpc(node.clone(), r).await;
                    responses.push(serde_json::to_value(resp).unwrap_or_else(|_| serde_json::json!(rpc_error(RPC_INTERNAL_ERROR, "Failed to serialize response", None))));
                }
                Err(_) => {
                    let id = item.get("id").cloned();
                    responses.push(serde_json::json!(rpc_error(RPC_INVALID_REQUEST, "Invalid request", id)));
                }
            }
        }
        Ok(Json(serde_json::Value::Array(responses)))
    } else if body.is_object() {
        let req: Result<JsonRpcRequest, _> = serde_json::from_value(body.clone());
        match req {
            Ok(r) => {
                let resp = handle_single_rpc(node, r).await;
                Ok(Json(serde_json::to_value(resp).unwrap_or_else(|_| serde_json::json!(rpc_error(RPC_INTERNAL_ERROR, "Failed to serialize response", None)))))
            }
            Err(_) => Ok(Json(serde_json::json!(rpc_error(RPC_INVALID_REQUEST, "Invalid request", body.get("id").cloned())))),
        }
    } else {
        Ok(Json(serde_json::json!(rpc_error(RPC_INVALID_REQUEST, "Invalid request payload", None))))
    }
}

// Helper functions for converting between domain types and DTOs

fn convert_transaction_command(dto: TransactionCommandDto) -> GarpResult<TransactionCommand> {
    match dto {
        TransactionCommandDto::CreateContract { template_id, signatories, observers, argument } => {
            Ok(TransactionCommand::CreateContract(CreateContractCommand {
                template_id,
                signatories: signatories.into_iter().map(ParticipantId).collect(),
                observers: observers.into_iter().map(ParticipantId).collect(),
                argument,
            }))
        }
        TransactionCommandDto::ExerciseContract { contract_id, choice, argument } => {
            let contract_id = Uuid::parse_str(&contract_id)
                .map_err(|_| GarpError::ValidationError("Invalid contract ID".to_string()))?;
            Ok(TransactionCommand::ExerciseContract(ExerciseContractCommand {
                contract_id: ContractId(contract_id),
                choice,
                argument,
            }))
        }
        TransactionCommandDto::ArchiveContract { contract_id } => {
            let contract_id = Uuid::parse_str(&contract_id)
                .map_err(|_| GarpError::ValidationError("Invalid contract ID".to_string()))?;
            Ok(TransactionCommand::ArchiveContract(ArchiveContractCommand {
                contract_id: ContractId(contract_id),
            }))
        }
        TransactionCommandDto::TransferAsset { from, to, asset_id, amount } => {
            if amount <= 0.0 {
                return Err(GarpError::ValidationError("Amount must be positive".to_string()));
            }
            if asset_id.is_empty() {
                return Err(GarpError::ValidationError("asset_id is required".to_string()));
            }
            Ok(TransactionCommand::TransferAsset(TransferAssetCommand {
                from: ParticipantId(from),
                to: ParticipantId(to),
                asset_id: AssetId(asset_id),
                amount,
            }))
        }
        TransactionCommandDto::CreateAsset { asset_type, initial_owner, metadata } => {
            // Basic validation: asset_type and owner must be present
            if asset_type.trim().is_empty() {
                return Err(GarpError::ValidationError("asset_type is required".to_string()));
            }
            if initial_owner.trim().is_empty() {
                return Err(GarpError::ValidationError("initial_owner is required".to_string()));
            }
            Ok(TransactionCommand::CreateAsset(CreateAssetCommand {
                asset_type,
                initial_owner: ParticipantId(initial_owner),
                metadata,
            }))
        }
    }
}

fn convert_transaction_to_dto(transaction: &Transaction) -> TransactionDto {
    let command = match &transaction.command {
        TransactionCommand::CreateContract(cmd) => TransactionCommandDto::CreateContract {
            template_id: cmd.template_id.clone(),
            signatories: cmd.signatories.iter().map(|p| p.0.clone()).collect(),
            observers: cmd.observers.iter().map(|p| p.0.clone()).collect(),
            argument: cmd.argument.clone(),
        },
        TransactionCommand::ExerciseContract(cmd) => TransactionCommandDto::ExerciseContract {
            contract_id: cmd.contract_id.0.to_string(),
            choice: cmd.choice.clone(),
            argument: cmd.argument.clone(),
        },
        TransactionCommand::ArchiveContract(cmd) => TransactionCommandDto::ArchiveContract {
            contract_id: cmd.contract_id.0.to_string(),
        },
        TransactionCommand::TransferAsset(cmd) => TransactionCommandDto::TransferAsset {
            from: cmd.from.0.clone(),
            to: cmd.to.0.clone(),
            asset_id: cmd.asset_id.0.clone(),
            amount: cmd.amount,
        },
        TransactionCommand::CreateAsset(cmd) => TransactionCommandDto::CreateAsset {
            asset_type: cmd.asset_type.clone(),
            initial_owner: cmd.initial_owner.0.clone(),
            metadata: cmd.metadata.clone(),
        },
    };

    TransactionDto {
        id: transaction.id.0.to_string(),
        command,
        submitter: transaction.submitter.0.clone(),
        timestamp: transaction.created_at,
        status: "completed".to_string(), // Would be determined from actual status
        metadata: None,
    }
}

fn convert_contract_to_dto(contract: &Contract) -> ContractDto {
    ContractDto {
        id: contract.id.0.to_string(),
        template_id: contract.template_id.clone(),
        signatories: contract.signatories.iter().map(|p| p.0.clone()).collect(),
        observers: contract.observers.iter().map(|p| p.0.clone()).collect(),
        argument: contract.argument.clone(),
        created_at: contract.created_at,
        archived: contract.archived,
    }
}

fn convert_asset_to_dto(asset: &Asset) -> AssetDto {
    AssetDto {
        id: asset.id.0.clone(),
        asset_type: asset.asset_type.to_string(),
        owner: asset.owner.0.clone(),
        metadata: asset.metadata.clone(),
        created_at: asset.created_at,
    }
}

fn convert_wallet_balance_to_dto(balance: &WalletBalance) -> WalletBalanceDto {
    WalletBalanceDto {
        participant_id: balance.participant_id.0.clone(),
        asset_id: balance.asset_id.0.clone(),
        balance: balance.balance,
        last_updated: balance.last_updated,
    }
}

/// List events with query parameters
async fn list_events(
    State(node): State<Arc<ParticipantNode>>,
    Query(query): Query<EventQueryParams>,
) -> Result<Json<ApiResponse<Vec<ContractEventDto>>>, StatusCode> {
    let event_query = crate::storage::EventQuery {
        contract_id: query.contract_id.and_then(|id| Uuid::parse_str(&id).ok()).map(garp_common::ContractId),
        event_type: query.event_type,
        participant_id: query.participant_id.map(garp_common::ParticipantId),
        from_timestamp: query.from_timestamp,
        to_timestamp: query.to_timestamp,
        limit: query.limit,
    };

    match node.get_storage().query_events(&event_query).await {
        Ok(events) => {
            let event_dtos: Vec<ContractEventDto> = events
                .into_iter()
                .map(|event| ContractEventDto {
                    id: event.id,
                    contract_id: event.contract_id.0.to_string(),
                    event_type: event.event_type,
                    data: event.data,
                    timestamp: event.timestamp,
                    emitter: event.emitter.0,
                })
                .collect();

            Ok(Json(ApiResponse {
                success: true,
                data: Some(event_dtos),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to list events: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Get events for a specific contract
async fn get_contract_events(
    State(node): State<Arc<ParticipantNode>>,
    Path(contract_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<ContractEventDto>>>, StatusCode> {
    let contract_uuid = match Uuid::parse_str(&contract_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Invalid contract ID".to_string()),
                timestamp: Utc::now(),
            }));
        }
    };

    match node.get_storage().get_contract_events(&garp_common::ContractId(contract_uuid), Some(100)).await {
        Ok(events) => {
            let event_dtos: Vec<ContractEventDto> = events
                .into_iter()
                .map(|event| ContractEventDto {
                    id: event.id,
                    contract_id: event.contract_id.0.to_string(),
                    event_type: event.event_type,
                    data: event.data,
                    timestamp: event.timestamp,
                    emitter: event.emitter.0,
                })
                .collect();

            Ok(Json(ApiResponse {
                success: true,
                data: Some(event_dtos),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to get contract events: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Get events emitted by a specific participant
async fn get_participant_events(
    State(node): State<Arc<ParticipantNode>>,
    Path(participant_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<ContractEventDto>>>, StatusCode> {
    match node.get_storage().get_participant_events(&garp_common::ParticipantId(participant_id), Some(100)).await {
        Ok(events) => {
            let event_dtos: Vec<ContractEventDto> = events
                .into_iter()
                .map(|event| ContractEventDto {
                    id: event.id,
                    contract_id: event.contract_id.0.to_string(),
                    event_type: event.event_type,
                    data: event.data,
                    timestamp: event.timestamp,
                    emitter: event.emitter.0,
                })
                .collect();

            Ok(Json(ApiResponse {
                success: true,
                data: Some(event_dtos),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to get participant events: {}", e);
            Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Ledger state DTO
#[derive(Debug, Serialize)]
pub struct LedgerStateDto {
    pub participant_id: String,
    pub active_contracts: Vec<String>,
    pub total_transactions: u64,
    pub last_transaction_id: Option<String>,
    pub has_wallet_balance: bool,
    pub checkpoint_time: DateTime<Utc>,
}

/// Get ledger checkpoint/state
async fn get_ledger_checkpoint(
    State(node): State<Arc<ParticipantNode>>,
) -> Result<Json<ApiResponse<LedgerStateDto>>, StatusCode> {
    match node.get_ledger_state().await {
        Ok(state) => {
            let dto = LedgerStateDto {
                participant_id: state.participant_id.0.clone(),
                active_contracts: state.active_contracts.iter().map(|c| c.0.clone()).collect(),
                total_transactions: state.total_transactions,
                last_transaction_id: state.last_transaction_id.as_ref().map(|id| id.0.clone()),
                has_wallet_balance: state.wallet_balance.is_some(),
                checkpoint_time: state.checkpoint_time,
            };
            Ok(Json(ApiResponse { success: true, data: Some(dto), error: None, timestamp: Utc::now() }))
        }
        Err(e) => {
            error!("Failed to get ledger checkpoint: {}", e);
            Ok(Json(ApiResponse { success: false, data: None, error: Some(e.to_string()), timestamp: Utc::now() }))
        }
    }
}
    async fn auth_middleware<B>(req: axum::http::Request<B>, next: middleware::Next<B>) -> Result<axum::response::Response, axum::http::StatusCode> {
    let required = std::env::var("PARTICIPANT_API_TOKEN").ok();
    if let Some(expected) = required {
        if let Some(header) = req.headers().get(axum::http::header::AUTHORIZATION) {
            if let Ok(hval) = header.to_str() {
                if hval.starts_with("Bearer ") {
                    let token = &hval[7..];
                    if token == expected {
                        return Ok(next.run(req).await);
                    }
                }
            }
        }
        return Err(axum::http::StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(req).await)
}

// -----------------------------------------------------------------------------
// TxV2 Simulation DTOs and handler
// -----------------------------------------------------------------------------
#[derive(Debug, Deserialize)]
pub struct SignatureDto {
    pub algorithm: String,
    pub signature: String, // hex
    pub public_key: String, // hex
}

#[derive(Debug, Deserialize)]
pub struct DurableNonceDto {
    pub nonce: String, // hex
    pub authority: String,
}

#[derive(Debug, Deserialize)]
pub struct ComputeBudgetDto {
    pub max_units: u64,
    pub heap_bytes: u32,
}

#[derive(Debug, Deserialize)]
pub struct AccountAccessDto {
    pub account: String,
    pub is_signer: bool,
    pub is_writable: bool,
}

#[derive(Debug, Deserialize)]
pub struct SimulationInstructionDto {
    pub program: String,
    pub accounts: Vec<AccountAccessDto>,
    pub data: String, // hex
}

#[derive(Debug, Deserialize)]
pub struct SimulationRequestDto {
    pub fee_payer: String,
    pub signatures: Vec<SignatureDto>,
    pub recent_blockhash: String, // hex
    pub slot: u64,
    pub durable_nonce: Option<DurableNonceDto>,
    pub compute_budget: Option<ComputeBudgetDto>,
    pub account_keys: Vec<String>,
    pub instructions: Vec<SimulationInstructionDto>,
}

#[derive(Debug, Serialize)]
pub struct SimulationResponseDto {
    pub accepted: bool,
    pub estimated_fee_lamports: u64,
    pub logs: Vec<String>,
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    hex::decode(s).map_err(|e| format!("invalid hex: {}", e))
}

fn convert_simulation_request_to_tx_v2(req: SimulationRequestDto) -> garp_common::GarpResult<garp_common::TxV2> {
    // Convert signatures
    let signatures: Vec<garp_common::Signature> = req.signatures.into_iter().map(|sd| {
        let sig = hex_decode(&sd.signature).map_err(|e| garp_common::GarpError::BadRequest(e))?;
        let pk = hex_decode(&sd.public_key).map_err(|e| garp_common::GarpError::BadRequest(e))?;
        Ok(garp_common::Signature { algorithm: sd.algorithm, signature: sig, public_key: pk })
    }).collect::<Result<_, _>>()?;

    // Recent blockhash
    let rb = garp_common::RecentBlockhash(hex_decode(&req.recent_blockhash).map_err(garp_common::GarpError::BadRequest)?);

    // Durable nonce
    let durable = if let Some(dn) = req.durable_nonce {
        Some(garp_common::DurableNonce { nonce: hex_decode(&dn.nonce).map_err(garp_common::GarpError::BadRequest)?, authority: garp_common::AccountId(dn.authority) })
    } else { None };

    // Compute budget
    let budget = req.compute_budget.map(|b| garp_common::ComputeBudget { max_units: b.max_units, heap_bytes: b.heap_bytes });

    // Account keys
    let account_keys: Vec<garp_common::AccountId> = req.account_keys.into_iter().map(garp_common::AccountId).collect();

    // Instructions
    let instructions: Vec<garp_common::ProgramInstruction> = req.instructions.into_iter().map(|ix| {
        let program = garp_common::ProgramId(ix.program);
        let accounts: Vec<garp_common::AccountMeta> = ix.accounts.into_iter().map(|a| garp_common::AccountMeta { account: garp_common::AccountId(a.account), is_signer: a.is_signer, is_writable: a.is_writable }).collect();
        let data = hex_decode(&ix.data).map_err(|e| garp_common::GarpError::BadRequest(e))?;
        Ok(garp_common::ProgramInstruction { program, accounts, data })
    }).collect::<Result<_, _>>()?;

    Ok(garp_common::TxV2 {
        id: garp_common::TransactionId(uuid::Uuid::new_v4()),
        fee_payer: garp_common::AccountId(req.fee_payer),
        signatures,
        recent_blockhash: rb,
        slot: req.slot,
        durable_nonce: durable,
        compute_budget: budget,
        account_keys,
        instructions,
        created_at: chrono::Utc::now(),
    })
}

async fn simulate_transaction_v2(
    State(node): State<Arc<ParticipantNode>>,
    Json(req): Json<SimulationRequestDto>,
) -> Result<Json<ApiResponse<SimulationResponseDto>>, StatusCode> {
    let tx = match convert_simulation_request_to_tx_v2(req) {
        Ok(t) => t,
        Err(e) => {
            return Ok(Json(ApiResponse { success: false, data: None, error: Some(format!("bad request: {}", e)), timestamp: chrono::Utc::now() })));
        }
    };
    match node.simulate_transaction_v2(&tx).await {
        Ok(sim) => {
            let dto = SimulationResponseDto { accepted: sim.accepted, estimated_fee_lamports: sim.estimated_fee_lamports, logs: sim.logs };
            Ok(Json(ApiResponse { success: true, data: Some(dto), error: None, timestamp: chrono::Utc::now() }))
        }
        Err(e) => {
            Ok(Json(ApiResponse { success: false, data: None, error: Some(e.to_string()), timestamp: chrono::Utc::now() }))
        }
    }
}
/// Mempool submission request
#[derive(Debug, Deserialize)]
pub struct SubmitMempoolRequest {
    pub fee: u64,
    pub command: TransactionCommandDto,
}

/// Mempool submission response
#[derive(Debug, Serialize)]
pub struct SubmitMempoolResponse {
    pub id: String,
    pub accepted: bool,
}

/// Submit a transaction to the mempool with a fee
async fn submit_mempool(
    State(node): State<Arc<ParticipantNode>>,
    Json(request): Json<SubmitMempoolRequest>,
) -> Result<Json<ApiResponse<SubmitMempoolResponse>>, StatusCode> {
    let command = convert_transaction_command(request.command)
        .map_err(|e| {
            warn!("Invalid transaction command: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    let tx = garp_common::Transaction {
        id: garp_common::TransactionId::new(),
        submitter: node.get_participant_id(),
        command,
        created_at: Utc::now(),
        signatures: vec![],
        encrypted_payload: None,
    };

    if let Err(e) = node.submit_to_mempool(tx.clone(), request.fee).await {
        error!("Failed to submit to mempool: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let resp = SubmitMempoolResponse { id: tx.id.0.to_string(), accepted: true };
    Ok(Json(ApiResponse { success: true, data: Some(resp), error: None, timestamp: Utc::now() }))
}

/// Basic mempool stats (count only for now)
#[derive(Debug, Serialize)]
pub struct MempoolStatsDto {
    pub count: usize,
}

async fn get_mempool_stats(
    State(node): State<Arc<ParticipantNode>>,
) -> Result<Json<ApiResponse<MempoolStatsDto>>, StatusCode> {
    let count = node.get_mempool_batch(usize::MAX).await.len();
    Ok(Json(ApiResponse { success: true, data: Some(MempoolStatsDto { count }), error: None, timestamp: Utc::now() }))
}