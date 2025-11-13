use garp_common::{
    Contract, ContractId, ParticipantId, Transaction, TransactionCommand,
    CreateContractCommand, ExerciseContractCommand, ArchiveContractCommand,
    CryptoService, GarpResult, GarpError, ContractError
};
use crate::storage::StorageBackend;
use crate::wasm_runtime::{WasmRuntime, WasmExecutionResult, WasmHostFunctions};
use crate::contract_state::ContractStateManager; // Add this import
use std::sync::Arc;
use std::collections::HashMap;
use serde_json::{Value, Map};
use chrono::Utc;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

/// Contract execution engine
pub struct ContractEngine {
    storage: Arc<dyn StorageBackend>,
    crypto_service: Arc<CryptoService>,
    template_registry: Arc<TemplateRegistry>,
    wasm_runtime: Arc<WasmRuntime>,
    contract_state_manager: Arc<ContractStateManager>, // Add contract state manager
}

/// Contract template registry
pub struct TemplateRegistry {
    templates: parking_lot::RwLock<HashMap<String, ContractTemplate>>,
}

/// Contract template definition
#[derive(Debug, Clone)]
pub struct ContractTemplate {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub parameters: Vec<TemplateParameter>,
    pub choices: Vec<TemplateChoice>,
    pub validation_rules: Vec<ValidationRule>,
    pub privacy_settings: PrivacySettings,
    pub wasm_bytecode: Option<Vec<u8>>, // Optional WASM bytecode for smart contracts
    pub upgrade_policy: UpgradePolicy, // Contract upgrade policy
}

/// Template parameter definition
#[derive(Debug, Clone)]
pub struct TemplateParameter {
    pub name: String,
    pub param_type: ParameterType,
    pub required: bool,
    pub description: String,
    pub default_value: Option<Value>,
}

/// Parameter types
#[derive(Debug, Clone)]
pub enum ParameterType {
    String,
    Number,
    Boolean,
    ParticipantId,
    AssetId,
    DateTime,
    Object(String), // Custom object type
}

/// Template choice (action that can be performed on the contract)
#[derive(Debug, Clone)]
pub struct TemplateChoice {
    pub name: String,
    pub description: String,
    pub parameters: Vec<TemplateParameter>,
    pub authorization: AuthorizationRule,
    pub effects: Vec<ContractEffect>,
}

/// Authorization rules for contract choices
#[derive(Debug, Clone)]
pub enum AuthorizationRule {
    RequireSignatory(ParticipantId),
    RequireAnySignatory,
    RequireAllSignatories,
    RequireObserver(ParticipantId),
    Custom(String), // Custom authorization logic
}

/// Contract effects (what happens when a choice is exercised)
#[derive(Debug, Clone)]
pub enum ContractEffect {
    CreateContract(String), // Template ID for new contract
    ArchiveContract,
    TransferAsset {
        from: String, // Parameter name
        to: String,   // Parameter name
        asset: String, // Parameter name
    },
    UpdateContractData {
        field: String,
        value: Value,
    },
    EmitEvent {
        event_type: String,
        data: Value,
    },
    ExecuteWasm {
        contract_id: String,
        function_name: String,
        arguments: Value,
    },
}

/// Validation rules for contract parameters
#[derive(Debug, Clone)]
pub struct ValidationRule {
    pub field: String,
    pub rule_type: ValidationRuleType,
    pub message: String,
}

/// Types of validation rules
#[derive(Debug, Clone)]
pub enum ValidationRuleType {
    Required,
    MinValue(f64),
    MaxValue(f64),
    MinLength(usize),
    MaxLength(usize),
    Pattern(String), // Regex pattern
    Custom(String),  // Custom validation function
}

/// Privacy settings for contract templates
#[derive(Debug, Clone)]
pub struct PrivacySettings {
    pub encrypt_arguments: bool,
    pub encrypt_choice_data: bool,
    pub visible_to_observers: Vec<String>, // Which fields observers can see
    pub audit_trail: bool,
}

