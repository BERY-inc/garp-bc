Global Synchronizer
===================

Overview
- Cross-domain atomic settlement with BFT consensus.
- Modular components: consensus, cross-domain, settlement, network, storage, API.

Persistent Storage
- Default backend now selects Postgres when `database.url` starts with `postgres://` or `postgresql://`.
- Fallback is in-memory if URL is unrecognized.
- Tables created automatically when `database.enable_migrations = true`:
  - `kv_store(key TEXT PRIMARY KEY, value BYTEA, created_at TIMESTAMPTZ, updated_at TIMESTAMPTZ)`
  - `kv_snapshots(snapshot_id TEXT PRIMARY KEY, created_at TIMESTAMPTZ)`
  - `kv_snapshot_entries(snapshot_id TEXT, key TEXT, value BYTEA, PRIMARY KEY(snapshot_id,key))`

Running Locally (Single Node)
- Prerequisites:
  - Rust toolchain (`rustup`), preferably MSVC on Windows (`rustup default stable-x86_64-pc-windows-msvc`).
  - Postgres running locally and accessible by the configured URL.
  - Create database `global_sync` and user `garp` with password `garp` or adjust `config/global-sync.toml`.
- Command:
  - `cargo run -p global-synchronizer -- --node-id global-sync-1 --database-url postgresql://garp:garp@localhost:5432/global_sync`
  - Optional: `--peers`, `--port`, `--consensus-port`, `--log-level`, `--enable-metrics`.

Running a Local Cluster (Multi-node)
- Windows PowerShell script provided: `scripts/run-local-cluster.ps1`.
- Example:
  - `pwsh scripts/run-local-cluster.ps1 -NodeCount 4 -StartApiPort 8000 -StartConsensusPort 7000 -DatabaseUrl postgresql://garp:garp@localhost:5432/global_sync`
- Each node gets its own `node_id`, API port, and consensus port.

API
- Health: `GET /health`
- Status: `GET /api/v1/status`
- Consensus: `GET /api/v1/status/consensus`
- Metrics (JSON): `GET /api/v1/status/metrics`
- Metrics (Prometheus): `GET /metrics`
- Blocks: `GET /api/v1/blocks/latest`, `GET /api/v1/blocks/:height`, `GET /api/v1/blocks/:height/details`
- Transactions: `POST /api/v1/transactions`, `POST /api/v1/transactions/signed`, `GET /api/v1/transactions/:id/status`, `GET /api/v1/transactions/:id/details`
- Validators: `GET /api/v1/validators`, `POST /api/v1/validators`, `DELETE /api/v1/validators/:id`, `PATCH /api/v1/validators/:id/status`
- Auth: set `SYNC_API_TOKEN` to enforce bearer token validation.

Testing
- Unit tests: `cargo test`
- Bridge-specific tests: `cargo test bridge`
- Integration tests are located in the `tests/` directory
- Comprehensive test coverage for wallet management, liquidity pools, price oracles, and cross-chain connectors

Operational Notes
- Configure logging level via `--log-level` or `monitoring.logging.level`.
- Enable metrics collection via `--enable-metrics`; scrape `GET /metrics` with Prometheus.
- Backups, replication, and cache managers initialize at start; further configuration TBD.

Security & Consensus Hardening Roadmap
- Validator set management and slashing policies are planned based on `GlobalState` primitives.
- Production-grade finality (e.g., Tendermint-like) to be integrated behind feature flags.
- Signed transaction verification (Ed25519) is wired via `POST /api/v1/transactions/signed`.
- Audit logging and encryption already configurable; formal audits pending.

Secrets Management
- Store validator private keys in KMS/Vault; perform client-side signing only.
- Inject `SYNC_API_TOKEN` and `DATABASE_URL` via environment or OS secrets store.
- Rotate tokens regularly and maintain key IDs in validator metadata to support rotation.

Client Examples
- Submit signed transaction:
  - Canonical message: `transaction_id|source_domain|target_domains_csv|hex(data)|required_confirmations`
  - Request:
    ```bash
    curl -X POST http://localhost:8080/api/v1/transactions/signed \
      -H "Authorization: Bearer $SYNC_API_TOKEN" \
      -H "Content-Type: application/json" \
      -d '{"tx": {"transaction_id": "<uuid>", "source_domain": "ethereum-mainnet", "target_domains": ["solana-mainnet"], "transaction_type": {"AssetTransfer": {"asset_id": "USDC", "amount": 1000, "from_address": "0xabc", "to_address": "So1..."}}, "data": "", "dependencies": [], "required_confirmations": 2, "confirmations": {}, "status": "Pending", "created_at": "2024-01-01T00:00:00Z", "updated_at": "2024-01-01T00:00:00Z", "timeout_at": "2024-01-01T00:10:00Z", "metadata": {} }, "public_key_hex": "<32-byte-hex>", "signature_hex": "<64-byte-hex>"}'
    ```
- Manage validators:
    ```bash
    curl -H "Authorization: Bearer $SYNC_API_TOKEN" http://localhost:8080/api/v1/validators
    curl -X POST http://localhost:8080/api/v1/validators -H "Authorization: Bearer $SYNC_API_TOKEN" -H "Content-Type: application/json" -d '{"id":"validator-1","public_key_hex":"<32-byte-hex>","voting_power":100}'
    curl -X PATCH http://localhost:8080/api/v1/validators/validator-1/status -H "Authorization: Bearer $SYNC_API_TOKEN" -H "Content-Type: application/json" -d '{"status":"jailed"}'
    ```

Troubleshooting (Windows)
- If build fails with `dlltool.exe not found`, switch to MSVC toolchain:
  - `rustup default stable-x86_64-pc-windows-msvc`
  - Ensure `Build Tools for Visual Studio` are installed.
- Postgres connection issues: verify `database.url`, reachability, and credentials.