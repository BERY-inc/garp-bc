use garp_common::{
    Contract, Transaction, TransactionId, ContractId, ParticipantId, Asset, WalletBalance,
    TransactionCommand, CreateContractCommand, ExerciseContractCommand, ArchiveContractCommand,
    GarpResult, GarpError, TransactionError, CryptoService, DigitalSignature
};
use crate::storage::{StorageBackend, LedgerState};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use async_trait::async_trait;
use uuid::Uuid;
use tracing::{info, warn, error, debug};

/// Local ledger for a participant node
pub struct LocalLedger {
    participant_id: ParticipantId,
    storage: Arc<dyn StorageBackend>,
    crypto_service: Arc<CryptoService>,
}

/// Transaction validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub affected_contracts: Vec<ContractId>,
    pub required_signatures: Vec<ParticipantId>,
}

/// Ledger view for privacy-preserving queries
#[derive(Debug, Clone)]
pub struct LedgerView {
    pub participant_id: ParticipantId,
    pub visible_contracts: Vec<Contract>,
    pub transaction_history: Vec<Transaction>,
    pub wallet_balance: Option<WalletBalance>,
    pub last_updated: DateTime<Utc>,
}

impl LocalLedger {
    /// Create new local ledger
    pub fn new(
        participant_id: ParticipantId,
        storage: Arc<dyn StorageBackend>,
        crypto_service: Arc<CryptoService>,
    ) -> Self {
        Self {
            participant_id,
            storage,
            crypto_service,
        }
    }

    /// Submit a transaction to the ledger
    pub async fn submit_transaction(&self, transaction: Transaction) -> GarpResult<ValidationResult> {
        info!("Submitting transaction {} from {}", transaction.id.0, transaction.submitter.0);

        // Validate transaction
        let validation = self.validate_transaction(&transaction).await?;
        if !validation.valid {
            warn!("Transaction validation failed: {:?}", validation.errors);
            return Ok(validation);
        }

        // Store transaction
        self.storage.store_transaction(&transaction).await?;

        // Apply transaction effects
        self.apply_transaction_effects(&transaction).await?;

        info!("Transaction {} successfully applied", transaction.id.0);
        Ok(validation)
    }

    /// Validate a transaction against the current ledger state
    pub async fn validate_transaction(&self, transaction: &Transaction) -> GarpResult<ValidationResult> {
        let mut result = ValidationResult {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            affected_contracts: Vec::new(),
            required_signatures: Vec::new(),
        };

        // Validate signatures
        if let Err(e) = self.validate_signatures(transaction).await {
            result.valid = false;
            result.errors.push(format!("Signature validation failed: {}", e));
        }

        // Validate command-specific logic
        match &transaction.command {
            TransactionCommand::CreateContract(cmd) => {
                self.validate_create_contract(cmd, &mut result).await?;
            }
            TransactionCommand::ExerciseContract(cmd) => {
                self.validate_exercise_contract(cmd, &mut result).await?;
            }
            TransactionCommand::ArchiveContract(cmd) => {
                self.validate_archive_contract(cmd, &mut result).await?;
            }
            TransactionCommand::TransferAsset(cmd) => {
                self.validate_transfer_asset(cmd, &mut result).await?;
            }
            TransactionCommand::CreateAsset(cmd) => {
                self.validate_create_asset(cmd, &mut result).await?;
            }
        }

        // Check authorization
        self.validate_authorization(transaction, &mut result).await?;

        Ok(result)
    }

    /// Get ledger view for the participant (privacy-preserving)
    pub async fn get_ledger_view(&self) -> GarpResult<LedgerView> {
        let contracts = self.storage.list_contracts(&self.participant_id, true).await?;
        let transactions = self.storage.list_transactions(&self.participant_id, Some(100)).await?;
        let wallet_balance = self.storage.get_wallet_balance(&self.participant_id).await?;

        Ok(LedgerView {
            participant_id: self.participant_id.clone(),
            visible_contracts: contracts,
            transaction_history: transactions,
            wallet_balance,
            last_updated: Utc::now(),
        })
    }

