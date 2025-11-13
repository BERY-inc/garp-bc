//! Contract debugging framework for GARP smart contracts
//! 
//! This framework provides utilities for debugging smart contracts including:
//! - Execution tracing and logging
//! - State inspection utilities
//! - Debug information tracking
//! - WASM contract debugging tools

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, debug, trace, warn, error};
use uuid::Uuid;

// Re-export necessary types from garp_common
use garp_common::{ContractId, ParticipantId, GarpResult, GarpError, ContractError};

// Import necessary modules
use crate::contract_engine::{ContractEngine, ExecutionContext, ExecutionResult};
use crate::storage::{StorageBackend, MemoryStorage};
use crate::crypto::CryptoService;
use crate::wasm_runtime::{WasmRuntime, WasmExecutionResult, WasmValue};
use crate::contract_state::ContractStateManager;

/// Debug level for contract execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DebugLevel {
    /// No debugging information
    None,
    
    /// Basic execution information
    Info,
    
    /// Detailed execution tracing
    Trace,
    
    /// Full debugging with all information
    Verbose,
}

/// Contract execution debug information
#[derive(Debug, Clone)]
pub struct ExecutionDebugInfo {
    /// Execution start time
    pub start_time: Instant,
    
    /// Execution duration in microseconds
    pub duration_us: u128,
    
    /// Contract ID
    pub contract_id: ContractId,
    
    /// Choice name being executed
    pub choice_name: String,
    
    /// Executor participant ID
    pub executor: ParticipantId,
    
    /// Debug level
    pub debug_level: DebugLevel,
    
    /// Execution trace entries
    pub trace_entries: Vec<TraceEntry>,
    
    /// State changes during execution
    pub state_changes: Vec<StateChangeInfo>,
    
    /// Events emitted during execution
    pub emitted_events: Vec<EventInfo>,
    
    /// Gas usage information
    pub gas_info: GasInfo,
    
    /// Memory usage information
    pub memory_info: MemoryInfo,
    
    /// Warnings encountered during execution
    pub warnings: Vec<String>,
    
    /// Errors encountered during execution
    pub errors: Vec<String>,
}

/// Trace entry for contract execution
#[derive(Debug, Clone)]
pub struct TraceEntry {
    /// Timestamp of the trace entry
    pub timestamp: Instant,
    
    /// Trace level
    pub level: DebugLevel,
    
    /// Trace message
    pub message: String,
    
    /// Additional context data
    pub context: HashMap<String, Value>,
}

/// State change information
#[derive(Debug, Clone)]
pub struct StateChangeInfo {
    /// State key that changed
    pub key: String,
    
    /// Old value (if any)
    pub old_value: Option<WasmValue>,
    
    /// New value
    pub new_value: WasmValue,
    
    /// Timestamp of the change
    pub timestamp: Instant,
}

/// Event information
#[derive(Debug, Clone)]
pub struct EventInfo {
    /// Event name
    pub name: String,
    
    /// Event data
    pub data: Value,
    
    /// Timestamp of the event
    pub timestamp: Instant,
}

/// Gas usage information
#[derive(Debug, Clone)]
pub struct GasInfo {
    /// Initial gas limit
    pub initial_gas: u64,
    
    /// Gas used
    pub gas_used: u64,
    
    /// Remaining gas
    pub remaining_gas: u64,
    
    /// Gas usage by function
    pub function_gas_usage: HashMap<String, u64>,
}

/// Memory usage information
#[derive(Debug, Clone)]
pub struct MemoryInfo {
    /// Initial memory limit
    pub initial_memory: usize,
    
    /// Peak memory usage
    pub peak_memory: usize,
    
    /// Current memory usage
    pub current_memory: usize,
    
    /// Memory usage by function
    pub function_memory_usage: HashMap<String, usize>,
}

/// Contract debugger
pub struct ContractDebugger {
    /// Debug level
    debug_level: DebugLevel,
    
    /// Execution debug information storage
    debug_info: Arc<RwLock<HashMap<String, ExecutionDebugInfo>>>,
    
