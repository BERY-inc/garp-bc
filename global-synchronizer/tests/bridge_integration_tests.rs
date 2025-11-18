//! Integration tests for the cross-chain bridge functionality
//!
//! These tests verify the end-to-end functionality of the bridge components
//! including wallet management, liquidity pools, price oracles, and cross-chain connectors.

use global_synchronizer::{
    bridge::{
        wallet::{WalletManager, CreateWalletRequest},
        liquidity::LiquidityPool,
        oracle::PriceOracle,
    },
};

#[tokio::test]
async fn test_wallet_lifecycle() {
    // Create wallet manager
    let wallet_manager = WalletManager::new();
    
    // Create a new wallet
    let request = CreateWalletRequest {
        chain_type: "ethereum".to_string(),
        password: "test_password".to_string(),
    };
    
    let create_result = wallet_manager.create_wallet(request).await;
    assert!(create_result.is_ok());
    
    let wallet_response = create_result.unwrap();
    assert!(!wallet_response.wallet_id.is_empty());
    assert!(!wallet_response.address.is_empty());
    
    // Retrieve the wallet
    let wallet = wallet_manager.get_wallet(&wallet_response.wallet_id).await;
    assert!(wallet.is_some());
    
    // List wallets
    let wallets = wallet_manager.list_wallets().await;
    assert_eq!(wallets.len(), 1);
    
    // Decrypt private key
    let decrypted_key = wallet_manager.decrypt_private_key(&wallet_response.wallet_id, "test_password").await;
    assert!(decrypted_key.is_ok());
    assert!(!decrypted_key.unwrap().is_empty());
}

#[tokio::test]
async fn test_liquidity_pool_operations() {
    // Create liquidity pool with 0.3% fee
    let pool = LiquidityPool::new(0.003);
    
    // Add initial liquidity
    assert!(pool.add_liquidity("ETH".to_string(), 1000.0).await.is_ok());
    assert!(pool.add_liquidity("BTC".to_string(), 50.0).await.is_ok());
    
    // Check reserves
    let eth_reserve = pool.get_reserve("ETH").await;
    let btc_reserve = pool.get_reserve("BTC").await;
    assert_eq!(eth_reserve, 1000.0);
    assert_eq!(btc_reserve, 50.0);
    
    // Check TVL
    let tvl = pool.get_tvl().await;
    assert_eq!(tvl, 1050.0);
    
    // Perform a swap
    let btc_received = pool.swap("ETH".to_string(), "BTC".to_string(), 100.0).await;
    assert!(btc_received.is_ok());
    let btc_amount = btc_received.unwrap();
    assert!(btc_amount > 0.0);
    
    // Check updated reserves
    let updated_eth_reserve = pool.get_reserve("ETH").await;
    let updated_btc_reserve = pool.get_reserve("BTC").await;
    assert!(updated_eth_reserve > 1000.0); // Should be ~1099.7 (100 minus 0.3% fee)
    assert!(updated_btc_reserve < 50.0);   // Should be less than original
    
    // Remove liquidity
    let removed_amount = pool.remove_liquidity("ETH".to_string(), 500.0).await;
    assert!(removed_amount.is_ok());
    assert_eq!(removed_amount.unwrap(), 500.0);
}

#[tokio::test]
async fn test_price_oracle_functionality() {
    // Create price oracle with 60 second update interval
    let oracle = PriceOracle::new(60);
    
    // Manually set some test prices
    {
        let mut prices = oracle.prices.write().await;
        prices.insert("ETH".to_string(), 3000.0);
        prices.insert("BTC".to_string(), 50000.0);
        prices.insert("SOL".to_string(), 100.0);
    }
    
    // Test getting individual prices
    let eth_price = oracle.get_price("ETH").await;
    assert_eq!(eth_price, Some(3000.0));
    
    let btc_price = oracle.get_price("BTC").await;
    assert_eq!(btc_price, Some(50000.0));
    
    // Test getting all prices
    let all_prices = oracle.get_all_prices().await;
    assert_eq!(all_prices.len(), 3);
    
    // Test conversion rates
    let eth_to_btc_rate = oracle.get_conversion_rate("ETH", "BTC").await;
    let expected_rate = 3000.0 / 50000.0; // 0.06
    assert_eq!(eth_to_btc_rate, expected_rate);
    
    // Test conversion with missing asset (should use default value of 1.0)
    let rate = oracle.get_conversion_rate("ETH", "XYZ").await;
    assert_eq!(rate, 3000.0); // 3000.0 / 1.0 (default)
}