# GARP Contract Testing Framework

The GARP Contract Testing Framework provides a comprehensive set of tools for testing smart contracts on the GARP blockchain. It includes utilities for deploying contracts, executing contract functions, and verifying contract behavior.

## Features

- **Test DSL**: A domain-specific language for writing contract tests in a natural, readable way
- **Test Runner**: Executes contract tests with configurable parameters
- **Assertion Utilities**: Specialized assertions for contract-specific validations
- **Mock Data Generation**: Tools for generating test data including participants, contracts, and transactions
- **Test Suites**: Organize related tests into suites for better organization
- **Detailed Reporting**: Comprehensive test results with timing and metadata

## Getting Started

### Installation

Add the contract testing framework to your `Cargo.toml`:

```toml
[dependencies]
garp-participant-node = { path = "../participant-node" }
```

### Basic Usage

```rust
use garp_participant_node::contract_testing::{
    ContractTestDSL, TestConfig, TestAssertions, MockDataGenerator
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create test configuration
    let config = TestConfig::default();
    
    // Create test DSL
    let mut test_dsl = ContractTestDSL::new("My Contract Tests".to_string(), config)?;
    
    // Add a test case
    test_dsl.add_test_case(
        "test_contract_deployment".to_string(),
        "Test that a contract can be deployed successfully".to_string(),
        || {
            // Test implementation here
            garp_participant_node::contract_testing::TestResult {
                passed: true,
                duration_ms: 10,
                error: None,
                metadata: std::collections::HashMap::new(),
            }
        }
    );
    
    // Run the tests
    let result = test_dsl.run().await?;
    
    Ok(())
}
```

## Core Components

### Test Configuration

The `TestConfig` struct allows you to configure various aspects of the testing environment:

```rust
pub struct TestConfig {
    pub gas_limit: u64,           // Maximum gas limit for test execution
    pub timeout_ms: u64,          // Maximum execution time in milliseconds
    pub debug_logging: bool,      // Whether to enable debug logging
    pub network_config: TestNetworkConfig, // Network simulation configuration
}
```

### Contract Test DSL

The `ContractTestDSL` provides a fluent interface for writing contract tests:

```rust
// Deploy a contract for testing
let contract_id = test_dsl.deploy_contract(
    wasm_bytecode,
    signatories,
    observers,
    arguments,
    deployer
).await?;

// Execute a contract choice
let result = test_dsl.execute_contract(
    &contract_id,
    "transfer",
    serde_json::json!({"amount": 100, "to": "participant_1"}),
    executor
).await?;
```

### Test Assertions

The `TestAssertions` struct provides specialized assertions for contract testing:

```rust
// Assert that contract execution was successful
TestAssertions::assert_execution_success(&result);

// Assert that contract execution failed with a specific error
TestAssertions::assert_execution_failure(&result, "InsufficientFunds");

// Assert that a contract has a specific number of effects
TestAssertions::assert_effect_count(&result, 2);

// Assert that a contract emits a specific event
TestAssertions::assert_event_emission(&result, "Transfer");
```

### Mock Data Generation

The `MockDataGenerator` provides utilities for generating test data:

```rust
// Generate mock participant IDs
let participants = MockDataGenerator::generate_mock_participants(3);

// Generate mock contract ID
let contract_id = MockDataGenerator::generate_mock_contract_id();

// Generate mock contract arguments
let args = MockDataGenerator::generate_mock_contract_arguments();

// Generate mock contract bytecode
let bytecode = MockDataGenerator::generate_mock_contract_bytecode();
```

## Advanced Usage

### Test Suites

Organize related tests into suites:

```rust
let mut test_suite = ContractTestSuite::new(
    "Financial Contract Tests".to_string(),
    TestConfig::default()
);

test_suite.add_test_case(ContractTestCase {
    name: "test_token_transfer".to_string(),
    description: "Test token transfer functionality".to_string(),
    test_fn: Box::new(|| {
        // Test implementation
        TestResult {
            passed: true,
            duration_ms: 25,
            error: None,
            metadata: HashMap::new(),
        }
    }),
});
```

### Custom Assertions

Create custom assertions for domain-specific validations:

