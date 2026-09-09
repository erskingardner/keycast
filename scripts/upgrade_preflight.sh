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
# Must resolve exactly like ${KEYCAST_STATE_DIR:-.} in the Compose files, or this
# would check and repair a stale copy while the containers mount another one.
KEYCAST_STATE_DIR="${KEYCAST_STATE_DIR:-$(env_value KEYCAST_STATE_DIR)}"
if [[ -z "$KEYCAST_STATE_DIR" ]]; then
    STATE_DIR="$ROOT_DIR"
elif [[ "$KEYCAST_STATE_DIR" = /* ]]; then
    STATE_DIR="$KEYCAST_STATE_DIR"
else
    STATE_DIR="$ROOT_DIR/$KEYCAST_STATE_DIR"
fi
DATABASE_DIR="$STATE_DIR/database"
ROOT_KEY="$STATE_DIR/master.key"
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

if [[ "$STATE_DIR" != "$ROOT_DIR" ]]; then
    ok "state directory is $STATE_DIR"
fi
if [[ -f "$ROOT_KEY" ]]; then
    key_value="$(tr -d '\r\n' < "$ROOT_KEY")"
    if [[ "$key_value" =~ ^[0-9a-fA-F]{64}$ || "$key_value" =~ ^[A-Za-z0-9+/]{43}=$ ]]; then
        ok "master.key encodes exactly 32 bytes"
    else
        fail "master.key must contain one base64 or hex encoded 32-byte key"
    fi
else
    fail "$ROOT_KEY is missing"
fi

if [[ -d "$DATABASE_DIR" ]]; then
    ok "database directory exists"
else
    fail "$DATABASE_DIR is missing"
fi

if [[ -f "$DATABASE_DIR/keycast.db" ]]; then
    warn "keycast.db is legacy and will be ignored by v2"
fi
# Read-only, and before the ownership repair below: opening a WAL database
# read-write creates -wal/-shm sidecars, and an interrupted run would leave them
# owned by root where the container user (10001) could not open the database.
# GNU form first: on Linux `stat -f` means --file-system, which would print
# filesystem status to stdout before failing and corrupt the captured value.
file_owner() {
    stat -c '%u' "$1" 2>/dev/null || stat -f '%u' "$1" 2>/dev/null || true
}
if [[ -f "$DATABASE_DIR/keycast-v2.db" ]] && command -v sqlite3 >/dev/null 2>&1; then
    # `-readonly` is the read-only guarantee. A `file:` URI would be read as a
    # literal path wherever URI filenames are not enabled.
    database="$DATABASE_DIR/keycast-v2.db"
    integrity="$(sqlite3 -readonly "$database" 'PRAGMA quick_check;' 2>/dev/null || true)"
    if [[ "$integrity" == "ok" ]]; then
        ok "v2 database quick_check passed"
    else
        fail "v2 database quick_check failed"
    fi
    schema="$(sqlite3 -readonly "$database" 'PRAGMA user_version;' 2>/dev/null || true)"
    if [[ "$schema" == "2" ]]; then
        ok "v2 schema version is 2"
    else
        fail "unexpected v2 schema version: $schema"
    fi
    for sidecar in wal shm; do
        if [[ -e "$database-$sidecar" && "$(file_owner "$database-$sidecar")" != "$KEYCAST_UID" ]]; then
            warn "keycast-v2.db-$sidecar is not owned by $KEYCAST_UID; --fix-permissions will repair it"
        fi
    done
else
    warn "no v2 database yet; the signer will create it on first start"
fi

if [[ "$FIX_PERMISSIONS" == true && -f "$ROOT_KEY" ]]; then
    chmod 700 "$DATABASE_DIR"
    chmod 600 "$ROOT_KEY"
    if chown -R "$KEYCAST_UID:$KEYCAST_GID" "$DATABASE_DIR" "$ROOT_KEY" 2>/dev/null; then
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
        # Always blocking. Carry the diagnostic so an authentication or network
        # failure is not reported as if the image simply had no provenance.
        if verify_output="$(gh attestation verify "oci://${image_value}@${digest_value}" \
            --repo "$ATTESTATION_REPO" 2>&1)"; then
            ok "$component image digest has verified build provenance"
        else
            fail "$component provenance verification failed against $ATTESTATION_REPO: $(printf '%s' "$verify_output" | tr '\n' ' ' | cut -c1-300)"
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
