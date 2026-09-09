#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIX_PERMISSIONS=false

usage() {
    echo "Usage: $0 [--fix-permissions]"
    echo "Validates a clean Keycast v2 deployment. Legacy database contents are not migrated."
}

while [[ "$#" -gt 0 ]]; do
    case "$1" in
        --fix-permissions) FIX_PERMISSIONS=true ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown argument: $1"; usage; exit 1 ;;
    esac
    shift
done

env_value() {
    local key="$1"
    if [[ -f "$ROOT_DIR/.env" ]]; then
        awk -F= -v key="$key" '$1 == key {sub(/^[^=]*=/, ""); print; exit}' "$ROOT_DIR/.env"
    fi
}

ALLOWED_PUBKEYS="${ALLOWED_PUBKEYS:-$(env_value ALLOWED_PUBKEYS)}"
DOMAIN="${DOMAIN:-$(env_value DOMAIN)}"
KEYCAST_OPERATOR_PUBKEYS="${KEYCAST_OPERATOR_PUBKEYS:-$(env_value KEYCAST_OPERATOR_PUBKEYS)}"
KEYCAST_UID=10001
KEYCAST_GID=10001
failures=0

ok() { echo "[ok] $1"; }
warn() { echo "[warn] $1"; }
fail() { failures=$((failures + 1)); echo "[fail] $1"; }

if [[ -n "$DOMAIN" && ! "$DOMAIN" =~ [^A-Za-z0-9.-] ]]; then
    ok "DOMAIN is a hostname"
else
    fail "DOMAIN is missing or is not a hostname"
fi
for pubkey_name in ALLOWED_PUBKEYS KEYCAST_OPERATOR_PUBKEYS; do
    if [[ "${!pubkey_name}" =~ ^[0-9a-fA-F]{64}(,[0-9a-fA-F]{64})*$ ]]; then
        ok "$pubkey_name contains valid hex pubkeys"
    else
        fail "$pubkey_name must contain comma-separated 64-character hex pubkeys without whitespace or empty fields"
    fi
done

if [[ -f "$ROOT_DIR/master.key" ]]; then
    key_value="$(tr -d '\r\n' < "$ROOT_DIR/master.key")"
    [[ "$key_value" =~ ^[0-9a-fA-F]{64}$ || "$key_value" =~ ^[A-Za-z0-9+/]{43}=$ ]] \
        && ok "master.key encodes exactly 32 bytes" \
        || fail "master.key must contain one base64 or hex encoded 32-byte key"
else
    fail "master.key is missing"
fi

for directory in database; do
    [[ -d "$ROOT_DIR/$directory" ]] && ok "$directory directory exists" || fail "$directory directory is missing"
done

if [[ -f "$ROOT_DIR/database/keycast.db" ]]; then
    warn "database/keycast.db is legacy and will be ignored by v2"
fi
# Read-only, and before the ownership repair below: opening a WAL database
# read-write creates -wal/-shm sidecars, and an interrupted run would leave them
# owned by root where the container user (10001) could not open the database.
if [[ -f "$ROOT_DIR/database/keycast-v2.db" ]] && command -v sqlite3 >/dev/null 2>&1; then
    db_uri="file:$ROOT_DIR/database/keycast-v2.db?mode=ro"
    integrity="$(sqlite3 -readonly "$db_uri" 'PRAGMA quick_check;' 2>/dev/null || true)"
    [[ "$integrity" == "ok" ]] && ok "v2 database quick_check passed" || fail "v2 database quick_check failed"
    schema="$(sqlite3 -readonly "$db_uri" 'PRAGMA user_version;' 2>/dev/null || true)"
    [[ "$schema" == "2" ]] && ok "v2 schema version is 2" || fail "unexpected v2 schema version: $schema"
    for sidecar in wal shm; do
        if [[ -e "$ROOT_DIR/database/keycast-v2.db-$sidecar" ]] \
            && [[ "$(stat -f '%u' "$ROOT_DIR/database/keycast-v2.db-$sidecar" 2>/dev/null \
                || stat -c '%u' "$ROOT_DIR/database/keycast-v2.db-$sidecar" 2>/dev/null)" != "$KEYCAST_UID" ]]; then
            warn "database/keycast-v2.db-$sidecar is not owned by $KEYCAST_UID; --fix-permissions will repair it"
        fi
    done
else
    warn "no v2 database yet; the signer will create it on first start"
fi

if [[ "$FIX_PERMISSIONS" == true && -f "$ROOT_DIR/master.key" ]]; then
    chmod 700 "$ROOT_DIR/database"
    chmod 600 "$ROOT_DIR/master.key"
    if chown -R "$KEYCAST_UID:$KEYCAST_GID" "$ROOT_DIR/database" "$ROOT_DIR/master.key" 2>/dev/null; then
        ok "container ownership and permissions updated"
    else
        fail "could not chown runtime files; retry with sudo"
    fi
else
    warn "permission repair not requested; use --fix-permissions before first container start"
fi

if [[ -z "$KEYCAST_OPERATOR_PUBKEYS" ]]; then
    fail "KEYCAST_OPERATOR_PUBKEYS is required for global relay management"
fi
ATTESTATION_REPO="${KEYCAST_ATTESTATION_REPO:-marmot-protocol/keycast}"
for component in API SIGNER WEB; do
    digest_name="KEYCAST_${component}_DIGEST"
    digest_value="${!digest_name:-$(env_value "$digest_name")}"
    if [[ ! "$digest_value" =~ ^sha256:[0-9a-f]{64}$ ]]; then
        fail "$digest_name must be a sha256 image digest"
        continue
    fi
    image_name="KEYCAST_${component}_IMAGE"
    image_value="${!image_name:-$(env_value "$image_name")}"
    image_value="${image_value:-ghcr.io/marmot-protocol/keycast-$(echo "$component" | tr 'A-Z' 'a-z')}"
    # A digest only pins what you already trust. Verify it was produced by this
    # repository's workflow instead of trusting a mutable tag it was read from.
    if command -v gh >/dev/null 2>&1; then
        if gh attestation verify "oci://${image_value}@${digest_value}" \
            --repo "$ATTESTATION_REPO" >/dev/null 2>&1; then
            ok "$component image digest has verified build provenance"
        else
            fail "$component image digest has no verifiable provenance from $ATTESTATION_REPO"
        fi
    else
        warn "gh is not installed; install it to verify $component build provenance with: gh attestation verify oci://${image_value}@${digest_value} --repo $ATTESTATION_REPO"
    fi
done
if command -v docker >/dev/null 2>&1; then
    if docker network inspect keycast >/dev/null 2>&1; then
        if [[ "$(docker network inspect keycast --format '{{.Internal}}' 2>/dev/null)" == "true" ]]; then
            ok "the keycast network is internal"
        else
            warn "the keycast network is not internal; api and web can reach the Internet. Recreate it with: docker network rm keycast && docker network create --internal keycast"
        fi
    else
        fail "the external 'keycast' network is missing; create it with: docker network create --internal keycast"
    fi
    if KEYCAST_OPERATOR_PUBKEYS="$KEYCAST_OPERATOR_PUBKEYS" DOMAIN="$DOMAIN" ALLOWED_PUBKEYS="$ALLOWED_PUBKEYS" \
        docker compose -f "$ROOT_DIR/docker-compose.prod.yml" config --quiet; then
        ok "Docker Compose configuration renders"
    else
        fail "Docker Compose configuration does not render"
    fi
fi

if [[ "$failures" -gt 0 ]]; then
    echo "Preflight failed with $failures blocking issue(s)."
    exit 1
fi
echo "Preflight passed."
