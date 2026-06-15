#!/usr/bin/env bash
# ============================================================================
# AURELIUM — Database Migration Runner
# Runs all SQL migrations in order against the target PostgreSQL database
# ============================================================================

set -euo pipefail

DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"
DB_USER="${DB_USER:-aurelium}"
DB_PASSWORD="${DB_PASSWORD:-aurelium}"
DB_NAME="${DB_NAME:-aurelium}"

MIGRATION_DIR="$(cd "$(dirname "$0")" && pwd)/migrations"

echo "============================================"
echo " AURELIUM — Migration Runner"
echo "============================================"
echo " Host:     $DB_HOST:$DB_PORT"
echo " Database: $DB_NAME"
echo " User:     $DB_USER"
echo " Dir:      $MIGRATION_DIR"
echo "============================================"

# Check if psql is available
if ! command -v psql &> /dev/null; then
    echo "ERROR: psql is not installed. Please install PostgreSQL client."
    exit 1
fi

# Create tracking table if it doesn't exist
export PGPASSWORD="$DB_PASSWORD"
psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -q <<SQL
CREATE TABLE IF NOT EXISTS _migrations (
    filename VARCHAR(255) PRIMARY KEY,
    applied_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
SQL

echo ""
echo "Running migrations..."
echo ""

APPLIED=0
SKIPPED=0

# Process migration files in order
for migration_file in "$MIGRATION_DIR"/*.sql; do
    filename=$(basename "$migration_file")

    # Check if already applied
    already_applied=$(psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -A \
        -c "SELECT COUNT(*) FROM _migrations WHERE filename = '$filename'")

    if [ "$already_applied" -gt 0 ]; then
        echo "  SKIP  $filename (already applied)"
        SKIPPED=$((SKIPPED + 1))
    else
        echo "  APPLY $filename ..."
        psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -q -f "$migration_file"
        psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -q \
            -c "INSERT INTO _migrations (filename) VALUES ('$filename')"
        echo "  DONE  $filename"
        APPLIED=$((APPLIED + 1))
    fi
done

echo ""
echo "============================================"
echo " Migration complete"
echo " Applied: $APPLIED | Skipped: $SKIPPED"
echo "============================================"
