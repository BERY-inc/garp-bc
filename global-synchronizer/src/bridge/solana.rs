use std::sync::Arc;
use tokio::sync::RwLock;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
    pubkey::Pubkey,
    commitment_config::CommitmentConfig,
};
use serde::{Deserialize, Serialize};
use tracing::{info, error, warn};
use crate::bridge::{BridgeTransaction, BridgeTransactionStatus};

/// Solana blockchain connector
pub struct SolanaConnector {
    /// RPC client
    client: Arc<RpcClient>,
    
    /// Network name
    network_name: String,
    
    /// Commitment config
    commitment_config: CommitmentConfig,
}

/// Solana transaction info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaTxInfo {
    /// Transaction signature
    pub signature: String,
    
    /// Slot
    pub slot: u64,
    
    /// Success status
    pub success: bool,
    
    /// Fee
    pub fee: u64,
}

impl SolanaConnector {
    /// Create new Solana connector
    pub fn new(rpc_url: &str, network_name: String) -> Result<Self, Box<dyn std::error::Error>> {
        let client = Arc::new(RpcClient::new(rpc_url.to_string()));
        
        // Test connection
        let _genesis_hash = client.get_genesis_hash()?;
        
        let connector = Self {
            client,
            network_name,
            commitment_config: CommitmentConfig::confirmed(),
        };
        
        info!("Solana connector initialized for network: {}", network_name);
        
        Ok(connector)
    }
    
    /// Get transaction info
    pub async fn get_transaction_info(&self, signature: &str) -> Result<Option<SolanaTxInfo>, Box<dyn std::error::Error>> {
        let signature = solana_sdk::signature::Signature::from_str(signature)?;
        let transaction = self.client.get_transaction_with_config(
            &signature,
            solana_client::rpc_config::RpcTransactionConfig {
                encoding: Some(solana_transaction_status::UiTransactionEncoding::Json),
                commitment: Some(self.commitment_config),
                max_supported_transaction_version: Some(0),
            },
        ).await;
        
        match transaction {
            Ok(confirmed_tx) => {
                let info = SolanaTxInfo {
                    signature: signature.to_string(),
                    slot: confirmed_tx.slot,
                    success: confirmed_tx.transaction.meta.as_ref().map(|m| m.status.is_ok()).unwrap_or(false),
                    fee: confirmed_tx.transaction.meta.as_ref().map(|m| m.fee).unwrap_or(0),
                };
                Ok(Some(info))
            }
            Err(_) => Ok(None),
        }
    }
    
    /// Send transaction
    pub async fn send_transaction(
        &self,
        keypair: &Keypair,
        transaction: Transaction,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let signature = self.client.send_and_confirm_transaction_with_spinner(&transaction).await?;
        info!("Sent Solana transaction: {:?}", signature);
        Ok(signature.to_string())
    }
    
    /// Check if transaction is confirmed
    pub async fn is_transaction_confirmed(&self, signature: &str, confirmations: usize) -> Result<bool, Box<dyn std::error::Error>> {
        let signature = solana_sdk::signature::Signature::from_str(signature)?;
        let confirmed = self.client.confirm_transaction_with_commitment(&signature, self.commitment_config).await?;
        Ok(confirmed.value)
    }
    
    /// Get account balance
    pub async fn get_account_balance(&self, pubkey: &Pubkey) -> Result<u64, Box<dyn std::error::Error>> {
        let balance = self.client.get_balance_with_commitment(pubkey, self.commitment_config).await?;
        Ok(balance.value)
    }
    
    /// Transfer tokens
    pub async fn transfer(
        &self,
        from_keypair: &Keypair,
        to_pubkey: &Pubkey,
        amount: u64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let from_pubkey = from_keypair.pubkey();
        let blockhash = self.client.get_latest_blockhash()?;
        
        let transaction = Transaction::new_signed_with_payer(
            &[solana_sdk::system_instruction::transfer(&from_pubkey, to_pubkey, amount)],
            Some(&from_pubkey),
            &[from_keypair],
            blockhash,
        );
        
        let signature = self.client.send_and_confirm_transaction_with_spinner(&transaction).await?;
        info!("Sent Solana transfer: {:?}", signature);
        Ok(signature.to_string())
    }
}