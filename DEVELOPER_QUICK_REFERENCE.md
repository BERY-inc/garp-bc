# GARP Blockchain Platform - Developer Quick Reference

This document provides a quick reference guide for developers working with the GARP blockchain platform.

## Project Structure

```
garp/
├── participant-node/          # Main node handling private transactions
├── sync-domain/              # Domain synchronization and consensus
├── global-synchronizer/      # Cross-domain settlement and global state sync
│   ├── src/
│   │   ├── bridge/           # Cross-chain bridge components
│   │   │   ├── tests/        # Unit tests for bridge components
│   │   │   ├── ethereum.rs   # Ethereum connector
│   │   │   ├── polygon.rs    # Polygon connector
│   │   │   ├── bsc.rs        # BSC connector
│   │   │   ├── solana.rs     # Solana connector
│   │   │   ├── oracle.rs     # Price oracle
│   │   │   ├── liquidity.rs  # Liquidity pools
│   │   │   └── wallet.rs     # Wallet management
│   │   └── bridge.rs         # Main bridge implementation
│   └── tests/                # Integration tests
├── common/                   # Shared libraries and utilities
├── backend-go/               # Go-based backend services
├── frontend/                 # Web frontend interface
└── sdk-{py,js,go,rs}/        # Language-specific SDKs
```

## Key Components

### Cross-Chain Bridge
- **Location**: `global-synchronizer/src/bridge/`
- **Main Module**: `bridge.rs`
- **Connectors**: `ethereum.rs`, `polygon.rs`, `bsc.rs`, `solana.rs`
- **Support Modules**: `oracle.rs`, `liquidity.rs`, `wallet.rs`

### Testing Framework
- **Unit Tests**: `global-synchronizer/src/bridge/tests/`
- **Integration Tests**: `global-synchronizer/tests/`

### API Endpoints
- **Bridge Operations**: `/api/v1/bridge/*`
- **Wallet Management**: `/api/v1/wallets/*`
- **Oracle Services**: `/api/v1/oracle/*`
- **Liquidity Pools**: `/api/v1/pool/*`

## Running Tests

### Using Scripts
```bash
# On Unix/Linux/macOS
./run_tests.sh

# On Windows
.\run_tests.ps1
```

### Manual Commands
```bash
# Run all tests
cargo test

# Run bridge-specific tests
cargo test bridge

# Run unit tests only
cargo test --lib

# Run integration tests only
cargo test --test "*"

# Check syntax only
cargo check --tests
```

## Key APIs

### Bridge Transactions
```
POST /api/v1/bridge/transfer
GET /api/v1/bridge/transfer/{id}
GET /api/v1/bridge/transfer/{id}/status
```

### Wallet Management
```
POST /api/v1/wallets
GET /api/v1/wallets/{id}
GET /api/v1/wallets
```

### Oracle Services
```
GET /api/v1/oracle/price/{symbol}
GET /api/v1/oracle/prices
GET /api/v1/oracle/conversion/{from}/{to}
```

### Liquidity Operations
```
POST /api/v1/pool/add
POST /api/v1/pool/remove
POST /api/v1/pool/swap
GET /api/v1/pool/info
GET /api/v1/pool/tvl
```

## Environment Variables

```bash
# API Authentication
SYNC_API_TOKEN=your_api_token

# Database Configuration
DATABASE_URL=postgresql://user:password@host:port/database

# Rate Limiting
SYNC_RATE_LIMIT_RPM=120

# Logging
RUST_LOG=info
```

## Development Workflow

1. **Setup Development Environment**
   ```bash
   # Install dependencies
   rustup install stable
   rustup default stable
   
   # Clone repository
   git clone <repository-url>
   cd garp
   ```

2. **Build Project**
   ```bash
   # Build all components
   cargo build
   
   # Build specific component
   cd global-synchronizer
   cargo build
   ```

3. **Run Tests**
   ```bash
   # Run all tests
   cargo test
   
   # Run specific test suite
   cargo test bridge
   ```

4. **Run Development Server**
   ```bash
   # Run global synchronizer
   cd global-synchronizer
   cargo run
   ```

## Common Development Tasks

### Adding a New Test
1. Create test file in appropriate `tests/` directory
2. Follow existing test patterns
3. Run tests to verify implementation

### Modifying Bridge Components
1. Update relevant module in `src/bridge/`
2. Add/update unit tests in `src/bridge/tests/`
3. Update integration tests if needed
4. Run full test suite

### Adding New API Endpoints
1. Add endpoint in `api.rs`
2. Implement handler function
3. Add corresponding method in `synchronizer.rs`
4. Update documentation

## Troubleshooting

### Dependency Issues
```bash
# Update dependencies
cargo update

# Clean build artifacts
cargo clean

# Check for outdated dependencies
cargo outdated
```

### Test Failures
```bash
# Run tests with verbose output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run tests in specific file
cargo test --test file_name
```

### Debugging
```bash
# Enable debug logging
export RUST_LOG=debug

# Run with debugger
gdb --args cargo run
```

## Documentation

### Main Documentation
- `GARP_BLOCKCHAIN_DOCUMENTATION.md` - Comprehensive platform documentation
- `README.md` - Project overview and quick start guide
- Component-specific README files

### Testing Documentation
- `BRIDGE_TESTING_IMPROVEMENTS.md` - Testing framework details
- `RECENT_UPDATES_SUMMARY.md` - Recent changes and improvements

### Roadmap
- `FUTURE_IMPROVEMENTS_ROADMAP.md` - Future development plans

## Contributing

1. Fork the repository
2. Create feature branch
3. Make changes
4. Add tests
5. Update documentation
6. Submit pull request

## Support

- Check documentation first
- Review existing issues
- Create new issue if needed
- Join community forums