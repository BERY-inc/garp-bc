#!/usr/bin/env bash
set -euo pipefail

# Rolling upgrade for services with minimal downtime.
# Rebuild images, then restart services sequentially.

STACK_FILES=("docker-compose.yml" "docker-compose.prod.yml")

echo "[upgrade] Pulling latest repo changes (if any)"
if git rev-parse --git-dir > /dev/null 2>&1; then
  git fetch --all --tags || true
  git pull --rebase || true
fi

echo "[upgrade] Building images"
docker compose -f ${STACK_FILES[@]} --env-file .env build

SERVICES=(
  api-gateway-go
  frontend
  backend-go
  participant-node
  sync-domain
  global-synchronizer
)

echo "[upgrade] Restarting services sequentially"
for svc in "${SERVICES[@]}"; do
  echo "[upgrade] Restarting $svc"
  docker compose -f ${STACK_FILES[@]} --env-file .env up -d --no-deps --build "$svc"
  # brief health wait
  sleep 5
done

echo "[upgrade] Pruning old images"
docker image prune -f
echo "[upgrade] Done"