    /// Contract engine reference
    contract_engine: Arc<ContractEngine>,
}

impl ContractDebugger {
    /// Create a new contract debugger
    pub fn new(debug_level: DebugLevel, contract_engine: Arc<ContractEngine>) -> Self {
        Self {
            debug_level,
            debug_info: Arc::new(RwLock::new(HashMap::new())),
            contract_engine,
        }
    }
    
    /// Start debugging a contract execution
    pub async fn start_execution_debug(
        &self,
        contract_id: &ContractId,
        choice_name: &str,
        executor: &ParticipantId,
    ) -> String {
        let execution_id = Uuid::new_v4().to_string();
        let start_time = Instant::now();
        
        let debug_info = ExecutionDebugInfo {
            start_time,
            duration_us: 0,
            contract_id: contract_id.clone(),
            choice_name: choice_name.to_string(),
            executor: executor.clone(),
            debug_level: self.debug_level,
            trace_entries: Vec::new(),
            state_changes: Vec::new(),
            emitted_events: Vec::new(),
            gas_info: GasInfo {
                initial_gas: 0,
                gas_used: 0,
                remaining_gas: 0,
                function_gas_usage: HashMap::new(),
            },
            memory_info: MemoryInfo {
                initial_memory: 0,
                peak_memory: 0,
                current_memory: 0,
                function_memory_usage: HashMap::new(),
            },
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        
        {
            let mut debug_storage = self.debug_info.write().await;
            debug_storage.insert(execution_id.clone(), debug_info);
        }
        
        if self.debug_level >= DebugLevel::Info {
            info!("Started debugging execution {} for contract {} choice {} by {}", 
                  execution_id, contract_id.0, choice_name, executor.0);
        }
        
        execution_id
    }
    
    /// Add a trace entry to the execution debug information
    pub async fn add_trace_entry(
        &self,
        execution_id: &str,
        level: DebugLevel,
        message: String,
        context: HashMap<String, Value>,
    ) {
        if level > self.debug_level {
            return;
        }
        
        let trace_entry = TraceEntry {
            timestamp: Instant::now(),
            level,
            message,
            context,
        };
        
        {
            let mut debug_storage = self.debug_info.write().await;
            if let Some(debug_info) = debug_storage.get_mut(execution_id) {
                debug_info.trace_entries.push(trace_entry);
            }
        }
        
        // Log the trace entry based on its level
        match level {
            DebugLevel::Info => info!("Execution {}: {}", execution_id, trace_entry.message),
            DebugLevel::Trace => trace!("Execution {}: {}", execution_id, trace_entry.message),
            DebugLevel::Verbose => debug!("Execution {}: {}", execution_id, trace_entry.message),
            DebugLevel::None => {} // No logging for None level
        }
    }
    
    /// Add a state change to the execution debug information
    pub async fn add_state_change(
        &self,
        execution_id: &str,
        key: String,
        old_value: Option<WasmValue>,
        new_value: WasmValue,
    ) {
        let state_change = StateChangeInfo {
            key,
            old_value,
            new_value,
            timestamp: Instant::now(),
        };
        
        {
            let mut debug_storage = self.debug_info.write().await;
            if let Some(debug_info) = debug_storage.get_mut(execution_id) {
                debug_info.state_changes.push(state_change);
            }
        }
        
        if self.debug_level >= DebugLevel::Trace {
            debug!("Execution {}: State changed for key {:?}", execution_id, state_change.key);
        }
    }
    
    /// Add an emitted event to the execution debug information
    pub async fn add_emitted_event(
        &self,
        execution_id: &str,
        name: String,
        data: Value,
    ) {
        let event_info = EventInfo {
            name,
            data,
            timestamp: Instant::now(),
        };
        
        {
            let mut debug_storage = self.debug_info.write().await;
            if let Some(debug_info) = debug_storage.get_mut(execution_id) {
                debug_info.emitted_events.push(event_info);
            }
        }
        
        if self.debug_level >= DebugLevel::Info {
            info!("Execution {}: Event emitted", execution_id);
        }
    }
    
    /// Update gas information
    pub async fn update_gas_info(
        &self,
        execution_id: &str,
        initial_gas: u64,
        gas_used: u64,
        remaining_gas: u64,
        function_gas_usage: HashMap<String, u64>,
    ) {
        {
            let mut debug_storage = self.debug_info.write().await;
            if let Some(debug_info) = debug_storage.get_mut(execution_id) {
                debug_info.gas_info = GasInfo {
                    initial_gas,
                    gas_used,
                    remaining_gas,
                    function_gas_usage,
                };
            }
        }
        
        if self.debug_level >= DebugLevel::Trace {
            debug!("Execution {}: Gas info updated - used: {}, remaining: {}", 
                   execution_id, gas_used, remaining_gas);
        }
    }
    
    /// Update memory information
    pub async fn update_memory_info(
        &self,
        execution_id: &str,
        initial_memory: usize,
        peak_memory: usize,
        current_memory: usize,
        function_memory_usage: HashMap<String, usize>,
    ) {
        {
            let mut debug_storage = self.debug_info.write().await;
            if let Some(debug_info) = debug_storage.get_mut(execution_id) {
                debug_info.memory_info = MemoryInfo {
                    initial_memory,
                    peak_memory,
                    current_memory,
                    function_memory_usage,
                };
            }
        }
        
        if self.debug_level >= DebugLevel::Trace {
            debug!("Execution {}: Memory info updated - peak: {}, current: {}", 
                   execution_id, peak_memory, current_memory);
        }
    }
    
    /// Add a warning to the execution debug information
    pub async fn add_warning(&self, execution_id: &str, warning: String) {
        {
            let mut debug_storage = self.debug_info.write().await;
            if let Some(debug_info) = debug_storage.get_mut(execution_id) {
                debug_info.warnings.push(warning.clone());
            }
        }
        
        warn!("Execution {}: {}", execution_id, warning);
    }
    
    /// Add an error to the execution debug information
    pub async fn add_error(&self, execution_id: &str, error: String) {
        {
            let mut debug_storage = self.debug_info.write().await;
            if let Some(debug_info) = debug_storage.get_mut(execution_id) {
                debug_info.errors.push(error.clone());
            }
        }
        
        error!("Execution {}: {}", execution_id, error);
    }
    
    /// End debugging a contract execution and calculate duration
    pub async fn end_execution_debug(&self, execution_id: &str) {
        let end_time = Instant::now();
        
        {
            let mut debug_storage = self.debug_info.write().await;
            if let Some(debug_info) = debug_storage.get_mut(execution_id) {
                debug_info.duration_us = debug_info.start_time.elapsed().as_micros();
            }
        }
        
        if self.debug_level >= DebugLevel::Info {
            info!("Ended debugging execution {}", execution_id);
        }
    }
    
    /// Get execution debug information
    pub async fn get_execution_debug_info(&self, execution_id: &str) -> Option<ExecutionDebugInfo> {
        let debug_storage = self.debug_info.read().await;
        debug_storage.get(execution_id).cloned()
    }
    
    /// Get all execution debug information
    pub async fn get_all_debug_info(&self) -> HashMap<String, ExecutionDebugInfo> {
        let debug_storage = self.debug_info.read().await;
        debug_storage.clone()
    }
    
    /// Clear debug information for a specific execution
    pub async fn clear_execution_debug_info(&self, execution_id: &str) {
        let mut debug_storage = self.debug_info.write().await;
        debug_storage.remove(execution_id);
    }
    
    /// Clear all debug information
    pub async fn clear_all_debug_info(&self) {
        let mut debug_storage = self.debug_info.write().await;
        debug_storage.clear();
    }
}

/// Debug utilities for contract inspection
pub struct DebugUtils;

impl DebugUtils {
    /// Print execution debug information in a human-readable format
    pub async fn print_execution_debug_info(debug_info: &ExecutionDebugInfo) {
        println!("=== Contract Execution Debug Information ===");
        println!("Contract ID: {}", debug_info.contract_id.0);
        println!("Choice Name: {}", debug_info.choice_name);
        println!("Executor: {}", debug_info.executor.0);
        println!("Duration: {}μs", debug_info.duration_us);
        println!("Debug Level: {:?}", debug_info.debug_level);
        
        if !debug_info.trace_entries.is_empty() {
            println!("\n--- Trace Entries ---");
            for (i, trace) in debug_info.trace_entries.iter().enumerate() {
                println!("{}. [{}] {} - {:?}", 
                         i + 1, 
                         format_trace_level(trace.level), 
                         trace.message, 
                         trace.timestamp.duration_since(debug_info.start_time).as_micros());
                
                if !trace.context.is_empty() {
                    println!("   Context: {:?}", trace.context);
                }
            }
        }
        
        if !debug_info.state_changes.is_empty() {
            println!("\n--- State Changes ---");
            for (i, change) in debug_info.state_changes.iter().enumerate() {
                println!("{}. Key: {} - {:?}", 
                         i + 1, 
                         change.key, 
                         change.timestamp.duration_since(debug_info.start_time).as_micros());
                println!("   Old Value: {:?}", change.old_value);
                println!("   New Value: {:?}", change.new_value);
            }
        }
        
        if !debug_info.emitted_events.is_empty() {
            println!("\n--- Emitted Events ---");
            for (i, event) in debug_info.emitted_events.iter().enumerate() {
                println!("{}. Name: {} - {:?}", 
                         i + 1, 
                         event.name, 
                         event.timestamp.duration_since(debug_info.start_time).as_micros());
                println!("   Data: {}", serde_json::to_string_pretty(&event.data).unwrap_or_default());
            }
        }
        
        println!("\n--- Resource Usage ---");
        println!("Gas - Initial: {}, Used: {}, Remaining: {}", 
                 debug_info.gas_info.initial_gas, 
                 debug_info.gas_info.gas_used, 
                 debug_info.gas_info.remaining_gas);
        
        println!("Memory - Initial: {}, Peak: {}, Current: {}", 
                 debug_info.memory_info.initial_memory, 
                 debug_info.memory_info.peak_memory, 
                 debug_info.memory_info.current_memory);
        
        if !debug_info.warnings.is_empty() {
            println!("\n--- Warnings ---");
            for (i, warning) in debug_info.warnings.iter().enumerate() {
                println!("{}. {}", i + 1, warning);
            }
        }
        
        if !debug_info.errors.is_empty() {
            println!("\n--- Errors ---");
            for (i, error) in debug_info.errors.iter().enumerate() {
                println!("{}. {}", i + 1, error);
            }
        }
    }
    
