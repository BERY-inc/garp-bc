#!/usr/bin/env bash
set -euo pipefail

# Restore Postgres logical backup created by db-backup.sh
# Usage: scripts/db-restore.sh </path/to/pgdump.sql>

if [ $# -lt 1 ]; then
  echo "Usage: $0 </path/to/pgdump.sql>"
  exit 1
fi

BACKUP_FILE=$1
if [ ! -f "$BACKUP_FILE" ]; then
  echo "[restore] File not found: $BACKUP_FILE"
  exit 1
fi

echo "[restore] Restoring from $BACKUP_FILE"
docker compose exec -T postgres bash -lc 'export PGPASSWORD=${POSTGRES_PASSWORD}; psql -U ${POSTGRES_USER} -f -' < "$BACKUP_FILE"
echo "[restore] Done"