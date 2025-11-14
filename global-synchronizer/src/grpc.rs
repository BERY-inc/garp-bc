use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{info, error};

use crate::{GlobalSynchronizer, bridge::{BridgeTransactionStatus, AssetMapping}};

// Include the generated protobuf code
include!("../proto/garp.rs");

// Implement the gRPC service
pub struct GlobalSynchronizerService {
    synchronizer: Arc<GlobalSynchronizer>,
}

impl GlobalSynchronizerService {
    pub fn new(synchronizer: Arc<GlobalSynchronizer>) -> Self {
        Self { synchronizer }
    }
}

#[tonic::async_trait]
impl global_synchronizer_server::GlobalSynchronizer for GlobalSynchronizerService {
    async fn get_node_status(
        &self,
        _request: Request<GetNodeStatusRequest>,
    ) -> Result<Response<GetNodeStatusResponse>, Status> {
        info!("gRPC: GetNodeStatus called");
        
        let state = self.synchronizer.get_state().await;
        let metrics = self.synchronizer.get_metrics().await.map_err(|e| {
            error!("Failed to get metrics: {}", e);
            Status::internal("Failed to get metrics")
        })?;
        
        let response = GetNodeStatusResponse {
            status: format!("{:?}", state.status),
            block_height: state.block_height,
            last_block_hash: state.last_block_hash,
            tps: metrics.tps,
            success_rate: metrics.success_rate,
        };
        
        Ok(Response::new(response))
    }
    
    async fn get_latest_block(
        &self,
        _request: Request<GetLatestBlockRequest>,
    ) -> Result<Response<GetLatestBlockResponse>, Status> {
        info!("gRPC: GetLatestBlock called");
        
        match self.synchronizer.get_latest_block().await {
            Ok(Some(block_info)) => {
                let block = convert_block_info(block_info);
                let response = GetLatestBlockResponse { block: Some(block) };
                Ok(Response::new(response))
            }
            Ok(None) => Err(Status::not_found("No blocks found")),
            Err(e) => {
                error!("Failed to get latest block: {}", e);
                Err(Status::internal("Failed to get latest block"))
            }
        }
    }
    
    async fn get_block_by_height(
        &self,
        request: Request<GetBlockByHeightRequest>,
    ) -> Result<Response<GetBlockByHeightResponse>, Status> {
        let height = request.into_inner().height;
        info!("gRPC: GetBlockByHeight called with height: {}", height);
        
        match self.synchronizer.get_block_by_height(height).await {
            Ok(Some(block_info)) => {
                let block = convert_block_info(block_info);
                let response = GetBlockByHeightResponse { block: Some(block) };
                Ok(Response::new(response))
            }
            Ok(None) => Err(Status::not_found("Block not found")),
            Err(e) => {
                error!("Failed to get block by height: {}", e);
                Err(Status::internal("Failed to get block by height"))
            }
        }
    }
    
    async fn submit_transaction(
        &self,
        request: Request<SubmitTransactionRequest>,
    ) -> Result<Response<SubmitTransactionResponse>, Status> {
        let transaction_data = request.into_inner().transaction_data;
        info!("gRPC: SubmitTransaction called with {} bytes", transaction_data.len());
        
        // For now, we'll just return a mock response
        // In a real implementation, this would submit the transaction to the network
        let transaction_id = format!("tx_{}", uuid::Uuid::new_v4());
        let response = SubmitTransactionResponse {
            transaction_id,
            status: "submitted".to_string(),
        };
        
        Ok(Response::new(response))
    }
    
    async fn get_transaction_status(
        &self,
        request: Request<GetTransactionStatusRequest>,
    ) -> Result<Response<GetTransactionStatusResponse>, Status> {
        let transaction_id = request.into_inner().transaction_id;
        info!("gRPC: GetTransactionStatus called for transaction: {}", transaction_id);
        
        // For now, we'll just return a mock response
        // In a real implementation, this would query the transaction status
        let transaction = TransactionInfo {
            id: transaction_id.clone(),
            submitter: "mock_submitter".to_string(),
            status: "confirmed".to_string(),
            created_at: chrono::Utc::now().timestamp(),
            error: "".to_string(),
        };
        
        let response = GetTransactionStatusResponse {
            status: "confirmed".to_string(),
            transaction: Some(transaction),
        };
        
        Ok(Response::new(response))
    }
    
