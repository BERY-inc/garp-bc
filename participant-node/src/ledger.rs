use garp_common::{
    Contract, Transaction, TransactionId, ContractId, ParticipantId, Asset, WalletBalance,
    TransactionCommand, CreateContractCommand, ExerciseContractCommand, ArchiveContractCommand,
    GarpResult, GarpError, TransactionError, CryptoService, DigitalSignature,
    AccountId, ProgramId, TxV2, AccountMeta, RecentBlockhash,
};
use crate::storage::{StorageBackend, LedgerState};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use async_trait::async_trait;
use uuid::Uuid;
use tracing::{info, warn, error, debug};
use bytes::Bytes;
use crate::poh::Poh;
use tokio::time::Duration as TokioDuration;

/// Local ledger for a participant node
pub struct LocalLedger {
    participant_id: ParticipantId,
    storage: Arc<dyn StorageBackend>,
    crypto_service: Arc<CryptoService>,
    // Accounts-as-state store
    accounts: parking_lot::RwLock<HashMap<AccountId, Account>>,
    account_locks: parking_lot::Mutex<HashSet<AccountId>>,
    recent_blockhashes: parking_lot::RwLock<Vec<RecentBlockhash>>, // rolling window
    // PoH and append-only entries
    poh: parking_lot::Mutex<Poh>,
    last_tick_slot: parking_lot::RwLock<u64>,
    tick_count_in_slot: parking_lot::RwLock<u64>,
    ledger_entries: parking_lot::RwLock<Vec<LedgerEntry>>, // append-only tick/tx entries
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
            accounts: parking_lot::RwLock::new(HashMap::new()),
            account_locks: parking_lot::Mutex::new(HashSet::new()),
            recent_blockhashes: parking_lot::RwLock::new(Vec::with_capacity(512)),
            poh: parking_lot::Mutex::new(Poh::new(participant_id.0.as_bytes(), 32)),
            last_tick_slot: parking_lot::RwLock::new(0),
            tick_count_in_slot: parking_lot::RwLock::new(0),
            ledger_entries: parking_lot::RwLock::new(Vec::with_capacity(4096)),
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

        // Append transaction entry (slot will be filled by caller via record_transaction_entry)
        // Store transaction
        self.storage.store_transaction(&transaction).await?;

        // Apply transaction effects
        self.apply_transaction_effects(&transaction).await?;

        info!("Transaction {} successfully applied", transaction.id.0);
        Ok(validation)
    }

    /// Record a PoH tick and enforce slot boundaries
    pub fn record_poh_tick(&self, current_slot: u64) {
        let mut poh = self.poh.lock();
        let hash = poh.tick();
        {
            let mut last_slot = self.last_tick_slot.write();
            let mut tick_count = self.tick_count_in_slot.write();
            if *last_slot != current_slot {
                *last_slot = current_slot;
                *tick_count = 0;
            }
            *tick_count += 1;
        }
        // Record in append-only ledger entries
        self.ledger_entries.write().push(LedgerEntry::Tick {
            slot: current_slot,
            hash: hash.to_vec(),
        });
        // Update recent blockhash window at slot boundary
        if poh.ticks_in_current_slot() == 0 {
            self.record_recent_blockhash(RecentBlockhash(hash.to_vec()));
        }
    }

    /// Record a transaction entry, interleaved after ticks
    pub fn record_transaction_entry(&self, tx_id: TransactionId, slot: u64) {
        self.ledger_entries.write().push(LedgerEntry::TransactionEntry { slot, tx_id });
    }

    /// Compute the PoH tick interval based on chain slot duration and ticks per slot
    pub fn tick_interval(&self, slot_duration_ms: u64) -> TokioDuration {
        let ticks_per_slot = self.poh.lock().ticks_per_slot();
        TokioDuration::from_millis(slot_duration_ms / ticks_per_slot.max(1))
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

// ----------------------------------------------------------------------------
// Accounts-as-state model
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub lamports: u64,
    pub owner: ProgramId,
    pub data: Bytes,
    pub executable: bool,
    pub rent_epoch: u64,
}

impl Account {
    pub fn data_len(&self) -> usize { self.data.len() }
}

/// Rent parameters
#[derive(Debug, Clone)]
pub struct RentParams {
    pub lamports_per_byte_year: u64,
    pub exemption_multiplier: f64,
}

