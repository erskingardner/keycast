# Development and contribution

[Keycast](../../README.md) · [Documentation](../README.md) · [Project history](history/README.md)

Start with [AGENTS.md](../../AGENTS.md) for repository rules and the
[architecture](../architecture.md) for process and authorization boundaries. [TODO.md](../../TODO.md)
tracks open work. Use disposable keys for development.

## Local setup

The CI toolchain uses Rust 1.96 and Bun 1.3.9. Local development also needs `cargo-watch`, OpenSSL,
and SQLx CLI if you use the database-reset helper. Read the component's `AGENTS.md` before editing it.

From the repository root:

```sh
bun install --frozen-lockfile
(cd web && bun install --frozen-lockfile)
bun run key:generate
ALLOWED_PUBKEYS=YOUR_64_CHARACTER_HEX_PUBKEY \
  KEYCAST_OPERATOR_PUBKEYS=YOUR_64_CHARACTER_HEX_PUBKEY bun run dev
```

Generate `master.key` only on first setup; the helper refuses to overwrite an existing credential.
Use a checkout/data directory separate from production. Never regenerate the root key to fix a
database or startup error.

Open `http://localhost:5173` and sign in with an external signer. The API listens on
`127.0.0.1:3100`, allows the local web origin, and communicates with the signer over
`runtime/signer.sock`. The signer watcher only watches source/migration paths, so SQLite and socket
writes do not restart an exchange. Dependency logs default to warnings while Keycast and HTTP
diagnostics remain visible.

For a loopback-only disposable test, `bun run dev:open` enables open management admission without
the public-key allowlist. Global relay/status access still needs an explicit `KEYCAST_OPERATOR_PUBKEYS`.
Never use this mode on a network-accessible API.

`bun run db:reset` resets only `database/keycast-v2.db` using the current migrations. It is destructive
to development data: stop local services and confirm the target is disposable before running it.
It is not an upgrade or recovery procedure.

## Code map

| Path | Responsibility |
|---|---|
| [`api/`](../../api/) | Bounded public Axum transport; forwards original signed management requests. |
| [`core/`](../../core/) | Schema helpers, strict policies, authenticated envelopes, and shared contracts. |
| [`signer/`](../../signer/) | SQLite, management authorization, NIP-46, relay supervision, and trusted host operations. |
| [`web/`](../../web/) | SvelteKit management UI and external-signer approvals. |
| [`database/migrations/`](../../database/migrations/) | Current schema and additive migrations. |
| [`scripts/`](../../scripts/) | Initialization, deployment preflight, container smoke, and host operations jobs. |

Keep Rust and TypeScript policy/API contracts together. Unknown policy data must fail closed.
Every management access rule belongs in the signer; UI restrictions are not authorization.
The API must not open SQLite or load the root credential. Preserve the explicit incompatibility
with original-release storage and credentials.

## Validation

Run from the repository root with the locked dependencies installed:

```sh
cargo fmt --all --check
cargo check --workspace --locked
cargo build --workspace --locked
cargo test --workspace --locked
cargo audit
python3 -m unittest discover -s scripts/operations -v
bun audit
bun pm scan
shellcheck -S warning scripts/*.sh

cd web
bun test
bun run check
bun run build
bun audit
bun pm scan
bun pm untrusted
```

Install `cargo-audit` for advisory checks and ShellCheck for shell validation. The
[CI workflow](../../.github/workflows/ci.yml) is the executable reference for validation jobs.
The focused relay interoperability suite is
`cargo test -p keycast_signer --test nip46_roundtrip -- --nocapture`; run it with disposable fixtures.

For deployment changes, render both Compose files with a dedicated test environment, build all
three Docker targets, and run the combined container smoke. From the repository root:

```sh
bash scripts/check-compose-hardening.sh
docker build --target api-runtime -t keycast-hardening-api:local .
docker build --target signer-runtime -t keycast-hardening-signer:local .
docker build --target web-runtime -t keycast-hardening-web:local .
scripts/container-smoke.sh
```

The hardening script supplies placeholder configuration, renders both application Compose files and
the proxy, and checks their security settings. It does not verify that placeholder images exist. The smoke uses
the local tags above, disposable containers/volumes, read-only root filesystems, dropped capabilities,
and signer-only root access. It exercises HTTP management through the private socket.

Local tests, container smoke, a deployed VM, relay acknowledgements, and observed client receipt
prove different things. Report the build and exact checks performed without treating one as the other.

## Documentation changes

Keep the [README](../../README.md) focused on what Keycast does and where to start. Put current user,
operator, and system explanations in the main `docs/` directory. Put contributor workflows here,
and dated decisions, plans, audit evidence, and experiments in [history/](history/README.md).
Update the relevant index and inbound links when moving documents. Preserve historical outcomes
and limitations instead of rewriting old reports as current deployment claims.
