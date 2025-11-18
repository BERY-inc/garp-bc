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
        // Wallet endpoints
        .route("/api/v1/wallets", post(create_wallet_handler(sync.clone())))
        .route("/api/v1/wallets/:id", get(get_wallet_handler(sync.clone())))
        .route("/api/v1/wallets", get(list_wallets_handler(sync.clone())))
        // Oracle endpoints
        .route("/api/v1/oracle/price/:symbol", get(get_asset_price_handler(sync.clone())))
        .route("/api/v1/oracle/prices", get(get_all_prices_handler(sync.clone())))
        .route("/api/v1/oracle/conversion/:from/:to", get(get_conversion_rate_handler(sync.clone())))
        // Liquidity pool endpoints
        .route("/api/v1/pool/add", post(add_liquidity_handler(sync.clone())))
        .route("/api/v1/pool/remove", post(remove_liquidity_handler(sync.clone())))
        .route("/api/v1/pool/swap", post(swap_tokens_handler(sync.clone())))
        .route("/api/v1/pool/info", get(get_pool_info_handler(sync.clone())))
        .route("/api/v1/pool/tvl", get(get_tvl_handler(sync.clone())))
        // Security: simple bearer token auth and concurrency limits
        .layer(middleware::from_fn(auth_middleware))
        .layer(tower::limit::ConcurrencyLimitLayer::new(64))
}

// Oracle API handlers
#[derive(Serialize)]
struct PriceDto {
    symbol: String,
    price: f64,
}

fn get_asset_price_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(Path<String>,), axum::body::Body> {
    axum::routing::get(move |Path(symbol): Path<String>| {
        let sync = sync.clone();
        async move {
            match sync.get_asset_price(&symbol).await {
                Ok(Some(price)) => {
                    let dto = PriceDto {
                        symbol,
                        price,
                    };
                    Json(ApiResponse { success: true, data: Some(dto), error: None })
                }
                Ok(None) => Json(ApiResponse::<PriceDto> { success: false, data: None, error: Some("Price not found".into()) }),
                Err(e) => Json(ApiResponse::<PriceDto> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

fn get_all_prices_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(), axum::body::Body> {
    axum::routing::get(move || {
        let sync = sync.clone();
        async move {
            match sync.get_all_prices().await {
                Ok(prices) => {
                    let dtos: Vec<PriceDto> = prices.into_iter().map(|(symbol, price)| PriceDto {
                        symbol,
                        price,
                    }).collect();
                    Json(ApiResponse { success: true, data: Some(dtos), error: None })
                }
                Err(e) => Json(ApiResponse::<Vec<PriceDto>> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

fn get_conversion_rate_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(Path<(String, String)>,), axum::body::Body> {
    axum::routing::get(move |Path((from, to)): Path<(String, String)>| {
        let sync = sync.clone();
        async move {
            match sync.get_conversion_rate(&from, &to).await {
                Ok(rate) => {
                    let dto = serde_json::json!({
                        "from": from,
                        "to": to,
                        "rate": rate
                    });
                    Json(ApiResponse { success: true, data: Some(dto), error: None })
                }
                Err(e) => Json(ApiResponse::<serde_json::Value> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

// Liquidity Pool API handlers
#[derive(Deserialize)]
struct AddLiquidityRequest {
    asset: String,
    amount: f64,
}

#[derive(Deserialize)]
struct RemoveLiquidityRequest {
    asset: String,
    amount: f64,
}

#[derive(Deserialize)]
struct SwapTokensRequest {
    from_asset: String,
    to_asset: String,
    amount: f64,
}

#[derive(Serialize)]
struct PoolInfoDto {
    asset_pair: String,
    reserves: HashMap<String, f64>,
    tvl: f64,
    fee_rate: f64,
}

fn add_liquidity_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(), axum::body::Body> {
    axum::routing::post(move |AxumJson(request): AxumJson<AddLiquidityRequest>| {
        let sync = sync.clone();
        async move {
            match sync.add_liquidity(request.asset, request.amount).await {
                Ok(()) => Json(ApiResponse { success: true, data: Some("Liquidity added successfully".to_string()), error: None }),
                Err(e) => Json(ApiResponse::<String> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

fn remove_liquidity_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(), axum::body::Body> {
    axum::routing::post(move |AxumJson(request): AxumJson<RemoveLiquidityRequest>| {
        let sync = sync.clone();
        async move {
            match sync.remove_liquidity(request.asset, request.amount).await {
                Ok(amount) => {
                    let dto = serde_json::json!({
                        "removed_amount": amount
                    });
                    Json(ApiResponse { success: true, data: Some(dto), error: None })
                }
                Err(e) => Json(ApiResponse::<serde_json::Value> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

fn swap_tokens_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(), axum::body::Body> {
    axum::routing::post(move |AxumJson(request): AxumJson<SwapTokensRequest>| {
        let sync = sync.clone();
        async move {
            match sync.swap_tokens(request.from_asset, request.to_asset, request.amount).await {
                Ok(amount_out) => {
                    let dto = serde_json::json!({
                        "amount_out": amount_out
                    });
                    Json(ApiResponse { success: true, data: Some(dto), error: None })
                }
                Err(e) => Json(ApiResponse::<serde_json::Value> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

fn get_pool_info_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(), axum::body::Body> {
    axum::routing::get(move || {
        let sync = sync.clone();
        async move {
            match sync.get_pool_info().await {
                Ok(pool_info) => {
                    let dto = PoolInfoDto {
                        asset_pair: pool_info.asset_pair,
                        reserves: pool_info.reserves,
                        tvl: pool_info.tvl,
                        fee_rate: pool_info.fee_rate,
                    };
                    Json(ApiResponse { success: true, data: Some(dto), error: None })
                }
                Err(e) => Json(ApiResponse::<PoolInfoDto> { success: false, data: None, error: Some(format!("{}", e)) }),
            }
        }
    })
}

fn get_tvl_handler(sync: Arc<GlobalSynchronizer>) -> impl axum::handler::Handler<(), axum::body::Body> {
    axum::routing::get(move || {
        let sync = sync.clone();
        async move {
            match sync.get_tvl().await {
                Ok(tvl) => {
                    let dto = serde_json::json!({
                        "tvl": tvl
                    });
                    Json(ApiResponse { success: true, data: Some(dto), error: None })
                }
                Err(e) => Json(ApiResponse::<serde_json::Value> { success: false, data: None, error: Some(format!("{}", e)) }),
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