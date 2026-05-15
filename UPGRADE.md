# Keycast Live Upgrade Notes

Use this when upgrading an existing Keycast deployment to the hardened runtime. This is not a blank
install path.

## What Changed

- Containers now run as a non-root user and use a read-only root filesystem.
- `master.key` is mounted from the host instead of being copied into the image.
- `ALLOWED_PUBKEYS` is enforced by the API, not just the browser.
- The Nostr Rust stack moved to current crates.io releases.
- Migration `0002_normalize_allowed_kinds_permissions.sql` normalizes old `allowed_kinds` permission
  JSON from `{"sign":[...]}` to `{"allowed_kinds":[...]}`.

## Before You Deploy

1. Make a cold backup of both files:

```sh
mkdir -p backups
docker compose stop keycast-api keycast-signer keycast-web
cp database/keycast.db "backups/keycast.$(date +%Y%m%d-%H%M%S).db"
cp master.key "backups/master.$(date +%Y%m%d-%H%M%S).key"
```

2. Do not generate a new `master.key` for an existing install. If the host file is missing but the old
   container still has `/app/master.key`, recover it before deploying:

```sh
docker cp keycast-api:/app/master.key ./master.key
chmod 600 master.key
```

3. Set the server allowlist before starting the new API:

```sh
ALLOWED_PUBKEYS=hexpubkey1,hexpubkey2
```

Every admin pubkey that needs to use the app after the upgrade must be included.

4. Run the preflight:

```sh
scripts/upgrade_preflight.sh
sudo scripts/upgrade_preflight.sh --fix-permissions
```

The second command is needed when the old root-running containers created root-owned database files.
The preflight also checks that `database/migrations/0002_normalize_allowed_kinds_permissions.sql` is
present in the checkout. The API and signer read migrations from the bind-mounted `database/`
directory at startup.

## Deploy

```sh
docker compose build
docker compose up -d
docker compose ps
```

The API and signer run SQLx migrations on startup. The new migration only normalizes old permission
JSON. It does not rotate keys, change stored-key ciphertext, or invalidate existing bunker connection
strings.

## Verify

```sh
curl -f https://your-domain.example/health
curl -f https://your-domain.example/api/health
docker compose logs --tail=100 keycast-api keycast-signer keycast-web
```

Then sign in with an allowlisted pubkey, open an existing team, inspect an existing policy, and test
one existing NIP-46 authorization. Existing bunker connection strings should keep working because the
stored keys, bunker keys, and connection secrets are unchanged.

## Rollback

If you need to roll back code after migration `0002` has run, restore the database backup too. The old
application may not understand the normalized `allowed_kinds` JSON.

```sh
docker compose down
cp backups/keycast.YYYYMMDD-HHMMSS.db database/keycast.db
cp backups/master.YYYYMMDD-HHMMSS.key master.key
docker compose up -d --build
```

Do not roll back with a different `master.key`; stored private keys and bunker keys will not decrypt.
