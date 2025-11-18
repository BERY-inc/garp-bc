#!/usr/bin/env bash
set -euo pipefail

# Simple helper to set up Docker and run the stack on a VPS.
# Tested on Ubuntu 22.04+. Run as a regular user with sudo privileges.

REPO_DIR=${REPO_DIR:-/opt/garp}

echo "[1/5] Installing Docker and Compose plugin"
sudo apt-get update -y
sudo apt-get install -y ca-certificates curl gnupg
sudo install -m 0755 -d /etc/apt/keyrings || true
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu $(. /etc/os-release && echo $VERSION_CODENAME) stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
sudo apt-get update -y
sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
sudo usermod -aG docker "$USER"

echo "[2/5] Ensuring repository directory exists at $REPO_DIR"
sudo mkdir -p "$REPO_DIR"
sudo chown -R "$USER":"$USER" "$REPO_DIR"

echo "[3/5] Using existing project files in $REPO_DIR"
cd "$REPO_DIR"

if [ ! -f .env ]; then
  echo "[info] No .env found; creating from .env.example"
  cp .env.example .env
  echo "[warn] Please edit .env with strong secrets before production use."
fi

echo "[4/5] Building and starting containers"
docker compose -f docker-compose.yml -f docker-compose.prod.yml --env-file .env up -d --build

echo "[5/5] Done. Showing status:"
docker compose ps
echo "Logs (frontend):"
docker compose logs -n 50 frontend || true

echo "Access the frontend on: http://$(curl -s ifconfig.me 2>/dev/null || echo localhost)/"