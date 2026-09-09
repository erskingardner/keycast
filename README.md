# Keycast

Keycast is a small, self-hosted NIP-46 remote signer for personal, family, and small-team Nostr
keys. Version 2 is intentionally a clean break from the original implementation: it uses a new
schema, a single multiplexed signer, one-time invitations, explicit sessions, and strict
default-deny policies.

Keycast is security-sensitive software. The repository has test-backed hardening, but it has not
received an independent security audit. Read [AUDIT.md](AUDIT.md) before trusting it with valuable
keys.

## Architecture

Three processes run on one VM:

- `keycast-web` serves the SvelteKit UI.
- `keycast-api` forwards signed management requests. It has neither a database nor a root credential.
- `keycast-signer` owns SQLite, verifies original approvals and team/operator authorization, evaluates
  policy, performs NIP-46 operations over shared relays, and writes redacted audit records.

The API connects to a private Unix socket through a read-only mount. The web process has neither
socket nor database access. Management writes require an external signer even when the same Nostr
identity is managed by Keycast. All grants hard-deny the private management event kind 27236.

See [docs/V2_ARCHITECTURE.md](docs/V2_ARCHITECTURE.md) for the design and its boundaries.

## Clean V2 Boundary

V2 creates `database/keycast-v2.db` from `database/migrations/0001_initial.sql`. It does not read or
migrate `database/keycast.db`. Old stored-key ciphertext, authorizations, redemption records,
invitations, shared secrets, and bunker URLs are invalid in v2. Re-import keys and issue new grants.

## Production Setup

Requirements:

- a Linux VM with current Docker Engine and the Compose plugin;
- DNS for the chosen hostname;
- an internal external Docker network named `keycast` (`docker network create --internal keycast`);
- a reverse proxy attached to that network. `caddy-docker-compose-example.yml` and
  `Caddyfile.example` are provided and use a static configuration with no Docker socket mount.

Initialize a checkout:

```sh
bash scripts/init.sh \
  --domain keycast.example.com \
  --allowed-pubkeys "64-character-hex-pubkey[,another-pubkey]" \
  --state-dir /srv/keycast   # optional; keeps state out of the checkout
# Set the three reviewed image digests in .env before production preflight/pull.
sudo scripts/upgrade_preflight.sh --fix-permissions
sudo docker compose -f docker-compose.prod.yml pull
sudo docker compose -f docker-compose.prod.yml up -d
```

Then copy `Caddyfile.example` to `Caddyfile`, review it, and start the proxy with
`docker compose -f caddy-docker-compose-example.yml up -d`.

`ALLOWED_PUBKEYS` is the instance admission allowlist. `KEYCAST_OPERATOR_PUBKEYS` controls global relay changes. It fails closed when absent. Open registration
exists only as an explicit development escape hatch through `KEYCAST_ALLOW_OPEN_REGISTRATION=true`;
do not enable it on an Internet-facing instance.

The `keycast` network is internal, so the API and web containers have no route off the host. The
signer gets its own egress network for relay connections, and the reverse proxy keeps a public
network for ACME and published ports. Nothing needs inbound access to the signer: the API reaches
it only through the private socket volume.

The setup creates a 32-byte root key in `master.key`, mode `0600`, and assigns the database and key
to the fixed container UID/GID `10001`. With `--state-dir` (recorded as `KEYCAST_STATE_DIR` in
`.env`) both live outside the checkout, so no `git` operation or source build can reach them. The file is mounted read-only into the signer only. V2 can
also load a systemd credential from `$CREDENTIALS_DIRECTORY` when run outside Compose.

Production publishes three images:

- `ghcr.io/marmot-protocol/keycast-signer:v2`
- `ghcr.io/marmot-protocol/keycast-api:v2`
- `ghcr.io/marmot-protocol/keycast-web:v2`

Production Compose requires a SHA-256 manifest digest for each of the three images. A digest only
pins what you already trust, so verify each one was built by this repository before deploying it:

```sh
gh attestation verify oci://ghcr.io/marmot-protocol/keycast-signer@sha256:... \
  --repo marmot-protocol/keycast
```

`scripts/upgrade_preflight.sh` runs that check for all three images when `gh` is installed.
Use source Compose (`docker compose up -d --build`) for a local build.

## Operations

The authenticated **Status** page reports:

- schema and envelope versions and the non-secret root-key fingerprint;
- active grants, sessions, and claimable invitations;
- enabled and connected relay counts;
- per-relay connection, receive, publish, failure, and error checkpoints;
- the management reply identity fingerprint, for comparison against `keycast_signer status`;
- the last processed request and redacted failure counters.

API health and signing readiness are separate so management remains available during relay outages.
Readiness tracks accepted subscriptions, integrity, and quarantined grants. A runtime progress
watchdog exits stalled processes; Docker restarts failed services.

Use the trusted CLI for private key import, online encrypted backup, restore with revoked old grants,
and offline root rotation. See [docs/V2_OPERATIONS.md](docs/V2_OPERATIONS.md) for exact commands,
restore review, external-signer approvals, browser-import limitations, and resource bounds.

Relay configuration is instance-wide and editable on the Status page. URLs must use `wss://`
(`ws://` is accepted only for loopback testing), duplicates are rejected, and the minimum-connected
threshold cannot exceed the number of enabled relays.

## Development

Requirements: Rust 1.96, Bun 1.3.9, `cargo-watch`, SQLx CLI, and OpenSSL.

```sh
bun install
cd web && bun install && cd ..
bun run key:generate
ALLOWED_PUBKEYS=<64-character-hex-pubkey> bun run dev
```

For a loopback-only disposable test, the open-registration shortcut avoids configuring an operator
admission allowlist (global relay changes still require an explicit operator):

```sh
bun run dev:open
```

Open `http://localhost:5173` and sign in with a NIP-07 extension or another Nostr Connect signer.
Never use `dev:open` on a network-accessible API. The API listens on `127.0.0.1:3100`, permits the
local web origin, and shares `runtime/signer.sock` with the signer. The signer watcher is scoped to
source and migration paths so socket and SQLite writes cannot restart an active NIP-46 exchange.
Development logging keeps dependency internals at warning level while retaining Keycast and HTTP
request diagnostics; normal idle operation should not continuously emit logs.
Reset only the v2 development database with:

```sh
bun run db:reset
```

## Validation

```sh
cargo fmt --all --check
cargo check --workspace --locked
cargo build --workspace --locked
cargo test --workspace --locked
cargo audit

bun install --frozen-lockfile
bun audit
bun pm scan

cd web
bun install --frozen-lockfile
bun test
bun run check
bun run build
bun audit
bun pm scan
bun pm untrusted
```

The Docker workflow additionally builds all three runtime targets and runs them together with
read-only root filesystems, dropped capabilities, a private socket volume, and signer-only root-key
access.

## License

[MIT](LICENSE)
