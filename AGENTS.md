# AGENTS.md

This is the canonical guidance file for agents and humans working in this repo. `CLAUDE.md` should be
a symlink to this file.

## Project Shape

Keycast is a Rust plus SvelteKit app for Nostr team key management and NIP-46 remote signing.

- `api/` is the Axum HTTP API.
- `core/` contains shared Rust types, permission checks, encryption, and database helpers.
- `signer/` runs the signer manager and per-authorization signer daemons.
- `web/` is the SvelteKit UI.
- `database/migrations/` is the SQLite schema source.

## Working Rules

- Focus on bugs and security before cosmetic cleanup.
- Treat private key handling, NIP-98 auth, policy evaluation, and team isolation as security-sensitive.
- Do not commit `master.key`, SQLite databases, `.env`, `node_modules`, `target`, or build output.
- Keep frontend and backend contract changes together. Permission config shape is one contract, not two.
- Frontend checks are not authorization. The API must enforce access control.
- Avoid broad rewrites. Fix the smallest surface that proves the behavior.
- Use `rg` for search and read the current code before assuming old docs are true.

## Validation Commands

Run the relevant subset for small changes and the full set for auth, permissions, signer, or release
work:

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

Known current state after the May 14, 2026 hardening pass:

- `cargo check --workspace` passes.
- `cargo build --workspace` passes.
- `cargo test --workspace` passes with migration-backed API and authorization tests.
- `cd web && bun test` passes for pure TypeScript auth, route, and permission helpers.
- `cd web && bun run check` passes with no warnings.
- `cd web && bun run build` passes with no warnings.
- `cargo audit` exits quietly; `.cargo/audit.toml` documents the wasm-only upstream `instant`
  exception from the current `nostr` crate.
- `bun audit` and `cd web && bun audit` report no vulnerabilities.
- `bun pm scan` and `cd web && bun pm scan` run through the configured Socket scanner and report no
  advisories in free mode.
- `cd web && bun pm untrusted` reports zero untrusted lifecycle-script packages.
- `bash -n` passes for the deployment scripts, including `scripts/upgrade_preflight.sh`.
- Migration `0002_normalize_allowed_kinds_permissions.sql` was smoke-tested against SQLite.
- Docker Compose config rendering and a full `DOCKER_BUILDKIT=1 docker build -t keycast-runtime-check .`
  pass with digest-pinned base images, frozen lockfiles, non-root runtime users, and production
  frontend dependencies.
- The built web image has been smoke-tested under `--read-only` plus `/tmp` tmpfs and returns the
  expected security headers.

## Security Hotspots

- `api/src/api/http/mod.rs` - NIP-98 auth validation.
- `api/src/api/http/teams.rs` - team, key, policy, and authorization access control.
- `core/src/types/authorization.rs` - signer approval policy.
- `core/src/custom_permissions/` - permission semantics.
- `core/src/encryption/file_key_manager.rs` - master-key backed encryption.
- `signer/src/signer_daemon.rs` - decrypted key use and NIP-46 request approval.
- `web/src/lib/keycast_api.svelte.ts` - NIP-98 event construction.
- `web/src/lib/components/PermissionForm.svelte` and `web/src/lib/types.ts` - web permission shape.

## Current Priority List

See `AUDIT.md` for details. The current short list:

1. Rehearse the real Caddy deployment and verify the forwarded headers used by NIP-98 URL validation.
2. For existing deployments, follow `UPGRADE.md` and run `scripts/upgrade_preflight.sh` before
   starting the hardened containers.
3. Re-check the `.cargo/audit.toml` `instant` exception whenever the Nostr stack changes.
4. Revisit CSP inline styles if tooltip implementation changes make a stricter `style-src` practical.

## Documentation

Keep the root README focused on how the project works and how to run it. Keep package-specific
operational notes in each package README. Put dated bug/security audit detail in `AUDIT.md`.
