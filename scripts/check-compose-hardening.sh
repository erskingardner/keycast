#!/usr/bin/env bash
# Asserts the shipped Compose defaults still carry their hardening. Run by CI and
# usable directly after editing a Compose file.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Placeholders only; these render the files, they are never deployed.
export DOMAIN="${DOMAIN:-keycast.example}"
export ALLOWED_PUBKEYS="${ALLOWED_PUBKEYS:-$(printf '0%.0s' {1..63})1}"
export KEYCAST_OPERATOR_PUBKEYS="${KEYCAST_OPERATOR_PUBKEYS:-$ALLOWED_PUBKEYS}"
export KEYCAST_API_DIGEST="${KEYCAST_API_DIGEST:-sha256:$(printf '1%.0s' {1..64})}"
export KEYCAST_SIGNER_DIGEST="${KEYCAST_SIGNER_DIGEST:-sha256:$(printf '2%.0s' {1..64})}"
export KEYCAST_WEB_DIGEST="${KEYCAST_WEB_DIGEST:-sha256:$(printf '3%.0s' {1..64})}"

COMPOSE_FILES=(docker-compose.yml docker-compose.prod.yml caddy-docker-compose-example.yml)
# Production is what operators actually deploy, so it must carry the same
# per-service hardening as the source Compose file.
APPLICATION_COMPOSE_FILES=(docker-compose.yml docker-compose.prod.yml)
failures=0
fail() {
    failures=$((failures + 1))
    echo "[fail] $1"
}
ok() { echo "[ok] $1"; }

for file in "${COMPOSE_FILES[@]}"; do
    docker compose -f "$file" config --quiet || fail "$file does not render"
done
ok "all Compose files render"

for file in "${APPLICATION_COMPOSE_FILES[@]}"; do
    rendered="$(docker compose -f "$file" config)"

    # Swap must be disabled wherever plaintext key material can live, and no
    # service may run with a writable root filesystem.
    for setting in memswap_limit read_only; do
        if [[ "$(printf '%s' "$rendered" | grep -c "$setting")" -ge 3 ]]; then
            ok "$file: $setting is set on every service"
        else
            fail "$file: $setting is missing from at least one service"
        fi
    done

    if printf '%s' "$rendered" | grep -q 'noexec,nosuid,nodev'; then
        ok "$file: tmpfs mounts are noexec, nosuid and nodev"
    else
        fail "$file: tmpfs mounts are not hardened"
    fi

    # The signer holds the keys and needs no inbound path, so it must not sit on
    # the ingress network beside the public transport.
    if printf '%s' "$rendered" | grep -q 'keycast-signer-egress'; then
        ok "$file: the signer uses its own egress network"
    else
        fail "$file: the signer egress network is missing"
    fi
done

# Inspect rendered mounts, never file text: the proxy example documents the
# removal of the socket mount in a comment that a text search would match.
for file in "${COMPOSE_FILES[@]}"; do
    if docker compose -f "$file" config --format json |
        python3 -c '
import json, sys

config = json.load(sys.stdin)
mounts = []
for name, service in config.get("services", {}).items():
    for volume in service.get("volumes", []):
        if isinstance(volume, dict):
            mount = "{}:{}".format(volume.get("source", ""), volume.get("target", ""))
        else:
            mount = str(volume)
        if "docker.sock" in mount:
            mounts.append("{} -> {}".format(name, mount))
if mounts:
    print("; ".join(mounts))
    sys.exit(1)
'; then
        ok "$file mounts no Docker socket"
    else
        fail "$file mounts the Docker socket"
    fi
done

# A denylist silently misses nested paths such as legacy-v1/master.key.
if grep -qx '\*' .dockerignore; then
    ok ".dockerignore is allowlist form"
else
    fail ".dockerignore is not allowlist form"
fi

if [[ "$failures" -gt 0 ]]; then
    echo "Compose hardening checks failed with $failures issue(s)."
    exit 1
fi
echo "Compose hardening checks passed."