/// Contract upgrade policy
#[derive(Debug, Clone)]
pub enum UpgradePolicy {
    /// Contract cannot be upgraded
    None,
    /// Contract can be upgraded by any signatory
    Signatory,
    /// Contract can be upgraded by a specific participant
    Participant(ParticipantId),
    /// Contract can be upgraded through a governance process
    Governance,
}

/// Contract execution context
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub contract: Contract,
    pub choice_name: String,
    pub choice_arguments: Value,
    pub executor: ParticipantId,
    pub timestamp: chrono::DateTime<Utc>,
}

/// Contract execution result
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub success: bool,
    pub effects: Vec<ExecutedEffect>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub new_contracts: Vec<Contract>,
    pub archived_contracts: Vec<ContractId>,
    pub events: Vec<ContractEvent>,
    pub upgraded_contracts: Vec<ContractId>, // Contracts that were upgraded
}

/// Executed effect
#[derive(Debug, Clone)]
pub struct ExecutedEffect {
    pub effect_type: String,
    pub description: String,
    pub data: Value,
}

/// Contract event
#[derive(Debug, Clone)]
pub struct ContractEvent {
    pub id: String,
    pub contract_id: ContractId,
    pub event_type: String,
    pub data: Value,
    pub timestamp: chrono::DateTime<Utc>,
    pub emitter: ParticipantId,
}

impl ContractEngine {
    /// Create new contract engine
    pub fn new(
        storage: Arc<dyn StorageBackend>,
        crypto_service: Arc<CryptoService>,
    ) -> Self {
        let template_registry = Arc::new(TemplateRegistry::new());
        let wasm_runtime = Arc::new(WasmRuntime::new(
            storage.clone(),
            crypto_service.clone(),
            1_000_000, // Default gas limit
            1024 * 1024 // Default memory limit (1MB)
        ));
        
        // Create contract state manager
        let contract_state_manager = Arc::new(ContractStateManager::new(storage.clone()));
        
        // Register built-in templates
        let mut engine = Self {
            storage,
            crypto_service,
            template_registry,
            wasm_runtime,
            contract_state_manager, // Add contract state manager
        };
        
        engine.register_builtin_templates();
        engine
    }

    /// Execute a contract choice
    pub async fn execute_contract(
        &self,
        contract_id: &ContractId,
        choice_name: &str,
        choice_arguments: Value,
        executor: &ParticipantId,
    ) -> GarpResult<ExecutionResult> {
        info!("Executing contract {} choice {} by {}", contract_id.0, choice_name, executor.0);

        // Get contract
        let contract = self.storage.get_contract(contract_id).await?
            .ok_or_else(|| ContractError::ContractNotFound(contract_id.clone()))?;

        // Check if contract is active
        if contract.archived {
            return Err(ContractError::ContractArchived(contract_id.clone()).into());
        }

        // Get template
        let template = self.template_registry.get_template(&contract.template_id)?;

        // Find choice
        let choice = template.choices.iter()
            .find(|c| c.name == choice_name)
            .ok_or_else(|| ContractError::ChoiceNotFound(choice_name.to_string()))?;

        // Create execution context
        let context = ExecutionContext {
            contract: contract.clone(),
            choice_name: choice_name.to_string(),
            choice_arguments: choice_arguments.clone(),
            executor: executor.clone(),
            timestamp: Utc::now(),
        };

        // Validate authorization
        self.validate_authorization(&context, choice)?;

        // Validate choice arguments
        self.validate_choice_arguments(&context, choice)?;

        // Execute effects
        let result = self.execute_effects(&context, choice).await?;

        info!("Contract execution completed: {} effects applied", result.effects.len());
        Ok(result)
    }

    /// Create a new contract
    pub async fn create_contract(
        &self,
        template_id: &str,
        signatories: Vec<ParticipantId>,
        observers: Vec<ParticipantId>,
        arguments: Value,
        creator: &ParticipantId,
    ) -> GarpResult<Contract> {
        info!("Creating contract with template {} by {}", template_id, creator.0);

        // Get template
        let template = self.template_registry.get_template(template_id)?;

        // Validate arguments
        self.validate_template_arguments(&arguments, &template)?;

        // Check authorization (creator must be a signatory)
        if !signatories.contains(creator) {
            return Err(ContractError::Unauthorized("Creator must be a signatory".to_string()).into());
        }

        // Create contract
        let contract = Contract {
            id: ContractId(Uuid::new_v4()),
            template_id: template_id.to_string(),
            signatories,
            observers,
            argument: arguments,
            created_at: Utc::now(),
            archived: false,
        };

        // Store contract
        self.storage.store_contract(&contract).await?;

        info!("Contract {} created successfully", contract.id.0);
        Ok(contract)
    }