    /// Export execution debug information to JSON
    pub async fn export_execution_debug_info(debug_info: &ExecutionDebugInfo) -> GarpResult<String> {
        let json = serde_json::to_string_pretty(debug_info)
            .map_err(|e| GarpError::Internal(format!("Failed to serialize debug info: {}", e)))?;
        Ok(json)
    }
}

/// Format trace level for display
fn format_trace_level(level: DebugLevel) -> &'static str {
    match level {
        DebugLevel::None => "NONE",
        DebugLevel::Info => "INFO",
        DebugLevel::Trace => "TRACE",
        DebugLevel::Verbose => "VERBOSE",
    }
}

/// Debug context for contract execution
pub struct DebugContext {
    /// Execution ID
    pub execution_id: String,
    
    /// Contract debugger
    pub debugger: Arc<ContractDebugger>,
}

impl DebugContext {
    /// Create a new debug context
    pub fn new(execution_id: String, debugger: Arc<ContractDebugger>) -> Self {
        Self {
            execution_id,
            debugger,
        }
    }
    
    /// Add a trace entry
    pub async fn trace(&self, level: DebugLevel, message: String, context: HashMap<String, Value>) {
        self.debugger.add_trace_entry(&self.execution_id, level, message, context).await;
    }
    
    /// Add a state change
    pub async fn state_change(&self, key: String, old_value: Option<WasmValue>, new_value: WasmValue) {
        self.debugger.add_state_change(&self.execution_id, key, old_value, new_value).await;
    }
    
