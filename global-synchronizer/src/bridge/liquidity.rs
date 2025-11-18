use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{info, error, warn};

/// Liquidity pool for cross-chain token swaps
pub struct LiquidityPool {
    /// Pool reserves
    reserves: Arc<RwLock<HashMap<String, f64>>>,
    
    /// Pool fees (in percentage)
    fee_rate: f64,
    
    /// Total value locked
    tvl: Arc<RwLock<f64>>,
}

/// Liquidity pool information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolInfo {
    /// Asset pair
    pub asset_pair: String,
    
    /// Reserve amounts
    pub reserves: HashMap<String, f64>,
    
    /// Total value locked
    pub tvl: f64,
    
    /// Fee rate
    pub fee_rate: f64,
}

/// Liquidity provision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityProvision {
    /// Provider address
    pub provider: String,
    
    /// Asset provided
    pub asset: String,
    
    /// Amount provided
    pub amount: f64,
    
    /// Timestamp
    pub timestamp: u64,
}

impl LiquidityPool {
    /// Create new liquidity pool
    pub fn new(fee_rate: f64) -> Self {
        Self {
            reserves: Arc::new(RwLock::new(HashMap::new())),
            fee_rate,
            tvl: Arc::new(RwLock::new(0.0)),
        }
    }
    
    /// Add liquidity to the pool
    pub async fn add_liquidity(&self, asset: String, amount: f64) -> Result<(), Box<dyn std::error::Error>> {
        let mut reserves = self.reserves.write().await;
        let current = reserves.entry(asset.clone()).or_insert(0.0);
        *current += amount;
        
        // Update TVL
        let mut tvl = self.tvl.write().await;
        *tvl += amount;
        
        info!("Added {} {} to liquidity pool. New reserve: {}", amount, asset, current);
        Ok(())
    }
    
    /// Remove liquidity from the pool
    pub async fn remove_liquidity(&self, asset: String, amount: f64) -> Result<f64, Box<dyn std::error::Error>> {
        let mut reserves = self.reserves.write().await;
        
        if let Some(reserve) = reserves.get_mut(&asset) {
            if *reserve >= amount {
                *reserve -= amount;
                
                // Update TVL
                let mut tvl = self.tvl.write().await;
                *tvl -= amount;
                
                info!("Removed {} {} from liquidity pool", amount, asset);
                Ok(amount)
            } else {
                let available = *reserve;
                *reserve = 0.0;
                
                // Update TVL
                let mut tvl = self.tvl.write().await;
                *tvl -= available;
                
                info!("Removed {} {} from liquidity pool (partial withdrawal)", available, asset);
                Ok(available)
            }
        } else {
            warn!("Asset {} not found in liquidity pool", asset);
            Ok(0.0)
        }
    }
    
    /// Swap tokens using constant product formula (x * y = k)
    pub async fn swap(&self, from_asset: String, to_asset: String, amount: f64) -> Result<f64, Box<dyn std::error::Error>> {
        let mut reserves = self.reserves.write().await;
        
        // Get reserves
        let from_reserve = *reserves.get(&from_asset).unwrap_or(&0.0);
        let to_reserve = *reserves.get(&to_asset).unwrap_or(&0.0);
        
        if from_reserve == 0.0 || to_reserve == 0.0 {
            return Err("Insufficient liquidity in pool".into());
        }
        
        // Calculate output amount with fee
        let fee = amount * self.fee_rate;
        let amount_in = amount - fee;
        
        // Constant product formula: (x + Δx) * (y - Δy) = x * y
        // Δy = (y * Δx) / (x + Δx)
        let amount_out = (to_reserve * amount_in) / (from_reserve + amount_in);
        
        // Check if we have enough liquidity
        if amount_out > to_reserve {
            return Err("Insufficient liquidity for swap".into());
        }
        
        // Update reserves
        reserves.insert(from_asset.clone(), from_reserve + amount_in);
        reserves.insert(to_asset.clone(), to_reserve - amount_out);
        
        // Update TVL (simplified)
        let mut tvl = self.tvl.write().await;
        *tvl = reserves.values().sum();
        
        info!("Swapped {} {} for {} {} (fee: {})", amount, from_asset, amount_out, to_asset, fee);
        
        Ok(amount_out)
    }
    
    /// Get pool information
    pub async fn get_pool_info(&self) -> PoolInfo {
        let reserves = self.reserves.read().await;
        let tvl = self.tvl.read().await;
        
        PoolInfo {
            asset_pair: "MULTI_ASSET_POOL".to_string(),
            reserves: reserves.clone(),
            tvl: *tvl,
            fee_rate: self.fee_rate,
        }
    }
    
    /// Get reserve for an asset
    pub async fn get_reserve(&self, asset: &str) -> f64 {
        let reserves = self.reserves.read().await;
        *reserves.get(asset).unwrap_or(&0.0)
    }
    
    /// Set reserve for an asset
    pub async fn set_reserve(&self, asset: String, amount: f64) {
        let mut reserves = self.reserves.write().await;
        reserves.insert(asset, amount);
        
        // Update TVL
        let mut tvl = self.tvl.write().await;
        *tvl = reserves.values().sum();
    }
    
    /// Get total value locked
    pub async fn get_tvl(&self) -> f64 {
        *self.tvl.read().await
    }
}