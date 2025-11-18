use std::sync::Arc;
use tokio::sync::RwLock;
use web3::{
    transports::Http,
    types::{Address, TransactionReceipt, H256, U256, TransactionParameters},
    Web3,
};
use serde::{Deserialize, Serialize};
use tracing::{info, error, warn};
use crate::bridge::{BridgeTransaction, BridgeTransactionStatus};

/// Polygon blockchain connector
pub struct PolygonConnector {
    /// Web3 client
    client: Web3<Http>,
    
    /// Network name
    network_name: String,
    
    /// Chain ID
    chain_id: u64,
    
    /// Gas price
    gas_price: Arc<RwLock<U256>>,
}

/// Polygon transaction info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolygonTxInfo {
    /// Transaction hash
    pub tx_hash: H256,
    
    /// From address
    pub from: Address,
    
    /// To address
    pub to: Option<Address>,
    
    /// Value
    pub value: U256,
    
    /// Gas used
    pub gas_used: Option<U256>,
    
    /// Status
    pub status: Option<bool>,
}

impl PolygonConnector {
    /// Create new Polygon connector
    pub async fn new(rpc_url: &str, network_name: String, chain_id: u64) -> Result<Self, Box<dyn std::error::Error>> {
        let transport = Http::new(rpc_url)?;
        let client = Web3::new(transport);
        
        // Test connection
        let _block_number = client.eth().block_number().await?;
        
        let connector = Self {
            client,
            network_name,
            chain_id,
            gas_price: Arc::new(RwLock::new(U256::zero())),
        };
        
        // Initialize gas price
        connector.update_gas_price().await?;
        
        info!("Polygon connector initialized for network: {}", network_name);
        
        Ok(connector)
    }
    
    /// Update gas price
    pub async fn update_gas_price(&self) -> Result<(), Box<dyn std::error::Error>> {
        // For Polygon, we use a different approach to get gas price
        // We can use the recommended gas price from the network
        let gas_price = self.client.eth().gas_price().await?;
        let mut price = self.gas_price.write().await;
        *price = gas_price;
        Ok(())
    }
    
    /// Get current gas price
    pub async fn get_gas_price(&self) -> U256 {
        *self.gas_price.read().await
    }
    
    /// Get transaction receipt
    pub async fn get_transaction_receipt(&self, tx_hash: H256) -> Result<Option<TransactionReceipt>, Box<dyn std::error::Error>> {
        let receipt = self.client.eth().transaction_receipt(tx_hash).await?;
        Ok(receipt)
    }
    
    /// Get transaction info
    pub async fn get_transaction_info(&self, tx_hash: H256) -> Result<Option<PolygonTxInfo>, Box<dyn std::error::Error>> {
        let tx = self.client.eth().transaction(web3::types::TransactionId::Hash(tx_hash)).await?;
        
        if let Some(tx) = tx {
            let receipt = self.get_transaction_receipt(tx_hash).await?;
            
            let info = PolygonTxInfo {
                tx_hash: tx.hash,
                from: tx.from,
                to: tx.to,
                value: tx.value,
                gas_used: receipt.as_ref().and_then(|r| r.gas_used),
                status: receipt.as_ref().and_then(|r| r.status.map(|s| s.as_u64() == 1)),
            };
            
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }
    
    /// Send transaction
    pub async fn send_transaction(
        &self,
        to: Address,
        value: U256,
        data: Vec<u8>,
        private_key: &str,
    ) -> Result<H256, Box<dyn std::error::Error>> {
        // Parse private key
        let key = hex::decode(private_key.trim_start_matches("0x"))?;
        let secret_key = secp256k1::SecretKey::parse_slice(&key)?;
        let public_key = secp256k1::PublicKey::from_secret_key(&secret_key);
        let sender_address = public_key_address(&public_key);
        
        // Get nonce
        let nonce = self.client.eth().transaction_count(sender_address, None).await?;
        
        // Get gas price
        let gas_price = self.get_gas_price().await;
        
        // Estimate gas
        let gas_estimate = self.client.eth().estimate_gas(
            TransactionParameters {
                to: Some(to),
                value,
                data: web3::types::Bytes::from(data.clone()),
                ..Default::default()
            },
            None,
        ).await?;
        
        // Create transaction
        let tx = TransactionParameters {
            to: Some(to),
            value,
            data: web3::types::Bytes::from(data),
            gas: gas_estimate,
            gas_price: Some(gas_price),
            nonce: Some(nonce),
            ..Default::default()
        };
        
        // Sign transaction
        let signed_tx = self.client.accounts().sign_transaction(tx, &secret_key).await?;
        
        // Send transaction
        let tx_hash = self.client.eth().send_raw_transaction(signed_tx.raw_transaction).await?;
        
        info!("Sent Polygon transaction: {:?}", tx_hash);
        
        Ok(tx_hash)
    }
    
    /// Check if transaction is confirmed
    pub async fn is_transaction_confirmed(&self, tx_hash: H256, confirmations: usize) -> Result<bool, Box<dyn std::error::Error>> {
        let receipt = self.get_transaction_receipt(tx_hash).await?;
        
        if let Some(receipt) = receipt {
            // Check if transaction was successful
            if let Some(status) = receipt.status {
                if status.as_u64() != 1 {
                    return Ok(false);
                }
            }
            
            // Check confirmations
            let tx_block = receipt.block_number.ok_or("Missing block number")?.as_u64();
            let latest_block = self.client.eth().block_number().await?.as_u64();
            
            Ok((latest_block - tx_block) as usize >= confirmations)
        } else {
            Ok(false)
        }
    }
}

/// Convert public key to Polygon address
fn public_key_address(public_key: &secp256k1::PublicKey) -> Address {
    let public_key = public_key.serialize();
    let hash = keccak256(&public_key[1..]);
    Address::from_slice(&hash[12..])
}

/// Simple keccak256 implementation
fn keccak256(data: &[u8]) -> [u8; 32] {
    use sha3::{Digest, Keccak256};
    let mut hasher = Keccak256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}