    /// Archive a contract
    pub async fn archive_contract(
        &self,
        contract_id: &ContractId,
        archiver: &ParticipantId,
    ) -> GarpResult<()> {
        info!("Archiving contract {} by {}", contract_id.0, archiver.0);

        // Get contract
        let contract = self.storage.get_contract(contract_id).await?
            .ok_or_else(|| ContractError::ContractNotFound(contract_id.clone()))?;

        // Check authorization
        if !contract.signatories.contains(archiver) {
            return Err(ContractError::Unauthorized("Only signatories can archive contracts".to_string()).into());
        }

        // Archive contract
        self.storage.archive_contract(contract_id).await?;

        info!("Contract {} archived successfully", contract_id.0);
        Ok(())
    }

    /// Get contract template
    pub fn get_template(&self, template_id: &str) -> GarpResult<ContractTemplate> {
        self.template_registry.get_template(template_id)
    }

    /// List available templates
    pub fn list_templates(&self) -> Vec<ContractTemplate> {
        self.template_registry.list_templates()
    }

    /// Register a new contract template
    pub fn register_template(&self, template: ContractTemplate) -> GarpResult<()> {
        self.template_registry.register_template(template)
    }

    /// Deploy a new contract from WASM bytecode
    pub async fn deploy_contract(
        &self,
        wasm_bytecode: Vec<u8>,
        signatories: Vec<ParticipantId>,
        observers: Vec<ParticipantId>,
        arguments: Value,
        deployer: &ParticipantId,
    ) -> GarpResult<Contract> {
        info!("Deploying contract from WASM bytecode by {}", deployer.0);

        // Validate WASM bytecode
        self.wasm_runtime.validate_bytecode(&wasm_bytecode)?;

        // Create a new template for this contract
        let template_id = format!("contract_{}", uuid::Uuid::new_v4());
        let template = ContractTemplate {
            id: template_id.clone(),
            name: "Deployed Contract".to_string(),
            version: "1.0.0".to_string(),
            description: "Deployed smart contract".to_string(),
            parameters: vec![],
            choices: vec![],
            validation_rules: vec![],
            privacy_settings: PrivacySettings {
                encrypt_arguments: false,
                encrypt_choice_data: false,
                visible_to_observers: vec![],
                audit_trail: true,
            },
            wasm_bytecode: Some(wasm_bytecode.clone()),
            upgrade_policy: UpgradePolicy::Signatory, // Allow signatories to upgrade
        };

        // Register the template
        self.register_template(template)?;

        // Create contract
        let contract = Contract {
            id: garp_common::ContractId(uuid::Uuid::new_v4()),
            template_id: template_id.clone(),
            signatories,
            observers,
            argument: arguments,
            created_at: chrono::Utc::now(),
            archived: false,
        };

        // Store contract
        self.storage.store_contract(&contract).await?;

        // Load WASM contract into runtime
        self.wasm_runtime.load_contract(&contract.id.0.to_string(), wasm_bytecode).await?;

        info!("Contract {} deployed successfully with template {}", contract.id.0, template_id);
        Ok(contract)
    }

