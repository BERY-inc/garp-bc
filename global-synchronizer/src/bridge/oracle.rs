use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use serde::{Deserialize, Serialize};
use tracing::{info, error, warn};
use reqwest::Client;

/// Price oracle for cryptocurrency prices
pub struct PriceOracle {
    /// Price data
    prices: Arc<RwLock<HashMap<String, f64>>>,
    
    /// HTTP client
    client: Client,
    
    /// Update interval (seconds)
    update_interval: u64,
}

/// Price data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    /// Asset symbol
    pub symbol: String,
    
    /// Price in USD
    pub price: f64,
    
    /// Timestamp
    pub timestamp: u64,
    
    /// Source
    pub source: String,
}

/// CoinGecko API response
#[derive(Debug, Deserialize)]
struct CoinGeckoResponse {
    /// Market data
    market_data: MarketData,
}

/// Market data
#[derive(Debug, Deserialize)]
struct MarketData {
    /// Current price
    current_price: HashMap<String, f64>,
}

impl PriceOracle {
    /// Create new price oracle
    pub fn new(update_interval: u64) -> Self {
        Self {
            prices: Arc::new(RwLock::new(HashMap::new())),
            client: Client::new(),
            update_interval,
        }
    }
    
    /// Start the price oracle
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let prices = self.prices.clone();
        let client = self.client.clone();
        let update_interval = self.update_interval;
        
        // Start background task to update prices
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(update_interval));
            
            loop {
                interval.tick().await;
                
                if let Err(e) = Self::update_prices(&client, &prices).await {
                    error!("Failed to update prices: {}", e);
                }
            }
        });
        
        info!("Price oracle started");
        Ok(())
    }
    
    /// Update prices from CoinGecko API
    async fn update_prices(client: &Client, prices: &Arc<RwLock<HashMap<String, f64>>>) -> Result<(), Box<dyn std::error::Error>> {
        // Define the assets we want to track
        let assets = vec![
            ("bitcoin", "BTC"),
            ("ethereum", "ETH"),
            ("matic-network", "MATIC"),
            ("binancecoin", "BNB"),
            ("solana", "SOL"),
            ("garp", "BRY"), // Assuming GARP has a CoinGecko listing
        ];
        
        let mut price_map = HashMap::new();
        
        // Fetch prices for each asset
        for (coin_id, symbol) in assets {
            match Self::fetch_price(client, coin_id).await {
                Ok(price) => {
                    price_map.insert(symbol.to_string(), price);
                    info!("Updated price for {}: ${}", symbol, price);
                }
                Err(e) => {
                    error!("Failed to fetch price for {}: {}", symbol, e);
                }
            }
        }
        
        // Update the shared price map
        {
            let mut shared_prices = prices.write().await;
            for (symbol, price) in price_map {
                shared_prices.insert(symbol, price);
            }
        }
        
        info!("Updated price data for {} assets", price_map.len());
        
        Ok(())
    }
    
    /// Fetch price for a specific asset from CoinGecko
    async fn fetch_price(client: &Client, coin_id: &str) -> Result<f64, Box<dyn std::error::Error>> {
        let url = format!("https://api.coingecko.com/api/v3/coins/{}?localization=false&tickers=false&market_data=true&community_data=false&developer_data=false&sparkline=false", coin_id);
        
        let response = client.get(&url).send().await?;
        let data: CoinGeckoResponse = response.json().await?;
        
        // Get USD price
        let price = *data.market_data.current_price.get("usd").unwrap_or(&0.0);
        
        Ok(price)
    }
    
    /// Get price for an asset
    pub async fn get_price(&self, symbol: &str) -> Option<f64> {
        let prices = self.prices.read().await;
        prices.get(symbol).copied()
    }
    
    /// Get conversion rate between two assets
    pub async fn get_conversion_rate(&self, from: &str, to: &str) -> f64 {
        let from_price = self.get_price(from).await.unwrap_or(1.0);
        let to_price = self.get_price(to).await.unwrap_or(1.0);
        
        if to_price == 0.0 {
            1.0
        } else {
            from_price / to_price
        }
    }
    
    /// Get all prices
    pub async fn get_all_prices(&self) -> HashMap<String, f64> {
        let prices = self.prices.read().await;
        prices.clone()
    }
}