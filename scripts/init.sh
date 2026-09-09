#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DOMAIN=""
ALLOWED_PUBKEYS=""
OPERATOR_PUBKEYS=""
KEYCAST_UID=10001
KEYCAST_GID=10001
STATE_DIR_ARG=""

usage() {
    echo "Usage: $0 --domain <domain> --allowed-pubkeys <hex[,hex...]> [--operator-pubkeys <hex[,hex...]>] [--state-dir <path>]"
    echo "  --state-dir  Where the database and root credential live. Defaults to this checkout."
}

while [[ "$#" -gt 0 ]]; do
    case "$1" in
        --domain) DOMAIN="${2:-}"; shift 2 ;;
        --allowed-pubkeys) ALLOWED_PUBKEYS="${2:-}"; shift 2 ;;
        --operator-pubkeys) OPERATOR_PUBKEYS="${2:-}"; shift 2 ;;
        --state-dir) STATE_DIR_ARG="${2:-}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown argument: $1"; usage; exit 1 ;;
    esac
done

DOMAIN="${DOMAIN#http://}"
DOMAIN="${DOMAIN#https://}"
DOMAIN="${DOMAIN%%/*}"
if [[ -z "$DOMAIN" || "$DOMAIN" =~ [^A-Za-z0-9.-] ]]; then
    echo "Error: provide a valid hostname with --domain"
    exit 1
fi
if [[ -z "$ALLOWED_PUBKEYS" ]]; then
    echo "Error: --allowed-pubkeys is required; Keycast fails closed without it"
    exit 1
fi
OPERATOR_PUBKEYS="${OPERATOR_PUBKEYS:-${ALLOWED_PUBKEYS%%,*}}"
for pubkey_list in "$ALLOWED_PUBKEYS" "$OPERATOR_PUBKEYS"; do
    if [[ ! "$pubkey_list" =~ ^[0-9a-fA-F]{64}(,[0-9a-fA-F]{64})*$ ]]; then
        echo "Error: pubkeys must be comma-separated 64-character hex values without whitespace or empty fields"
        exit 1
    fi
done

cd "$ROOT_DIR"
if [[ -e .env ]]; then
    echo "Error: .env already exists; refusing to overwrite it"
    exit 1
fi
# Must match ${KEYCAST_STATE_DIR:-.} in the Compose files.
if [[ -z "$STATE_DIR_ARG" ]]; then
    STATE_DIR="$ROOT_DIR"
elif [[ "$STATE_DIR_ARG" = /* ]]; then
    STATE_DIR="$STATE_DIR_ARG"
else
    STATE_DIR="$ROOT_DIR/$STATE_DIR_ARG"
fi
DATABASE_DIR="$STATE_DIR/database"
ROOT_KEY="$STATE_DIR/master.key"
mkdir -p "$STATE_DIR"
if [[ ! -f "$ROOT_KEY" ]]; then
    KEYCAST_ROOT_KEY_PATH="$ROOT_KEY" bash "$ROOT_DIR/scripts/generate_key.sh"
fi
mkdir -p "$DATABASE_DIR"
chmod 700 "$DATABASE_DIR"
chmod 600 "$ROOT_KEY"
if ! chown -R "$KEYCAST_UID:$KEYCAST_GID" "$DATABASE_DIR" "$ROOT_KEY" 2>/dev/null; then
    echo "Warning: could not set container ownership."
    echo "Run: sudo chown -R $KEYCAST_UID:$KEYCAST_GID $DATABASE_DIR $ROOT_KEY"
fi

{
    echo "DOMAIN=$DOMAIN"
    if [[ "$STATE_DIR" != "$ROOT_DIR" ]]; then
        echo "KEYCAST_STATE_DIR=$STATE_DIR"
    fi
    echo "ALLOWED_PUBKEYS=$ALLOWED_PUBKEYS"
    echo "KEYCAST_OPERATOR_PUBKEYS=$OPERATOR_PUBKEYS"
    echo "KEYCAST_API_IMAGE=ghcr.io/marmot-protocol/keycast-api"
    echo "KEYCAST_SIGNER_IMAGE=ghcr.io/marmot-protocol/keycast-signer"
    echo "KEYCAST_WEB_IMAGE=ghcr.io/marmot-protocol/keycast-web"
    echo "KEYCAST_API_DIGEST="
    echo "KEYCAST_SIGNER_DIGEST="
    echo "KEYCAST_WEB_DIGEST="
} > .env
chmod 600 .env

if command -v docker >/dev/null 2>&1; then
    # Internal: keycast-api and keycast-web get no route off-host. The signer uses
    # its own egress network and the reverse proxy keeps a public one.
    if ! docker network inspect keycast >/dev/null 2>&1; then
        docker network create --internal keycast >/dev/null
    elif [ "$(docker network inspect keycast --format '{{.Internal}}' 2>/dev/null)" != "true" ]; then
        echo "Warning: the existing 'keycast' network is not internal."
        echo "To isolate the API and web containers, recreate it after stopping the stack:"
        echo "  docker compose -f docker-compose.prod.yml down"
        echo "  docker network rm keycast && docker network create --internal keycast"
    fi
fi

echo "Keycast v2 initialized for $DOMAIN."
echo "The old database/keycast.db, old bunker URLs, and old invitations are intentionally ignored."
echo "Start with: docker compose up -d --build"
