# Upgrade to Keycast V2

Keycast v2 is a clean replacement, not an in-place database migration. The old database, stored-key
ciphertext, authorizations, invitation/redemption state, secrets, and bunker URLs are not loaded.
Plan a short maintenance window, re-import every key, and reconnect every client.

## Before the Window

1. Record the instance operator pubkeys and hostname.
2. Record which keys, teams, policies, and clients must be recreated without copying private
   material into notes.
3. Optionally archive `database/keycast.db` and the old `master.key` for rollback to V1. They are
   not needed by V2.
4. Make sure the external `keycast` Docker network and reverse proxy are available. The network is
   now created with `--internal`; see "Hardening migration" below if yours already exists.

## Replace V1

Stop the old deployment:

~~~sh
sudo docker compose down
~~~

Keep any legacy files as an offline archive if desired. Move them **outside** the checkout: a root
key inside the repository would be copied into the Docker build context by a source build.

~~~sh
sudo install -d -m 0700 /srv/keycast-legacy-v1
sudo mv database/keycast.db* /srv/keycast-legacy-v1/ 2>/dev/null || true
sudo mv master.key /srv/keycast-legacy-v1/master.key 2>/dev/null || true
~~~

Create the v2 configuration and a fresh root credential:

~~~sh
bash scripts/init.sh \
  --domain keycast.example.com \
  --allowed-pubkeys "64-character-hex-pubkey[,another-pubkey]"
sudo scripts/upgrade_preflight.sh --fix-permissions
~~~

If an existing `.env` prevented initialization, update it deliberately from `.env.example`, run
`bun run key:generate` only when `master.key` is absent, create `database/`, and assign both to
UID/GID 10001:

~~~sh
sudo chown -R 10001:10001 database master.key
chmod 700 database
chmod 600 master.key
~~~

Set `KEYCAST_OPERATOR_PUBKEYS` and the three image digests in `.env`, then start the v2 images:

~~~sh
sudo docker compose -f docker-compose.prod.yml pull
sudo docker compose -f docker-compose.prod.yml up -d
sudo docker compose -f docker-compose.prod.yml ps
~~~

The signer should become healthy immediately on a blank database. The API then becomes healthy and
the web process starts. Sign in with an allowed operator pubkey, open **Status**, and verify schema
version 2 and the expected relay configuration.

## Recreate Access

1. Recreate teams and membership.
2. Import each managed key.
3. Create explicit policies.
4. Create grants and copy each newly displayed bunker URL once.
5. Connect each client with its new one-time invitation.
6. Verify at least one allowed and one denied request per policy.
7. Confirm recent receive/publish timestamps and zero unexpected denial/parse counters on Status.

Every V1 bunker URL is intentionally invalid. Do not try to preserve or transform its secret.

## Rollback

V1 rollback requires the archived V1 database and its matching old root key. Stop v2 before
restoring them. V2 changes made after the cutover do not exist in V1 and cannot be converted back.
Do not mix a database with a different root credential.

## Hardening migration

These changes tighten the shipped defaults and need one deliberate step each on an existing
deployment.

**The `keycast` network is now internal.** The API and web containers no longer have a route off
the host; the signer gets its own egress network. Recreate the network once:

~~~sh
sudo docker compose -f docker-compose.prod.yml down
sudo docker compose -f caddy-docker-compose-example.yml down
sudo docker network rm keycast
sudo docker network create --internal keycast
~~~

**The reverse proxy no longer needs the Docker socket.** The previous example mounted
`/var/run/docker.sock` into Caddy, which is root-equivalent on the host: read-only applies to the
socket file, not to the Docker API commands sent over it. The proxy now uses a static file, and the
`caddy=` labels have been removed from both Compose files.

~~~sh
cp Caddyfile.example Caddyfile   # review it, then
sudo docker compose -f caddy-docker-compose-example.yml up -d
~~~

**Management writes now pin a reply identity.** The signer derives a stable reply keypair from the
root credential and publishes its public half through `/api/config`. Your browser pins it on first
use. After a root-credential rotation the identity changes, so the Status page shows a warning with
both fingerprints and a button to trust the new one. Compare it against the host first:

~~~sh
sudo docker compose -f docker-compose.prod.yml exec -T keycast-signer \
  /app/keycast_signer status | python3 -m json.tool | grep management_reply
~~~

**Swap is now disabled for the containers.** `memswap_limit` matches `mem_limit` so decrypted key
material cannot be paged to host swap. No action needed, but confirm the host has enough RAM.

## Future V2 Upgrades

Once on v2, preserve `database/keycast-v2.db` and `master.key` together, pin all three images to
per-image SHA-256 digests, run `scripts/upgrade_preflight.sh`, pull, and restart. Review release
notes for schema changes before every upgrade.

A digest pins only what you already trust, so verify provenance before pinning one. The publish
workflow attests each image and prints the digests in its run summary:

~~~sh
gh attestation verify oci://ghcr.io/marmot-protocol/keycast-signer@sha256:... \
  --repo marmot-protocol/keycast
~~~

`scripts/upgrade_preflight.sh` performs this check for all three images when `gh` is installed, and
fails when an image has no verifiable provenance.

The September 5 prerelease authority schema deliberately invalidates earlier V2 migration checksums.
Choose a fresh development database; no automatic deletion or V1 conversion is performed.
See `docs/V2_OPERATIONS.md` for external approvals and disaster recovery.
