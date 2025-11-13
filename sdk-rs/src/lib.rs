use std::time::Duration;

use reqwest::Client as HttpClient;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SdkError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("rpc error {code}: {message}")]
    Rpc { code: i64, message: String },
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'static str,
    id: u64,
    method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(default)]
    data: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum JsonRpcResponse<T> {
    Ok { jsonrpc: String, id: u64, result: T },
    Err { jsonrpc: String, id: u64, error: JsonRpcError },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlockTx {
    pub id: String,
    #[serde(default)]
    pub submitter: Option<String>,
    #[serde(default)]
    pub command_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlockInfo {
    pub slot: i64,
    pub hash: String,
    #[serde(default)]
    pub parent_hash: Option<String>,
    #[serde(default)]
    pub timestamp_ms: Option<i64>,
    #[serde(default)]
    pub leader: Option<String>,
    #[serde(default)]
    pub transactions: Option<Vec<BlockTx>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransactionInfo {
    pub id: String,
    #[serde(default)]
    pub submitter: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct GarpClient {
    base_url: String,
    http: HttpClient,
    timeout: Duration,
}

impl GarpClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self, SdkError> {
        let timeout = Duration::from_secs(10);
        let http = HttpClient::builder().timeout(timeout).build()?;
        Ok(Self { base_url: base_url.into().trim_end_matches('/').to_string(), http, timeout })
    }

    pub fn with_timeout(base_url: impl Into<String>, timeout: Duration) -> Result<Self, SdkError> {
        let http = HttpClient::builder().timeout(timeout).build()?;
        Ok(Self { base_url: base_url.into().trim_end_matches('/').to_string(), http, timeout })
    }

    pub fn with_http_client(base_url: impl Into<String>, http: HttpClient) -> Self {
        let timeout = Duration::from_secs(10);
        Self { base_url: base_url.into().trim_end_matches('/').to_string(), http, timeout }
    }

    async fn rpc<R: DeserializeOwned>(&self, method: &str, params: Option<Value>) -> Result<R, SdkError> {
        let req = JsonRpcRequest { jsonrpc: "2.0", id: 1, method, params };
        let resp = self
            .http
            .post(format!("{}/rpc", self.base_url))
            .json(&req)
            .send()
            .await?;
        let v = resp.json::<JsonRpcResponse<R>>().await?;
        match v {
            JsonRpcResponse::Ok { result, .. } => Ok(result),
            JsonRpcResponse::Err { error, .. } => Err(SdkError::Rpc { code: error.code, message: error.message }),
        }
    }

    // Timing & consensus
    pub async fn get_slot(&self) -> Result<i64, SdkError> {
        self.rpc::<i64>("getSlot", None).await
    }

    pub async fn get_slot_leader(&self) -> Result<String, SdkError> {
        self.rpc::<String>("getSlotLeader", None).await
    }

    // Blocks
    pub async fn get_block_by_slot(&self, slot: i64) -> Result<Option<BlockInfo>, SdkError> {
        self.rpc::<Option<BlockInfo>>("getBlock", Some(json!([slot]))).await
    }

    pub async fn get_block_by_hash(&self, hash_hex: &str) -> Result<Option<BlockInfo>, SdkError> {
        self.rpc::<Option<BlockInfo>>("getBlock", Some(json!([hash_hex]))).await
    }

    // Transactions
    pub async fn get_transaction(&self, tx_id_hex: &str) -> Result<Option<TransactionInfo>, SdkError> {
        self.rpc::<Option<TransactionInfo>>("getTransaction", Some(json!([tx_id_hex]))).await
    }

    pub async fn send_transaction_raw(&self, serialized: &str) -> Result<String, SdkError> {
        self.rpc::<String>("sendTransaction", Some(json!([serialized]))).await
    }

    #[derive(Debug, Clone, Deserialize, Serialize)]
    pub struct SimulationResult {
        pub ok: bool,
        #[serde(default)]
        pub logs: Option<Vec<String>>,
        #[serde(default)]
        pub error: Option<String>,
    }

    pub async fn simulate_transaction_raw(&self, serialized: &str) -> Result<SimulationResult, SdkError> {
        self.rpc::<SimulationResult>("simulateTransaction", Some(json!([serialized]))).await
    }

    // Wallets
    pub async fn get_balance(&self, address_hex: &str) -> Result<serde_json::Value, SdkError> {
        // Balance may be bigint or number; return as JSON value to avoid precision loss
        self.rpc::<serde_json::Value>("getBalance", Some(json!([address_hex]))).await
    }

    // Node info
    pub async fn get_version(&self) -> Result<String, SdkError> {
        self.rpc::<String>("getVersion", None).await
    }

    pub async fn get_health(&self) -> Result<String, SdkError> {
        self.rpc::<String>("getHealth", None).await
    }

    // Batch RPC: returns Vec<serde_json::Value> preserving order
    pub async fn rpc_batch(&self, calls: Vec<(&str, Option<Value>)>) -> Result<Vec<Value>, SdkError> {
        // Build batch payload
        let mut id = 1u64;
        let payload: Vec<JsonRpcRequest> = calls
            .iter()
            .map(|(m, p)| {
                let req = JsonRpcRequest { jsonrpc: "2.0", id, method: m, params: p.clone() };
                id += 1;
                req
            })
            .collect();
        let resp = self
            .http
            .post(format!("{}/rpc", self.base_url))
            .json(&payload)
            .send()
            .await?;
        let v: Vec<JsonRpcResponse<Value>> = resp.json().await?;
        let mut out = Vec::with_capacity(v.len());
        for item in v {
            match item {
                JsonRpcResponse::Ok { result, .. } => out.push(result),
                JsonRpcResponse::Err { error, .. } => {
                    return Err(SdkError::Rpc { code: error.code, message: error.message })
                }
            }
        }
        Ok(out)
    }
}