    /// Upgrade an existing contract with new WASM bytecode
    pub async fn upgrade_contract(
        &self,
        contract_id: &garp_common::ContractId,
        new_wasm_bytecode: Vec<u8>,
        upgrader: &ParticipantId,
    ) -> GarpResult<()> {
        info!("Upgrading contract {} by {}", contract_id.0, upgrader.0);

        // Get contract
        let contract = self.storage.get_contract(contract_id).await?
            .ok_or_else(|| ContractError::ContractNotFound(contract_id.clone()))?;

        // Get template
        let template = self.template_registry.get_template(&contract.template_id)?;

        // Check upgrade policy
        match &template.upgrade_policy {
            UpgradePolicy::None => {
                return Err(ContractError::Unauthorized("Contract cannot be upgraded".to_string()).into());
            }
            UpgradePolicy::Signatory => {
                if !contract.signatories.contains(upgrader) {
                    return Err(ContractError::Unauthorized("Only signatories can upgrade this contract".to_string()).into());
                }
            }
            UpgradePolicy::Participant(required_participant) => {
                if upgrader != required_participant {
                    return Err(ContractError::Unauthorized(
                        format!("Only participant {} can upgrade this contract", required_participant.0)
                    ).into());
                }
            }
            UpgradePolicy::Governance => {
                // Governance upgrade would require a separate process
                return Err(ContractError::Unauthorized("Governance upgrade not implemented".to_string()).into());
            }
        }

        // Validate new WASM bytecode
        self.wasm_runtime.validate_bytecode(&new_wasm_bytecode)?;

        // Create updated template
        let mut updated_template = template.clone();
        updated_template.wasm_bytecode = Some(new_wasm_bytecode.clone());
        updated_template.version = format!("{}.{}.{}", 
            updated_template.version.split('.').nth(0).unwrap_or("0"),
            updated_template.version.split('.').nth(1).unwrap_or("0"),
            updated_template.version.split('.').nth(2).unwrap_or("0").parse::<u32>().unwrap_or(0) + 1
        );

        // Update template in registry
        self.template_registry.register_template(updated_template)?;

        // Reload WASM contract in runtime
        self.wasm_runtime.load_contract(&contract.id.0.to_string(), new_wasm_bytecode).await?;

        info!("Contract {} upgraded successfully", contract_id.0);
        Ok(())
    }

    /// Validate authorization for contract execution
    fn validate_authorization(
        &self,
        context: &ExecutionContext,
        choice: &TemplateChoice,
    ) -> GarpResult<()> {
        match &choice.authorization {
            AuthorizationRule::RequireSignatory(required) => {
                if context.executor != *required {
                    return Err(ContractError::Unauthorized(
                        format!("Choice requires signatory {}", required.0)
                    ).into());
                }
            }
            AuthorizationRule::RequireAnySignatory => {
                if !context.contract.signatories.contains(&context.executor) {
                    return Err(ContractError::Unauthorized(
                        "Choice requires any signatory".to_string()
                    ).into());
                }
            }
            AuthorizationRule::RequireAllSignatories => {
                // This would require multi-party signatures in a real implementation
                if !context.contract.signatories.contains(&context.executor) {
                    return Err(ContractError::Unauthorized(
                        "Choice requires all signatories".to_string()
                    ).into());
                }
            }
            AuthorizationRule::RequireObserver(required) => {
                if context.executor != *required {
                    return Err(ContractError::Unauthorized(
                        format!("Choice requires observer {}", required.0)
                    ).into());
                }
            }
            AuthorizationRule::Custom(_rule) => {
                // Custom authorization logic would be implemented here
                debug!("Custom authorization rule not implemented");
            }
        }

        Ok(())
    }

    /// Validate choice arguments
    fn validate_choice_arguments(
        &self,
        context: &ExecutionContext,
        choice: &TemplateChoice,
    ) -> GarpResult<()> {
        for param in &choice.parameters {
            self.validate_parameter(&context.choice_arguments, param)?;
        }
        Ok(())
    }

    /// Validate template arguments
    fn validate_template_arguments(
        &self,
        arguments: &Value,
        template: &ContractTemplate,
    ) -> GarpResult<()> {
        for param in &template.parameters {
            self.validate_parameter(arguments, param)?;
        }

        // Apply validation rules
        for rule in &template.validation_rules {
            self.apply_validation_rule(arguments, rule)?;
        }

        Ok(())
    }

    /// Validate a single parameter
    fn validate_parameter(&self, arguments: &Value, param: &TemplateParameter) -> GarpResult<()> {
        let value = arguments.get(&param.name);

        // Check if required parameter is present
        if param.required && value.is_none() {
            return Err(ContractError::ValidationFailed(
                format!("Required parameter '{}' is missing", param.name)
            ).into());
        }

        // Validate type if value is present
        if let Some(value) = value {
            self.validate_parameter_type(value, &param.param_type, &param.name)?;
        }

        Ok(())
    }