impl Default for RentParams {
    fn default() -> Self {
        Self { lamports_per_byte_year: 3480, exemption_multiplier: 2.0 }
    }
}

impl LocalLedger {
    /// Create a new account
    pub fn create_account(&self, id: AccountId, owner: ProgramId, lamports: u64, data: Bytes, executable: bool) {
        let mut accounts = self.accounts.write();
        accounts.insert(id.clone(), Account { id, lamports, owner, data, executable, rent_epoch: 0 });
    }

    /// Get zero-copy account data slice
    pub fn get_account_data_zero_copy(&self, id: &AccountId) -> Option<Bytes> {
        self.accounts.read().get(id).map(|a| a.data.clone())
    }

    /// Lock accounts for a transaction (write locks)
    pub fn lock_accounts(&self, metas: &[AccountMeta]) -> GarpResult<()> {
        let mut locks = self.account_locks.lock();
        for m in metas.iter().filter(|m| m.is_writable) {
            if locks.contains(&m.account) {
                return Err(GarpError::Internal(format!("Account {} already locked", (m.account).0)));
            }
        }
        for m in metas.iter().filter(|m| m.is_writable) {
            locks.insert(m.account.clone());
        }
        Ok(())
    }

    /// Unlock accounts after a transaction
    pub fn unlock_accounts(&self, metas: &[AccountMeta]) {
        let mut locks = self.account_locks.lock();
        for m in metas.iter().filter(|m| m.is_writable) {
            locks.remove(&m.account);
        }
    }

    /// Enforce program ownership for writable accounts
    pub fn enforce_program_ownership(&self, metas: &[AccountMeta], program: &ProgramId) -> GarpResult<()> {
        let accounts = self.accounts.read();
        for m in metas.iter().filter(|m| m.is_writable) {
            let Some(acc) = accounts.get(&m.account) else {
                return Err(GarpError::Internal(format!("Missing account {}", (m.account).0)));
            };
            if &acc.owner != program {
                return Err(GarpError::Internal(format!("Program ownership violation for {}", (m.account).0)));
            }
        }
        Ok(())
    }

    /// Check rent exemption threshold for an account
    pub fn is_rent_exempt(&self, acc: &Account, params: &RentParams) -> bool {
        let threshold = (acc.data_len() as f64 * params.exemption_multiplier) as u64 * params.lamports_per_byte_year;
        acc.lamports >= threshold
    }

    /// Apply rent to inactive accounts; purge those that run out of lamports
    pub fn apply_rent(&self, current_epoch: u64, params: &RentParams) {
        let mut accounts = self.accounts.write();
        accounts.retain(|_id, acc| {
            if acc.executable { return true; }
            // simplistic: charge per epoch based on data size
            let charge = (acc.data_len() as u64).saturating_mul(params.lamports_per_byte_year);
            if acc.rent_epoch < current_epoch {
                let epochs = current_epoch - acc.rent_epoch;
                let total = charge.saturating_mul(epochs);
                acc.lamports = acc.lamports.saturating_sub(total);
                acc.rent_epoch = current_epoch;
            }
            acc.lamports > 0 || self.is_rent_exempt(acc, params)
        });
    }

    /// Save accounts snapshot to a file
    pub fn save_accounts_snapshot(&self, path: &str) -> GarpResult<()> {
        let accounts = self.accounts.read();
        let serialized = serde_json::to_vec(&*accounts).map_err(|e| GarpError::Internal(e.to_string()))?;
        std::fs::write(path, serialized).map_err(|e| GarpError::Internal(e.to_string()))?;
        Ok(())
    }

    /// Load accounts snapshot from a file
    pub fn load_accounts_snapshot(&self, path: &str) -> GarpResult<()> {
        let bytes = std::fs::read(path).map_err(|e| GarpError::Internal(e.to_string()))?;
        let map: HashMap<AccountId, Account> = serde_json::from_slice(&bytes).map_err(|e| GarpError::Internal(e.to_string()))?;
        let mut accounts = self.accounts.write();
        *accounts = map;
        Ok(())
    }

    /// Maintain recent blockhash window for anti-replay
    pub fn record_recent_blockhash(&self, bh: RecentBlockhash) {
        let mut window = self.recent_blockhashes.write();
        if window.len() >= 512 { window.remove(0); }
        window.push(bh);
    }

