#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DOMAIN=""
ALLOWED_PUBKEYS=""
KEYCAST_UID=10001
KEYCAST_GID=10001

usage() {
    echo "Usage: $0 --domain <domain> --allowed-pubkeys <hex[,hex...]>"
}

while [[ "$#" -gt 0 ]]; do
    case "$1" in
        --domain) DOMAIN="${2:-}"; shift 2 ;;
        --allowed-pubkeys) ALLOWED_PUBKEYS="${2:-}"; shift 2 ;;
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
IFS=',' read -r -a pubkeys <<< "$ALLOWED_PUBKEYS"
for pubkey in "${pubkeys[@]}"; do
    normalized="${pubkey//[[:space:]]/}"
    if [[ ! "$normalized" =~ ^[0-9a-fA-F]{64}$ ]]; then
        echo "Error: every allowed pubkey must be 64 hexadecimal characters"
        exit 1
    fi
done

cd "$ROOT_DIR"
if [[ -e .env ]]; then
    echo "Error: .env already exists; refusing to overwrite it"
    exit 1
fi
if [[ ! -f master.key ]]; then
    bash scripts/generate_key.sh
fi
mkdir -p database
chmod 700 database
chmod 600 master.key
if ! chown -R "$KEYCAST_UID:$KEYCAST_GID" database master.key 2>/dev/null; then
    echo "Warning: could not set container ownership."
    echo "Run: sudo chown -R $KEYCAST_UID:$KEYCAST_GID database master.key"
fi

{
    echo "DOMAIN=$DOMAIN"
    echo "ALLOWED_PUBKEYS=$ALLOWED_PUBKEYS"
    echo "KEYCAST_OPERATOR_PUBKEYS=$ALLOWED_PUBKEYS"
    echo "KEYCAST_API_IMAGE=ghcr.io/marmot-protocol/keycast-api"
    echo "KEYCAST_SIGNER_IMAGE=ghcr.io/marmot-protocol/keycast-signer"
    echo "KEYCAST_WEB_IMAGE=ghcr.io/marmot-protocol/keycast-web"
    echo "KEYCAST_API_DIGEST="
    echo "KEYCAST_SIGNER_DIGEST="
    echo "KEYCAST_WEB_DIGEST="
} > .env
chmod 600 .env

if command -v docker >/dev/null 2>&1; then
    docker network inspect keycast >/dev/null 2>&1 || docker network create keycast >/dev/null
fi

echo "Keycast v2 initialized for $DOMAIN."
echo "The old database/keycast.db, old bunker URLs, and old invitations are intentionally ignored."
echo "Start with: docker compose up -d --build"