    /// Get contract by ID (only if participant is authorized)
    pub async fn get_contract(&self, contract_id: &ContractId) -> GarpResult<Option<Contract>> {
        if let Some(contract) = self.storage.get_contract(contract_id).await? {
            // Check if participant is authorized to view this contract
            if contract.signatories.contains(&self.participant_id) || 
               contract.observers.contains(&self.participant_id) {
                Ok(Some(contract))
            } else {
                Ok(None) // Privacy: hide unauthorized contracts
            }
        } else {
            Ok(None)
        }
    }

    /// Get wallet balance
    pub async fn get_wallet_balance(&self) -> GarpResult<Option<WalletBalance>> {
        self.storage.get_wallet_balance(&self.participant_id).await
    }

    /// Create ledger checkpoint
    pub async fn create_checkpoint(&self) -> GarpResult<LedgerState> {
        let active_contracts = self.storage.list_contracts(&self.participant_id, true).await?;
        let active_contract_ids: Vec<ContractId> = active_contracts.iter().map(|c| c.id.clone()).collect();
        
        let transactions = self.storage.list_transactions(&self.participant_id, None).await?;
        let last_transaction_id = transactions.first().map(|t| t.id.clone());
        
        let wallet_balance = self.storage.get_wallet_balance(&self.participant_id).await?;

        let state = LedgerState {
            participant_id: self.participant_id.clone(),
            active_contracts: active_contract_ids,
            total_transactions: transactions.len() as u64,
            last_transaction_id,
            wallet_balance,
            checkpoint_time: Utc::now(),
        };

        self.storage.store_ledger_checkpoint(&self.participant_id, &state).await?;
        Ok(state)
    }

    /// Validate transaction signatures
    async fn validate_signatures(&self, transaction: &Transaction) -> GarpResult<()> {
        if transaction.signatures.is_empty() {
            return Err(TransactionError::MissingSignatures.into());
        }

        // Create message to verify
        let message = self.create_signature_message(transaction)?;

        for signature in &transaction.signatures {
            if !self.crypto_service.verify_signature(&signature.public_key, &message, &signature.signature)? {
                return Err(TransactionError::InvalidSignature(signature.signer.clone()).into());
            }
        }

        Ok(())
    }

    /// Create message for signature verification
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

    /// Validate create contract command
    async fn validate_create_contract(&self, cmd: &CreateContractCommand, result: &mut ValidationResult) -> GarpResult<()> {
        // Check if contract ID already exists
        if let Some(_) = self.storage.get_contract(&cmd.contract_id).await? {
            result.valid = false;
            result.errors.push("Contract ID already exists".to_string());
        }

        // Validate signatories and observers
        if cmd.signatories.is_empty() {
            result.valid = false;
            result.errors.push("Contract must have at least one signatory".to_string());
        }

        // Check if participant is authorized to create this contract
        if !cmd.signatories.contains(&self.participant_id) && !cmd.observers.contains(&self.participant_id) {
            result.valid = false;
            result.errors.push("Participant not authorized for this contract".to_string());
        }

        result.affected_contracts.push(cmd.contract_id.clone());
        result.required_signatures.extend(cmd.signatories.iter().cloned());

        Ok(())
    }

    /// Validate exercise contract command
    async fn validate_exercise_contract(&self, cmd: &ExerciseContractCommand, result: &mut ValidationResult) -> GarpResult<()> {
        // Check if contract exists and is active
        if let Some(contract) = self.storage.get_contract(&cmd.contract_id).await? {
            if contract.archived {
                result.valid = false;
                result.errors.push("Cannot exercise archived contract".to_string());
            }

            // Check authorization
            if !contract.signatories.contains(&self.participant_id) {
                result.valid = false;
                result.errors.push("Participant not authorized to exercise this contract".to_string());
            }

            result.affected_contracts.push(cmd.contract_id.clone());
            result.required_signatures.extend(contract.signatories.iter().cloned());
        } else {
            result.valid = false;
            result.errors.push("Contract not found".to_string());
        }

        Ok(())
    }

    /// Validate archive contract command
    async fn validate_archive_contract(&self, cmd: &ArchiveContractCommand, result: &mut ValidationResult) -> GarpResult<()> {
        if let Some(contract) = self.storage.get_contract(&cmd.contract_id).await? {
            if contract.archived {
                result.warnings.push("Contract already archived".to_string());
            }

            // Check authorization
            if !contract.signatories.contains(&self.participant_id) {
                result.valid = false;
                result.errors.push("Participant not authorized to archive this contract".to_string());
            }

            result.affected_contracts.push(cmd.contract_id.clone());
            result.required_signatures.extend(contract.signatories.iter().cloned());
        } else {
            result.valid = false;
            result.errors.push("Contract not found".to_string());
        }

        Ok(())
    }

