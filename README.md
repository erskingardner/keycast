# Keycast

Secure remote signing and team-managed permissions for Nostr keys.

Keycast is an early-stage project. A May 2026 audit found and fixed several auth, permission, data
integrity, and dependency issues, but this should still get a deployment review before it is trusted
with real team keys on the public internet. See [AUDIT.md](AUDIT.md) for the current status.

## What It Does

Keycast lets a team:

- sign in with a Nostr key and manage team membership,
- store Nostr private keys encrypted at rest in SQLite,
- create policies and permissions for stored keys,
- generate NIP-46 bunker connection strings,
- run signer processes that approve or deny remote signing requests from those policies.

The project was written for teams rather than individual remote-signing workflows like `nsec.app`,
Knox, or Amber.

## Project Layout

- `core/` - shared Rust models, database helpers, encryption, and permission checks.
- `api/` - Axum API, SQLite setup, NIP-98 HTTP auth middleware, and team/key/policy routes.
- `signer/` - signer manager plus one `signer_daemon` process per authorization.
- `web/` - SvelteKit app that asks a NIP-07 signer to sign NIP-98 auth events.
- `database/migrations/` - SQLite schema.
- `scripts/` - key generation, Docker entrypoint, health checks, and deployment init.

## Current Validation State

These commands were run after the May 14, 2026 hardening pass:

- `cargo check --workspace` passes with no warnings.
- `cargo build --workspace` passes.
- `cargo test --workspace` passes.
- `cd web && bun test` passes.
- `cd web && bun run check` passes with no warnings.
- `cd web && bun run build` passes with no warnings.
- `bun audit` and `cd web && bun audit` report no vulnerabilities.
- `bun pm scan` and `cd web && bun pm scan` run through the configured Socket scanner and report no
  advisories in free mode.
- `cargo audit` exits quietly. `.cargo/audit.toml` documents the ignored upstream `instant` advisory,
  which is pulled into the lockfile by `nostr` for wasm targets and is not used by the native runtime.
- `bash -n` passes for `scripts/init.sh`, `scripts/upgrade_preflight.sh`,
  `scripts/docker-entrypoint.sh`, `scripts/healthcheck.sh`, and `scripts/generate_key.sh`.
- Migration `0002_normalize_allowed_kinds_permissions.sql` was smoke-tested against a migrated
  SQLite database and converts the legacy `{"sign":[...]}` permission shape to
  `{"allowed_kinds":[...]}`.
- Docker Compose config rendering passes with explicit `ALLOWED_PUBKEYS`, `KEYCAST_UID`, and
  `KEYCAST_GID` values.
- `DOCKER_BUILDKIT=1 docker build -t keycast-runtime-check .` passes with digest-pinned base images,
  frozen lockfiles, and production frontend dependencies.
- The built API image starts under `--read-only`, creates a fresh SQLite database on a writable bind
  mount, and applies migrations 1 and 2.
- The built web image starts as UID/GID `10001`, serves `/health` under `--read-only` plus `/tmp`
  tmpfs, and emits the expected CSP, HSTS, frame, referrer, permissions, and content-type headers.

## Setup

Requirements:

- Rust toolchain
- Bun
- SQLx CLI for database reset commands
- `cargo-watch` for the combined dev command
- `openssl` or `/dev/urandom` for `master.key` generation

Install dependencies:

```sh
bun install
cd web && bun install
```

Generate the local encryption key:

```sh
bun run key:generate
```

This creates `master.key` at the repo root. It is ignored by git and must never be committed. The
current `FileKeyManager` reads this root file directly.

Create `.env` from `.env.example` for Docker/deployment settings:

```sh
cp .env.example .env
```

`ALLOWED_PUBKEYS` is enforced by the API for NIP-98-authenticated requests. The API also exposes an
unauthenticated `/api/config?pubkey=<hex>` check so the browser can ask whether the current pubkey is
allowed without receiving the full server allowlist.

## Development

Run API, web, and signer together:

```sh
bun run dev
```

The local dev scripts run the API on `http://localhost:3100` and point the web app at that origin.
The Docker/runtime default API port remains `3000`.

Run pieces separately:

```sh
bun run dev:api
bun run dev:web
bun run dev:signer
```

Reset the SQLite database:

```sh
bun run db:reset
```

Useful validation commands:

```sh
cargo check --workspace
cargo build --workspace
cargo test --workspace
cd web && bun test
cd web && bun run check
cd web && bun run build
cargo audit
bun audit
cd web && bun audit
bun pm scan
cd web && bun pm scan
cd web && bun pm untrusted
```

## Bun Security Scanning

`bun pm scan` is Bun's pluggable package scanner. Bun does not include a scanner by default; it loads
an npm package named in `bunfig.toml`:

```toml
[install.security]
scanner = "@socketsecurity/bun-security-scanner"
```

This repo now installs `@socketsecurity/bun-security-scanner@1.1.2` exactly in both Bun projects. It
runs in Socket free mode without credentials. Set `SOCKET_API_KEY` only if you want scans to use a
Socket organization policy.

