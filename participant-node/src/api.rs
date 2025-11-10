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
};

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

impl ApiServer {
    /// Create new API server
    pub fn new(node: Arc<ParticipantNode>, config: ApiConfig) -> Self {
        Self { node, config }
    }

    /// Create router with all endpoints
    pub fn create_router(&self) -> Router {
        Router::new()
            // Transaction endpoints
            .route("/api/v1/transactions", post(submit_transaction))
            .route("/api/v1/transactions", get(list_transactions))
            .route("/api/v1/transactions/:id", get(get_transaction))
            // Block endpoints (synthetic)
            .route("/api/v1/blocks/latest", get(get_latest_block))
            .route("/api/v1/blocks/:number", get(get_block_by_number))
            .route("/api/v1/blocks/hash/:hash", get(get_block_by_hash))
            
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
            
            // Node endpoints
            .route("/api/v1/node/status", get(get_node_status))
            .route("/api/v1/node/stats", get(get_node_stats))
            .route("/api/v1/node/peers", get(get_node_peers))
            // Ledger checkpoint endpoint
            .route("/api/v1/ledger/checkpoint", get(get_ledger_checkpoint))
            
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

    match node.get_ledger_view(&node.get_participant_id()).await {
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
    match node.get_ledger_view(&node.get_participant_id()).await {
        Ok(view) => {
            let tx_count = view.transaction_history.len() as u32;
            let number = tx_count as u64;
            let info = BlockInfoDto {
                number,
                hash: format!("0xblk{}", number),
                parent_hash: if number > 0 { format!("0xblk{}", number - 1) } else { "0x0".to_string() },
                timestamp: Utc::now(),
                transaction_count: tx_count,
                size: 1024,
                gas_used: 0,
                gas_limit: 0,
            };
            Ok(Json(ApiResponse { success: true, data: Some(info), error: None, timestamp: Utc::now() }))
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
    match node.get_ledger_view(&node.get_participant_id()).await {
        Ok(view) => {
            let txs = view.transaction_history.iter().take(number as usize).cloned().collect::<Vec<_>>();
            let transactions: Vec<TransactionDto> = txs.into_iter().map(convert_transaction_to_dto).collect();
            let info = BlockInfoDto {
                number,
                hash: format!("0xblk{}", number),
                parent_hash: if number > 0 { format!("0xblk{}", number - 1) } else { "0x0".to_string() },
                timestamp: Utc::now(),
                transaction_count: transactions.len() as u32,
                size: 1024,
                gas_used: 0,
                gas_limit: 0,
            };
            Ok(Json(ApiResponse { success: true, data: Some(BlockDetailsDto { info, transactions }), error: None, timestamp: Utc::now() }))
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
    match node.get_ledger_view(&node.get_participant_id()).await {
        Ok(view) => {
            let number = view.transaction_history.len() as u64;
            let info = BlockInfoDto {
                number,
                hash: hash.clone(),
                parent_hash: if number > 0 { format!("0xblk{}", number - 1) } else { "0x0".to_string() },
                timestamp: Utc::now(),
                transaction_count: view.transaction_history.len() as u32,
                size: 1024,
                gas_used: 0,
                gas_limit: 0,
            };
            let transactions: Vec<TransactionDto> = view.transaction_history.into_iter().map(convert_transaction_to_dto).collect();
            Ok(Json(ApiResponse { success: true, data: Some(BlockDetailsDto { info, transactions }), error: None, timestamp: Utc::now() }))
        }
        Err(e) => {
            error!("Failed to get block by hash {}: {}", hash, e);
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