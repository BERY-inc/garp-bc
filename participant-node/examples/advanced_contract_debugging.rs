//! Advanced example of contract debugging integrated with the contract engine

use garp_participant_node::{
    contract_debug::{
        ContractDebugger, DebugLevel, DebugContext, DebugUtils
    },
    storage::MemoryStorage,
    crypto::CryptoService,
    contract_engine::ContractEngine,
    wasm_runtime::WasmValue,
};
use std::sync::Arc;
use std::collections::HashMap;
use serde_json::Value;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create storage and crypto service
    let storage = Arc::new(MemoryStorage::new());
    let crypto_service = Arc::new(CryptoService::new());
    
    // Create contract engine
    let contract_engine = Arc::new(ContractEngine::new(storage.clone(), crypto_service.clone()));
    
    // Create contract debugger with trace level debugging
    let debugger = Arc::new(ContractDebugger::new(DebugLevel::Trace, contract_engine.clone()));
    
    println!("=== Advanced Contract Debugging Example ===");
    
    // Simulate a more complex contract execution scenario
    simulate_complex_contract_execution(contract_engine, debugger).await?;
    
    // Show how to retrieve and analyze debug information
    analyze_debug_information(debugger).await;
    
    println!("\nAdvanced debugging example completed successfully!");
    Ok(())
}

