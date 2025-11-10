# Privacy Engine Test Suite

This directory contains comprehensive tests for the GARP privacy engine, including unit tests, integration tests, and performance benchmarks.

## Test Structure

### Unit Tests
- **Location**: `privacy_engine_tests.rs`
- **Purpose**: Test individual components in isolation
- **Coverage**: 
  - Privacy engine initialization
  - Contract deployment and execution
  - Zero-knowledge proof generation
  - State management operations
  - Transaction processing
  - Error handling scenarios

### Integration Tests
- **Location**: `integration_tests.rs`
- **Purpose**: Test complete workflows and component interactions
- **Scenarios**:
  - Multi-party private computation
  - Privacy-preserving auction system
  - Privacy-preserving voting system
  - Cross-chain privacy bridging
  - DeFi lending with privacy
  - Performance under concurrent load
  - Error recovery and resilience

### Test Configuration
- **Location**: `test_config.rs`
- **Purpose**: Common utilities and setup functions
- **Features**:
  - Test logging initialization
  - Configurable privacy settings
  - Mock data generators
  - Test assertions and utilities
  - Performance metrics comparison

### Performance Benchmarks
- **Location**: `../benches/privacy_engine_benchmarks.rs`
- **Purpose**: Measure and track performance metrics
- **Benchmarks**:
  - Privacy engine initialization
  - Contract deployment (various sizes)
  - ZK proof generation (multiple schemes)
  - State operations (CRUD with various data sizes)
  - Transaction processing (batch operations)
  - Selective disclosure operations
  - Concurrent operation performance
  - Memory usage analysis

## Running Tests

### Prerequisites
```bash
# Install Rust and Cargo (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install test dependencies
cargo install cargo-tarpaulin  # For code coverage
cargo install cargo-audit      # For security auditing
```

### Quick Test Commands

```bash
# Run all unit tests
cargo test --lib

# Run specific integration tests
cargo test --test privacy_engine_tests
cargo test --test integration_tests

# Run all tests
cargo test

# Run benchmarks
cargo bench --bench privacy_engine_benchmarks

# Run with verbose output
cargo test -- --nocapture
```

### Using the Test Runner Script

The PowerShell script `scripts/run_tests.ps1` provides comprehensive test execution:

```powershell
# Run all tests with full reporting
.\scripts\run_tests.ps1 -TestType all -Verbose -Coverage

# Run only unit tests
.\scripts\run_tests.ps1 -TestType unit

# Run integration tests
.\scripts\run_tests.ps1 -TestType integration

# Run benchmarks only
.\scripts\run_tests.ps1 -TestType benchmarks

# Skip benchmarks (faster execution)
.\scripts\run_tests.ps1 -NoBenchmarks

# Generate code coverage report
.\scripts\run_tests.ps1 -Coverage -OutputDir "coverage-results"
```

### Test Configuration Options

The test suite supports various configuration options through environment variables:

```bash
# Set log level for detailed debugging
export RUST_LOG=debug

# Enable backtraces for error analysis
export RUST_BACKTRACE=1

# Configure test timeouts
export TEST_TIMEOUT=300

# Set custom test data directory
export TEST_DATA_DIR=/path/to/test/data
```

## Test Categories

### 1. Functional Tests
- **Privacy Engine Core**: Initialization, configuration, lifecycle
- **Contract Management**: Deployment, execution, state management
- **Zero-Knowledge Proofs**: Generation, verification, different schemes
- **Private Transactions**: Processing, validation, selective disclosure
- **State Management**: Encrypted storage, commitment trees, versioning

### 2. Security Tests
- **Access Control**: Permission validation, role-based access
- **Cryptographic Operations**: Key management, encryption, signatures
- **Privacy Guarantees**: Data leakage prevention, anonymity preservation
- **Input Validation**: Malformed data handling, boundary conditions

### 3. Performance Tests
- **Throughput**: Transaction processing rates, batch operations
- **Latency**: Response times for various operations
- **Scalability**: Performance under increasing load
- **Resource Usage**: Memory consumption, CPU utilization

