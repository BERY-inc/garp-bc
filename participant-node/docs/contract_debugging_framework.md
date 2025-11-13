# GARP Contract Debugging Framework

The GARP Contract Debugging Framework provides comprehensive tools for debugging smart contracts on the GARP blockchain. It includes utilities for execution tracing, state inspection, resource monitoring, and detailed debugging information capture.

## Features

- **Execution Tracing**: Detailed tracing of contract execution with configurable debug levels
- **State Inspection**: Tracking of state changes during contract execution
- **Event Monitoring**: Capture and analysis of events emitted by contracts
- **Resource Usage Tracking**: Monitoring of gas and memory usage
- **Warning and Error Handling**: Comprehensive logging of warnings and errors
- **Debug Information Export**: Export debug information in various formats (JSON, human-readable)
- **Integration with Contract Engine**: Seamless integration with the GARP contract engine

## Getting Started

### Installation

Add the contract debugging framework to your `Cargo.toml`:

```toml
[dependencies]
garp-participant-node = { path = "../participant-node" }
```

### Basic Usage

```rust
use garp_participant_node::contract_debug::{
    ContractDebugger, DebugLevel, DebugContext
};
use std::sync::Arc;

// Create contract debugger
let storage = Arc::new(MemoryStorage::new());
let crypto_service = Arc::new(CryptoService::new());
let contract_engine = Arc::new(ContractEngine::new(storage, crypto_service));
let debugger = Arc::new(ContractDebugger::new(DebugLevel::Info, contract_engine));

// Start debugging a contract execution
let execution_id = debugger.start_execution_debug(&contract_id, "transfer", &executor).await;

// Create debug context
let debug_context = DebugContext::new(execution_id.clone(), debugger.clone());

// Add trace entries during execution
debug_context.trace(DebugLevel::Info, "Executing transfer".to_string(), HashMap::new()).await;

// End debugging
debugger.end_execution_debug(&execution_id).await;
```

## Core Components

### Debug Levels

The framework supports different debug levels for controlling the amount of information captured:

```rust
pub enum DebugLevel {
    None,     // No debugging information
    Info,     // Basic execution information
    Trace,    // Detailed execution tracing
    Verbose,  // Full debugging with all information
}
```

### Contract Debugger

The `ContractDebugger` is the main component for managing debug information:

```rust
// Create debugger with specific debug level
let debugger = ContractDebugger::new(DebugLevel::Trace, contract_engine);

// Start debugging an execution
let execution_id = debugger.start_execution_debug(&contract_id, "choice_name", &executor).await;

// Add debug information
debugger.add_trace_entry(&execution_id, DebugLevel::Info, "Message".to_string(), context).await;
debugger.add_state_change(&execution_id, "key".to_string(), old_value, new_value).await;
debugger.add_emitted_event(&execution_id, "EventName".to_string(), event_data).await;

// End debugging
debugger.end_execution_debug(&execution_id).await;
```

### Debug Context

The `DebugContext` provides a convenient interface for adding debug information during contract execution:

```rust
let debug_context = DebugContext::new(execution_id, debugger);

// Add trace entries
debug_context.trace(DebugLevel::Info, "Executing function".to_string(), context).await;

// Add state changes
debug_context.state_change("balance".to_string(), old_value, new_value).await;

// Add events
debug_context.emit_event("TransferComplete".to_string(), event_data).await;

// Add warnings and errors
debug_context.warn("Low balance warning".to_string()).await;
debug_context.error("Insufficient funds error".to_string()).await;
```

### Execution Debug Information

The framework captures comprehensive debug information for each contract execution:

```rust
pub struct ExecutionDebugInfo {
    pub start_time: Instant,
    pub duration_us: u128,
    pub contract_id: ContractId,
    pub choice_name: String,
    pub executor: ParticipantId,
    pub debug_level: DebugLevel,
    pub trace_entries: Vec<TraceEntry>,
    pub state_changes: Vec<StateChangeInfo>,
    pub emitted_events: Vec<EventInfo>,
    pub gas_info: GasInfo,
    pub memory_info: MemoryInfo,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}
```

## Advanced Usage

### Resource Usage Monitoring

Track gas and memory usage during contract execution:

```rust
// Update gas information
let mut function_gas_usage = HashMap::new();
function_gas_usage.insert("transfer".to_string(), 25000u64);

debugger.update_gas_info(
    &execution_id,
    1000000,  // initial gas
    25000,    // gas used
    975000,   // remaining gas
    function_gas_usage
).await;

// Update memory information
let mut function_memory_usage = HashMap::new();
function_memory_usage.insert("transfer".to_string(), 1024usize);

debugger.update_memory_info(
    &execution_id,
    1024,     // initial memory
    2048,     // peak memory
    1536,     // current memory
    function_memory_usage
).await;
```

### Debug Information Analysis

Analyze and export debug information:

```rust
// Get debug information
let debug_info = debugger.get_execution_debug_info(&execution_id).await;

if let Some(debug_info) = debug_info {
    // Print human-readable debug information
    DebugUtils::print_execution_debug_info(&debug_info).await;
    
    // Export to JSON
    let json = DebugUtils::export_execution_debug_info(&debug_info).await?;
    println!("Debug info as JSON: {}", json);
}
```

### Integration with Contract Engine

The debugging framework integrates seamlessly with the contract engine:

