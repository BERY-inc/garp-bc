#!/usr/bin/env bash
set -euo pipefail

# Vulnerability scanning of images using Trivy.
# Installs Trivy if missing; scans built images for high/critical issues.

if ! command -v trivy >/dev/null 2>&1; then
  echo "[scan] Installing trivy"
  sudo apt-get update -y
  sudo apt-get install -y wget apt-transport-https gnupg lsb-release
  wget -qO - https://aquasecurity.github.io/trivy-repo/deb/public.key | sudo apt-key add -
  echo deb https://aquasecurity.github.io/trivy-repo/deb $(lsb_release -sc) main | sudo tee /etc/apt/sources.list.d/trivy.list
  sudo apt-get update -y && sudo apt-get install -y trivy
fi

STACK_FILES=("docker-compose.yml" "docker-compose.prod.yml")
echo "[scan] Building images before scan"
docker compose -f ${STACK_FILES[@]} --env-file .env build

IMAGES=(
  garp-api-gateway-go
  garp-frontend
  garp-backend-go
  garp-participant-node
  garp-sync-domain
  garp-global-synchronizer
)

echo "[scan] Scanning images"
for img in "${IMAGES[@]}"; do
  echo "[scan] $img"
  trivy image --severity HIGH,CRITICAL --ignore-unfixed "$img" || true
done

echo "[scan] Done"