## Runtime Flow

1. The web app asks the raw NIP-07 browser extension API for the active Nostr pubkey.
2. API calls include a NIP-98 HTTP auth event in the `Authorization` header.
3. The API verifies the event, extracts the pubkey, and checks team admin rights for most routes.
4. Stored keys and per-authorization bunker keys are encrypted with the root `master.key`.
5. The signer manager watches authorization rows and starts a `signer_daemon` for each one.
6. Each signer daemon decrypts the stored key and bunker key, listens on configured relays, and calls
   `Authorization::validate_policy` before approving NIP-46 requests.

## Security Notes

The current hardening posture:

- NIP-98 auth now requires exactly one `u` tag and one `method` tag, rejects stale or far-future
  timestamps, validates request-body `payload` hashes, and rejects oversized authenticated bodies.
- `ALLOWED_PUBKEYS` is parsed exactly and enforced server-side.
- Allowed-kinds permission config now uses one Rust/TypeScript shape and rejects unknown fields.
- Empty policies default-deny sign/encrypt/decrypt requests, and policy creation rejects empty,
  unknown, or malformed permission configs.
- SQLite connections enable foreign-key enforcement, and team deletion no longer loses permission join
  data before cleanup.
- Server-side route protection now covers nested app routes such as `/teams/:id`, not only exact
  top-level paths.
- Web responses set CSP, frame, content-type, referrer, permissions, and HTTPS HSTS headers.

Keep future fixes narrow and test-backed. The code handles private key material, so correctness and
access control matter more than cosmetic cleanup.

## Deployment

Docker deployment uses:

- `docker-compose.yml` for local source builds of API, web, and signer containers,
- `docker-compose.prod.yml` for pulling the published `ghcr.io/erskingardner/keycast` image,
- `master.key` mounted into API and signer containers,
- an external Docker network named `keycast`,
- Caddy labels for routing `/api/*` to the API and the rest to the web app.

The compose defaults now require `ALLOWED_PUBKEYS`, expose the API only on the Docker network, build
with frozen Bun/Cargo lockfiles, and avoid copying `master.key` into the image. Runtime containers run
as a non-root UID/GID, use a read-only root filesystem, drop Linux capabilities, set
`no-new-privileges`, and get a writable `/tmp` tmpfs. The Caddy example uses a read-only Docker socket
and a digest-pinned `caddy-docker-proxy` image. `.dockerignore` excludes local build output, package
installs, databases, `.env` files, and `master.key` from the build context.

Initialize a server checkout:

```sh
bash scripts/init.sh --domain keycast.example.com --allowed-pubkeys "hexpubkey1,hexpubkey2"
sudo docker compose -f docker-compose.prod.yml pull
sudo docker compose -f docker-compose.prod.yml up -d
```

`scripts/init.sh` writes `KEYCAST_UID` and `KEYCAST_GID` into `.env`, defaults them to the current
user, and tries to set matching ownership on `database/` and `master.key`. Pass `--uid` and `--gid`
if the container user should be different.

Every push to `master` publishes one reusable image to GitHub Container Registry with `master`,
`latest`, and `sha-<commit>` tags. Set `KEYCAST_IMAGE_TAG=sha-<commit>` in `.env` when you want a
pin-and-roll-forward deployment instead of tracking the moving `master` tag. If GitHub creates the
first package as private, change the package visibility to public in the GitHub Packages settings.

For an existing deployment, do not use the blank-install flow blindly. Read [UPGRADE.md](UPGRADE.md),
back up `database/keycast.db` and `master.key` together, verify the host `master.key` matches the
currently running container, set `ALLOWED_PUBKEYS`, and run:

```sh
scripts/upgrade_preflight.sh
sudo scripts/upgrade_preflight.sh --fix-permissions
```

The new SQL migration normalizes old allowed-kinds permission JSON during API or signer startup. It
does not rotate keys, change encrypted stored-key material, or intentionally invalidate existing
NIP-46 bunker connection strings. Keep `database/migrations/` present on the server because the API
and signer read migrations from the bind-mounted `database/` directory. If you roll back after the
migration has run, restore the database backup as well as the matching `master.key`.

Before using this for real keys, review the residual notes in [AUDIT.md](AUDIT.md), configure the
proxy headers used by NIP-98 URL validation, and rerun the validation commands above.

## Custom Permissions

Permissions are implemented in `core/src/custom_permissions`. To add or change a permission, update
all of these places together:

- Rust permission implementation and config type in `core/src/custom_permissions/`
- `AVAILABLE_PERMISSIONS` in `core/src/custom_permissions/mod.rs`
- `Permission::to_custom_permission` in `core/src/types/permission.rs`
- web permission types in `web/src/lib/types.ts`
- web form rendering in `web/src/lib/components/PermissionForm.svelte`
- README or audit notes if the semantics affect signing approval

Do not rely on frontend-only validation for permission safety.

## License

[MIT](LICENSE)