```rust
impl ContractEngine {
    pub async fn execute_contract_with_debug(
        &self,
        contract_id: &ContractId,
        choice_name: &str,
        choice_arguments: Value,
        executor: &ParticipantId,
        debugger: Option<Arc<ContractDebugger>>,
    ) -> GarpResult<ExecutionResult> {
        let execution_id = if let Some(ref debugger) = debugger {
            Some(debugger.start_execution_debug(contract_id, choice_name, executor).await)
        } else {
            None
        };
        
        // Execute contract normally
        let result = self.execute_contract(contract_id, choice_name, choice_arguments, executor).await;
        
        // Add debug information if debugger is provided
        if let (Some(debugger), Some(execution_id)) = (debugger, execution_id) {
            if let Ok(ref result) = result {
                // Add execution results to debug info
                for effect in &result.effects {
                    let mut context = HashMap::new();
                    context.insert("effect_type".to_string(), Value::String(effect.effect_type.clone()));
                    debugger.add_trace_entry(
                        &execution_id,
                        DebugLevel::Info,
                        format!("Executed effect: {}", effect.description),
                        context
                    ).await;
                }
                
                for event in &result.events {
                    debugger.add_emitted_event(
                        &execution_id,
                        event.event_type.clone(),
                        event.data.clone()
                    ).await;
                }
            }
            
            debugger.end_execution_debug(&execution_id).await;
        }
        
        result
    }
}
```

## Debugging Best Practices

### 1. Use Appropriate Debug Levels

Choose the right debug level for your needs:

```rust
// For production environments, use minimal debugging
let debugger = ContractDebugger::new(DebugLevel::Info, contract_engine);

// For development and testing, use detailed debugging
let debugger = ContractDebugger::new(DebugLevel::Verbose, contract_engine);
```

### 2. Add Contextual Information

Include relevant context with trace entries:

```rust
let mut context = HashMap::new();
context.insert("function".to_string(), Value::String("transfer".to_string()));
context.insert("amount".to_string(), Value::Number(serde_json::Number::from(100)));
context.insert("from".to_string(), Value::String("account1".to_string()));
context.insert("to".to_string(), Value::String("account2".to_string()));

debug_context.trace(DebugLevel::Trace, "Transferring funds".to_string(), context).await;
```

### 3. Monitor Resource Usage

Track gas and memory usage to optimize contract performance:

```rust
// Monitor gas usage per function
debugger.update_gas_info(
    &execution_id,
    initial_gas,
    gas_used,
    remaining_gas,
    function_gas_usage
).await;

// Monitor memory usage per function
debugger.update_memory_info(
    &execution_id,
    initial_memory,
    peak_memory,
    current_memory,
    function_memory_usage
).await;
```

### 4. Handle Warnings and Errors Appropriately

Log warnings and errors with sufficient detail:

```rust
// Log warnings for non-critical issues
if balance < minimum_required {
    debug_context.warn(format!("Low balance: {} < {}", balance, minimum_required)).await;
}

// Log errors for critical issues
if transfer_amount > available_funds {
    debug_context.error(format!("Insufficient funds: {} > {}", transfer_amount, available_funds)).await;
}
```

## Performance Considerations

### 1. Debug Level Impact

Higher debug levels have more performance overhead:

```rust
// DebugLevel::None - No overhead
// DebugLevel::Info - Minimal overhead
// DebugLevel::Trace - Moderate overhead
// DebugLevel::Verbose - Maximum overhead
```

### 2. Memory Usage

Debug information is stored in memory, so consider clearing it periodically:

```rust
// Clear debug information for specific execution
debugger.clear_execution_debug_info(&execution_id).await;

// Clear all debug information
debugger.clear_all_debug_info().await;
```

### 3. Conditional Debugging

Enable debugging conditionally to minimize overhead:

```rust
let debugger = if cfg!(debug_assertions) {
    Some(Arc::new(ContractDebugger::new(DebugLevel::Verbose, contract_engine)))
} else {
    None
};
```

## Integration with Testing

The debugging framework works well with the testing framework:

```rust
#[tokio::test]
async fn test_contract_with_debugging() {
    let debugger = Arc::new(ContractDebugger::new(DebugLevel::Trace, contract_engine));
    
    // Execute contract with debugging
    let result = contract_engine.execute_contract_with_debug(
        &contract_id,
        "transfer",
        arguments,
        &executor,
        Some(debugger.clone())
    ).await;
    
    // Assert successful execution
    assert!(result.is_ok());
    
    // Analyze debug information
    let debug_info = debugger.get_execution_debug_info(&execution_id).await;
    assert!(debug_info.is_some());
    
    let debug_info = debug_info.unwrap();
    assert!(debug_info.trace_entries.len() > 0);
    assert!(debug_info.state_changes.len() > 0);
}
```

## Export and Analysis Tools

### JSON Export

Export debug information in JSON format for external analysis:

```rust
let json = DebugUtils::export_execution_debug_info(&debug_info).await?;
// Save to file or send to external system
```

### Custom Analysis

Create custom analysis tools for specific debugging needs:

```rust
pub struct PerformanceAnalyzer;

impl PerformanceAnalyzer {
    pub async fn analyze_execution_performance(debug_info: &ExecutionDebugInfo) -> PerformanceReport {
        PerformanceReport {
            execution_time_ms: (debug_info.duration_us as f64) / 1000.0,
            gas_efficiency: (debug_info.gas_info.gas_used as f64) / (debug_info.gas_info.initial_gas as f64),
            memory_efficiency: (debug_info.memory_info.peak_memory as f64) / (debug_info.memory_info.initial_memory as f64),
            trace_density: debug_info.trace_entries.len() as f64 / ((debug_info.duration_us as f64) / 1000000.0),
        }
    }
}
```

## Conclusion

The GARP Contract Debugging Framework provides powerful tools for debugging smart contracts. By leveraging its features, you can gain deep insights into contract execution, optimize performance, and quickly identify and resolve issues.