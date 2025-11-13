use garp_common::{GarpResult, GarpError, ContractError, CryptoService, ContractId};
use crate::storage::StorageBackend;
use crate::contract_state::ContractStateManager; // Add this import
use std::sync::Arc;
use std::collections::HashMap;
use serde_json::Value;
use tracing::{info, warn, error, debug};
use uuid::Uuid;
use chrono::Utc;
use wasmtime::{
    Engine, Store, Module, Instance, Func, TypedFunc, Linker, Memory, MemoryType,
    StoreContextMut, AsContextMut, AsContext, Caller, Extern, Global, GlobalType, Mutability, Val, ValType
};
use wasmtime_wasi::WasiCtx;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::cell::RefCell;
use std::rc::Rc;

/// WASM runtime for executing smart contracts
pub struct WasmRuntime {
    storage: Arc<dyn StorageBackend>,
    crypto_service: Arc<CryptoService>,
    contract_state_manager: Arc<ContractStateManager>, // Add contract state manager
    instances: parking_lot::RwLock<HashMap<String, WasmInstance>>,
    modules: parking_lot::RwLock<HashMap<[u8; 32], Module>>,
    engine: Engine,
    gas_limit: u64,
    memory_limit: usize,
}

/// WASM contract instance
#[derive(Clone)]
pub struct WasmInstance {
    pub contract_id: String,
    pub bytecode: Vec<u8>,
    pub module_hash: [u8; 32], // Hash of the module for identification
    pub gas_used: u64,
    pub state: WasmState,
    pub exports: Vec<String>,
}

/// Contract state for WASM execution
pub struct ContractState {
    pub storage: Arc<dyn StorageBackend>,
    pub crypto_service: Arc<CryptoService>,
    pub caller: String,
    pub contract_id: String, // Add contract ID to track which contract is executing
    pub gas_meter: GasMeter,
    pub logs: Vec<String>,
    pub events: Vec<WasmEvent>,
    pub state_changes: Vec<StateChange>,
    pub return_value: Option<WasmValue>,
}

impl ContractState {
    fn wasi_ctx(&mut self) -> &mut WasiCtx {
        // In a real implementation, you would initialize and return a WasiCtx
        // For now, we'll create a minimal one
        // This is a placeholder implementation
        unsafe {
            let wasi_ctx = std::mem::MaybeUninit::<WasiCtx>::uninit();
            std::mem::transmute(wasi_ctx)
        }
    }
}

/// WASM execution state
#[derive(Debug, Clone)]
pub enum WasmState {
    Initialized,
    Running,
    Suspended,
    Completed,
    Failed(String),
}

/// WASM execution context
#[derive(Debug, Clone)]
pub struct WasmExecutionContext {
    pub contract_id: String,
    pub function_name: String,
    pub arguments: Vec<WasmValue>,
    pub caller: String,
    pub gas_limit: u64,
    pub timestamp: chrono::DateTime<Utc>,
}

/// WASM value types
#[derive(Debug, Clone)]
pub enum WasmValue {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    String(String),
    Bytes(Vec<u8>),
    Bool(bool),
}

/// WASM execution result
#[derive(Debug, Clone)]
pub struct WasmExecutionResult {
    pub success: bool,
    pub return_value: Option<WasmValue>,
    pub gas_used: u64,
    pub memory_used: usize,
    pub logs: Vec<String>,
    pub events: Vec<WasmEvent>,
    pub state_changes: Vec<StateChange>,
    pub error: Option<String>,
}

/// WASM contract event
#[derive(Debug, Clone)]
pub struct WasmEvent {
    pub name: String,
    pub data: Value,
    pub timestamp: chrono::DateTime<Utc>,
}

/// State change record
#[derive(Debug, Clone)]
pub struct StateChange {
    pub key: String,
    pub old_value: Option<WasmValue>,
    pub new_value: WasmValue,
}

/// Host function interface for WASM contracts
pub trait WasmHostFunctions {
    /// Get storage value
    fn storage_get(&self, key: &str) -> GarpResult<Option<WasmValue>>;
    
