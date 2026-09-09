# Upgrading

[Documentation](README.md) · [Deployment](deployment.md) · [Backup and recovery](backup-and-recovery.md)

## Update an existing current-generation instance

Preserve `database/` and its matching `master.key` in the configured `KEYCAST_STATE_DIR` (the
checkout by default). Review the intended source changes and migrations,
take an encrypted backup, and record the currently deployed source revision and all three image
digests before changing anything. Rehearse recovery before relying on the backup.

Check [Hardening migration](#hardening-migration) if your instance predates the internal network
and static reverse proxy. Select the intended checkout and set the three reviewed image digests
from the same build in `.env`. [Verify their provenance](deployment.md#choose-images-or-a-source-build)
before deploying.
From the repository root:

```sh
sudo scripts/upgrade_preflight.sh
sudo docker compose -f docker-compose.prod.yml pull
sudo docker compose -f docker-compose.prod.yml up -d
sudo docker compose -f docker-compose.prod.yml ps
sudo docker compose -f docker-compose.prod.yml exec -T keycast-signer /app/keycast_signer status
```

Use `--fix-permissions` only when you intend preflight to repair database/root ownership and modes.
Do not run initialization or key generation again. Source-build deployments use
`docker compose up -d --build` with their reviewed checkout instead of the published-image commands.

Check management sign-in, root fingerprint, signing readiness, and an existing client's allowed
and denied operations. Normal restarts preserve sessions. Review [relay route compatibility](relays.md)
before changing endpoints at the same time as an upgrade.

Migrations apply on signer startup. Do not assume that older images can read a database after an
upgrade. Reverting application images and restoring historical state are different operations:
use the [restore workflow](backup-and-recovery.md#recovery-after-loss-or-rollback) if rolling state
back. Restore revokes old access and requires review and client reconnection.

![A changed management reply identity with a browser re-trust control](images/reply-identity-change.png)

*If an upgrade includes a reply-identity change, verify the fingerprint on the trusted host. This example was captured after deliberate root rotation; an ordinary restart does not rotate the root.*

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
`caddy=` labels have been removed from both Compose files. Prepare and review that file before
starting anything (`-n` preserves an existing Caddyfile, which you must update deliberately):

~~~sh
cp -n Caddyfile.example Caddyfile
~~~

Then bring the whole stack back up and confirm it is healthy:

~~~sh
sudo docker compose -f docker-compose.prod.yml up -d
sudo docker compose -f caddy-docker-compose-example.yml up -d
sudo docker compose -f docker-compose.prod.yml ps
~~~

**Management writes now pin a reply identity.** The signer derives a stable reply keypair from the
root credential and publishes its public half through `/api/config`. Your browser pins it on first
use. After a root-credential rotation the identity changes, so the Instance page shows a warning with
both fingerprints and a button to trust the new one. Compare it against the host first:

~~~sh
sudo docker compose -f docker-compose.prod.yml exec -T keycast-signer \
  /app/keycast_signer status
~~~

**Swap is now disabled for the containers.** `memswap_limit` matches `mem_limit` so decrypted key
material cannot be paged to host swap. No action needed, but confirm the host has enough RAM.

**State can now live outside the checkout.** `KEYCAST_STATE_DIR` moves the runtime database and
`master.key` away from the working tree, keeping them separate from source and normal build inputs. It defaults to the checkout, so existing deployments are unaffected.

Move the database *files*, not the `database/` directory: `database/migrations` is tracked source
that both the image build and the Rust tests read, and relocating it breaks them. The signer reads
migrations from inside the image, so the external directory holds only the database.

This example assumes state is still in the checkout and `/srv/keycast` has no existing database
or root credential. Take a backup first; stop if either destination already contains state.

The move runs inside one privileged shell. `database/` is mode `0700` owned by UID 10001, so an
ordinary operator's shell cannot list it to expand the wildcard: bash would pass the literal
`database/keycast-v2.db*` to `mv` and zsh would refuse outright, and the credential move and `.env`
edit that follow would still succeed, leaving the database behind and the signer pointed at an empty
directory. Running the whole move under `sudo bash -euc` expands the glob with the right privileges
and stops at the first failure.

~~~sh
sudo docker compose -f docker-compose.prod.yml down
sudo bash -euc '
  test ! -e /srv/keycast/database/keycast-v2.db
  test ! -e /srv/keycast/master.key
  test -f master.key
  shopt -s nullglob
  database=(database/keycast-v2.db*)
  (( ${#database[@]} )) || { echo "no keycast-v2.db found to move"; exit 1; }
  install -d -m 0700 -o 10001 -g 10001 /srv/keycast /srv/keycast/database
  mv -- "${database[@]}" /srv/keycast/database/
  mv -- master.key /srv/keycast/
  echo "KEYCAST_STATE_DIR=/srv/keycast" >> .env
'
~~~

Leave stale lock files alone until the move is verified; the signer creates locks at its configured
location. Validate and restart:

~~~sh
sudo scripts/upgrade_preflight.sh --fix-permissions
sudo docker compose -f docker-compose.prod.yml up -d
sudo docker compose -f docker-compose.prod.yml ps
~~~

Confirm the signer reports schema version 2 and your existing keys on the **Instance** page before
deleting anything from the old location.

`scripts/init.sh --state-dir /srv/keycast` sets this up for a fresh install, and the preflight
resolves the same path so it never validates a stale copy.

## Move from the original release to version 2

This transition requires a fresh installation. The original database, encrypted keys, authorizations,
invitation/redemption state, secrets, and bunker URLs are incompatible. There is no in-place conversion.

1. Record the public identities, team memberships, policies, and clients you need to recreate.
   Keep private material out of planning notes.
2. Ensure you have each original private key available for import and an external management signer.
   Preserve an offline archive of the old database with its matching root credential for a possible
   rollback to the original deployment.
3. Stop the original deployment and keep its checkout, configuration, and data separate. Initialize
   a fresh checkout and root credential using [Deployment](deployment.md). Do not point the new
   signer at the legacy database.
4. Recreate teams, import keys, define policies, and issue fresh grants and invitations.
5. Reconnect every client with its new invitation. Verify allowed and denied requests before
   considering the transition complete.

The current database is named `database/keycast-v2.db`; `database/keycast.db` is not migrated or read.
Do not transform or reuse an old bunker secret. Rollback to the original release requires its
original database/root pair; changes made after the transition cannot be converted back into it.

## Early prerelease databases

Some early version-2 builds deliberately changed initial migration checksums. A checksum failure
does not authorize deleting or resetting an existing database. Preserve the files and establish
which build created them. Use a separate disposable database for development, or plan a deliberate
fresh setup with re-imported keys when moving away from an incompatible prerelease.

The [historical transition notes](development/history/V1_UPGRADE.md) record the original cutover
procedure. Use this guide and the current deployment configuration for a new upgrade.