```rust
pub struct FinancialAssertions;

impl FinancialAssertions {
    pub fn assert_balance_updated(
        result: &ExecutionResult,
        account: &str,
        expected_change: i64
    ) {
        // Custom assertion logic
        assert!(/* balance was updated correctly */);
    }
    
    pub fn assert_no_double_spending(result: &ExecutionResult) {
        // Custom assertion logic
        assert!(/* no double spending occurred */);
    }
}
```

## Testing Best Practices

### 1. Isolate Tests

Each test should be independent and not rely on the state from other tests:

```rust
#[tokio::test]
async fn test_isolated_contract_execution() {
    // Create a fresh test environment for each test
    let test_runner = ContractTestRunner::new(TestConfig::default())?;
    
    // Deploy contract
    let contract_id = test_runner.deploy_test_contract(/* ... */).await?;
    
    // Execute contract
    let result = test_runner.execute_contract_choice(/* ... */).await?;
    
    // Assert results
    TestAssertions::assert_execution_success(&result);
}
```

### 2. Use Descriptive Test Names

Use clear, descriptive names for tests:

```rust
// Good
test_dsl.add_test_case(
    "test_token_transfer_with_sufficient_balance".to_string(),
    "Test that tokens can be transferred when sender has sufficient balance".to_string(),
    /* ... */
);

// Avoid
test_dsl.add_test_case(
    "test1".to_string(),
    "Test something".to_string(),
    /* ... */
);
```

### 3. Test Edge Cases

Include tests for edge cases and error conditions:

```rust
// Test normal operation
test_dsl.add_test_case(
    "test_normal_transfer".to_string(),
    "Test normal token transfer".to_string(),
    /* ... */
);

// Test edge case: zero amount transfer
test_dsl.add_test_case(
    "test_zero_amount_transfer".to_string(),
    "Test transfer with zero amount".to_string(),
    /* ... */
);

// Test error case: insufficient funds
test_dsl.add_test_case(
    "test_transfer_insufficient_funds".to_string(),
    "Test transfer with insufficient funds".to_string(),
    /* ... */
);
```

## Integration with CI/CD

The testing framework is designed to work well with continuous integration systems:

```yaml
# Example GitHub Actions workflow
name: Contract Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v2
    - name: Set up Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
    - name: Run contract tests
      run: |
        cd participant-node
        cargo test contract_testing
    - name: Generate test coverage
      run: |
        cargo tarpaulin --out Xml --output-dir coverage/
```

## Performance Testing

The framework includes utilities for performance testing:

```rust
// Configure test for performance measurement
let config = TestConfig {
    timeout_ms: 5000, // 5 second timeout
    ..TestConfig::default()
};

let mut test_dsl = ContractTestDSL::new("Performance Tests".to_string(), config)?;

// Add performance test
test_dsl.add_test_case(
    "test_high_volume_transfers".to_string(),
    "Test contract performance under high volume".to_string(),
    || {
        let start = std::time::Instant::now();
        
        // Execute many contract calls
        for _ in 0..1000 {
            // Contract execution
        }
        
        let duration = start.elapsed().as_millis() as u64;
        
        TestResult {
            passed: duration < 1000, // Should complete in under 1 second
            duration_ms: duration,
            error: if duration >= 1000 { 
                Some("Performance threshold exceeded".to_string()) 
            } else { 
                None 
            },
            metadata: {
                let mut map = HashMap::new();
                map.insert("transactions_per_second".to_string(), 
                          Value::Number(serde_json::Number::from(1000000 / duration)));
                map
            },
        }
    }
);
```

## Extending the Framework

The framework is designed to be extensible. You can add custom components by implementing the appropriate traits:

```rust
// Custom test runner
pub struct CustomTestRunner {
    base_runner: ContractTestRunner,
    custom_features: CustomFeatures,
}

impl CustomTestRunner {
    pub fn new(config: TestConfig) -> GarpResult<Self> {
        let base_runner = ContractTestRunner::new(config)?;
        Ok(Self {
            base_runner,
            custom_features: CustomFeatures::new(),
        })
    }
    
    // Add custom methods
    pub async fn custom_contract_operation(&self) -> GarpResult<()> {
        // Custom implementation
        Ok(())
    }
}
```

## Conclusion

The GARP Contract Testing Framework provides a robust foundation for testing smart contracts. By following the best practices outlined in this document and leveraging the framework's features, you can ensure your contracts are reliable, secure, and performant.