    /// Set storage value
    fn storage_set(&self, key: &str, value: WasmValue) -> GarpResult<()>;
    
    /// Emit event
    fn emit_event(&self, name: &str, data: Value) -> GarpResult<()>;
    
    /// Log message
    fn log(&self, message: &str) -> GarpResult<()>;
    
    /// Get caller address
    fn get_caller(&self) -> String;
    
    /// Get current timestamp
    fn get_timestamp(&self) -> i64;
    
    /// Hash data
    fn hash(&self, data: &[u8]) -> GarpResult<Vec<u8>>;
    
    /// Verify signature
    fn verify_signature(&self, message: &[u8], signature: &[u8], public_key: &[u8]) -> GarpResult<bool>;
}

impl WasmRuntime {
    /// Create new WASM runtime
    pub fn new(
        storage: Arc<dyn StorageBackend>,
        crypto_service: Arc<CryptoService>,
        gas_limit: u64,
        memory_limit: usize,
    ) -> Self {
        // Create WASM engine with gas metering enabled
        let mut config = wasmtime::Config::new();
        config.consume_fuel(true);
        config.max_wasm_stack(1024 * 1024); // 1MB stack limit
        let engine = Engine::new(&config).expect("Failed to create WASM engine");
        
        // Create contract state manager
        let contract_state_manager = Arc::new(ContractStateManager::new(storage.clone()));
        
        Self {
            storage,
            crypto_service,
            contract_state_manager, // Add contract state manager
            instances: parking_lot::RwLock::new(HashMap::new()),
            modules: parking_lot::RwLock::new(HashMap::new()),
            engine,
            gas_limit,
            memory_limit,
        }
    }

    /// Hash module bytecode
    fn hash_module(&self, bytecode: &[u8]) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(bytecode);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    /// Load WASM contract from bytecode
    pub async fn load_contract(&self, contract_id: &str, bytecode: Vec<u8>) -> GarpResult<()> {
        info!("Loading WASM contract: {}", contract_id);

        // Validate WASM bytecode
        self.validate_bytecode(&bytecode)?;

        // Create WASM module
        let module = Module::from_binary(&self.engine, &bytecode)
            .map_err(|e| ContractError::ValidationFailed(format!("Invalid WASM module: {}", e)))?;

        // Calculate module hash for storage
        let module_hash = self.hash_module(&bytecode);

        // Store module
        {
            let mut modules = self.modules.write();
            modules.insert(module_hash, module);
        }

        // Extract exports (simplified)
        let exports = vec!["init".to_string(), "execute".to_string(), "query".to_string()];

        // Create instance
        let wasm_instance = WasmInstance {
            contract_id: contract_id.to_string(),
            bytecode: bytecode.clone(),
            module_hash,
            gas_used: 0,
            state: WasmState::Initialized,
            exports,
        };

        // Store instance
        let mut instances = self.instances.write();
        instances.insert(contract_id.to_string(), wasm_instance);

        info!("WASM contract loaded successfully: {}", contract_id);
        Ok(())
    }