    /// Validate parameter type
    fn validate_parameter_type(
        &self,
        value: &Value,
        param_type: &ParameterType,
        param_name: &str,
    ) -> GarpResult<()> {
        match param_type {
            ParameterType::String => {
                if !value.is_string() {
                    return Err(ContractError::ValidationFailed(
                        format!("Parameter '{}' must be a string", param_name)
                    ).into());
                }
            }
            ParameterType::Number => {
                if !value.is_number() {
                    return Err(ContractError::ValidationFailed(
                        format!("Parameter '{}' must be a number", param_name)
                    ).into());
                }
            }
            ParameterType::Boolean => {
                if !value.is_boolean() {
                    return Err(ContractError::ValidationFailed(
                        format!("Parameter '{}' must be a boolean", param_name)
                    ).into());
                }
            }
            ParameterType::ParticipantId => {
                if !value.is_string() {
                    return Err(ContractError::ValidationFailed(
                        format!("Parameter '{}' must be a participant ID (string)", param_name)
                    ).into());
                }
            }
            ParameterType::AssetId => {
                if !value.is_string() {
                    return Err(ContractError::ValidationFailed(
                        format!("Parameter '{}' must be an asset ID (string)", param_name)
                    ).into());
                }
            }
            ParameterType::DateTime => {
                if !value.is_string() {
                    return Err(ContractError::ValidationFailed(
                        format!("Parameter '{}' must be a datetime (ISO string)", param_name)
                    ).into());
                }
            }
            ParameterType::Object(_object_type) => {
                if !value.is_object() {
                    return Err(ContractError::ValidationFailed(
                        format!("Parameter '{}' must be an object", param_name)
                    ).into());
                }
            }
        }

        Ok(())
    }

    /// Apply validation rule
    fn apply_validation_rule(&self, arguments: &Value, rule: &ValidationRule) -> GarpResult<()> {
        let value = arguments.get(&rule.field);

        match &rule.rule_type {
            ValidationRuleType::Required => {
                if value.is_none() {
                    return Err(ContractError::ValidationFailed(rule.message.clone()).into());
                }
            }
            ValidationRuleType::MinValue(min) => {
                if let Some(Value::Number(num)) = value {
                    if num.as_f64().unwrap_or(0.0) < *min {
                        return Err(ContractError::ValidationFailed(rule.message.clone()).into());
                    }
                }
            }
            ValidationRuleType::MaxValue(max) => {
                if let Some(Value::Number(num)) = value {
                    if num.as_f64().unwrap_or(0.0) > *max {
                        return Err(ContractError::ValidationFailed(rule.message.clone()).into());
                    }
                }
            }
            ValidationRuleType::MinLength(min) => {
                if let Some(Value::String(s)) = value {
                    if s.len() < *min {
                        return Err(ContractError::ValidationFailed(rule.message.clone()).into());
                    }
                }
            }
            ValidationRuleType::MaxLength(max) => {
                if let Some(Value::String(s)) = value {
                    if s.len() > *max {
                        return Err(ContractError::ValidationFailed(rule.message.clone()).into());
                    }
                }
            }
            ValidationRuleType::Pattern(_pattern) => {
                // Regex validation would be implemented here
                debug!("Pattern validation not implemented");
            }
            ValidationRuleType::Custom(_function) => {
                // Custom validation would be implemented here
                debug!("Custom validation not implemented");
            }
        }

        Ok(())
    }

    /// Execute contract effects
    async fn execute_effects(
        &self,
        context: &ExecutionContext,
        choice: &TemplateChoice,
    ) -> GarpResult<ExecutionResult> {
        let mut result = ExecutionResult {
            success: true,
            effects: Vec::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
            new_contracts: Vec::new(),
            archived_contracts: Vec::new(),
            events: Vec::new(),
            upgraded_contracts: Vec::new(),
        };

        for effect in &choice.effects {
            match self.execute_single_effect(context, effect).await {
                Ok(executed_effect) => {
                    result.effects.push(executed_effect);
                }
                Err(e) => {
                    result.success = false;
                    result.errors.push(e.to_string());
                }
            }
        }

        Ok(result)
    }

