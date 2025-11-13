use garp_common::{
    ParticipantId, Asset, AssetType, WalletBalance, Transaction, TransactionId, TransactionCommand,
    CreateAssetCommand, TransferAssetCommand, DigitalSignature, CryptoService,
    GarpResult, GarpError, TransactionError
};
use crate::storage::StorageBackend;
use std::sync::Arc;
use chrono::Utc;
use uuid::Uuid;
use tracing::{info, warn, error, debug};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Asset reservation entry
#[derive(Debug, Clone)]
pub struct AssetReservation {
    pub reservation_id: String,
    pub assets: Vec<Asset>,
    pub created_at: chrono::DateTime<Utc>,
    pub expires_at: chrono::DateTime<Utc>,
}

/// Wallet manager for handling digital assets
pub struct WalletManager {
    participant_id: ParticipantId,
    storage: Arc<dyn StorageBackend>,
    crypto_service: Arc<CryptoService>,
    reservations: Arc<RwLock<HashMap<String, AssetReservation>>>,
}

/// Asset transaction history entry
#[derive(Debug, Clone)]
pub struct AssetTransaction {
    pub transaction_id: TransactionId,
    pub asset_id: String,
    pub transaction_type: AssetTransactionType,
    pub amount: u64,
    pub counterparty: Option<ParticipantId>,
    pub timestamp: chrono::DateTime<Utc>,
    pub status: TransactionStatus,
}

/// Types of asset transactions
#[derive(Debug, Clone)]
pub enum AssetTransactionType {
    Create,
    Transfer,
    Receive,
    Burn,
}

/// Transaction status
#[derive(Debug, Clone)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed(String),
}

/// Wallet statistics
#[derive(Debug, Clone)]
pub struct WalletStats {
    pub total_assets: u64,
    pub total_value: HashMap<String, u64>, // Currency -> Amount
    pub transaction_count: u64,
    pub last_transaction: Option<chrono::DateTime<Utc>>,
}