    /// Execute WASM contract function
    pub async fn execute_function(
        &self,
        context: WasmExecutionContext,
    ) -> GarpResult<WasmExecutionResult> {
        info!("Executing WASM function: {}::{}", context.contract_id, context.function_name);

        // Get instance
        let instances = self.instances.read();
        let wasm_instance = instances.get(&context.contract_id)
            .ok_or_else(|| ContractError::ContractNotFound(
                garp_common::ContractId(Uuid::parse_str(&context.contract_id).unwrap_or_default())
            ))?;

        // Check if function exists
        if !wasm_instance.exports.contains(&context.function_name) {
            return Err(ContractError::ChoiceNotFound(context.function_name.clone()).into());
        }

        // Get module
        let modules = self.modules.read();
        let module = modules.get(&wasm_instance.module_hash)
            .ok_or_else(|| ContractError::ExecutionFailed("Module not found".to_string()))?;

        // Create store with contract state
        let contract_state = ContractState {
            storage: self.storage.clone(),
            crypto_service: self.crypto_service.clone(),
            caller: context.caller.clone(),
            contract_id: context.contract_id.clone(), // Add contract_id to the state
            gas_meter: GasMeter::new(context.gas_limit),
            logs: Vec::new(),
            events: Vec::new(),
            state_changes: Vec::new(),
            return_value: None,
        };
        
        let mut store = Store::new(&self.engine, contract_state);
        store.set_fuel(context.gas_limit)
            .map_err(|e| ContractError::ExecutionFailed(format!("Failed to set gas limit: {}", e)))?;

        // Create linker for host functions
        let mut linker = Linker::new(&self.engine);
        self.register_host_functions(&mut linker)?;

        // Instantiate module
        let instance = linker.instantiate_async(&mut store, module).await
            .map_err(|e| ContractError::ExecutionFailed(format!("Failed to instantiate module: {}", e)))?;

        // Execute function
        let result = self.execute_wasm_function(&instance, &mut store, &context).await?;

        // Get consumed fuel
        let gas_used = context.gas_limit - store.get_fuel()
            .map_err(|e| ContractError::ExecutionFailed(format!("Failed to get fuel: {}", e)))?;

        info!("WASM execution completed: {} gas used", gas_used);
        Ok(result)
    }

    /// Get contract instance info
    pub fn get_instance_info(&self, contract_id: &str) -> GarpResult<WasmInstance> {
        let instances = self.instances.read();
        instances.get(contract_id)
            .cloned()
            .ok_or_else(|| ContractError::ContractNotFound(
                garp_common::ContractId(Uuid::parse_str(contract_id).unwrap_or_default())
            ).into())
    }

    /// Remove contract instance
    pub fn unload_contract(&self, contract_id: &str) -> GarpResult<()> {
        let mut instances = self.instances.write();
        instances.remove(contract_id)
            .ok_or_else(|| ContractError::ContractNotFound(
                garp_common::ContractId(Uuid::parse_str(contract_id).unwrap_or_default())
            ).into())?;
        
        info!("WASM contract unloaded: {}", contract_id);
        Ok(())
    }

    /// Validate WASM bytecode
    pub fn validate_bytecode(&self, bytecode: &[u8]) -> GarpResult<()> {
        // Check magic number (0x00 0x61 0x73 0x6D)
        if bytecode.len() < 8 {
            return Err(ContractError::ValidationFailed("Invalid WASM bytecode: too short".to_string()).into());
        }

        let magic = &bytecode[0..4];
        if magic != [0x00, 0x61, 0x73, 0x6D] {
            return Err(ContractError::ValidationFailed("Invalid WASM magic number".to_string()).into());
        }

        // Check version (0x01 0x00 0x00 0x00)
        let version = &bytecode[4..8];
        if version != [0x01, 0x00, 0x00, 0x00] {
            return Err(ContractError::ValidationFailed("Unsupported WASM version".to_string()).into());
        }

        // Additional validation would be implemented here
        // - Check for forbidden instructions
        // - Validate memory limits
        // - Check import/export sections

        Ok(())
    }

    /// Extract exported functions from WASM bytecode
    fn extract_exports(&self, bytecode: &[u8]) -> GarpResult<Vec<String>> {
        // This is a simplified implementation
        // In a real implementation, you would parse the WASM binary format
        // to extract the export section and function names
        
        let mut exports = Vec::new();
        
        // Add common contract functions
        exports.push("init".to_string());
        exports.push("execute".to_string());
        exports.push("query".to_string());
        
        // Parse actual exports from bytecode would be implemented here
        // This would involve parsing the WASM binary format sections
        
        Ok(exports)
    }