    pub fn is_recent_blockhash(&self, bh: &RecentBlockhash) -> bool {
        self.recent_blockhashes.read().iter().any(|x| x == bh)
    }
}

// ----------------------------------------------------------------------------
// TxV2 validation and simulation
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimulationResult {
    pub accepted: bool,
    pub estimated_fee_lamports: u64,
    pub logs: Vec<String>,
}

impl LocalLedger {
    /// Preflight validation: recent blockhash, signatures, account locks, ownership
    pub async fn preflight_validate_v2(&self, tx: &TxV2) -> GarpResult<bool> {
        // Recent blockhash check
        if !self.is_recent_blockhash(&tx.recent_blockhash) && tx.durable_nonce.is_none() {
            return Ok(false);
        }

        // Parallel signature verification (GPU-accelerated placeholder)
        let sig_ok = {
            let sigs = tx.signatures.clone();
            let msg = self.create_v2_message(tx)?;
            let crypto = self.crypto_service.clone();
            tokio::task::spawn_blocking(move || {
                sigs.iter().all(|s| crypto.verify_signature(&s.public_key, &msg, &s.signature).unwrap_or(false))
            })
            .await
            .map_err(|e| GarpError::Internal(e.to_string()))?
        };
        if !sig_ok { return Ok(false); }

        // Account locks and ownership per instruction
        for ix in &tx.instructions {
            self.lock_accounts(&ix.accounts)?;
            self.enforce_program_ownership(&ix.accounts, &ix.program)?;
            self.unlock_accounts(&ix.accounts);
        }
        Ok(true)
    }

    /// Simulate transaction execution for fee estimation and DX
    pub async fn simulate_v2(&self, tx: &TxV2) -> GarpResult<SimulationResult> {
        let preflight = self.preflight_validate_v2(tx).await?;
        let mut logs = vec![];
        let mut estimated_fee = 5000; // base fee
        if let Some(b) = &tx.compute_budget { estimated_fee += (b.max_units / 10) as u64; }
        logs.push("Preflight complete".into());
        Ok(SimulationResult { accepted: preflight, estimated_fee_lamports: estimated_fee, logs })
    }

    fn create_v2_message(&self, tx: &TxV2) -> GarpResult<Vec<u8>> {
        let mut msg = Vec::new();
        msg.extend_from_slice(tx.id.0.as_bytes());
        msg.extend_from_slice(&tx.recent_blockhash.0);
        msg.extend_from_slice(&tx.slot.to_le_bytes());
        // Include fee payer and account list
        msg.extend_from_slice(tx.fee_payer.0.as_bytes());
        for a in &tx.account_keys { msg.extend_from_slice(a.0.as_bytes()); }
        Ok(msg)
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
// -----------------------------------------------------------------------------
// Deterministic validation rules for blocks and transactions
// -----------------------------------------------------------------------------

/// Validates block size and aggregate gas usage against configured limits.
pub fn validate_block_limits(block_bytes: &[u8], tx_gas_costs: &[u64], max_block_size_bytes: usize, max_block_gas: u64) -> Result<(), String> {
    if block_bytes.len() > max_block_size_bytes {
        return Err(format!("block size {} exceeds limit {}", block_bytes.len(), max_block_size_bytes));
    }
    let total_gas: u64 = tx_gas_costs.iter().copied().sum();
    if total_gas > max_block_gas {
        return Err(format!("total gas {} exceeds limit {}", total_gas, max_block_gas));
    }
    Ok(())
}

/// Minimal deterministic transaction correctness checks (placeholder).
pub fn validate_transaction_basic(tx_bytes: &[u8], max_tx_size_bytes: usize) -> Result<(), String> {
    if tx_bytes.is_empty() { return Err("empty transaction".into()); }
    if tx_bytes.len() > max_tx_size_bytes { return Err("transaction too large".into()); }
    // TODO: verify canonical encoding, signatures, nonces, and fees deterministically.
    Ok(())
}

/// Append-only ledger entry for PoH and transactions
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LedgerEntry {
    Tick { slot: u64, hash: Vec<u8> },
    TransactionEntry { slot: u64, tx_id: TransactionId },
}