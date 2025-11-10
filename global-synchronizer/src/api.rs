use axum::{routing::{get, post}, extract::{Path, Json as AxumJson}, response::Json, Router};
use axum::middleware;
use std::sync::Arc;
use serde::Serialize;
use serde_json::Value as JsonValue;
use crate::{GlobalSynchronizer, storage::BlockInfo};
use crate::cross_domain::CrossDomainTransaction;
use garp_common::types::TransactionId;
use uuid::Uuid;
use hex;

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
        .route("/api/v1/blocks/latest", get(latest_block_handler(sync.clone())))
        .route("/api/v1/blocks/:height", get(block_by_height_handler(sync.clone())))
        .route("/api/v1/blocks/:height/details", get(block_details_handler(sync)))
        .route("/api/v1/blocks/:height/transactions", get(block_transactions_handler(sync.clone())))
        .route("/api/v1/mempool", get(mempool_handler(sync.clone())))
        .route("/api/v1/transactions/:id/status", get(tx_status_handler(sync.clone())))
        .route("/api/v1/transactions/:id/details", get(tx_details_handler(sync.clone())))
        .route("/api/v1/transactions", post(submit_transaction_handler(sync)))
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
                                    block_hash: st.block_hash.map(|h| h.0),
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
                                block_hash: st.block_hash.map(|h| h.0),
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
async fn auth_middleware<B>(req: axum::http::Request<B>, next: middleware::Next<B>) -> Result<axum::response::Response, axum::http::StatusCode> {
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
        return Err(axum::http::StatusCode::UNAUTHORIZED);
    }
    // If no token configured, allow
    Ok(next.run(req).await)
}