    /// Execute WASM function
    async fn execute_wasm_function(
        &self,
        instance: &Instance,
        store: &mut Store<ContractState>,
        context: &WasmExecutionContext,
    ) -> GarpResult<WasmExecutionResult> {
        // Call the function
        let func = instance.get_typed_func::<(), ()>(store, &context.function_name)
            .map_err(|e| ContractError::ExecutionFailed(format!("Function not found: {}", e)))?;

        // Execute the function
        let result = func.call_async(store, ()).await;

        // Get contract state
        let contract_state = store.data().clone();

        // Create execution result
        let execution_result = WasmExecutionResult {
            success: result.is_ok(),
            return_value: contract_state.return_value.clone(),
            gas_used: 0, // Will be updated by caller
            memory_used: 0, // Would need to calculate actual memory usage
            logs: contract_state.logs.clone(),
            events: contract_state.events.clone(),
            state_changes: contract_state.state_changes.clone(),
            error: result.err().map(|e| e.to_string()),
        };

        Ok(execution_result)
    }

    /// Convert WasmValue to WASM Val
    fn wasm_value_to_val(&self, value: &WasmValue) -> GarpResult<Val> {
        match value {
            WasmValue::I32(v) => Ok(Val::I32(*v)),
            WasmValue::I64(v) => Ok(Val::I64(*v)),
            WasmValue::F32(v) => Ok(Val::F32(*v)),
            WasmValue::F64(v) => Ok(Val::F64(*v)),
            _ => Err(ContractError::ExecutionFailed("Unsupported value type".to_string()).into()),
        }
    }

    /// Register host functions
    fn register_host_functions(&self, linker: &mut Linker<ContractState>) -> GarpResult<()> {
        // Clone necessary values for closures
        let state_manager = self.contract_state_manager.clone();
        
        // Register storage_get function
        linker.func_wrap("env", "storage_get", move |mut caller: Caller<ContractState>, key_ptr: i32, key_len: i32| {
            // Get the contract state
            let state = caller.data();
            
            // Parse the contract ID
            let contract_id = match Uuid::parse_str(&state.contract_id) {
                Ok(uuid) => ContractId(uuid),
                Err(_) => {
                    error!("Invalid contract ID: {}", state.contract_id);
                    return Ok(0i32); // Return null pointer on error
                }
            };
            
            // In a real implementation, you would read the key from WASM memory
            // For now, we'll simulate reading a key
            let key = "test_key"; // This would be read from WASM memory
            
            // Get state from contract state manager (async operation in a sync context)
            // In a real implementation, we would need to handle this properly
            info!("WASM contract {} getting state for key {}", state.contract_id, key);
            Ok(0i32) // Return pointer to result
        })?;

        // Register storage_set function
        let set_state_manager = self.contract_state_manager.clone();
        linker.func_wrap("env", "storage_set", move |mut caller: Caller<ContractState>, key_ptr: i32, key_len: i32, value_ptr: i32, value_len: i32| {
            // Get the contract state
            let state = caller.data();
            
            // Parse the contract ID
            let contract_id = match Uuid::parse_str(&state.contract_id) {
                Ok(uuid) => ContractId(uuid),
                Err(_) => {
                    error!("Invalid contract ID: {}", state.contract_id);
                    return Ok(()); // Return on error
                }
            };
            
            // In a real implementation, you would read the key and value from WASM memory
            // For now, we'll simulate setting a key-value pair
            let key = "test_key".to_string(); // This would be read from WASM memory
            let value = b"test_value".to_vec(); // This would be read from WASM memory
            
            // Set state in contract state manager (async operation in a sync context)
            // In a real implementation, we would need to handle this properly
            info!("WASM contract {} setting state for key {} with value length {}", state.contract_id, key, value.len());
            Ok(())
        })?;

        // Register log function
        linker.func_wrap("env", "log", move |mut caller: Caller<ContractState>, msg_ptr: i32, msg_len: i32| {
            // In a real implementation, you would read the message from WASM memory
            // and add it to the logs
            Ok(())
        })?;

        // Register emit_event function
        linker.func_wrap("env", "emit_event", move |mut caller: Caller<ContractState>, name_ptr: i32, name_len: i32, data_ptr: i32, data_len: i32| {
            // Get the contract state
            let mut state = caller.data_mut();
            
            // In a real implementation, you would read the name and data from WASM memory
            // For now, we'll create a mock event
            let event = WasmEvent {
                name: "WasmEvent".to_string(), // This would be read from WASM memory
                data: serde_json::json!({"message": "Event emitted from WASM contract"}),
                timestamp: Utc::now(),
            };
            
            // Add the event to the contract state
            state.events.push(event);
            
            // Also store the event in the database
            info!("WASM contract {} emitted an event", state.contract_id);
            Ok(())
        })?;

        // Register get_caller function
        linker.func_wrap("env", "get_caller", move |mut caller: Caller<ContractState>, ptr: i32| {
            // In a real implementation, you would write the caller address to WASM memory
            // and return the length
            Ok(0i32)
        })?;

        // Register get_timestamp function
        linker.func_wrap("env", "get_timestamp", move |mut caller: Caller<ContractState>| {
            Ok(Utc::now().timestamp())
        })?;

        // Register hash function
        let crypto_service = self.crypto_service.clone();
        linker.func_wrap("env", "hash", move |mut caller: Caller<ContractState>, data_ptr: i32, data_len: i32| {
            // In a real implementation, you would read the data from WASM memory,
            // hash it, and return a pointer to the result
            Ok(0i32) // Return pointer to result
        })?;

        // Register verify_signature function
        let verify_crypto_service = self.crypto_service.clone();
        linker.func_wrap("env", "verify_signature", move |mut caller: Caller<ContractState>, msg_ptr: i32, msg_len: i32, sig_ptr: i32, sig_len: i32, pk_ptr: i32, pk_len: i32| {
            // In a real implementation, you would read the message, signature, and public key
            // from WASM memory and verify the signature
            Ok(0i32) // Return 1 for valid, 0 for invalid
        })?;

        Ok(())
    }

