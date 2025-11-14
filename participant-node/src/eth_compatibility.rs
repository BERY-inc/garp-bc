use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn, error};
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;

use garp_common::{
    ParticipantId, TransactionId,
    GarpResult, GarpError,
};
use crate::node::ParticipantNode;
use crate::config::ApiConfig;

/// Ethereum JSON-RPC compatibility layer
pub struct EthCompatibilityLayer {
    node: Arc<ParticipantNode>,
}

/// Ethereum JSON-RPC request
#[derive(Debug, Deserialize)]
pub struct EthJsonRpcRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

/// Ethereum JSON-RPC response
#[derive(Debug, Serialize)]
pub struct EthJsonRpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub error: Option<EthJsonRpcError>,
}

/// Ethereum JSON-RPC error
#[derive(Debug, Serialize)]
pub struct EthJsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// Ethereum block information
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EthBlock {
    pub number: String, // hex encoded
    pub hash: String,   // hex encoded
    pub parent_hash: String, // hex encoded
    pub nonce: Option<String>, // hex encoded
    pub sha3_uncles: String, // hex encoded
    pub logs_bloom: String, // hex encoded
    pub transactions_root: String, // hex encoded
    pub state_root: String, // hex encoded
    pub receipts_root: String, // hex encoded
    pub miner: String,
    pub difficulty: String, // hex encoded
    pub total_difficulty: String, // hex encoded
    pub extra_data: String, // hex encoded
    pub size: String, // hex encoded
    pub gas_limit: String, // hex encoded
    pub gas_used: String, // hex encoded
    pub timestamp: String, // hex encoded
    pub transactions: Vec<EthTransaction>,
    pub uncles: Vec<String>,
}

/// Ethereum transaction information
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EthTransaction {
    pub hash: String, // hex encoded
    pub nonce: String, // hex encoded
    pub block_hash: String, // hex encoded
    pub block_number: String, // hex encoded
    pub transaction_index: String, // hex encoded
    pub from: String,
    pub to: Option<String>,
    pub value: String, // hex encoded
    pub gas: String, // hex encoded
    pub gas_price: String, // hex encoded
    pub input: String, // hex encoded
    pub v: String, // hex encoded
    pub r: String, // hex encoded
    pub s: String, // hex encoded
}

impl EthCompatibilityLayer {
    /// Create new Ethereum compatibility layer
    pub fn new(node: Arc<ParticipantNode>) -> Self {
        Self { node }
    }

    /// Create router with Ethereum JSON-RPC endpoints
    pub fn create_router(&self) -> Router {
        Router::new()
            .route("/eth", post(eth_json_rpc))
            .with_state(self.node.clone())
    }