    /// Add an emitted event
    pub async fn emit_event(&self, name: String, data: Value) {
        self.debugger.add_emitted_event(&self.execution_id, name, data).await;
    }
    
    /// Add a warning
    pub async fn warn(&self, warning: String) {
        self.debugger.add_warning(&self.execution_id, warning).await;
    }
    
    /// Add an error
    pub async fn error(&self, error: String) {
        self.debugger.add_error(&self.execution_id, error).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_contract_debugger_creation() {
        let storage = Arc::new(MemoryStorage::new());
        let crypto_service = Arc::new(CryptoService::new());
        let contract_engine = Arc::new(ContractEngine::new(storage, crypto_service));
        let debugger = ContractDebugger::new(DebugLevel::Info, contract_engine);
        
        assert_eq!(debugger.debug_level, DebugLevel::Info);
    }

    #[tokio::test]
    async fn test_execution_debug_lifecycle() {
        let storage = Arc::new(MemoryStorage::new());
        let crypto_service = Arc::new(CryptoService::new());
        let contract_engine = Arc::new(ContractEngine::new(storage, crypto_service));
        let debugger = ContractDebugger::new(DebugLevel::Info, contract_engine);
        
        let contract_id = ContractId(Uuid::new_v4());
        let executor = ParticipantId("test_executor".to_string());
        
        // Start debugging
        let execution_id = debugger.start_execution_debug(&contract_id, "test_choice", &executor).await;
        assert!(!execution_id.is_empty());
        
        // Add some debug information
        let mut context = HashMap::new();
        context.insert("test_key".to_string(), Value::String("test_value".to_string()));
        debugger.add_trace_entry(&execution_id, DebugLevel::Info, "Test trace".to_string(), context).await;
        
        debugger.add_warning(&execution_id, "Test warning".to_string()).await;
        debugger.add_error(&execution_id, "Test error".to_string()).await;
        
        // End debugging
        debugger.end_execution_debug(&execution_id).await;
        
        // Get debug information
        let debug_info = debugger.get_execution_debug_info(&execution_id).await;
        assert!(debug_info.is_some());
        
        let debug_info = debug_info.unwrap();
        assert_eq!(debug_info.contract_id, contract_id);
        assert_eq!(debug_info.choice_name, "test_choice");
        assert_eq!(debug_info.executor, executor);
        assert_eq!(debug_info.trace_entries.len(), 1);
        assert_eq!(debug_info.warnings.len(), 1);
        assert_eq!(debug_info.errors.len(), 1);
        assert!(debug_info.duration_us > 0);
    }

    #[tokio::test]
    async fn test_debug_utils() {
        let debug_info = ExecutionDebugInfo {
            start_time: Instant::now(),
            duration_us: 1000,
            contract_id: ContractId(Uuid::new_v4()),
            choice_name: "test_choice".to_string(),
            executor: ParticipantId("test_executor".to_string()),
            debug_level: DebugLevel::Info,
            trace_entries: vec![TraceEntry {
                timestamp: Instant::now(),
                level: DebugLevel::Info,
                message: "Test trace".to_string(),
                context: HashMap::new(),
            }],
            state_changes: vec![StateChangeInfo {
                key: "test_key".to_string(),
                old_value: None,
                new_value: WasmValue::I32(42),
                timestamp: Instant::now(),
            }],
            emitted_events: vec![EventInfo {
                name: "TestEvent".to_string(),
                data: Value::String("test_data".to_string()),
                timestamp: Instant::now(),
            }],
            gas_info: GasInfo {
                initial_gas: 1000000,
                gas_used: 50000,
                remaining_gas: 950000,
                function_gas_usage: HashMap::new(),
            },
            memory_info: MemoryInfo {
                initial_memory: 1024,
                peak_memory: 2048,
                current_memory: 1536,
                function_memory_usage: HashMap::new(),
            },
            warnings: vec!["Test warning".to_string()],
            errors: vec!["Test error".to_string()],
        };
        
        // Test JSON export
        let json_result = DebugUtils::export_execution_debug_info(&debug_info).await;
        assert!(json_result.is_ok());
        assert!(!json_result.unwrap().is_empty());
    }
}