    /// Extract exports from WASM instance
    fn extract_exports_from_instance(&self, instance: &Instance, store: &mut impl AsContext) -> GarpResult<Vec<String>> {
        let mut exports = Vec::new();
        for export in instance.exports(store) {
            if let Extern::Func(_) = export.get() {
                exports.push(export.name().to_string());
            }
        }
        Ok(exports)
    }
}

/// Host functions implementation
pub struct DefaultHostFunctions {
    storage: Arc<dyn StorageBackend>,
    crypto_service: Arc<CryptoService>,
    caller: String,
    contract_id: String, // Add contract ID to track which contract is emitting events
    logs: parking_lot::RwLock<Vec<String>>,
    events: parking_lot::RwLock<Vec<WasmEvent>>,
}

impl DefaultHostFunctions {
    pub fn new(
        storage: Arc<dyn StorageBackend>,
        crypto_service: Arc<CryptoService>,
        caller: String,
        contract_id: String, // Add contract_id parameter
    ) -> Self {
        Self {
            storage,
            crypto_service,
            caller,
            contract_id,
            logs: parking_lot::RwLock::new(Vec::new()),
            events: parking_lot::RwLock::new(Vec::new()),
        }
    }

    pub fn get_logs(&self) -> Vec<String> {
        self.logs.read().clone()
    }

    pub fn get_events(&self) -> Vec<WasmEvent> {
        self.events.read().clone()
    }
}

impl WasmHostFunctions for DefaultHostFunctions {
    fn storage_get(&self, key: &str) -> GarpResult<Option<WasmValue>> {
        // In a real implementation, this would interact with the storage backend
        debug!("WASM storage_get: {}", key);
        
        // Simulate storage access
        match key {
            "balance" => Ok(Some(WasmValue::I64(1000))),
            "owner" => Ok(Some(WasmValue::String("owner_address".to_string()))),
            _ => Ok(None),
        }
    }

    fn storage_set(&self, key: &str, value: WasmValue) -> GarpResult<()> {
        debug!("WASM storage_set: {} = {:?}", key, value);
        
        // In a real implementation, this would store the value
        // using the storage backend
        
        Ok(())
    }

