# VPS Deployment Guide (Docker Compose)

This guide describes how to deploy the GARP blockchain stack on a Linux VPS using Docker Compose. It is optimized for limited resources while maintaining reliability and security.

## Prerequisites

- Ubuntu 22.04+ VPS with `sudo` privileges
- 2 vCPU, 4 GB RAM minimum (8 GB recommended)
- Disk: 20 GB minimum (40+ GB recommended for data)
- Ports open: `80/tcp` and optionally `8080/tcp` for API gateway

## Quick Start

1. Copy `.env.example` to `.env` and set secure values.
   - Set `POSTGRES_PASSWORD`, `JWT_SECRET` to strong secrets.
2. Initialize Docker and repo directory:
   - `REPO_DIR=/opt/garp bash scripts/deploy-vps.sh init`
3. Build and start services:
   - `REPO_DIR=/opt/garp bash scripts/deploy-vps.sh up`
4. Check status:
   - `REPO_DIR=/opt/garp bash scripts/deploy-vps.sh status`
5. Access:
   - Frontend: `http://<server-ip>/`
   - API Gateway: `http://<server-ip>:8080/` (if exposed)

## Services and Networks

- Internal services (no external ports): `postgres`, `redis`, `zookeeper`, `kafka`, `participant-node`, `sync-domain`, `global-synchronizer`, `backend-go`.
- Public services: `api-gateway-go` (port 8080), `frontend` (port 80).
- Two networks:
  - `internal` (isolated) and `public` (browser/API access).

## Operational Commands

- Rolling upgrade:
  - `REPO_DIR=/opt/garp bash scripts/deploy-vps.sh upgrade`
  - Rebuilds images and restarts services sequentially.
- Backup Postgres:
  - `REPO_DIR=/opt/garp bash scripts/deploy-vps.sh backup /var/backups/garp`
- Restore Postgres:
  - `REPO_DIR=/opt/garp bash scripts/deploy-vps.sh restore /var/backups/garp/pgdump-YYYYMMDD-HHMMSS.sql`
- Tail logs:
  - `REPO_DIR=/opt/garp bash scripts/deploy-vps.sh logs api-gateway-go`
- Stop stack:
  - `REPO_DIR=/opt/garp bash scripts/deploy-vps.sh down`

## Configuration

- Edit `.env`:
  - `DATABASE_URL`, `REDIS_URL`, `KAFKA_BROKERS` are pre-set for internal networking.
  - Logging rotation: `LOG_MAX_SIZE`, `LOG_MAX_FILE` control container log file size.
  - Node IDs and service URLs: `NODE_ID`, `PARTICIPANT_URL`, `SYNCHRONIZER_URL`, `BACKEND_URL`.

## Security Best Practices

- Secrets:
  - Never commit `.env`. Use long random strings for `POSTGRES_PASSWORD` and `JWT_SECRET`.
- Networking:
  - Only expose `frontend:80` and, if needed, `api-gateway-go:8080`.
  - Leave internal services without ports (reachable only within Docker networks).
- Container hardening:
  - `no-new-privileges`, `cap_drop: ALL` applied to app containers.
  - `read_only` filesystem for stateless services (frontend, api-gateway-go).
- OS hardening:
  - Keep the VPS updated: `sudo apt-get update && sudo apt-get upgrade -y`.
  - Use a firewall: allow `80/tcp` and `8080/tcp` (optional), deny others.

## Performance Tips

- Use SSD-backed storage for Postgres volume.
- Consider increasing PostgreSQL shared buffers and Kafka retention based on workload.
- Monitor logs with `docker compose logs -f <service>`.

## Troubleshooting

- Build failures:
  - Ensure Rust dependencies are aligned (`axum = "0.6"` across crates).
  - Reduce Docker build context (see `.dockerignore`).
- Compose errors:
  - Validate config: `docker compose -f docker-compose.yml -f docker-compose.prod.yml config`.
- Health checks:
  - If services restart repeatedly, check Postgres and Redis health; verify URLs in `.env`.
- Vulnerability scanning:
  - `bash scripts/vuln-scan.sh` to run Trivy scans on built images.

## Maintenance

- Regular backups: schedule `scripts/db-backup.sh` via cron.
- Updates: run `deploy-vps.sh upgrade` monthly or after releases.
- Prune old images: `docker image prune -f` after upgrades.