    /// Validate transfer asset command
    async fn validate_transfer_asset(&self, cmd: &garp_common::TransferAssetCommand, result: &mut ValidationResult) -> GarpResult<()> {
        // Check sender balance
        if let Some(balance) = self.storage.get_wallet_balance(&cmd.from).await? {
            let sender_asset = balance.assets.iter().find(|a| a.id == cmd.asset.id);
            if let Some(asset) = sender_asset {
                if asset.amount < cmd.asset.amount {
                    result.valid = false;
                    result.errors.push("Insufficient balance".to_string());
                }
            } else {
                result.valid = false;
                result.errors.push("Asset not found in sender wallet".to_string());
            }
        } else {
            result.valid = false;
            result.errors.push("Sender wallet not found".to_string());
        }

        // Check authorization
        if cmd.from != self.participant_id {
            result.valid = false;
            result.errors.push("Participant not authorized to transfer from this wallet".to_string());
        }

        result.required_signatures.push(cmd.from.clone());
        if cmd.from != cmd.to {
            result.required_signatures.push(cmd.to.clone());
        }

        Ok(())
    }

    /// Validate create asset command
    async fn validate_create_asset(&self, cmd: &garp_common::CreateAssetCommand, result: &mut ValidationResult) -> GarpResult<()> {
        // Basic validation
        if cmd.asset.amount == 0 {
            result.warnings.push("Creating asset with zero amount".to_string());
        }

        // Check if participant is authorized to create assets
        // This could be based on roles, permissions, etc.
        result.required_signatures.push(self.participant_id.clone());

        Ok(())
    }

    /// Validate transaction authorization
    async fn validate_authorization(&self, transaction: &Transaction, result: &mut ValidationResult) -> GarpResult<()> {
        // Check if all required signatories have signed
        let signed_by: HashSet<ParticipantId> = transaction.signatures.iter()
            .map(|s| s.signer.clone())
            .collect();

        let required: HashSet<ParticipantId> = result.required_signatures.iter().cloned().collect();

        for required_signer in &required {
            if !signed_by.contains(required_signer) {
                result.valid = false;
                result.errors.push(format!("Missing signature from {}", required_signer.0));
            }
        }

        Ok(())
    }

    /// Apply transaction effects to the ledger
    async fn apply_transaction_effects(&self, transaction: &Transaction) -> GarpResult<()> {
        match &transaction.command {
            TransactionCommand::CreateContract(cmd) => {
                let contract = Contract {
                    id: cmd.contract_id.clone(),
                    template_id: cmd.template_id.clone(),
                    signatories: cmd.signatories.clone(),
                    observers: cmd.observers.clone(),
                    argument: cmd.argument.clone(),
                    created_at: transaction.created_at,
                    archived: false,
                };
                self.storage.store_contract(&contract).await?;
            }
            TransactionCommand::ExerciseContract(cmd) => {
                // Exercise effects would be defined by the contract template
                // For now, we just log the exercise
                debug!("Exercised contract {} with choice {}", cmd.contract_id.0, cmd.choice_name);
            }
            TransactionCommand::ArchiveContract(cmd) => {
                self.storage.archive_contract(&cmd.contract_id).await?;
            }
            TransactionCommand::TransferAsset(cmd) => {
                // Debit sender
                self.storage.update_asset_balance(&cmd.from, &cmd.asset, -(cmd.asset.amount as i64)).await?;
                // Credit receiver
                self.storage.update_asset_balance(&cmd.to, &cmd.asset, cmd.asset.amount as i64).await?;
            }
            TransactionCommand::CreateAsset(cmd) => {
                // Credit the owner
                self.storage.update_asset_balance(&cmd.owner, &cmd.asset, cmd.asset.amount as i64).await?;
            }
        }

        Ok(())
    }

