use crate::bridge::liquidity::LiquidityPool;

#[tokio::test]
async fn test_liquidity_pool_creation() {
    let pool = LiquidityPool::new(0.003); // 0.3% fee
    assert_eq!(pool.fee_rate, 0.003);
    
    let info = pool.get_pool_info().await;
    assert_eq!(info.fee_rate, 0.003);
    assert_eq!(info.tvl, 0.0);
    assert!(info.reserves.is_empty());
}

#[tokio::test]
async fn test_add_liquidity() {
    let pool = LiquidityPool::new(0.003);
    
    // Add ETH liquidity
    pool.add_liquidity("ETH".to_string(), 100.0).await.unwrap();
    
    let eth_reserve = pool.get_reserve("ETH").await;
    assert_eq!(eth_reserve, 100.0);
    
    let tvl = pool.get_tvl().await;
    assert_eq!(tvl, 100.0);
}

#[tokio::test]
async fn test_remove_liquidity() {
    let pool = LiquidityPool::new(0.003);
    
    // Add liquidity
    pool.add_liquidity("ETH".to_string(), 100.0).await.unwrap();
    
    // Remove partial liquidity
    let removed = pool.remove_liquidity("ETH".to_string(), 30.0).await.unwrap();
    assert_eq!(removed, 30.0);
    
    let eth_reserve = pool.get_reserve("ETH").await;
    assert_eq!(eth_reserve, 70.0);
}

#[tokio::test]
async fn test_swap_tokens() {
    let pool = LiquidityPool::new(0.003); // 0.3% fee
    
    // Add initial liquidity
    pool.add_liquidity("ETH".to_string(), 1000.0).await.unwrap();
    pool.add_liquidity("BTC".to_string(), 50.0).await.unwrap();
    
    // Swap ETH for BTC
    let btc_received = pool.swap("ETH".to_string(), "BTC".to_string(), 100.0).await.unwrap();
    
    // Should receive less than the proportional amount due to fees
    // With constant product formula and 0.3% fee, 100 ETH should give roughly 4.5 BTC
    assert!(btc_received > 0.0);
    assert!(btc_received < 5.0); // Less than proportional due to fee
    
    // Check reserves updated correctly
    let eth_reserve = pool.get_reserve("ETH").await;
    let btc_reserve = pool.get_reserve("BTC").await;
    
    assert!(eth_reserve > 1000.0); // Should be ~1099.7 (100 minus 0.3% fee)
    assert!(btc_reserve < 50.0);   // Should be ~45.5
}

#[tokio::test]
async fn test_insufficient_liquidity_swap() {
    let pool = LiquidityPool::new(0.003);
    
    // Add some liquidity
    pool.add_liquidity("ETH".to_string(), 10.0).await.unwrap();
    pool.add_liquidity("BTC".to_string(), 1.0).await.unwrap();
    
    // Try to swap more than available
    let result = pool.swap("ETH".to_string(), "BTC".to_string(), 1000.0).await;
    
    // Should fail due to insufficient liquidity
    assert!(result.is_err());
}