/// Simulate a complex contract execution with multiple steps
async fn simulate_complex_contract_execution(
    contract_engine: Arc<ContractEngine>,
    debugger: Arc<ContractDebugger>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n--- Simulating Complex Contract Execution ---");
    
    // Create a mock contract ID and executor
    let contract_id = garp_common::ContractId(uuid::Uuid::new_v4());
    let executor = garp_common::ParticipantId("advanced_user".to_string());
    
    // Start debugging the execution
    let execution_id = debugger.start_execution_debug(&contract_id, "complex_operation", &executor).await;
    println!("Started debugging execution: {}", execution_id);
    
    // Create debug context
    let debug_context = DebugContext::new(execution_id.clone(), debugger.clone());
    
    // Step 1: Validate inputs
    {
        let mut context = HashMap::new();
        context.insert("step".to_string(), Value::String("input_validation".to_string()));
        context.insert("inputs".to_string(), serde_json::json!({
            "param1": "value1",
            "param2": 42,
            "param3": true
        }));
        debug_context.trace(DebugLevel::Trace, "Validating input parameters".to_string(), context).await;
        
        // Simulate validation success
        debug_context.trace(DebugLevel::Info, "Input validation successful".to_string(), HashMap::new()).await;
    }
    
    // Step 2: Load contract state
    {
        let mut context = HashMap::new();
        context.insert("step".to_string(), Value::String("state_loading".to_string()));
        context.insert("state_keys".to_string(), Value::Array(vec![
            Value::String("balance".to_string()),
            Value::String("permissions".to_string()),
            Value::String("metadata".to_string())
        ]));
        debug_context.trace(DebugLevel::Trace, "Loading contract state".to_string(), context).await;
        
        // Simulate state loading
        debug_context.state_change(
            "balance".to_string(),
            Some(WasmValue::I64(10000)),
            WasmValue::I64(10000)
        ).await;
        
        debug_context.state_change(
            "permissions".to_string(),
            Some(WasmValue::String("admin".to_string())),
            WasmValue::String("admin".to_string())
        ).await;
    }
    
    // Step 3: Execute business logic
    {
        let mut context = HashMap::new();
        context.insert("step".to_string(), Value::String("business_logic".to_string()));
        context.insert("operation".to_string(), Value::String("transfer_funds".to_string()));
        debug_context.trace(DebugLevel::Trace, "Executing business logic".to_string(), context).await;
        
        // Simulate fund transfer
        debug_context.state_change(
            "balance".to_string(),
            Some(WasmValue::I64(10000)),
            WasmValue::I64(9500)
        ).await;
        
        // Simulate event emission
        let transfer_event = serde_json::json!({
            "type": "FundTransfer",
            "from": "contract_account",
            "to": "recipient_account",
            "amount": 500,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        debug_context.emit_event("FundTransfer".to_string(), transfer_event).await;
        
        // Simulate resource usage for this step
        {
            let mut function_gas_usage = HashMap::new();
            function_gas_usage.insert("transfer_funds".to_string(), 35000u64);
            
            let mut function_memory_usage = HashMap::new();
            function_memory_usage.insert("transfer_funds".to_string(), 2048usize);
            
            debugger.update_gas_info(
                &execution_id,
                1000000,  // initial gas
                35000,    // gas used
                965000,   // remaining gas
                function_gas_usage
            ).await;
            
            debugger.update_memory_info(
                &execution_id,
                2048,     // initial memory
                4096,     // peak memory
                3072,     // current memory
                function_memory_usage
            ).await;
        }
    }
    
    // Step 4: Handle edge case - low balance warning
    {
        let current_balance = 9500i64;
        let transfer_amount = 10000i64;
        
        if current_balance < transfer_amount {
            debug_context.warn("Insufficient funds for transfer".to_string()).await;
        } else {
            debug_context.trace(DebugLevel::Info, "Sufficient funds available".to_string(), HashMap::new()).await;
        }
    }
    
    // Step 5: Finalize and commit changes
    {
        let mut context = HashMap::new();
        context.insert("step".to_string(), Value::String("finalization".to_string()));
        context.insert("changes_committed".to_string(), Value::Bool(true));
        debug_context.trace(DebugLevel::Trace, "Finalizing contract execution".to_string(), context).await;
        
        // Simulate successful completion
        let completion_event = serde_json::json!({
            "type": "ExecutionComplete",
            "status": "success",
            "result": "Funds transferred successfully"
        });
        debug_context.emit_event("ExecutionComplete".to_string(), completion_event).await;
    }
    
    // End debugging
    debugger.end_execution_debug(&execution_id).await;
    println!("Completed debugging execution: {}", execution_id);
    
    Ok(())
}

/// Analyze and display debug information
async fn analyze_debug_information(debugger: Arc<ContractDebugger>) {
    println!("\n--- Analyzing Debug Information ---");
    
    // Get all debug information
    let all_debug_info = debugger.get_all_debug_info().await;
    
    if all_debug_info.is_empty() {
        println!("No debug information available");
        return;
    }
    
    for (execution_id, debug_info) in all_debug_info.iter() {
        println!("\n=== Execution: {} ===", execution_id);
        println!("Contract: {}", debug_info.contract_id.0);
        println!("Choice: {}", debug_info.choice_name);
        println!("Executor: {}", debug_info.executor.0);
        println!("Duration: {}μs", debug_info.duration_us);
        
        // Analyze trace entries
        println!("Trace Entries: {}", debug_info.trace_entries.len());
        for (i, trace) in debug_info.trace_entries.iter().enumerate() {
            if i < 3 { // Show first 3 traces
                println!("  {}. [{}] {}", i + 1, 
                    match trace.level {
                        DebugLevel::None => "NONE",
                        DebugLevel::Info => "INFO",
                        DebugLevel::Trace => "TRACE",
                        DebugLevel::Verbose => "VERBOSE",
                    },
                    trace.message
                );
            }
        }
        if debug_info.trace_entries.len() > 3 {
            println!("  ... and {} more trace entries", debug_info.trace_entries.len() - 3);
        }
        
        // Analyze state changes
        println!("State Changes: {}", debug_info.state_changes.len());
        for (i, change) in debug_info.state_changes.iter().enumerate() {
            if i < 2 { // Show first 2 changes
                println!("  {}. {} -> {:?}", i + 1, change.key, change.new_value);
            }
        }
        if debug_info.state_changes.len() > 2 {
            println!("  ... and {} more state changes", debug_info.state_changes.len() - 2);
        }
        
        // Analyze events
        println!("Events Emitted: {}", debug_info.emitted_events.len());
        for (i, event) in debug_info.emitted_events.iter().enumerate() {
            println!("  {}. {}", i + 1, event.name);
        }
        
        // Analyze resource usage
        println!("Gas Usage: {} used of {}", debug_info.gas_info.gas_used, debug_info.gas_info.initial_gas);
        println!("Memory Usage: {} peak of {}", debug_info.memory_info.peak_memory, debug_info.memory_info.initial_memory);
        
        // Analyze warnings and errors
        if !debug_info.warnings.is_empty() {
            println!("Warnings: {}", debug_info.warnings.len());
            for warning in &debug_info.warnings {
                println!("  - {}", warning);
            }
        }
        
        if !debug_info.errors.is_empty() {
            println!("Errors: {}", debug_info.errors.len());
            for error in &debug_info.errors {
                println!("  - {}", error);
            }
        }
    }
}