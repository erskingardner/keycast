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
4. Make sure the external `keycast` Docker network and reverse proxy are available.

## Replace V1

Stop the old deployment:

~~~sh
sudo docker compose down
~~~

Keep any legacy files as an offline archive if desired:

~~~sh
mkdir -p legacy-v1
mv database/keycast.db* legacy-v1/ 2>/dev/null || true
mv master.key legacy-v1/master.key 2>/dev/null || true
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

## Future V2 Upgrades

Once on v2, preserve `database/keycast-v2.db` and `master.key` together, pin all three images to
reviewed per-image SHA-256 digests, run `scripts/upgrade_preflight.sh`, pull, and restart. Review release
notes for schema changes before every upgrade.

The September 5 prerelease authority schema deliberately invalidates earlier V2 migration checksums.
Choose a fresh development database; no automatic deletion or V1 conversion is performed.
See `docs/V2_OPERATIONS.md` for external approvals and disaster recovery.
