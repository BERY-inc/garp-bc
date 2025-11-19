#!/usr/bin/env bash
set -euo pipefail

# Postgres logical backup using pg_dumpall from the running container.
# Usage: scripts/db-backup.sh [output_dir]

OUTPUT_DIR=${1:-/var/backups/garp}
FILE_NAME="pgdump-$(date +%Y%m%d-%H%M%S).sql"
mkdir -p "$OUTPUT_DIR"

echo "[backup] Creating backup at $OUTPUT_DIR/$FILE_NAME"
docker compose exec -T postgres bash -lc 'export PGPASSWORD=${POSTGRES_PASSWORD}; pg_dumpall -U ${POSTGRES_USER}' > "$OUTPUT_DIR/$FILE_NAME"

echo "[backup] Completed: $OUTPUT_DIR/$FILE_NAME"