impl WalletManager {
    /// Create new wallet manager
    pub fn new(
        participant_id: ParticipantId,
        storage: Arc<dyn StorageBackend>,
        crypto_service: Arc<CryptoService>,
    ) -> Self {
        Self {
            participant_id,
            storage,
            crypto_service,
            reservations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get current wallet balance
    pub async fn get_balance(&self) -> GarpResult<Option<WalletBalance>> {
        self.storage.get_wallet_balance(&self.participant_id).await
    }

    /// Get balance for a specific asset
    pub async fn get_asset_balance(&self, asset_id: &str) -> GarpResult<u64> {
        if let Some(balance) = self.get_balance().await? {
            for asset in &balance.assets {
                if asset.id == asset_id {
                    return Ok(asset.amount);
                }
            }
        }
        Ok(0)
    }

    /// Create a new asset
    pub async fn create_asset(&self, mut asset: Asset) -> GarpResult<TransactionId> {
        info!("Creating asset {} with amount {}", asset.id, asset.amount);

        // Generate transaction ID
        let transaction_id = TransactionId(Uuid::new_v4());

        // Set asset owner to this participant
        asset.metadata.insert("owner".to_string(), self.participant_id.0.clone());

        // Create transaction command
        let command = TransactionCommand::CreateAsset(CreateAssetCommand {
            asset: asset.clone(),
            owner: self.participant_id.clone(),
        });

        // Create and sign transaction
        let transaction = self.create_signed_transaction(transaction_id.clone(), command).await?;

        // Store transaction
        self.storage.store_transaction(&transaction).await?;

        // Update balance
        self.storage.update_asset_balance(&self.participant_id, &asset, asset.amount as i64).await?;

        info!("Asset {} created successfully", asset.id);
        Ok(transaction_id)
    }

    /// Transfer asset to another participant
    pub async fn transfer_asset(&self, to: ParticipantId, asset: Asset) -> GarpResult<TransactionId> {
        info!("Transferring {} units of asset {} to {}", asset.amount, asset.id, to.0);

        // Check if we have sufficient balance
        let current_balance = self.get_asset_balance(&asset.id).await?;
        if current_balance < asset.amount {
            return Err(TransactionError::InsufficientBalance {
                required: asset.amount,
                available: current_balance,
            }.into());
        }

        // Generate transaction ID
        let transaction_id = TransactionId(Uuid::new_v4());

        // Create transaction command
        let command = TransactionCommand::TransferAsset(TransferAssetCommand {
            from: self.participant_id.clone(),
            to: to.clone(),
            asset: asset.clone(),
        });

        // Create and sign transaction
        let transaction = self.create_signed_transaction(transaction_id.clone(), command).await?;

        // Store transaction
        self.storage.store_transaction(&transaction).await?;

        // Update balances
        self.storage.update_asset_balance(&self.participant_id, &asset, -(asset.amount as i64)).await?;
        self.storage.update_asset_balance(&to, &asset, asset.amount as i64).await?;

        info!("Asset transfer completed: {} -> {}", self.participant_id.0, to.0);
        Ok(transaction_id)
    }

    /// Burn (destroy) assets
    pub async fn burn_asset(&self, asset_id: &str, amount: u64) -> GarpResult<TransactionId> {
        info!("Burning {} units of asset {}", amount, asset_id);

        // Check if we have sufficient balance
        let current_balance = self.get_asset_balance(asset_id).await?;
        if current_balance < amount {
            return Err(TransactionError::InsufficientBalance {
                required: amount,
                available: current_balance,
            }.into());
        }

        // Create asset for burning
        let asset = Asset {
            id: asset_id.to_string(),
            asset_type: AssetType::Token, // Simplified
            amount,
            metadata: HashMap::new(),
        };

        // Generate transaction ID
        let transaction_id = TransactionId(Uuid::new_v4());

        // For burning, we can use a transfer to a special "burn" address
        let burn_address = ParticipantId("BURN_ADDRESS".to_string());
        let command = TransactionCommand::TransferAsset(TransferAssetCommand {
            from: self.participant_id.clone(),
            to: burn_address,
            asset: asset.clone(),
        });

        // Create and sign transaction
        let transaction = self.create_signed_transaction(transaction_id.clone(), command).await?;

        // Store transaction
        self.storage.store_transaction(&transaction).await?;

        // Update balance (deduct from our balance)
        self.storage.update_asset_balance(&self.participant_id, &asset, -(amount as i64)).await?;

        info!("Asset burn completed: {} units of {}", amount, asset_id);
        Ok(transaction_id)
    }

    /// Get transaction history for this wallet
    pub async fn get_transaction_history(&self, limit: Option<u32>) -> GarpResult<Vec<AssetTransaction>> {
        let transactions = self.storage.list_transactions(&self.participant_id, limit).await?;
        let mut asset_transactions = Vec::new();

        for tx in transactions {
            if let Some(asset_tx) = self.convert_to_asset_transaction(&tx) {
                asset_transactions.push(asset_tx);
            }
        }

        Ok(asset_transactions)
    }

    /// Get wallet statistics
    pub async fn get_stats(&self) -> GarpResult<WalletStats> {
        let balance = self.get_balance().await?;
        let transactions = self.get_transaction_history(None).await?;

        let total_assets = balance.as_ref().map(|b| b.assets.len() as u64).unwrap_or(0);
        
        // Calculate total value by currency
        let mut total_value = HashMap::new();
        if let Some(balance) = &balance {
            for asset in &balance.assets {
                // Simplified: assume asset ID contains currency info
                let currency = asset.asset_type.to_string();
                *total_value.entry(currency).or_insert(0) += asset.amount;
            }
        }

        let last_transaction = transactions.first().map(|tx| tx.timestamp);

        Ok(WalletStats {
            total_assets,
            total_value,
            transaction_count: transactions.len() as u64,
            last_transaction,
        })
    }

    /// Create a signed transaction
    async fn create_signed_transaction(
        &self,
        transaction_id: TransactionId,
        command: TransactionCommand,
    ) -> GarpResult<Transaction> {
        let transaction = Transaction {
            id: transaction_id,
            submitter: self.participant_id.clone(),
            command,
            created_at: Utc::now(),
            signatures: Vec::new(),
            encrypted_payload: None,
        };

        // Sign the transaction
        let message = self.create_signature_message(&transaction)?;
        let signature = self.crypto_service.sign(&message)?;
        let public_key = self.crypto_service.get_public_key()?;

        let digital_signature = DigitalSignature {
            signer: self.participant_id.clone(),
            public_key,
            signature,
        };

        let mut signed_transaction = transaction;
        signed_transaction.signatures.push(digital_signature);

        Ok(signed_transaction)
    }

    /// Create message for signature
    fn create_signature_message(&self, transaction: &Transaction) -> GarpResult<Vec<u8>> {
        let mut message = Vec::new();
        message.extend_from_slice(transaction.id.0.as_bytes());
        message.extend_from_slice(transaction.submitter.0.as_bytes());
        
        let command_bytes = bincode::serialize(&transaction.command)
            .map_err(|e| TransactionError::SerializationFailed(e.to_string()))?;
        message.extend_from_slice(&command_bytes);
        
        message.extend_from_slice(&transaction.created_at.timestamp().to_le_bytes());
        
        Ok(message)
    }

    /// Convert transaction to asset transaction
    fn convert_to_asset_transaction(&self, tx: &Transaction) -> Option<AssetTransaction> {
        match &tx.command {
            TransactionCommand::CreateAsset(cmd) => {
                Some(AssetTransaction {
                    transaction_id: tx.id.clone(),
                    asset_id: cmd.asset.id.clone(),
                    transaction_type: AssetTransactionType::Create,
                    amount: cmd.asset.amount,
                    counterparty: None,
                    timestamp: tx.created_at,
                    status: TransactionStatus::Confirmed, // Simplified
                })
            }
            TransactionCommand::TransferAsset(cmd) => {
                let (transaction_type, counterparty) = if cmd.from == self.participant_id {
                    (AssetTransactionType::Transfer, Some(cmd.to.clone()))
                } else {
                    (AssetTransactionType::Receive, Some(cmd.from.clone()))
                };

                Some(AssetTransaction {
                    transaction_id: tx.id.clone(),
                    asset_id: cmd.asset.id.clone(),
                    transaction_type,
                    amount: cmd.asset.amount,
                    counterparty,
                    timestamp: tx.created_at,
                    status: TransactionStatus::Confirmed, // Simplified
                })
            }
            _ => None, // Other transaction types don't directly affect wallet
        }
    }

    /// Get assets by type
    pub async fn get_assets_by_type(&self, asset_type: AssetType) -> GarpResult<Vec<Asset>> {
        if let Some(balance) = self.get_balance().await? {
            let filtered_assets: Vec<Asset> = balance.assets
                .into_iter()
                .filter(|asset| asset.asset_type == asset_type)
                .collect();
            Ok(filtered_assets)
        } else {
            Ok(Vec::new())
        }
    }

    /// Check if wallet has sufficient balance for multiple assets
    pub async fn check_sufficient_balance(&self, required_assets: &[Asset]) -> GarpResult<bool> {
        for required_asset in required_assets {
            let current_balance = self.get_asset_balance(&required_asset.id).await?;
            if current_balance < required_asset.amount {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Reserve assets for a pending transaction
    pub async fn reserve_assets(&self, assets: &[Asset]) -> GarpResult<String> {
        // Check if we have sufficient balance for all assets
        for asset in assets {
            let available_balance = self.get_asset_balance(&asset.id).await?;
            let reserved_amount = self.get_reserved_amount(&asset.id).await;
            
            if available_balance < asset.amount + reserved_amount {
                return Err(GarpError::Transaction(TransactionError::InsufficientBalance {
                    required: asset.amount,
                    available: available_balance.saturating_sub(reserved_amount),
                }));
            }
        }
        
        // Create reservation
        let reservation_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let expires_at = now + chrono::Duration::minutes(10); // 10 minute expiry
        
        let reservation = AssetReservation {
            reservation_id: reservation_id.clone(),
            assets: assets.to_vec(),
            created_at: now,
            expires_at,
        };
        
        // Store reservation
        {
            let mut reservations = self.reservations.write().await;
            reservations.insert(reservation_id.clone(), reservation);
        }
        
        // Clean up expired reservations
        self.cleanup_expired_reservations().await;
        
        debug!("Reserved assets for transaction: {}", reservation_id);
        Ok(reservation_id)
    }

    /// Release reserved assets
    pub async fn release_reservation(&self, reservation_id: &str) -> GarpResult<()> {
        let mut reservations = self.reservations.write().await;
        
        if reservations.remove(reservation_id).is_some() {
            debug!("Released asset reservation: {}", reservation_id);
            Ok(())
        } else {
            warn!("Attempted to release non-existent reservation: {}", reservation_id);
            Err(GarpError::Transaction(TransactionError::Invalid(
                format!("Invalid reservation ID: {}", reservation_id)
            )))
        }
    }

    /// Get total reserved amount for an asset
    async fn get_reserved_amount(&self, asset_id: &str) -> u64 {
        let reservations = self.reservations.read().await;
        let mut total_reserved = 0u64;
        
        for reservation in reservations.values() {
            // Skip expired reservations
            if reservation.expires_at < Utc::now() {
                continue;
            }
            
            for asset in &reservation.assets {
                if asset.id == asset_id {
                    total_reserved += asset.amount;
                }
            }
        }
        
        total_reserved
    }

    /// Clean up expired reservations
    async fn cleanup_expired_reservations(&self) {
        let mut reservations = self.reservations.write().await;
        let now = Utc::now();
        
        reservations.retain(|_, reservation| {
            let is_valid = reservation.expires_at > now;
            if !is_valid {
                debug!("Cleaned up expired reservation: {}", reservation.reservation_id);
            }
            is_valid
        });
    }

    /// Get asset metadata
    pub async fn get_asset_metadata(&self, asset_id: &str) -> GarpResult<Option<HashMap<String, String>>> {
        if let Some(balance) = self.get_balance().await? {
            for asset in &balance.assets {
                if asset.id == asset_id {
                    return Ok(Some(asset.metadata.clone()));
                }
            }
        }
        Ok(None)
    }

    /// Update asset metadata (for assets owned by this participant)
    pub async fn update_asset_metadata(
        &self,
        asset_id: &str,
        metadata: HashMap<String, String>,
    ) -> GarpResult<()> {
        // Check if we own this asset
        let current_balance = self.get_asset_balance(asset_id).await?;
        if current_balance == 0 {
            return Err(GarpError::Unauthorized("Cannot update metadata for unowned asset".to_string()));
        }

        // In a real implementation, this would create a transaction to update metadata
        // For now, we'll just log it
        info!("Updated metadata for asset {}: {:?}", asset_id, metadata);
        
        Ok(())
    }

    /// Get total portfolio value in a specific currency
    pub async fn get_portfolio_value(&self, currency: &str) -> GarpResult<u64> {
        let balance = self.get_balance().await?;
        let mut total_value = 0u64;

        if let Some(balance) = balance {
            for asset in &balance.assets {
                // In a real implementation, you'd have exchange rates
                // For now, we'll use a simplified calculation
                match asset.asset_type {
                    AssetType::Currency if asset.id == currency => {
                        total_value += asset.amount;
                    }
                    AssetType::Token => {
                        // Simplified: assume 1:1 exchange rate for tokens
                        total_value += asset.amount;
                    }
                    _ => {
                        // Other asset types would need proper valuation
                    }
                }
            }
        }

        Ok(total_value)
    }

    /// Export wallet data (for backup or migration)
    pub async fn export_wallet_data(&self) -> GarpResult<WalletExport> {
        let balance = self.get_balance().await?;
        let transactions = self.get_transaction_history(None).await?;
        let stats = self.get_stats().await?;

        Ok(WalletExport {
            participant_id: self.participant_id.clone(),
            balance,
            transactions,
            stats,
            export_time: Utc::now(),
        })
    }
}

/// Wallet export data structure
#[derive(Debug, Clone)]
pub struct WalletExport {
    pub participant_id: ParticipantId,
    pub balance: Option<WalletBalance>,
    pub transactions: Vec<AssetTransaction>,
    pub stats: WalletStats,
    pub export_time: chrono::DateTime<Utc>,
}

impl ToString for AssetType {
    fn to_string(&self) -> String {
        match self {
            AssetType::Currency => "Currency".to_string(),
            AssetType::Token => "Token".to_string(),
            AssetType::Product => "Product".to_string(),
            AssetType::LoyaltyPoints => "LoyaltyPoints".to_string(),
            AssetType::Stablecoin => "Stablecoin".to_string(),
        }
    }
}
// -----------------------------------------------------------------------------
// Wallet flows: key management, signing, and broadcast (scaffolding)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WalletAccount {
    pub account_id: String,
    pub public_key_hex: String,
    pub nonce: u64,
}

/// Placeholder: derive an account from a seed. In production, use KMS/HSM.
pub fn derive_account_from_seed(seed_hex: &str, account_id: &str) -> Result<WalletAccount, String> {
    if seed_hex.is_empty() { return Err("seed required".into()); }
    // Do not keep private keys in memory in production.
    let pk = blake3::hash(seed_hex.as_bytes());
    Ok(WalletAccount { account_id: account_id.to_string(), public_key_hex: hex::encode(pk.as_bytes()), nonce: 0 })
}

/// Placeholder: sign bytes. In production, offload to KMS/HSM.
pub fn sign_bytes(_account_id: &str, _message: &[u8]) -> Result<Vec<u8>, String> {
    // Intentionally not implementing private key handling here.
    Err("signing not implemented; use KMS/HSM".into())
}