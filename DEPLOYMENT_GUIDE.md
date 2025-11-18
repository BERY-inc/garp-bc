# GARP VPS Deployment Guide

This guide walks you through deploying the GARP stack on a Linux VPS using Docker Compose. It assumes Ubuntu 22.04+, a user with `sudo`, and optionally a domain name.

## Prerequisites
- A VPS (Ubuntu 22.04+ recommended), public IP, `ssh` access
- Open ports: `80` (HTTP). If adding TLS later, also `443` (HTTPS)
- Basic familiarity with SSH and Docker

## Install Docker and Compose
```bash
sudo apt-get update
sudo apt-get install -y ca-certificates curl gnupg
sudo install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu $(. /etc/os-release && echo $VERSION_CODENAME) stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
sudo apt-get update
sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
sudo usermod -aG docker $USER
newgrp docker
```

## Get the project onto the VPS
Option A: Clone from your Git remote
```bash
git clone <your-repo-url> /opt/garp
cd /opt/garp
```

Option B: Copy your local workspace
```bash
scp -r ./garp <user>@<vps-ip>:/opt/garp
ssh <user>@<vps-ip>
cd /opt/garp
```

## Configure environment
```bash
cp .env.example .env
```
Edit `.env` and set real secrets and settings:
- `POSTGRES_PASSWORD` set to a strong value
- `JWT_SECRET` set to a strong value
- Adjust other values if needed

## Build and start services
```bash
docker compose -f docker-compose.yml -f docker-compose.prod.yml --env-file .env up -d --build
```

This launches:
- `postgres`, `redis`, `zookeeper`, `kafka`
- `participant-node`, `sync-domain`, `global-synchronizer`
- `backend-go`, `api-gateway-go`, `frontend` (Nginx serving port `80`)

## Verify
```bash
docker compose ps
docker compose logs -f frontend
curl -I http://localhost
curl -I http://localhost/api/health || true
curl -I http://localhost/node/health || true
curl -I http://localhost/sync/health || true
```

If accessing from outside, use your VPS IP: `http://<vps-ip>/`.

## Optional: Attach a domain and TLS
If you have a domain, point DNS `A` record to your VPS IP. For automated TLS, consider Caddy or Nginx + Certbot. Example Caddyfile (runs separately from the stack):
```
your.domain.com {
    reverse_proxy 127.0.0.1:80
}
```
Start Caddy: `sudo apt install -y caddy && sudo systemctl enable --now caddy`.

## Maintenance
- Stop: `docker compose down`
- Update images: `docker compose pull && docker compose up -d`
- Rebuild after code changes: `docker compose up -d --build`
- Logs: `docker compose logs -f <service>`

## Notes
- Frontend Nginx proxies are aligned to compose services:
  - `/api/` -> `backend-go:8081`
  - `/node/` -> `participant-node:8090`
  - `/sync/` and `/bridge/` -> `global-synchronizer:8000`
- Ensure adequate VPS resources; building Rust/Go images can be CPU/RAM intensive.