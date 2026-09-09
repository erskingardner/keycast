# Deploy Keycast

[Documentation](README.md) · [Getting started](getting-started.md) · [Operations](operations.md)

Keycast runs as one active signer on one Linux host. Docker Compose starts the signer, public API,
and web UI. SQLite and the root credential live on the host; only the signer mounts them.

## Requirements

- A Linux host with Docker Engine, the Compose plugin, Git, GitHub CLI (`gh`) for image provenance, and permission to administer Docker.
- A hostname with DNS pointing to the host, and HTTPS access through a reverse proxy.
- Outbound connectivity to your signing relays.
- An external Nostr signer for each management identity, and the identities' public keys in
  64-character hex format for configuration.
- Reviewed images for all three services, or a source checkout you intend to build.

Run the commands below from the repository root. If you already have an instance, follow
[Upgrading](upgrading.md) instead of initializing over its state.

```sh
git clone https://github.com/marmot-protocol/keycast.git
cd keycast
```

Select the source revision you intend to deploy before continuing.

## Initialize configuration

Replace the hostname and public-key placeholders with your values:

```sh
bash scripts/init.sh \
  --domain keycast.example.com \
  --allowed-pubkeys "YOUR_64_CHARACTER_HEX_PUBKEY" \
  --operator-pubkeys "YOUR_64_CHARACTER_HEX_PUBKEY"
```

To keep persistent state outside the checkout, add `--state-dir /srv/keycast` (the invoking
account needs permission to create that directory). This is saved as `KEYCAST_STATE_DIR` in `.env`.
Without it, state defaults to the checkout.

The script creates `.env`, a private `database/` directory, and a 32-byte root credential in
`master.key` if one is absent. It refuses to overwrite `.env` or regenerate an existing root key.
It attempts to set database/root ownership to container UID/GID `10001`, and to create the
external Docker network `keycast` with `--internal`. If Docker requires elevated access, ensure the network exists:

```sh
sudo docker network inspect keycast >/dev/null 2>&1 || sudo docker network create --internal keycast
```

Configuration in [`.env.example`](../.env.example):

| Setting | Meaning |
|---|---|
| `KEYCAST_STATE_DIR` | Host directory containing `database/` and `master.key`; defaults to the checkout. Use an absolute external path to separate live state from source. |
| `DOMAIN` | Public hostname, without scheme or path. Compose derives the HTTPS API URL and web origin. |
| `ALLOWED_PUBKEYS` | Comma-separated hex public keys admitted to management. Empty configuration denies access. |
| `KEYCAST_OPERATOR_PUBKEYS` | Identities allowed to inspect instance status and change global relays. These identities also need admission. Initialization defaults to the first allowed key if omitted. |
| `KEYCAST_API_IMAGE`, `KEYCAST_SIGNER_IMAGE`, `KEYCAST_WEB_IMAGE` | Registry image names; defaults are the three Keycast packages under `ghcr.io/marmot-protocol`. |
| `KEYCAST_API_DIGEST`, `KEYCAST_SIGNER_DIGEST`, `KEYCAST_WEB_DIGEST` | Separate reviewed `sha256:…` manifest digests, required by production Compose. |

Team administrator roles are stored separately in the database. Operator status does not grant
administration of every team. See [Policies and access](policies-and-access.md).

## Choose images or a source build

For published images, set each digest in `.env` before running preflight. The
[image workflow](../.github/workflows/docker.yml) pushes commit-tagged images, smoke-tests those
exact digests, and attests their provenance before promoting them to `v2` and `latest`. Tags are
lookup aids; production Compose deploys the digests you select. Use the three images from the same
reviewed build; its workflow summary records the digests.

For example, inspect an image for a chosen commit with:

```sh
sudo docker buildx imagetools inspect ghcr.io/marmot-protocol/keycast-signer:sha-CHOSEN_COMMIT
```

Repeat for `keycast-api` and `keycast-web`, and record each manifest digest in its corresponding
setting. Verify each image against this repository before trusting it:

```sh
gh attestation verify oci://ghcr.io/marmot-protocol/keycast-signer@sha256:CHOSEN_DIGEST \
  --repo marmot-protocol/keycast
```

Repeat for the API and web digests. Preflight performs these checks when `gh` is available and fails
on verification errors; without `gh` it only warns, so install it to verify provenance. Do not copy image digests from a historical VM report as an implicit release selection.

```sh
sudo scripts/upgrade_preflight.sh --fix-permissions
sudo docker compose -f docker-compose.prod.yml pull
```

Preflight checks configuration, credential format, database state when present, permissions when
requested, and production Compose rendering. Resolve any failure before starting the stack.

Alternatively, [source Compose](../docker-compose.yml) builds the same three runtime targets from
the checkout and does not require published image digests. Set the same domain and allowlists,
repair ownership if initialization could not, then build. These paths assume state is in the
checkout; substitute the configured state directory if using `KEYCAST_STATE_DIR`:

```sh
sudo chown -R 10001:10001 database master.key
sudo chmod 700 database
sudo chmod 600 master.key
sudo docker compose build
```

Use `docker-compose.yml` consistently for this path. The production preflight requires image
digests, so it is not a source-build preflight.

## Configure HTTPS

Attach the reverse proxy to the `keycast` Docker network. Route `/api/*` to `keycast-api:3000`
without stripping the `/api` prefix, and other paths to `keycast-web:5173`. The services expose
ports on the Docker network; Compose does not publish them directly on the host.

[The Caddy example](../caddy-docker-compose-example.yml) uses a static
[`Caddyfile.example`](../Caddyfile.example), with no Docker socket mount or label discovery.
It persists TLS state in named volumes. Copy and review the configuration before first use:

```sh
cp Caddyfile.example Caddyfile
```

The `keycast` network must be internal: the API and web have no route off-host. The signer joins
its own egress network for relays and is reached by the API only through the Unix socket. Caddy
joins `keycast` and a separate ingress network for ACME and published ports. If an older `keycast`
network already exists without `--internal`, follow [Hardening migration](upgrading.md#hardening-migration)
to recreate it during a maintenance window.

The public URL must match the URL management users actually visit. The signer requires
`KEYCAST_PUBLIC_URL=https://HOST/api`; Compose supplies it from `DOMAIN`. An origin or path mismatch
can invalidate signed approvals. Configure public proxy connection/rate limits too; the API
deliberately does not trust forwarded IP headers for its own admission limits.

## Start and verify

With the Caddyfile reviewed and image digests configured:

```sh
sudo docker compose -f docker-compose.prod.yml up -d
sudo docker compose -f caddy-docker-compose-example.yml up -d
sudo docker compose -f docker-compose.prod.yml ps
sudo docker compose -f docker-compose.prod.yml exec -T keycast-signer /app/keycast_signer status
```

If you use another reverse proxy, start it with equivalent routing instead of the Caddy command.
For a source build, use `sudo docker compose up -d --build` and omit `-f docker-compose.prod.yml`
from subsequent commands.

Open your HTTPS hostname and sign in with an admitted external signer. On **Instance**, check the
root and management-reply fingerprints, relay configuration, and signing readiness. A blank instance can be ready with
no active grants and no relay connections. After connecting a disposable app, verify both an
allowed request and a denied operation, and inspect the relay receive/publish checkpoints.

API availability and signing readiness are separate. Relay acknowledgement proves acceptance by
a relay; successful receipt in the client is a separate check. See [Operations](operations.md)
and [Relays](relays.md) when diagnosing failures.

## Keep the instance recoverable

Preserve `database/` and its matching `master.key` in the configured state directory; never regenerate the credential for an existing
database. The root file is mode `0600` and mounted read-only into the signer. The API has only a
read-only socket mount; the web has neither socket nor database access.

Set up [encrypted off-host backups and a recovery rehearsal](backup-and-recovery.md), then connect
the [monitoring jobs](operations.md#host-monitoring-and-off-host-jobs) to an external alert receiver.
Those jobs and storage are not enabled by bringing up Compose. The
[security model](security.md) describes the remaining host and browser trust boundaries.