    /// Execute a single effect
    async fn execute_single_effect(
        &self,
        context: &ExecutionContext,
        effect: &ContractEffect,
    ) -> GarpResult<ExecutedEffect> {
        match effect {
            ContractEffect::CreateContract(template_id) => {
                // This would create a new contract based on the template
                Ok(ExecutedEffect {
                    effect_type: "CreateContract".to_string(),
                    description: format!("Created contract with template {}", template_id),
                    data: serde_json::json!({ "template_id": template_id }),
                })
            }
            ContractEffect::ArchiveContract => {
                // Archive the current contract
                self.storage.archive_contract(&context.contract.id).await?;
                Ok(ExecutedEffect {
                    effect_type: "ArchiveContract".to_string(),
                    description: "Archived contract".to_string(),
                    data: serde_json::json!({ "contract_id": context.contract.id.0 }),
                })
            }
            ContractEffect::TransferAsset { from, to, asset } => {
                // This would trigger an asset transfer
                Ok(ExecutedEffect {
                    effect_type: "TransferAsset".to_string(),
                    description: format!("Transfer asset from {} to {}", from, to),
                    data: serde_json::json!({
                        "from": from,
                        "to": to,
                        "asset": asset
                    }),
                })
            }
            ContractEffect::UpdateContractData { field, value } => {
                // This would update contract data
                Ok(ExecutedEffect {
                    effect_type: "UpdateContractData".to_string(),
                    description: format!("Updated field {}", field),
                    data: serde_json::json!({
                        "field": field,
                        "value": value
                    }),
                })
            }
            ContractEffect::EmitEvent { event_type, data } => {
                // Emit a contract event and store it
                let event = ContractEvent {
                    id: uuid::Uuid::new_v4().to_string(),
                    contract_id: context.contract.id.clone(),
                    event_type: event_type.clone(),
                    data: data.clone(),
                    timestamp: chrono::Utc::now(),
                    emitter: context.executor.clone(),
                };
                
                // Store the event
                self.storage.store_contract_event(&event).await?;
                
                Ok(ExecutedEffect {
                    effect_type: "EmitEvent".to_string(),
                    description: format!("Emitted event {}", event_type),
                    data: serde_json::json!({
                        "event_type": event_type,
                        "data": data
                    }),
                })
            }
            ContractEffect::ExecuteWasm { contract_id, function_name, arguments } => {
                // Execute WASM function
                let wasm_context = crate::wasm_runtime::WasmExecutionContext {
                    contract_id: contract_id.clone(),
                    function_name: function_name.clone(),
                    arguments: vec![], // In a real implementation, you would convert the arguments
                    caller: context.executor.0.clone(),
                    gas_limit: 1_000_000,
                    timestamp: chrono::Utc::now(),
                };
                
                let execution_result = self.wasm_runtime.execute_function(wasm_context).await?;
                
                // Store any events emitted by the WASM contract
                for wasm_event in execution_result.events {
                    let contract_event = ContractEvent {
                        id: uuid::Uuid::new_v4().to_string(),
                        contract_id: ContractId(uuid::Uuid::parse_str(contract_id).unwrap_or_default()),
                        event_type: wasm_event.name,
                        data: wasm_event.data,
                        timestamp: wasm_event.timestamp,
                        emitter: context.executor.clone(),
                    };
                    
                    self.storage.store_contract_event(&contract_event).await?;
                }
                
                Ok(ExecutedEffect {
                    effect_type: "ExecuteWasm".to_string(),
                    description: format!("Executed WASM function {} on contract {}", function_name, contract_id),
                    data: serde_json::json!({
                        "contract_id": contract_id,
                        "function_name": function_name,
                        "arguments": arguments,
                        "success": execution_result.success,
                        "gas_used": execution_result.gas_used,
                    }),
                })
            }
        }
    }