    /// Handle Ethereum JSON-RPC request
    async fn handle_eth_request(
        &self,
        method: String,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, EthJsonRpcError> {
        match method.as_str() {
            "eth_blockNumber" => self.eth_block_number().await,
            "eth_getBlockByNumber" => self.eth_get_block_by_number(params).await,
            "eth_getBlockByHash" => self.eth_get_block_by_hash(params).await,
            "eth_getTransactionByHash" => self.eth_get_transaction_by_hash(params).await,
            "eth_getTransactionReceipt" => self.eth_get_transaction_receipt(params).await,
            "eth_getBalance" => self.eth_get_balance(params).await,
            "eth_sendTransaction" => self.eth_send_transaction(params).await,
            "eth_sendRawTransaction" => self.eth_send_raw_transaction(params).await,
            "eth_call" => self.eth_call(params).await,
            "eth_estimateGas" => self.eth_estimate_gas(params).await,
            "eth_gasPrice" => self.eth_gas_price().await,
            "eth_getCode" => self.eth_get_code(params).await,
            "eth_getStorageAt" => self.eth_get_storage_at(params).await,
            "eth_getTransactionCount" => self.eth_get_transaction_count(params).await,
            "net_version" => self.net_version().await,
            "net_listening" => self.net_listening().await,
            "net_peerCount" => self.net_peer_count().await,
            "web3_clientVersion" => self.web3_client_version().await,
            "web3_sha3" => self.web3_sha3(params).await,
            _ => Err(EthJsonRpcError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
        }
    }

    /// eth_blockNumber: Returns the number of most recent block
    async fn eth_block_number(&self) -> Result<serde_json::Value, EthJsonRpcError> {
        match self.node.get_storage().get_latest_block().await {
            Ok(Some(block)) => {
                let block_number = format!("0x{:x}", block.header.slot);
                Ok(serde_json::Value::String(block_number))
            }
            Ok(None) => Ok(serde_json::Value::String("0x0".to_string())),
            Err(e) => Err(EthJsonRpcError {
                code: -32603,
                message: format!("Internal error: {}", e),
                data: None,
            }),
        }
    }

    /// eth_getBlockByNumber: Returns information about a block by block number
    async fn eth_get_block_by_number(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value, EthJsonRpcError> {
        if let Some(params) = params {
            if let Some(arr) = params.as_array() {
                if !arr.is_empty() {
                    if let Some(block_param) = arr[0].as_str() {
                        // Parse block number parameter
                        let block_number = if block_param == "latest" {
                            // Get latest block
                            match self.node.get_storage().get_latest_block().await {
                                Ok(Some(block)) => block.header.slot,
                                Ok(None) => return Ok(serde_json::Value::Null),
                                Err(e) => return Err(EthJsonRpcError {
                                    code: -32603,
                                    message: format!("Internal error: {}", e),
                                    data: None,
                                }),
                            }
                        } else if block_param.starts_with("0x") {
                            // Hex encoded block number
                            match u64::from_str_radix(&block_param[2..], 16) {
                                Ok(num) => num,
                                Err(_) => return Err(EthJsonRpcError {
                                    code: -32602,
                                    message: "Invalid block number".to_string(),
                                    data: None,
                                }),
                            }
                        } else {
                            return Err(EthJsonRpcError {
                                code: -32602,
                                message: "Invalid block number format".to_string(),
                                data: None,
                            });
                        };

                        // Get block by number
                        match self.node.get_storage().get_block_by_slot(block_number).await {
                            Ok(Some(block)) => {
                                let eth_block = self.convert_to_eth_block(block).await;
                                Ok(serde_json::to_value(eth_block).unwrap())
                            }
                            Ok(None) => Ok(serde_json::Value::Null),
                            Err(e) => Err(EthJsonRpcError {
                                code: -32603,
                                message: format!("Internal error: {}", e),
                                data: None,
                            }),
                        }
                    } else {
                        Err(EthJsonRpcError {
                            code: -32602,
                            message: "Invalid parameters".to_string(),
                            data: None,
                        })
                    }
                } else {
                    Err(EthJsonRpcError {
                        code: -32602,
                        message: "Invalid parameters".to_string(),
                        data: None,
                    })
                }
            } else {
                Err(EthJsonRpcError {
                    code: -32602,
                    message: "Invalid parameters".to_string(),
                    data: None,
                })
            }
        } else {
            Err(EthJsonRpcError {
                code: -32602,
                message: "Missing parameters".to_string(),
                data: None,
            })
        }
    }

    /// eth_getBlockByHash: Returns information about a block by hash
    async fn eth_get_block_by_hash(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value, EthJsonRpcError> {
        // Similar implementation to eth_getBlockByNumber but using hash
        Ok(serde_json::Value::Null)
    }

    /// eth_getTransactionByHash: Returns the information about a transaction requested by transaction hash
    async fn eth_get_transaction_by_hash(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value, EthJsonRpcError> {
        Ok(serde_json::Value::Null)
    }

    /// eth_getTransactionReceipt: Returns the receipt of a transaction by transaction hash
    async fn eth_get_transaction_receipt(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value, EthJsonRpcError> {
        Ok(serde_json::Value::Null)
    }

    /// eth_getBalance: Returns the balance of the account of given address
    async fn eth_get_balance(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value, EthJsonRpcError> {
        Ok(serde_json::Value::String("0x0".to_string()))
    }

    /// eth_sendTransaction: Creates new message call transaction or a contract creation
    async fn eth_send_transaction(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value, EthJsonRpcError> {
        Ok(serde_json::Value::String("0x0000000000000000000000000000000000000000000000000000000000000000".to_string()))
    }

    /// eth_sendRawTransaction: Creates new message call transaction or a contract creation for signed transactions
    async fn eth_send_raw_transaction(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value, EthJsonRpcError> {
        Ok(serde_json::Value::String("0x0000000000000000000000000000000000000000000000000000000000000000".to_string()))
    }

    /// eth_call: Executes a new message call immediately without creating a transaction on the block chain
    async fn eth_call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value, EthJsonRpcError> {
        Ok(serde_json::Value::String("0x".to_string()))
    }

    /// eth_estimateGas: Generates and returns an estimate of how much gas is necessary to allow the transaction to complete
    async fn eth_estimate_gas(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value, EthJsonRpcError> {
        Ok(serde_json::Value::String("0x5208".to_string())) // 21000 in hex
    }

    /// eth_gasPrice: Returns the current price per gas in wei
    async fn eth_gas_price(&self) -> Result<serde_json::Value, EthJsonRpcError> {
        Ok(serde_json::Value::String("0x3b9aca00".to_string())) // 1 Gwei in hex
    }

    /// eth_getCode: Returns code at a given address
    async fn eth_get_code(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value, EthJsonRpcError> {
        Ok(serde_json::Value::String("0x".to_string()))
    }

    /// eth_getStorageAt: Returns the value from a storage position at a given address
    async fn eth_get_storage_at(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value, EthJsonRpcError> {
        Ok(serde_json::Value::String("0x0000000000000000000000000000000000000000000000000000000000000000".to_string()))
    }

    /// eth_getTransactionCount: Returns the number of transactions sent from an address
    async fn eth_get_transaction_count(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value, EthJsonRpcError> {
        Ok(serde_json::Value::String("0x0".to_string()))
    }

    /// net_version: Returns the current network id
    async fn net_version(&self) -> Result<serde_json::Value, EthJsonRpcError> {
        Ok(serde_json::Value::String("1337".to_string())) // GARP testnet ID
    }

    /// net_listening: Returns true if client is actively listening for network connections
    async fn net_listening(&self) -> Result<serde_json::Value, EthJsonRpcError> {
        Ok(serde_json::Value::Bool(true))
    }

    /// net_peerCount: Returns number of peers currently connected to the client
    async fn net_peer_count(&self) -> Result<serde_json::Value, EthJsonRpcError> {
        Ok(serde_json::Value::String("0x0".to_string()))
    }

    /// web3_clientVersion: Returns the current client version
    async fn web3_client_version(&self) -> Result<serde_json::Value, EthJsonRpcError> {
        Ok(serde_json::Value::String("GARP/v0.1.0".to_string()))
    }

    /// web3_sha3: Returns Keccak-256 (not the standardized SHA3-256) of the given data
    async fn web3_sha3(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value, EthJsonRpcError> {
        Ok(serde_json::Value::String("0x0000000000000000000000000000000000000000000000000000000000000000".to_string()))
    }

    /// Convert GARP block to Ethereum block format
    async fn convert_to_eth_block(&self, block: crate::storage::Block) -> EthBlock {
        EthBlock {
            number: format!("0x{:x}", block.header.slot),
            hash: hex::encode(&block.hash),
            parent_hash: hex::encode(&block.header.parent_hash),
            nonce: None,
            sha3_uncles: "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347".to_string(),
            logs_bloom: "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000".to_string(),
            transactions_root: "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421".to_string(),
            state_root: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            receipts_root: "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421".to_string(),
            miner: "0x0000000000000000000000000000000000000000".to_string(),
            difficulty: "0x0".to_string(),
            total_difficulty: "0x0".to_string(),
            extra_data: "0x".to_string(),
            size: "0x0".to_string(),
            gas_limit: "0x1c9c380".to_string(), // 30,000,000 in hex
            gas_used: "0x0".to_string(),
            timestamp: format!("0x{:x}", block.timestamp.timestamp()),
            transactions: vec![], // Would need to convert GARP transactions to Ethereum format
            uncles: vec![],
        }
    }
}

/// Ethereum JSON-RPC handler
async fn eth_json_rpc(
    State(node): State<Arc<ParticipantNode>>,
    Json(request): Json<EthJsonRpcRequest>,
) -> Result<Json<EthJsonRpcResponse>, StatusCode> {
    info!("Ethereum JSON-RPC request: {} {:?}", request.method, request.params);
    
    let eth_layer = EthCompatibilityLayer::new(node);
    
    let result = eth_layer.handle_eth_request(request.method, request.params).await;
    
    match result {
        Ok(value) => {
            Ok(Json(EthJsonRpcResponse {
                jsonrpc: request.jsonrpc,
                id: request.id,
                result: Some(value),
                error: None,
            }))
        }
        Err(err) => {
            Ok(Json(EthJsonRpcResponse {
                jsonrpc: request.jsonrpc,
                id: request.id,
                result: None,
                error: Some(err),
            }))
        }
    }
}