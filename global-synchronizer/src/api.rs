use axum::{routing::{get, post}, extract::{Path, Json as AxumJson}, response::Json, Router};
use axum::middleware;
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use serde_json::Value as JsonValue;
use crate::{GlobalSynchronizer, storage::BlockInfo};
use crate::cross_domain::CrossDomainTransaction;
use crate::validator::{ValidatorInfo, ValidatorStatus};
use crate::bridge::{BridgeTransaction, BridgeTransactionStatus, AssetMapping, BridgeValidator};
use garp_common::types::TransactionId;
use uuid::Uuid;
use hex;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use chrono::Utc;

#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

pub fn create_router(sync: Arc<GlobalSynchronizer>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/status", get(status_handler(sync.clone())))
        .route("/api/v1/status/consensus", get(consensus_status_handler(sync.clone())))
        .route("/api/v1/status/metrics", get(metrics_handler(sync.clone())))
        .route("/metrics", get(prometheus_metrics_handler(sync.clone())))
        .route("/api/v1/blocks/latest", get(latest_block_handler(sync.clone())))
        .route("/api/v1/blocks/:height", get(block_by_height_handler(sync.clone())))
        .route("/api/v1/blocks/:height/details", get(block_details_handler(sync.clone())))
        .route("/api/v1/blocks/:height/transactions", get(block_transactions_handler(sync.clone())))
        .route("/api/v1/mempool", get(mempool_handler(sync.clone())))
        .route("/api/v1/transactions/:id/status", get(tx_status_handler(sync.clone())))
        .route("/api/v1/transactions/:id/details", get(tx_details_handler(sync.clone())))
        .route("/api/v1/transactions", post(submit_transaction_handler(sync.clone())))
        .route("/api/v1/transactions/signed", post(submit_signed_transaction_handler(sync.clone())))
        .route("/api/v1/validators", get(validators_list_handler(sync.clone())).post(validators_add_handler(sync.clone())))
        .route("/api/v1/validators/:id", axum::routing::delete(validators_remove_handler(sync.clone())))
        .route("/api/v1/validators/:id/status", axum::routing::patch(validators_update_status_handler(sync.clone())))
        // Bridge endpoints
        .route("/api/v1/bridge/transfer", post(initiate_bridge_transfer_handler(sync.clone())))
        .route("/api/v1/bridge/transfer/:id", get(get_bridge_transfer_handler(sync.clone())))
        .route("/api/v1/bridge/transfer/:id/status", get(get_bridge_transfer_status_handler(sync.clone())))
        .route("/api/v1/bridge/assets", post(add_asset_mapping_handler(sync.clone())))
        .route("/api/v1/bridge/assets/:source_chain/:source_asset/:target_chain", get(get_asset_mapping_handler(sync.clone())))
        .route("/api/v1/bridge/validators", post(add_validator_handler(sync.clone())))
        .route("/api/v1/bridge/validators/:id", get(get_validator_handler(sync.clone())))
        // Security: simple bearer token auth and concurrency limits
        .layer(middleware::from_fn(auth_middleware))
        .layer(tower::limit::ConcurrencyLimitLayer::new(64))
}

async fn health() -> Json<ApiResponse<&'static str>> {
    Json(ApiResponse { success: true, data: Some("OK"), error: None })
}

fn status_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(), axum::body::Body> {
    axum::routing::get(move || {
        let sync = sync.clone();
        async move {
            let state = sync.get_state().await;
            Json(ApiResponse { success: true, data: Some(state), error: None })
        }
    })
}

#[derive(Serialize)]
struct ConsensusStatusDto {
    total_proposals: u64,
    successful_consensus: u64,
    failed_consensus: u64,
    view_changes: u64,
    avg_consensus_time_ms: f64,
    current_view: u64,
    active_sessions: usize,
}

