//! Example of how to use the contract debugging framework

use garp_participant_node::{
    contract_debug::{
        ContractDebugger, DebugLevel, DebugContext, DebugUtils
    },
    storage::MemoryStorage,
    crypto::CryptoService,
    contract_engine::ContractEngine,
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
    let contract_engine = Arc::new(ContractEngine::new(storage, crypto_service));
    
    // Create contract debugger with verbose debugging
    let debugger = Arc::new(ContractDebugger::new(DebugLevel::Verbose, contract_engine));
    
    // Start debugging a contract execution
    let contract_id = garp_common::ContractId(uuid::Uuid::new_v4());
    let executor = garp_common::ParticipantId("test_participant".to_string());
    
    let execution_id = debugger.start_execution_debug(&contract_id, "transfer", &executor).await;
    println!("Started debugging execution: {}", execution_id);
    
    // Create debug context
    let debug_context = DebugContext::new(execution_id.clone(), debugger.clone());
    
    // Add trace entries during contract execution
    let mut context = HashMap::new();
    context.insert("function".to_string(), Value::String("transfer".to_string()));
    context.insert("amount".to_string(), Value::Number(serde_json::Number::from(100)));
    debug_context.trace(DebugLevel::Info, "Executing transfer function".to_string(), context).await;
    
    // Simulate state changes
    debug_context.state_change(
        "balance_participant_1".to_string(),
        Some(garp_participant_node::wasm_runtime::WasmValue::I64(1000)),
        garp_participant_node::wasm_runtime::WasmValue::I64(900)
    ).await;
    
    debug_context.state_change(
        "balance_participant_2".to_string(),
        Some(garp_participant_node::wasm_runtime::WasmValue::I64(500)),
        garp_participant_node::wasm_runtime::WasmValue::I64(600)
    ).await;
    
    // Simulate event emission
    let event_data = serde_json::json!({
        "from": "participant_1",
        "to": "participant_2",
        "amount": 100
    });
    debug_context.emit_event("TransferCompleted".to_string(), event_data).await;
    
    // Simulate resource usage
    {
        let mut function_gas_usage = HashMap::new();
        function_gas_usage.insert("transfer".to_string(), 25000u64);
        
        let mut function_memory_usage = HashMap::new();
        function_memory_usage.insert("transfer".to_string(), 1024usize);
        
        debugger.update_gas_info(
            &execution_id,
            1000000,  // initial gas
            25000,    // gas used
            975000,   // remaining gas
            function_gas_usage
        ).await;
        
        debugger.update_memory_info(
            &execution_id,
            1024,     // initial memory
            2048,     // peak memory
            1536,     // current memory
            function_memory_usage
        ).await;
    }
    
    // Simulate a warning
    debug_context.warn("Low balance warning for participant_1".to_string()).await;
    
    // End debugging
    debugger.end_execution_debug(&execution_id).await;
    println!("Ended debugging execution: {}", execution_id);
    
    // Retrieve and display debug information
    if let Some(debug_info) = debugger.get_execution_debug_info(&execution_id).await {
        println!("\n=== Debug Information ===");
        DebugUtils::print_execution_debug_info(&debug_info).await;
        
        // Export to JSON
        match DebugUtils::export_execution_debug_info(&debug_info).await {
            Ok(json) => {
                println!("\n=== JSON Export ===");
                println!("{}", json);
            }
            Err(e) => {
                eprintln!("Failed to export debug info to JSON: {}", e);
            }
        }
    }
    
    // Clean up
    debugger.clear_execution_debug_info(&execution_id).await;
    
    println!("\nDebugging example completed successfully!");
    Ok(())
}