    fn emit_event(&self, name: &str, data: Value) -> GarpResult<()> {
        let event = WasmEvent {
            name: name.to_string(),
            data,
            timestamp: Utc::now(),
        };
        
        // Store the event in the database
        let contract_event = crate::contract_engine::ContractEvent {
            id: uuid::Uuid::new_v4().to_string(),
            contract_id: garp_common::ContractId(uuid::Uuid::parse_str(&self.contract_id).unwrap_or_default()),
            event_type: name.to_string(),
            data: event.data.clone(),
            timestamp: event.timestamp,
            emitter: garp_common::ParticipantId(self.caller.clone()),
        };
        
        // Store the event in the database
        let storage = self.storage.clone();
        // In a real implementation, we would use a blocking call or spawn a task
        // For now, we'll just store it in memory and log that it should be stored
        self.events.write().push(event);
        
        // Log that the event should be stored in the database
        info!("WASM contract {} emitted event {} that should be stored in database", self.contract_id, name);
        
        Ok(())
    }

    fn log(&self, message: &str) -> GarpResult<()> {
        self.logs.write().push(message.to_string());
        debug!("WASM log: {}", message);
        Ok(())
    }

    fn get_caller(&self) -> String {
        self.caller.clone()
    }

    fn get_timestamp(&self) -> i64 {
        Utc::now().timestamp()
    }

    fn hash(&self, data: &[u8]) -> GarpResult<Vec<u8>> {
        self.crypto_service.hash(data)
            .map_err(|e| GarpError::Crypto(e))
    }

    fn verify_signature(&self, message: &[u8], signature: &[u8], public_key: &[u8]) -> GarpResult<bool> {
        self.crypto_service.verify_signature(message, signature, public_key)
            .map_err(|e| GarpError::Crypto(e))
    }
}

impl From<WasmValue> for Value {
    fn from(wasm_value: WasmValue) -> Self {
        match wasm_value {
            WasmValue::I32(v) => Value::Number(v.into()),
            WasmValue::I64(v) => Value::Number(v.into()),
            WasmValue::F32(v) => Value::Number(serde_json::Number::from_f64(v as f64).unwrap_or_default()),
            WasmValue::F64(v) => Value::Number(serde_json::Number::from_f64(v).unwrap_or_default()),
            WasmValue::String(v) => Value::String(v),
            WasmValue::Bytes(v) => Value::String(base64::encode(v)),
            WasmValue::Bool(v) => Value::Bool(v),
        }
    }
}

impl TryFrom<Value> for WasmValue {
    type Error = GarpError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(WasmValue::I64(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(WasmValue::F64(f))
                } else {
                    Err(GarpError::Internal("Invalid number format".to_string()))
                }
            }
            Value::String(s) => Ok(WasmValue::String(s)),
            Value::Bool(b) => Ok(WasmValue::Bool(b)),
            _ => Err(GarpError::Internal("Unsupported value type for WASM".to_string())),
        }
    }
}
// -----------------------------------------------------------------------------
// Wasm runtime gas metering and deterministic imports (scaffolding)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GasSchedule {
    pub instr_cost: u64,     // cost per wasm instruction
    pub mem_read_cost: u64,  // per byte
    pub mem_write_cost: u64, // per byte
    pub syscall_base_cost: u64,
}

#[derive(Debug, Default, Clone)]
pub struct GasMeter {
    pub used: u64,
    pub limit: u64,
}

impl GasMeter {
    pub fn new(limit: u64) -> Self { Self { used: 0, limit } }
    pub fn charge(&mut self, amount: u64) -> Result<(), String> {
        self.used = self.used.saturating_add(amount);
        if self.used > self.limit { return Err("out of gas".into()); }
        Ok(())
    }
}

/// Placeholder to constrain imports; real implementation should bind a minimal, deterministic API.
pub fn allowed_imports() -> &'static [&'static str] {
    &["env.print", "env.read_state", "env.write_state"]
}