fn consensus_status_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(), axum::body::Body> {
    axum::routing::get(move || {
        let sync = sync.clone();
        async move {
            match sync.get_consensus_metrics().await {
                Ok(snap) => {
                    let dto = ConsensusStatusDto {
                        total_proposals: snap.total_proposals,
                        successful_consensus: snap.successful_consensus,
                        failed_consensus: snap.failed_consensus,
                        view_changes: snap.view_changes,
                        avg_consensus_time_ms: snap.avg_consensus_time_ms,
                        current_view: snap.current_view,
                        active_sessions: snap.active_sessions,
                    };
                    Json(ApiResponse { success: true, data: Some(dto), error: None })
                }
                Err(e) => Json(ApiResponse::<serde_json::Value> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

#[derive(Serialize)]
struct MetricsDto {
    total_transactions: u64,
    successful_transactions: u64,
    failed_transactions: u64,
    active_domains: u64,
    consensus_rounds: u64,
    settlement_operations: u64,
    network_messages: u64,
    storage_operations: u64,
    uptime_seconds: u64,
    avg_transaction_time_ms: f64,
}

fn metrics_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(), axum::body::Body> {
    axum::routing::get(move || {
        let sync = sync.clone();
        async move {
            match sync.get_metrics().await {
                Ok(m) => {
                    let dto = MetricsDto {
                        total_transactions: *m.total_transactions.read().await,
                        successful_transactions: *m.successful_transactions.read().await,
                        failed_transactions: *m.failed_transactions.read().await,
                        active_domains: *m.active_domains.read().await,
                        consensus_rounds: *m.consensus_rounds.read().await,
                        settlement_operations: *m.settlement_operations.read().await,
                        network_messages: *m.network_messages.read().await,
                        storage_operations: *m.storage_operations.read().await,
                        uptime_seconds: m.uptime.read().await.as_secs(),
                        avg_transaction_time_ms: *m.avg_transaction_time.read().await,
                    };
                    Json(ApiResponse { success: true, data: Some(dto), error: None })
                }
                Err(e) => Json(ApiResponse::<serde_json::Value> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

fn prometheus_metrics_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(), axum::body::Body> {
    axum::routing::get(move || {
        let sync = sync.clone();
        async move {
            let mut body = String::new();
            if let Ok(m) = sync.get_metrics().await {
                let total = *m.total_transactions.read().await;
                let succ = *m.successful_transactions.read().await;
                let fail = *m.failed_transactions.read().await;
                let active = *m.active_domains.read().await;
                let consensus_rounds = *m.consensus_rounds.read().await;
                body.push_str(&format!("# HELP global_sync_total_transactions Total transactions submitted\n"));
                body.push_str(&format!("# TYPE global_sync_total_transactions counter\n"));
                body.push_str(&format!("global_sync_total_transactions {}\n", total));
                body.push_str(&format!("# HELP global_sync_successful_transactions Successful transactions\n"));
                body.push_str(&format!("# TYPE global_sync_successful_transactions counter\n"));
                body.push_str(&format!("global_sync_successful_transactions {}\n", succ));
                body.push_str(&format!("# HELP global_sync_failed_transactions Failed transactions\n"));
                body.push_str(&format!("# TYPE global_sync_failed_transactions counter\n"));
                body.push_str(&format!("global_sync_failed_transactions {}\n", fail));
                body.push_str(&format!("# HELP global_sync_active_domains Active domains\n"));
                body.push_str(&format!("# TYPE global_sync_active_domains gauge\n"));
                body.push_str(&format!("global_sync_active_domains {}\n", active));
                body.push_str(&format!("# HELP global_sync_consensus_rounds Consensus rounds\n"));
                body.push_str(&format!("# TYPE global_sync_consensus_rounds counter\n"));
                body.push_str(&format!("global_sync_consensus_rounds {}\n", consensus_rounds));
            }
            let headers = [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")];
            let mut resp = axum::response::Response::new(axum::body::Body::from(body));
            for (k, v) in headers {
                resp.headers_mut().insert(k, axum::http::HeaderValue::from_static(v));
            }
            resp
        }
    })
}

fn latest_block_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(), axum::body::Body> {
    axum::routing::get(move || {
        let sync = sync.clone();
        async move {
            match sync.get_latest_block().await {
                Ok(Some(info)) => Json(ApiResponse { success: true, data: Some(info), error: None }),
                Ok(None) => Json(ApiResponse::<BlockInfo> { success: false, data: None, error: Some("No blocks".into()) }),
                Err(e) => Json(ApiResponse::<BlockInfo> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

fn block_by_height_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(Path<u64>,), axum::body::Body> {
    axum::routing::get(move |Path(height): Path<u64>| {
        let sync = sync.clone();
        async move {
            match sync.get_block_by_height(height).await {
                Ok(Some(info)) => Json(ApiResponse { success: true, data: Some(info), error: None }),
                Ok(None) => Json(ApiResponse::<BlockInfo> { success: false, data: None, error: Some("Not found".into()) }),
                Err(e) => Json(ApiResponse::<BlockInfo> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

#[derive(Serialize)]
struct BlockDetailsDto {
    info: BlockInfo,
    transaction_ids: Vec<String>,
}

fn block_details_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(Path<u64>,), axum::body::Body> {
    axum::routing::get(move |Path(height): Path<u64>| {
        let sync = sync.clone();
        async move {
            match (sync.get_block_by_height(height).await, sync.get_transactions_by_height(height).await) {
                (Ok(Some(info)), Ok(txids)) => {
                    let dto = BlockDetailsDto { info, transaction_ids: txids.into_iter().map(|t| t.0.to_string()).collect() };
                    Json(ApiResponse { success: true, data: Some(dto), error: None })
                }
                (Ok(None), _) => Json(ApiResponse::<BlockDetailsDto> { success: false, data: None, error: Some("Not found".into()) }),
                (_, Err(e)) => Json(ApiResponse::<BlockDetailsDto> { success: false, data: None, error: Some(format!("{}", e)) }),
                (Err(e), _) => Json(ApiResponse::<BlockDetailsDto> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

fn mempool_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(), axum::body::Body> {
    axum::routing::get(move || {
        let sync = sync.clone();
        async move {
            let ids = sync.get_mempool().await;
            Json(ApiResponse { success: true, data: Some(serde_json::json!({"pending": ids})), error: None })
        }
    })
}

#[derive(Serialize)]
struct StoredTransactionDto {
    transaction_id: String,
    transaction_type: String,
    source_domain: String,
    target_domains: Vec<String>,
    status: String,
    created_at: u64,
    updated_at: u64,
    block_height: Option<u64>,
    block_hash: Option<String>,
    transaction_data_hex: String,
    transaction: Option<JsonValue>,
}

fn block_transactions_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(Path<u64>,), axum::body::Body> {
    axum::routing::get(move |Path(height): Path<u64>| {
        let sync = sync.clone();
        async move {
            match sync.get_transactions_by_height(height).await {
                Ok(txids) => {
                    let mut dtos = Vec::new();
                    for tid in txids {
                        match sync.get_transaction(&tid).await {
                            Ok(Some(st)) => {
                                dtos.push(StoredTransactionDto {
                                    transaction_id: st.transaction_id.0.to_string(),
                                    transaction_type: st.transaction_type,
                                    source_domain: st.source_domain.0.clone(),
                                    target_domains: st.target_domains.into_iter().map(|d| d.0).collect(),
                                    status: format!("{:?}", st.status),
                                    created_at: st.created_at.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                                    updated_at: st.updated_at.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                                    block_height: st.block_height,
                                    block_hash: st.block_hash.as_ref().map(|h| hex::encode(h)),
                                    transaction_data_hex: hex::encode(st.transaction_data),
                                    transaction: serde_json::from_slice(&st.transaction_data).ok(),
                                });
                            }
                            _ => {}
                        }
                    }
                    Json(ApiResponse { success: true, data: Some(dtos), error: None })
                }
                Err(e) => Json(ApiResponse::<Vec<StoredTransactionDto>> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}


fn submit_transaction_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(AxumJson<CrossDomainTransaction>,), axum::body::Body> {
    axum::routing::post(move |AxumJson(tx): AxumJson<CrossDomainTransaction>| {
        let sync = sync.clone();
        async move {
            // Basic validation before submission
            let mut rejected_reason = None;
            if tx.target_domains.is_empty() { rejected_reason = Some("no target domains".to_string()); }
            if tx.required_confirmations == 0 { rejected_reason = Some("required confirmations must be > 0".to_string()); }
            if tx.required_confirmations > tx.target_domains.len() { rejected_reason = Some("required confirmations exceeds target domains".to_string()); }
            if tx.data.len() > 1_048_576 { rejected_reason = Some("payload too large".to_string()); }
            if rejected_reason.is_some() {
                return Json(ApiResponse { success: false, data: Some(serde_json::json!({"status":"Rejected","reason": rejected_reason})), error: None });
            }

            match sync.submit_transaction(tx.clone()).await {
                Ok(id) => Json(ApiResponse { success: true, data: Some(serde_json::json!({"status":"Accepted","transaction_id": id.0.to_string()})), error: None }),
                Err(e) => Json(ApiResponse::<serde_json::Value> { success: false, data: Some(serde_json::json!({"status":"Rejected"})), error: Some(format!("{}", e)) }),
            }
        }
    })
}

#[derive(serde::Deserialize)]
struct SignedTransactionDto {
    tx: CrossDomainTransaction,
    public_key_hex: String,
    signature_hex: String,
}

fn submit_signed_transaction_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(AxumJson<SignedTransactionDto>,), axum::body::Body> {
    axum::routing::post(move |AxumJson(signed): AxumJson<SignedTransactionDto>| {
        let sync = sync.clone();
        async move {
            // Verify signature using ed25519 over canonical message
            let message = canonical_tx_message(&signed.tx);
            match verify_ed25519_signature(&signed.public_key_hex, &signed.signature_hex, &message) {
                Ok(true) => {
                    match sync.submit_transaction(signed.tx.clone()).await {
                        Ok(id) => Json(ApiResponse { success: true, data: Some(serde_json::json!({"status":"Accepted","transaction_id": id.0.to_string()})), error: None }),
                        Err(e) => Json(ApiResponse::<serde_json::Value> { success: false, data: Some(serde_json::json!({"status":"Rejected"})), error: Some(format!("{}", e)) }),
                    }
                }
                Ok(false) => Json(ApiResponse::<serde_json::Value> { success: false, data: Some(serde_json::json!({"status":"Rejected","reason":"invalid signature"})), error: None }),
                Err(err) => Json(ApiResponse::<serde_json::Value> { success: false, data: Some(serde_json::json!({"status":"Rejected"})), error: Some(format!("{}", err)) }),
            }
        }
    })
}

fn canonical_tx_message(tx: &CrossDomainTransaction) -> Vec<u8> {
    let mut s = String::new();
    s.push_str(&tx.transaction_id.0.to_string());
    s.push('|');
    s.push_str(&tx.source_domain.0);
    s.push('|');
    s.push_str(&tx.target_domains.iter().map(|d| d.0.clone()).collect::<Vec<_>>().join(","));
    s.push('|');
    s.push_str(&hex::encode(&tx.data));
    s.push('|');
    s.push_str(&tx.required_confirmations.to_string());
    s.into_bytes()
}

fn verify_ed25519_signature(pk_hex: &str, sig_hex: &str, message: &[u8]) -> Result<bool, String> {
    use ed25519_dalek::{Verifier, Signature, PublicKey};
    let pk_bytes = hex::decode(pk_hex).map_err(|e| format!("invalid public key hex: {}", e))?;
    let sig_bytes = hex::decode(sig_hex).map_err(|e| format!("invalid signature hex: {}", e))?;
    let pk_arr: [u8; 32] = pk_bytes.try_into().map_err(|_| "invalid public key length")?;
    let sig_arr: [u8; 64] = sig_bytes.try_into().map_err(|_| "invalid signature length")?;
    let pk = PublicKey::from_bytes(&pk_arr).map_err(|e| format!("invalid public key: {}", e))?;
    let sig = Signature::from_bytes(&sig_arr).map_err(|e| format!("invalid signature: {}", e))?;
    pk.verify_strict(message, &sig).map(|_| true).map_err(|e| format!("signature verification failed: {}", e))
}

#[derive(serde::Deserialize)]
struct ValidatorAddDto {
    id: String,
    public_key_hex: String,
    voting_power: u64,
    metadata: Option<std::collections::HashMap<String, String>>,
}

fn validators_list_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(), axum::body::Body> {
    axum::routing::get(move || {
        let sync = sync.clone();
        async move {
            match sync.list_validators().await {
                Ok(list) => Json(ApiResponse { success: true, data: Some(list), error: None }),
                Err(e) => Json(ApiResponse::<serde_json::Value> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

fn validators_add_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(AxumJson<ValidatorAddDto>,), axum::body::Body> {
    axum::routing::post(move |AxumJson(dto): AxumJson<ValidatorAddDto>| {
        let sync = sync.clone();
        async move {
            let id = garp_common::types::ParticipantId::new(&dto.id);
            let mut v = ValidatorInfo {
                id,
                public_key_hex: dto.public_key_hex.clone(),
                voting_power: dto.voting_power,
                status: ValidatorStatus::Active,
                joined_at: chrono::Utc::now(),
                metadata: dto.metadata.unwrap_or_default(),
            };
            match sync.add_validator(v).await {
                Ok(_) => Json(ApiResponse { success: true, data: Some(serde_json::json!({"added": dto.id})), error: None }),
                Err(e) => Json(ApiResponse::<serde_json::Value> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

fn validators_remove_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(Path<String>,), axum::body::Body> {
    axum::routing::delete(move |Path(id_str): Path<String>| {
        let sync = sync.clone();
        async move {
            let id = garp_common::types::ParticipantId::new(&id_str);
            match sync.remove_validator(id).await {
                Ok(_) => Json(ApiResponse { success: true, data: Some(serde_json::json!({"removed": id_str})), error: None }),
                Err(e) => Json(ApiResponse::<serde_json::Value> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

#[derive(serde::Deserialize)]
struct UpdateValidatorStatusDto { status: String }

fn validators_update_status_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(Path<String>, AxumJson<UpdateValidatorStatusDto>), axum::body::Body> {
    axum::routing::patch(move |Path(id_str): Path<String>, AxumJson(dto): AxumJson<UpdateValidatorStatusDto>| {
        let sync = sync.clone();
        async move {
            let id = garp_common::types::ParticipantId::new(&id_str);
            let status = match dto.status.to_lowercase().as_str() {
                "active" => ValidatorStatus::Active,
                "inactive" => ValidatorStatus::Inactive,
                "jailed" => ValidatorStatus::Jailed,
                _ => return Json(ApiResponse::<serde_json::Value> { success: false, data: None, error: Some("invalid status".into()) }),
            };
            match sync.update_validator_status(id, status).await {
                Ok(_) => Json(ApiResponse { success: true, data: Some(serde_json::json!({"updated": id_str})), error: None }),
                Err(e) => Json(ApiResponse::<serde_json::Value> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

fn tx_status_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(Path<String>,), axum::body::Body> {
    axum::routing::get(move |Path(id_str): Path<String>| {
        let sync = sync.clone();
        async move {
            match Uuid::parse_str(&id_str) {
                Ok(u) => {
                    let tid = TransactionId(u);
                    match sync.get_transaction_status(&tid).await {
                        Ok(Some(status)) => Json(ApiResponse { success: true, data: Some(serde_json::json!({"status": status})), error: None }),
                        Ok(None) => Json(ApiResponse::<serde_json::Value> { success: false, data: None, error: Some("Not found".into()) }),
                        Err(e) => Json(ApiResponse::<serde_json::Value> { success: false, data: None, error: Some(format!("{}", e)) }),
                    }
                }
                Err(_) => Json(ApiResponse::<serde_json::Value> { success: false, data: None, error: Some("Invalid transaction id".into()) }),
            }
        }
    })
}

#[derive(Serialize)]
struct StoredTransactionDto {
    transaction_id: String,
    transaction_type: String,
    source_domain: String,
    target_domains: Vec<String>,
    status: String,
    created_at: u64,
    updated_at: u64,
    block_height: Option<u64>,
    block_hash: Option<String>,
    transaction_data_hex: String,
    transaction: Option<JsonValue>,
}

fn tx_details_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(Path<String>,), axum::body::Body> {
    axum::routing::get(move |Path(id_str): Path<String>| {
        let sync = sync.clone();
        async move {
            match Uuid::parse_str(&id_str) {
                Ok(u) => {
                    let tid = TransactionId(u);
                    match sync.get_transaction(&tid).await {
                        Ok(Some(st)) => {
                            let dto = StoredTransactionDto {
                                transaction_id: st.transaction_id.0.to_string(),
                                transaction_type: st.transaction_type,
                                source_domain: st.source_domain.0.clone(),
                                target_domains: st.target_domains.into_iter().map(|d| d.0).collect(),
                                status: format!("{:?}", st.status),
                                created_at: st.created_at.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                                updated_at: st.updated_at.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                                block_height: st.block_height,
                                block_hash: st.block_hash.as_ref().map(|h| hex::encode(h)),
                                transaction_data_hex: hex::encode(st.transaction_data),
                                transaction: serde_json::from_slice(&st.transaction_data).ok(),
                            };
                            Json(ApiResponse { success: true, data: Some(dto), error: None })
                        }
                        Ok(None) => Json(ApiResponse::<StoredTransactionDto> { success: false, data: None, error: Some("Not found".into()) }),
                        Err(e) => Json(ApiResponse::<StoredTransactionDto> { success: false, data: None, error: Some(format!("{}", e)) }),
                    }
                }
                Err(_) => Json(ApiResponse::<StoredTransactionDto> { success: false, data: None, error: Some("Invalid transaction id".into()) }),
            }
        }
    })
}

// Bridge API handlers
#[derive(Deserialize)]
struct InitiateBridgeTransferRequest {
    source_chain: String,
    source_tx_id: String,
    target_chain: String,
    amount: u64,
    source_address: String,
    target_address: String,
    asset_id: String,
}

#[derive(Serialize)]
struct InitiateBridgeTransferResponse {
    bridge_tx_id: String,
}

fn initiate_bridge_transfer_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(), axum::body::Body> {
    axum::routing::post(move |AxumJson(request): AxumJson<InitiateBridgeTransferRequest>| {
        let sync = sync.clone();
        async move {
            match sync.initiate_bridge_transfer(
                request.source_chain,
                request.source_tx_id,
                request.target_chain,
                request.amount,
                request.source_address,
                request.target_address,
                request.asset_id,
            ).await {
                Ok(bridge_tx_id) => {
                    let response = InitiateBridgeTransferResponse { bridge_tx_id };
                    Json(ApiResponse { success: true, data: Some(response), error: None })
                }
                Err(e) => Json(ApiResponse::<serde_json::Value> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

#[derive(Serialize)]
struct BridgeTransferDto {
    bridge_tx_id: String,
    source_chain: String,
    source_tx_id: String,
    target_chain: String,
    target_tx_id: Option<String>,
    amount: u64,
    source_address: String,
    target_address: String,
    status: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

fn get_bridge_transfer_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(Path<String>,), axum::body::Body> {
    axum::routing::get(move |Path(bridge_tx_id): Path<String>| {
        let sync = sync.clone();
        async move {
            match sync.get_bridge_transaction(&bridge_tx_id).await {
                Ok(Some(bridge_tx)) => {
                    let dto = BridgeTransferDto {
                        bridge_tx_id: bridge_tx.bridge_tx_id,
                        source_chain: bridge_tx.source_chain,
                        source_tx_id: bridge_tx.source_tx_id,
                        target_chain: bridge_tx.target_chain,
                        target_tx_id: bridge_tx.target_tx_id,
                        amount: match &bridge_tx.bridge_type {
                            crate::bridge::BridgeTransactionType::AssetTransfer { .. } => bridge_tx.amount,
                            _ => 0,
                        },
                        source_address: bridge_tx.source_address,
                        target_address: bridge_tx.target_address,
                        status: format!("{:?}", bridge_tx.status),
                        created_at: bridge_tx.created_at,
                        updated_at: bridge_tx.updated_at,
                    };
                    Json(ApiResponse { success: true, data: Some(dto), error: None })
                }
                Ok(None) => Json(ApiResponse::<BridgeTransferDto> { success: false, data: None, error: Some("Bridge transaction not found".into()) }),
                Err(e) => Json(ApiResponse::<BridgeTransferDto> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

fn get_bridge_transfer_status_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(Path<String>,), axum::body::Body> {
    axum::routing::get(move |Path(bridge_tx_id): Path<String>| {
        let sync = sync.clone();
        async move {
            match sync.get_bridge_transaction_status(&bridge_tx_id).await {
                Ok(status) => {
                    let status_str = format!("{:?}", status);
                    Json(ApiResponse { success: true, data: Some(status_str), error: None })
                }
                Err(e) => Json(ApiResponse::<String> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

#[derive(Deserialize)]
struct AddAssetMappingRequest {
    source_asset_id: String,
    source_chain: String,
    target_asset_id: String,
    target_chain: String,
    conversion_rate: f64,
}

fn add_asset_mapping_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(), axum::body::Body> {
    axum::routing::post(move |AxumJson(request): AxumJson<AddAssetMappingRequest>| {
        let sync = sync.clone();
        async move {
            let mapping = AssetMapping {
                source_asset_id: request.source_asset_id,
                source_chain: request.source_chain,
                target_asset_id: request.target_asset_id,
                target_chain: request.target_chain,
                conversion_rate: request.conversion_rate,
                last_updated: Utc::now(),
            };
            
            match sync.add_asset_mapping(mapping).await {
                Ok(()) => Json(ApiResponse { success: true, data: Some("Asset mapping added successfully".to_string()), error: None }),
                Err(e) => Json(ApiResponse::<String> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

fn get_asset_mapping_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(Path<(String, String, String)>,), axum::body::Body> {
    axum::routing::get(move |Path((source_chain, source_asset_id, target_chain)): Path<(String, String, String)>| {
        let sync = sync.clone();
        async move {
            match sync.get_asset_mapping(&source_chain, &source_asset_id, &target_chain).await {
                Ok(Some(mapping)) => Json(ApiResponse { success: true, data: Some(mapping), error: None }),
                Ok(None) => Json(ApiResponse::<AssetMapping> { success: false, data: None, error: Some("Asset mapping not found".into()) }),
                Err(e) => Json(ApiResponse::<AssetMapping> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

#[derive(Deserialize)]
struct AddValidatorRequest {
    validator_id: String,
    supported_chains: Vec<String>,
    public_key: String,
    status: String,
    reputation: u64,
}

fn add_validator_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(), axum::body::Body> {
    axum::routing::post(move |AxumJson(request): AxumJson<AddValidatorRequest>| {
        let sync = sync.clone();
        async move {
            let status = match request.status.as_str() {
                "Active" => crate::bridge::ValidatorStatus::Active,
                "Inactive" => crate::bridge::ValidatorStatus::Inactive,
                "Slashed" => crate::bridge::ValidatorStatus::Slashed,
                _ => crate::bridge::ValidatorStatus::Inactive,
            };
            
            let validator = BridgeValidator {
                validator_id: request.validator_id,
                supported_chains: request.supported_chains.into_iter().collect(),
                public_key: request.public_key,
                status,
                reputation: request.reputation,
                last_seen: Utc::now(),
            };
            
            match sync.add_bridge_validator(validator).await {
                Ok(()) => Json(ApiResponse { success: true, data: Some("Validator added successfully".to_string()), error: None }),
                Err(e) => Json(ApiResponse::<String> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

fn get_validator_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(Path<String>,), axum::body::Body> {
    axum::routing::get(move |Path(validator_id): Path<String>| {
        let sync = sync.clone();
        async move {
            match sync.get_bridge_validator(&validator_id).await {
                Ok(Some(validator)) => Json(ApiResponse { success: true, data: Some(validator), error: None }),
                Ok(None) => Json(ApiResponse::<BridgeValidator> { success: false, data: None, error: Some("Validator not found".into()) }),
                Err(e) => Json(ApiResponse::<BridgeValidator> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

async fn auth_middleware<B>(req: axum::http::Request<B>, next: middleware::Next<B>) -> Result<axum::response::Response, axum::http::StatusCode> {
    // Per-IP throttle (simple in-memory)
    static IP_THROTTLE: OnceLock<Mutex<HashMap<String, (u32, i64)>>> = OnceLock::new();
    fn client_ip_from_req(req: &axum::http::Request<axum::body::Body>) -> String {
        let headers = req.headers();
        if let Some(ip) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            ip.split(',').next().unwrap_or("unknown").trim().to_string()
        } else if let Some(ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
            ip.trim().to_string()
        } else {
            "unknown".to_string()
        }
    }
    fn check_ip_throttle(req: &axum::http::Request<axum::body::Body>) -> Result<(), axum::http::StatusCode> {
        let rpm: u32 = std::env::var("SYNC_RATE_LIMIT_RPM").ok().and_then(|v| v.parse().ok()).unwrap_or(120);
        let ip = client_ip_from_req(req);
        let map = IP_THROTTLE.get_or_init(|| Mutex::new(HashMap::new()));
        let mut guard = map.lock().unwrap();
        let now_ms = Utc::now().timestamp_millis();
        let window_ms: i64 = 60_000;
        let entry = guard.entry(ip.clone()).or_insert((0u32, now_ms));
        let (ref mut count, ref mut start_ms) = *entry;
        if now_ms - *start_ms >= window_ms {
            *start_ms = now_ms;
            *count = 0;
        }
        *count += 1;
        if *count > rpm {
            tracing::warn!("Rate limit exceeded", ip = %ip, count = *count);
            return Err(axum::http::StatusCode::TOO_MANY_REQUESTS);
        }
        Ok(())
    }

    // Apply per-IP throttle before auth
    let empty_body_req = req.map(|_| axum::body::Body::empty());
    if let Err(code) = check_ip_throttle(&empty_body_req) {
        return Err(code);
    }
    // Expect Authorization: Bearer <token>
    let required = std::env::var("SYNC_API_TOKEN").ok();
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
        let ip = client_ip_from_req(&empty_body_req);
        let path = empty_body_req.uri().path().to_string();
        tracing::warn!("Unauthorized request", ip = %ip, path = %path);
        return Err(axum::http::StatusCode::UNAUTHORIZED);
    }
    // If no token configured, allow
    Ok(next.run(req).await)
}