    async fn initiate_bridge_transfer(
        &self,
        request: Request<InitiateBridgeTransferRequest>,
    ) -> Result<Response<InitiateBridgeTransferResponse>, Status> {
        let req = request.into_inner();
        info!("gRPC: InitiateBridgeTransfer called from {} to {} for amount {}", 
              req.source_chain, req.target_chain, req.amount);
        
        match self.synchronizer.initiate_bridge_transfer(
            req.source_chain,
            req.source_tx_id,
            req.target_chain,
            req.amount,
            req.source_address,
            req.target_address,
            req.asset_id,
        ).await {
            Ok(bridge_tx_id) => {
                let response = InitiateBridgeTransferResponse { bridge_tx_id };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to initiate bridge transfer: {}", e);
                Err(Status::internal("Failed to initiate bridge transfer"))
            }
        }
    }
    
    async fn get_bridge_transfer_status(
        &self,
        request: Request<GetBridgeTransferStatusRequest>,
    ) -> Result<Response<GetBridgeTransferStatusResponse>, Status> {
        let bridge_tx_id = request.into_inner().bridge_tx_id;
        info!("gRPC: GetBridgeTransferStatus called for bridge transaction: {}", bridge_tx_id);
        
        match self.synchronizer.get_bridge_transaction_status(&bridge_tx_id).await {
            Ok(status) => {
                let status_str = format!("{:?}", status);
                let response = GetBridgeTransferStatusResponse { status: status_str };
                Ok(Response::new(response))
            }
            Err(e) => {
                error!("Failed to get bridge transfer status: {}", e);
                Err(Status::internal("Failed to get bridge transfer status"))
            }
        }
    }
    
    async fn add_asset_mapping(
        &self,
        request: Request<AddAssetMappingRequest>,
    ) -> Result<Response<AddAssetMappingResponse>, Status> {
        let mapping = request.into_inner().mapping;
        info!("gRPC: AddAssetMapping called");
        
        if let Some(mapping) = mapping {
            let asset_mapping = AssetMapping {
                source_asset_id: mapping.source_asset_id,
                source_chain: mapping.source_chain,
                target_asset_id: mapping.target_asset_id,
                target_chain: mapping.target_chain,
                conversion_rate: mapping.conversion_rate,
                last_updated: chrono::Utc::now(),
            };
            
            match self.synchronizer.add_asset_mapping(asset_mapping).await {
                Ok(()) => {
                    let response = AddAssetMappingResponse {
                        success: true,
                        message: "Asset mapping added successfully".to_string(),
                    };
                    Ok(Response::new(response))
                }
                Err(e) => {
                    error!("Failed to add asset mapping: {}", e);
                    let response = AddAssetMappingResponse {
                        success: false,
                        message: format!("Failed to add asset mapping: {}", e),
                    };
                    Ok(Response::new(response))
                }
            }
        } else {
            Err(Status::invalid_argument("Missing asset mapping"))
        }
    }
    
    async fn get_asset_mapping(
        &self,
        request: Request<GetAssetMappingRequest>,
    ) -> Result<Response<GetAssetMappingResponse>, Status> {
        let req = request.into_inner();
        info!("gRPC: GetAssetMapping called for {}:{} -> {}", 
              req.source_chain, req.source_asset_id, req.target_chain);
        
        match self.synchronizer.get_asset_mapping(
            &req.source_chain, 
            &req.source_asset_id, 
            &req.target_chain
        ).await {
            Ok(Some(mapping)) => {
                let proto_mapping = garp::AssetMapping {
                    source_asset_id: mapping.source_asset_id,
                    source_chain: mapping.source_chain,
                    target_asset_id: mapping.target_asset_id,
                    target_chain: mapping.target_chain,
                    conversion_rate: mapping.conversion_rate,
                    last_updated: mapping.last_updated.timestamp(),
                };
                
                let response = GetAssetMappingResponse {
                    mapping: Some(proto_mapping),
                };
                Ok(Response::new(response))
            }
            Ok(None) => Err(Status::not_found("Asset mapping not found")),
            Err(e) => {
                error!("Failed to get asset mapping: {}", e);
                Err(Status::internal("Failed to get asset mapping"))
            }
        }
    }
}

fn convert_block_info(block_info: crate::storage::BlockInfo) -> BlockInfo {
    BlockInfo {
        height: block_info.height,
        hash: block_info.hash,
        parent_hash: block_info.parent_hash.unwrap_or_default(),
        timestamp: block_info.timestamp.unwrap_or(0),
        transactions: vec![], // In a real implementation, we would convert transactions
    }
}

// Function to start the gRPC server
pub async fn start_grpc_server(
    synchronizer: Arc<GlobalSynchronizer>,
    addr: std::net::SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting gRPC server on {}", addr);
    
    let service = GlobalSynchronizerService::new(synchronizer);
    let server = global_synchronizer_server::GlobalSynchronizerServer::new(service);
    
    Server::builder()
        .add_service(server)
        .serve(addr)
        .await?;
        
    Ok(())
}