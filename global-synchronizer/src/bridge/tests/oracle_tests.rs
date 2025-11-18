use crate::bridge::oracle::PriceOracle;
use std::time::Duration;

#[tokio::test]
async fn test_oracle_creation() {
    let oracle = PriceOracle::new(60); // 60 seconds update interval
    assert_eq!(oracle.update_interval, 60);
}

#[tokio::test]
async fn test_price_storage_and_retrieval() {
    let oracle = PriceOracle::new(60);
    
    // Manually set some prices for testing
    {
        let mut prices = oracle.prices.write().await;
        prices.insert("ETH".to_string(), 3000.0);
        prices.insert("BTC".to_string(), 50000.0);
    }
    
    // Test getting individual prices
    let eth_price = oracle.get_price("ETH").await;
    assert_eq!(eth_price, Some(3000.0));
    
    let btc_price = oracle.get_price("BTC").await;
    assert_eq!(btc_price, Some(50000.0));
    
    // Test getting non-existent price
    let nonexistent = oracle.get_price("XYZ").await;
    assert_eq!(nonexistent, None);
}

#[tokio::test]
async fn test_all_prices() {
    let oracle = PriceOracle::new(60);
    
    // Manually set some prices for testing
    {
        let mut prices = oracle.prices.write().await;
        prices.insert("ETH".to_string(), 3000.0);
        prices.insert("BTC".to_string(), 50000.0);
        prices.insert("SOL".to_string(), 100.0);
    }
    
    let all_prices = oracle.get_all_prices().await;
    assert_eq!(all_prices.len(), 3);
    assert_eq!(all_prices.get("ETH"), Some(&3000.0));
    assert_eq!(all_prices.get("BTC"), Some(&50000.0));
    assert_eq!(all_prices.get("SOL"), Some(&100.0));
}

#[tokio::test]
async fn test_conversion_rate() {
    let oracle = PriceOracle::new(60);
    
    // Manually set some prices for testing
    {
        let mut prices = oracle.prices.write().await;
        prices.insert("ETH".to_string(), 3000.0);
        prices.insert("BTC".to_string(), 50000.0);
    }
    
    // Test conversion rate calculation
    let eth_to_btc_rate = oracle.get_conversion_rate("ETH", "BTC").await;
    let expected_rate = 3000.0 / 50000.0; // 0.06
    assert_eq!(eth_to_btc_rate, expected_rate);
    
    let btc_to_eth_rate = oracle.get_conversion_rate("BTC", "ETH").await;
    let expected_rate = 50000.0 / 3000.0; // ~16.67
    assert_eq!(btc_to_eth_rate, expected_rate);
}

#[tokio::test]
async fn test_conversion_with_missing_prices() {
    let oracle = PriceOracle::new(60);
    
    // Manually set one price
    {
        let mut prices = oracle.prices.write().await;
        prices.insert("ETH".to_string(), 3000.0);
    }
    
    // Conversion with missing price should use default value of 1.0
    let rate = oracle.get_conversion_rate("ETH", "XYZ").await;
    assert_eq!(rate, 3000.0); // 3000.0 / 1.0 (default)
    
    let rate = oracle.get_conversion_rate("XYZ", "ETH").await;
    assert_eq!(rate, 1.0 / 3000.0); // 1.0 / 3000.0 (default)
}