### 4. Integration Tests
- **End-to-End Workflows**: Complete privacy-preserving scenarios
- **Component Interactions**: Cross-module communication and data flow
- **Error Propagation**: Failure handling across system boundaries
- **Concurrent Operations**: Multi-threaded execution safety

## Test Data and Fixtures

### Mock Data Generation
The test suite includes comprehensive mock data generators:

```rust
// Generate test contracts of various complexities
let simple_contract = TestUtils::generate_simple_contract();
let complex_contract = TestUtils::generate_complex_contract();

// Create test transactions with different privacy levels
let private_tx = TestUtils::generate_private_transaction();
let selective_disclosure_tx = TestUtils::generate_selective_disclosure_transaction();

// Generate cryptographic test data
let zk_proof_data = TestUtils::generate_zk_proof_data(ProofScheme::Groth16);
let commitment_data = TestUtils::generate_commitment_data();
```

### Test Assertions
Custom assertion helpers for privacy-specific validations:

```rust
// Assert privacy guarantees
TestAssertions::assert_privacy_preserved(&result);
TestAssertions::assert_no_data_leakage(&transaction);

// Assert performance requirements
TestAssertions::assert_performance_within_bounds(&metrics, &expected);

// Assert cryptographic correctness
TestAssertions::assert_proof_valid(&proof, &public_inputs);
```

## Continuous Integration

### GitHub Actions Integration
The test suite is designed to work with CI/CD pipelines:

```yaml
# Example GitHub Actions workflow
- name: Run Privacy Engine Tests
  run: |
    cargo test --all-features
    cargo bench --bench privacy_engine_benchmarks
    
- name: Generate Coverage Report
  run: |
    cargo tarpaulin --out Xml --output-dir coverage/
    
- name: Security Audit
  run: |
    cargo audit
```

### Test Reporting
The test runner generates comprehensive reports:

- **Test Results**: Pass/fail status for all test categories
- **Performance Metrics**: Benchmark results and trend analysis
- **Code Coverage**: Line and branch coverage statistics
- **Security Audit**: Vulnerability scan results

## Debugging Tests

### Common Issues and Solutions

1. **Test Timeouts**
   ```bash
   # Increase timeout for slow operations
   cargo test -- --test-threads=1 --timeout=300
   ```

2. **Memory Issues**
   ```bash
   # Run tests with memory profiling
   cargo test --release -- --nocapture
   ```

3. **Cryptographic Failures**
   ```bash
   # Enable detailed crypto logging
   RUST_LOG=garp_privacy=trace cargo test
   ```

### Test Isolation
Tests are designed to be independent and can run in parallel:

- Each test uses isolated state and resources
- Mock services prevent external dependencies
- Cleanup procedures ensure no test pollution

## Contributing

### Adding New Tests

1. **Unit Tests**: Add to existing test modules or create new ones
2. **Integration Tests**: Create comprehensive scenario tests
3. **Benchmarks**: Add performance measurements for new features
4. **Documentation**: Update this README with new test information

### Test Guidelines

- **Naming**: Use descriptive test names that explain the scenario
- **Documentation**: Add comments explaining complex test logic
- **Assertions**: Use specific assertions that clearly indicate failures
- **Cleanup**: Ensure tests clean up resources and state
- **Performance**: Consider test execution time and resource usage

### Code Coverage Goals

- **Unit Tests**: Aim for >90% line coverage
- **Integration Tests**: Cover all major user workflows
- **Edge Cases**: Test boundary conditions and error scenarios
- **Security**: Validate all security-critical code paths

## Resources

- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Criterion Benchmarking](https://bheisler.github.io/criterion.rs/book/)
- [Tarpaulin Coverage](https://github.com/xd009642/tarpaulin)
- [GARP Privacy Engine Documentation](../README.md)

---

For questions or issues with the test suite, please refer to the main project documentation or open an issue in the repository.