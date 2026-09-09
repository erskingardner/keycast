#!/usr/bin/env bash
set -euo pipefail
# Uses only newly created, disposable volumes and containers. Images must already be built.
smoke_id="keycast-hardening-smoke-$$"
smoke_net="$smoke_id-network"
cleanup() {
  docker rm -f "$smoke_id-signer" "$smoke_id-api" "$smoke_id-web" >/dev/null 2>&1 || true
  docker network rm "$smoke_net" >/dev/null 2>&1 || true
  docker volume rm "$smoke_id-db" "$smoke_id-runtime" "$smoke_id-root" >/dev/null 2>&1 || true
}
trap cleanup EXIT
for volume in db runtime root; do docker volume create "$smoke_id-$volume" >/dev/null; done
docker network create "$smoke_net" >/dev/null
operator=$(docker run --rm --entrypoint node keycast-hardening-web:local --input-type=module -e "import {getPublicKey} from 'nostr-tools/pure';console.log(getPublicKey(new Uint8Array(32).fill(11)))")
docker run --rm --user 0 --entrypoint node -v "$smoke_id-root:/credential" keycast-hardening-web:local -e "const fs=require('fs');fs.writeFileSync('/credential/root.key',require('crypto').randomBytes(32).toString('hex'),{mode:0o600});fs.chownSync('/credential/root.key',10001,10001)"
common=(--detach --read-only --user 10001:10001 --cap-drop ALL --security-opt no-new-privileges --pids-limit 128 --memory 512m --ulimit core=0 --tmpfs /tmp:size=64m --network "$smoke_net")
docker run "${common[@]}" --name "$smoke_id-signer" --network-alias signer -v "$smoke_id-db:/app/database" -v "$smoke_id-runtime:/run/keycast" -v "$smoke_id-root:/run/secrets:ro" -e KEYCAST_ROOT_KEY_FILE=/run/secrets/root.key -e KEYCAST_PUBLIC_URL=https://keycast.test/api -e ALLOWED_PUBKEYS="$operator" -e KEYCAST_OPERATOR_PUBKEYS="$operator" keycast-hardening-signer:local >/dev/null
docker run "${common[@]}" --name "$smoke_id-api" --network-alias api -v "$smoke_id-runtime:/run/keycast:ro" keycast-hardening-api:local >/dev/null
docker run "${common[@]}" --name "$smoke_id-web" --network-alias web -e ORIGIN=https://keycast.test keycast-hardening-web:local >/dev/null
for _ in $(seq 1 30); do
  if docker exec "$smoke_id-signer" /app/keycast_signer healthcheck >/dev/null 2>&1 && docker exec "$smoke_id-api" /app/keycast_api healthcheck >/dev/null 2>&1; then break; fi
  sleep 1
done
docker exec "$smoke_id-api" sh -c 'test ! -e /run/secrets/root.key && test ! -e /app/database/keycast-v2.db'
docker run --rm -i --read-only -v "$smoke_id-runtime:/run/keycast:ro" --cap-drop ALL --security-opt no-new-privileges --network "$smoke_net" --entrypoint node keycast-hardening-web:local --input-type=module < "$(dirname "$0")/management-smoke.mjs"
