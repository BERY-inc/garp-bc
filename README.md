# GARP Blockchain Platform

A distributed blockchain-inspired system with privacy-preserving transaction processing, cross-domain state synchronization, and secure execution environment.

## Project Structure

- `participant-node` - Main node handling private transactions, secure execution, WASM runtime, ZK systems, and wallet functionality
- `sync-domain` - Manages domain synchronization using Kafka, vector clocks, and a sequencer; implements consensus
- `global-synchronizer` - Coordinates cross-domain settlement and global state sync
- `common` - Shared Rust library with consensus, crypto, network, and type definitions
- `backend-go` - Go-based backend service (API or admin interface) with config, middleware, OTel, and storage modules
- `frontend` - HTML/CSS/JS frontend interface for interacting with the blockchain
- `k8s` - Kubernetes deployment manifests
- `.github/workflows` - CI/CD pipeline configurations

## Prerequisites

- Rust 1.73+
- Go 1.21+
- Docker and Docker Compose
- PostgreSQL client
- Redis client

## Quick Start

### Development Environment

```bash
# Start all services with docker-compose
make run-dev

# Stop all services
make stop-dev
```

### Manual Setup

1. Start databases:

   ```bash
   docker run -d --name garp-postgres -p 5432:5432 -e POSTGRES_DB=garp -e POSTGRES_USER=garp -e POSTGRES_PASSWORD=garp postgres:13
   docker run -d --name garp-redis -p 6379:6379 redis:6-alpine
   ```

2. Build components:

   ```bash
   make build
   ```

3. Run services:

   ```bash
   # Run participant node
   cd participant-node
   cargo run -- --config config.toml

   # Run backend API (in another terminal)
   cd backend-go
   ./garp-backend
   ```

## CI/CD Pipeline

The project uses GitHub Actions for CI/CD with the following workflow:

1. **Test** - Runs unit tests for all Rust and Go components
2. **Build** - Compiles all components and creates binaries
3. **Docker** - Builds and pushes Docker images to DockerHub
4. **Deploy** - Deploys to Kubernetes (main branch only)
5. **Security** - Runs security scans on dependencies

### Setting up CI/CD

1. Add the following secrets to your GitHub repository:

   - `DOCKER_USERNAME` - DockerHub username
   - `DOCKER_PASSWORD` - DockerHub password or access token
   - `KUBECONFIG_DATA` - Base64 encoded kubeconfig for Kubernetes deployment

2. The pipeline will automatically trigger on pushes to `main` and `develop` branches.

## API Endpoints

### Backend API (port 8081)

- `GET /health` - Health check
- `GET /health/ready` - Readiness check
- `POST /accounts` - Create new account
- `GET /accounts/{address}` - Get account details
- `POST /transactions` - Submit transaction
- `GET /transactions/{id}/status` - Get transaction status
- `GET /blocks/latest` - Get latest block
- `GET /blocks/{number}` - Get block by number
- `GET /contracts` - List contracts
- `POST /contracts` - Deploy contract
- `POST /contracts/{id}/exercise` - Exercise contract
- `GET /wallet/balances` - Get wallet balances
- `GET /wallet/history` - Get wallet transaction history

## Testing

Run all tests:

```bash
make test
```

## Building

Build all components:

```bash
make build
```

## License

[License information would go here]
## Build Status

![Rust CI](https://github.com/BERY-inc/garp-bc/actions/workflows/ci-rust.yml/badge.svg)
![Go CI](https://github.com/BERY-inc/garp-bc/actions/workflows/ci-go.yml/badge.svg)
![CD](https://github.com/BERY-inc/garp-bc/actions/workflows/cd.yml/badge.svg)
![Unified CI/CD](https://github.com/BERY-inc/garp-bc/actions/workflows/ci-cd.yml/badge.svg)
