#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB_PATH="$ROOT_DIR/database/keycast.db"
MIGRATIONS_DIR="$ROOT_DIR/database/migrations"
UPGRADE_MIGRATION="$MIGRATIONS_DIR/0002_normalize_allowed_kinds_permissions.sql"
MASTER_KEY_PATH="$ROOT_DIR/master.key"
FIX_PERMISSIONS=false

usage() {
    echo "Usage: $0 [--fix-permissions]"
    echo ""
    echo "Checks a live Keycast checkout before deploying the hardened runtime."
    echo "Use --fix-permissions to recursively chown database/ and master.key for KEYCAST_UID/GID."
}

while [[ "$#" -gt 0 ]]; do
    case "$1" in
        --fix-permissions) FIX_PERMISSIONS=true ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown parameter: $1"; usage; exit 1 ;;
    esac
    shift
done

env_value() {
    local key="$1"
    if [ -f "$ROOT_DIR/.env" ]; then
        grep -E "^${key}=" "$ROOT_DIR/.env" | tail -n 1 | cut -d= -f2- || true
    fi
}

KEYCAST_UID="${KEYCAST_UID:-$(env_value KEYCAST_UID)}"
KEYCAST_GID="${KEYCAST_GID:-$(env_value KEYCAST_GID)}"
ALLOWED_PUBKEYS="${ALLOWED_PUBKEYS:-$(env_value ALLOWED_PUBKEYS)}"
DOMAIN="${DOMAIN:-$(env_value DOMAIN)}"
KEYCAST_UID="${KEYCAST_UID:-10001}"
KEYCAST_GID="${KEYCAST_GID:-10001}"

failures=0

ok() {
    echo "[ok] $1"
}

warn() {
    echo "[warn] $1"
}

fail() {
    failures=$((failures + 1))
    echo "[fail] $1"
}

echo "Keycast upgrade preflight"
echo "Repo: $ROOT_DIR"
echo "Runtime uid/gid: $KEYCAST_UID:$KEYCAST_GID"

if [ -n "$DOMAIN" ]; then
    ok "DOMAIN is set"
else
    fail "DOMAIN is not set in environment or .env"
fi

if [ -n "$ALLOWED_PUBKEYS" ]; then
    ok "ALLOWED_PUBKEYS is set"
else
    fail "ALLOWED_PUBKEYS is required by the hardened API container"
fi

if [ -f "$MASTER_KEY_PATH" ]; then
    key_chars=$(tr -d '\r\n' < "$MASTER_KEY_PATH" | wc -c | tr -d ' ')
    if [ "$key_chars" -ge 40 ]; then
        ok "master.key exists and looks like a base64-encoded 256-bit key"
    else
        fail "master.key exists but is unexpectedly short"
    fi
else
    fail "master.key is missing; do not generate a new one for an existing install"
    echo "       If the old container has the key baked in, recover it first:"
    echo "       docker cp keycast-api:/app/master.key ./master.key"
fi

if command -v docker >/dev/null 2>&1 && [ -f "$MASTER_KEY_PATH" ]; then
    for container in keycast-api keycast-signer keycast-web; do
        if docker ps --format '{{.Names}}' | grep -Fxq "$container"; then
            tmp_key="$(mktemp)"
            if docker cp "$container:/app/master.key" "$tmp_key" >/dev/null 2>&1; then
                if cmp -s "$MASTER_KEY_PATH" "$tmp_key"; then
                    ok "host master.key matches $container:/app/master.key"
                else
                    fail "host master.key differs from $container:/app/master.key"
                fi
            fi
            rm -f "$tmp_key"
            break
        fi
    done
fi

if [ -f "$DB_PATH" ]; then
    ok "database/keycast.db exists"
else
    fail "database/keycast.db is missing"
fi

if [ -f "$UPGRADE_MIGRATION" ]; then
    ok "database migrations include 0002 upgrade migration"
else
    fail "database migrations are missing 0002_normalize_allowed_kinds_permissions.sql"
fi

if [ "$FIX_PERMISSIONS" = true ]; then
    if [ -d "$ROOT_DIR/database" ] && [ -f "$MASTER_KEY_PATH" ]; then
        chmod 700 "$ROOT_DIR/database"
        chmod 600 "$MASTER_KEY_PATH"
        if chown -R "$KEYCAST_UID:$KEYCAST_GID" "$ROOT_DIR/database" 2>/dev/null \
            && chown "$KEYCAST_UID:$KEYCAST_GID" "$MASTER_KEY_PATH" 2>/dev/null; then
            ok "set runtime ownership on database/ and master.key"
        else
            fail "could not set runtime ownership; run with sudo or run: sudo chown -R $KEYCAST_UID:$KEYCAST_GID database master.key"
        fi
    else
        fail "cannot fix permissions until database/ and master.key both exist"
    fi
else
    warn "permission check is read-only; use --fix-permissions before starting non-root containers"
fi

if command -v sqlite3 >/dev/null 2>&1 && [ -f "$DB_PATH" ]; then
    integrity="$(sqlite3 "$DB_PATH" 'PRAGMA integrity_check;' 2>/dev/null || true)"
    if [ "$integrity" = "ok" ]; then
        ok "SQLite integrity_check is ok"
    else
        fail "SQLite integrity_check failed: $integrity"
    fi

    fk_rows="$(sqlite3 "$DB_PATH" 'PRAGMA foreign_key_check;' 2>/dev/null || true)"
    if [ -z "$fk_rows" ]; then
        ok "SQLite foreign_key_check is clean"
    else
        fail "SQLite foreign_key_check reported rows:"
        echo "$fk_rows"
    fi

    legacy_allowed_kinds="$(sqlite3 "$DB_PATH" "
        SELECT COUNT(*)
        FROM permissions
        WHERE identifier = 'allowed_kinds'
          AND json_valid(config)
          AND json_type(config, '$.allowed_kinds') IS NULL
          AND json_type(config, '$.sign') IN ('array', 'null');
    " 2>/dev/null || echo "unknown")"
    if [ "$legacy_allowed_kinds" = "0" ]; then
        ok "no legacy allowed_kinds configs detected"
    elif [ "$legacy_allowed_kinds" = "unknown" ]; then
        warn "could not inspect legacy allowed_kinds configs"
    else
        warn "$legacy_allowed_kinds legacy allowed_kinds config(s) will be normalized by migration 0002"
    fi

    invalid_permissions="$(sqlite3 "$DB_PATH" "
        SELECT COUNT(*)
        FROM permissions
        WHERE NOT json_valid(config);
    " 2>/dev/null || echo "unknown")"
    if [ "$invalid_permissions" = "0" ]; then
        ok "permission configs are valid JSON"
    elif [ "$invalid_permissions" = "unknown" ]; then
        warn "could not inspect permission JSON validity"
    else
        fail "$invalid_permissions permission config row(s) contain invalid JSON"
    fi
else
    warn "sqlite3 is not installed; skipping integrity, foreign-key, and permission-config checks"
fi

if [ "$failures" -gt 0 ]; then
    echo "Preflight failed with $failures blocking issue(s)."
    exit 1
fi

echo "Preflight passed."
