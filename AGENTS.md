# AGENTS.md

This is the canonical guidance for this repository. `CLAUDE.md` must remain a symlink to it.

## Project Shape

Keycast v2 is a Rust and SvelteKit NIP-46 remote signer for a small self-hosted instance.

- `api/` is the public Axum transport. It opens neither SQLite nor the root credential.
- `core/` contains the v2 schema helpers, strict policy model, envelope encryption, and contracts.
- `signer/` is one multiplexed signer with shared relays and a private Unix control socket.
- `web/` is the SvelteKit UI.
- `database/migrations/0001_initial.sql` is the clean v2 schema.

V1 databases, ciphertext, authorization rows, secrets, and bunker URLs are deliberately incompatible.

## Working Rules

- Fix security and reliability before cosmetic work.
- Treat private keys, NIP-98, NIP-46, policy, invitation/session lifecycle, and team isolation as
  security boundaries.
- Never commit or log `master.key`, databases, `.env`, ciphertext, invitation secrets, complete
  bunker URLs, `node_modules`, `target`, or build output.
- Only the signer may load the root credential. The API forwards original signed HTTP requests over
  the socket; actor-only lifecycle requests are never accepted.
- Frontend checks are not authorization. Enforce every access rule in the signer management router.
- Keep Rust and TypeScript policy/API contracts together. Unknown policy data must fail closed.
- Preserve the clean v2 boundary; do not add a V1 migration without an explicit architecture decision.
- Use `rg` and current code rather than trusting old documentation.

## Validation

~~~sh
cargo fmt --all --check
cargo check --workspace --locked
cargo build --workspace --locked
cargo test --workspace --locked
cargo audit
python3 -m unittest discover -s scripts/operations -v
bun audit
bun pm scan
cd web
bun test
bun run check
bun run build
bun audit
bun pm scan
bun pm untrusted
~~~

For deployment changes, also render both Compose files, build all three Docker targets, and run the
combined read-only-container smoke.

## Security Hotspots

- `signer/src/management/api/http/mod.rs`: signed approvals, NIP-98 and instance allowlist.
- `signer/src/management/api/http/teams.rs`: team/key/policy/grant/relay authorization.
- `core/src/v2/policy.rs`: default-deny capability semantics.
- `core/src/v2/envelope.rs`: root credential and authenticated encryption.
- `signer/src/store.rs`: invitation/session/dedupe/audit transactions.
- `signer/src/protocol.rs`: NIP-46 validation and key use.
- `signer/src/runtime.rs`: relay readiness and worker supervision.
- `signer/src/maintenance.rs` and `signer/src/backup.rs`: trusted host operations and authenticated archives.
- `signer/src/admission.rs`: shared live/recovery resource admission.
- `web/src/lib/keycast_api.svelte.ts`: NIP-98 event construction.

See `AUDIT.md` for residual risk and `TODO.md` for post-baseline work.