    /// Get transaction history for a specific contract
    pub async fn get_contract_history(&self, contract_id: &ContractId) -> GarpResult<Vec<Transaction>> {
        // Check if participant is authorized to view this contract
        if let Some(contract) = self.get_contract(contract_id).await? {
            let all_transactions = self.storage.list_transactions(&self.participant_id, None).await?;
            
            // Filter transactions that affect this contract
            let contract_transactions: Vec<Transaction> = all_transactions
                .into_iter()
                .filter(|tx| self.transaction_affects_contract(tx, contract_id))
                .collect();

            Ok(contract_transactions)
        } else {
            Ok(Vec::new()) // Privacy: return empty if not authorized
        }
    }

    /// Check if a transaction affects a specific contract
    fn transaction_affects_contract(&self, transaction: &Transaction, contract_id: &ContractId) -> bool {
        match &transaction.command {
            TransactionCommand::CreateContract(cmd) => cmd.contract_id == *contract_id,
            TransactionCommand::ExerciseContract(cmd) => cmd.contract_id == *contract_id,
            TransactionCommand::ArchiveContract(cmd) => cmd.contract_id == *contract_id,
            _ => false,
        }
    }

    /// Get assets owned by the participant
    pub async fn get_owned_assets(&self) -> GarpResult<Vec<Asset>> {
        if let Some(balance) = self.get_wallet_balance().await? {
            Ok(balance.assets)
        } else {
            Ok(Vec::new())
        }
    }

    /// Check if participant can view a specific transaction
    pub async fn can_view_transaction(&self, transaction: &Transaction) -> GarpResult<bool> {
        // Participant can view if they are the submitter
        if transaction.submitter == self.participant_id {
            return Ok(true);
        }

        // Or if they are involved in any affected contracts
        match &transaction.command {
            TransactionCommand::CreateContract(cmd) => {
                Ok(cmd.signatories.contains(&self.participant_id) || 
                   cmd.observers.contains(&self.participant_id))
            }
            TransactionCommand::ExerciseContract(cmd) | 
            TransactionCommand::ArchiveContract(cmd) => {
                if let Some(contract) = self.storage.get_contract(&cmd.contract_id).await? {
                    Ok(contract.signatories.contains(&self.participant_id) || 
                       contract.observers.contains(&self.participant_id))
                } else {
                    Ok(false)
                }
            }
            TransactionCommand::TransferAsset(cmd) => {
                Ok(cmd.from == self.participant_id || cmd.to == self.participant_id)
            }
            TransactionCommand::CreateAsset(cmd) => {
                Ok(cmd.owner == self.participant_id)
            }
        }
    }

    /// Perform GDPR-compliant data deletion
    pub async fn forget_participant_data(&self, participant_id: &ParticipantId) -> GarpResult<()> {
        if participant_id != &self.participant_id {
            return Err(GarpError::Unauthorized("Cannot delete other participant's data".to_string()));
        }

        warn!("Performing GDPR data deletion for participant {}", participant_id.0);

        // This is a simplified implementation
        // In practice, you'd need to:
        // 1. Archive all contracts where participant is involved
        // 2. Anonymize transaction history
        // 3. Clear wallet balances
        // 4. Remove personal identifiers while preserving cryptographic integrity

        // For now, we'll just clear the wallet balance as an example
        let empty_balance = WalletBalance {
            participant_id: participant_id.clone(),
            assets: Vec::new(),
            last_updated: Utc::now(),
        };
        
        self.storage.store_wallet_balance(&empty_balance).await?;

        info!("GDPR data deletion completed for participant {}", participant_id.0);
        Ok(())
    }
}

/// Ledger statistics
#[derive(Debug, Clone)]
pub struct LedgerStats {
    pub total_contracts: u64,
    pub active_contracts: u64,
    pub total_transactions: u64,
    pub total_assets: u64,
    pub last_transaction_time: Option<DateTime<Utc>>,
}

impl LocalLedger {
    /// Get ledger statistics
    pub async fn get_stats(&self) -> GarpResult<LedgerStats> {
        let all_contracts = self.storage.list_contracts(&self.participant_id, false).await?;
        let active_contracts = self.storage.list_contracts(&self.participant_id, true).await?;
        let transactions = self.storage.list_transactions(&self.participant_id, None).await?;
        let assets = self.get_owned_assets().await?;

        let last_transaction_time = transactions.first().map(|t| t.created_at);

        Ok(LedgerStats {
            total_contracts: all_contracts.len() as u64,
            active_contracts: active_contracts.len() as u64,
            total_transactions: transactions.len() as u64,
            total_assets: assets.len() as u64,
            last_transaction_time,
        })
    }
}