    /// Register built-in contract templates
    fn register_builtin_templates(&mut self) {
        // E-commerce purchase contract
        let purchase_template = ContractTemplate {
            id: "ecommerce.purchase".to_string(),
            name: "E-commerce Purchase".to_string(),
            version: "1.0.0".to_string(),
            description: "Atomic purchase transaction with escrow".to_string(),
            parameters: vec![
                TemplateParameter {
                    name: "buyer".to_string(),
                    param_type: ParameterType::ParticipantId,
                    required: true,
                    description: "Buyer participant ID".to_string(),
                    default_value: None,
                },
                TemplateParameter {
                    name: "seller".to_string(),
                    param_type: ParameterType::ParticipantId,
                    required: true,
                    description: "Seller participant ID".to_string(),
                    default_value: None,
                },
                TemplateParameter {
                    name: "product_id".to_string(),
                    param_type: ParameterType::AssetId,
                    required: true,
                    description: "Product asset ID".to_string(),
                    default_value: None,
                },
                TemplateParameter {
                    name: "price".to_string(),
                    param_type: ParameterType::Number,
                    required: true,
                    description: "Purchase price".to_string(),
                    default_value: None,
                },
            ],
            choices: vec![
                TemplateChoice {
                    name: "confirm_payment".to_string(),
                    description: "Buyer confirms payment".to_string(),
                    parameters: vec![],
                    authorization: AuthorizationRule::RequireSignatory(
                        ParticipantId("buyer".to_string()) // Would be resolved at runtime
                    ),
                    effects: vec![
                        ContractEffect::TransferAsset {
                            from: "buyer".to_string(),
                            to: "escrow".to_string(),
                            asset: "payment".to_string(),
                        }
                    ],
                },
                TemplateChoice {
                    name: "ship_product".to_string(),
                    description: "Seller ships product".to_string(),
                    parameters: vec![],
                    authorization: AuthorizationRule::RequireSignatory(
                        ParticipantId("seller".to_string())
                    ),
                    effects: vec![
                        ContractEffect::UpdateContractData {
                            field: "status".to_string(),
                            value: serde_json::json!("shipped"),
                        }
                    ],
                },
                TemplateChoice {
                    name: "complete_purchase".to_string(),
                    description: "Complete the purchase".to_string(),
                    parameters: vec![],
                    authorization: AuthorizationRule::RequireAnySignatory,
                    effects: vec![
                        ContractEffect::TransferAsset {
                            from: "escrow".to_string(),
                            to: "seller".to_string(),
                            asset: "payment".to_string(),
                        },
                        ContractEffect::TransferAsset {
                            from: "seller".to_string(),
                            to: "buyer".to_string(),
                            asset: "product".to_string(),
                        },
                        ContractEffect::ArchiveContract,
                    ],
                },
            ],
            validation_rules: vec![
                ValidationRule {
                    field: "price".to_string(),
                    rule_type: ValidationRuleType::MinValue(0.0),
                    message: "Price must be positive".to_string(),
                },
            ],
            privacy_settings: PrivacySettings {
                encrypt_arguments: true,
                encrypt_choice_data: true,
                visible_to_observers: vec!["status".to_string()],
                audit_trail: true,
            },
            wasm_bytecode: None,
            upgrade_policy: UpgradePolicy::None,
        };

        let _ = self.register_template(purchase_template);

        // Add more built-in templates as needed
    }
}

impl TemplateRegistry {
    /// Create new template registry
    pub fn new() -> Self {
        Self {
            templates: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    /// Register a template
    pub fn register_template(&self, template: ContractTemplate) -> GarpResult<()> {
        let mut templates = self.templates.write();
        templates.insert(template.id.clone(), template);
        Ok(())
    }

    /// Get a template by ID
    pub fn get_template(&self, template_id: &str) -> GarpResult<ContractTemplate> {
        let templates = self.templates.read();
        templates.get(template_id)
            .cloned()
            .ok_or_else(|| ContractError::TemplateNotFound(template_id.to_string()).into())
    }

    /// List all templates
    pub fn list_templates(&self) -> Vec<ContractTemplate> {
        let templates = self.templates.read();
        templates.values().cloned().collect()
    }
}