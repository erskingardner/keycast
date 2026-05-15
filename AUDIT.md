# Keycast Audit - May 14, 2026

This audit focused on bugs and security issues, not cosmetic UI polish.

## Current Status

The initial audit found critical request-auth and policy-evaluation bugs. The follow-up hardening
passes fixed the high-risk application issues, removed abandoned web auth code, cleared active Rust
and Bun audit vulnerabilities, removed build/runtime warnings, and tightened the Docker deployment
defaults. The latest passes also added a live-upgrade runbook, preflight checks for existing
deployments, and a broader Rust/Bun test suite that found and fixed more correctness issues.

Validation after the fixes:

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
bash -n scripts/init.sh scripts/upgrade_preflight.sh scripts/docker-entrypoint.sh scripts/healthcheck.sh scripts/generate_key.sh
DOMAIN=example.com ALLOWED_PUBKEYS=abcdef KEYCAST_UID=10001 KEYCAST_GID=10001 docker compose config
docker compose -f caddy-docker-compose-example.yml config
DOCKER_BUILDKIT=1 docker build -t keycast-runtime-check .
```

Results:

- `cargo check --workspace` passed with no warnings.
- `cargo build --workspace` passed.
- `cargo test --workspace` passed with expanded API route, authorization, and permission tests.
- `cd web && bun test` passed with helper coverage for route protection, allowlists, NIP-98 tag
  building, and permission config parsing.
- `cd web && bun run check` passed with no warnings.
- `cd web && bun run build` passed with no warnings.
- `cargo audit` exited quietly. `.cargo/audit.toml` documents the ignored upstream `instant` advisory,
  which comes from `nostr`'s wasm32 target dependency and is not used by the native runtime.
- `bun audit` and `cd web && bun audit` reported no vulnerabilities.
- `bun pm scan` and `cd web && bun pm scan` reported no advisories with
  `@socketsecurity/bun-security-scanner@1.1.2` in Socket free mode.
- `cd web && bun pm untrusted` reported zero untrusted packages with lifecycle scripts.
- Shell syntax validation passed for the deployment scripts.
- Migration `0002_normalize_allowed_kinds_permissions.sql` was smoke-tested against a SQLite database
  and converted legacy `allowed_kinds` configs from `{"sign":[...]}` to `{"allowed_kinds":[...]}`.
- `scripts/upgrade_preflight.sh --help` renders usage successfully.
- Docker Compose config rendering passed with explicit `ALLOWED_PUBKEYS`, `KEYCAST_UID`, and
  `KEYCAST_GID` values.
- The full Docker image builds with a small `.dockerignore`-controlled context, digest-pinned base
  images, frozen frontend installs, production frontend dependencies, and `cargo build --release
  --locked`.
- The built API image starts under a read-only root filesystem, writes only to the bind-mounted
  database directory, and applies SQLx migrations 1 and 2 on a fresh SQLite database.
- The built web image starts as UID/GID `10001`, serves `/health` under a read-only root filesystem
  with `/tmp` tmpfs, and returns CSP, HSTS, frame, referrer, permissions, and content-type headers.

## Fixed Findings

### Fixed: NIP-98 auth did not bind write bodies

The API middleware now buffers authenticated bodies up to 1 MiB, validates the NIP-98 `payload` tag
against the exact request bytes, and reconstructs the body before handing the request to Axum JSON
extractors. Body-bearing requests without a payload tag are rejected.

Coverage:

- `api::http::tests::validates_payload_hash_for_body_requests`
- `api::http::tests::rejects_missing_or_mismatched_payload_hashes`

### Fixed: NIP-98 `u` and `method` tags were optional, and future timestamps were accepted

The API now requires exactly one `u` tag and exactly one `method` tag, validates them against the
current request, and rejects stale or far-future events. The URL validator reconstructs the public API
URL from `Host`, `x-forwarded-proto`, `x-forwarded-prefix`, and the nested Axum route URI.

Coverage:

- `api::http::tests::validates_required_url_and_method_tags`
- `api::http::tests::rejects_missing_url_or_method_tags`
- `api::http::tests::rejects_future_timestamps`

### Fixed: Intended allowed-kinds restrictions could become allow-all

Rust and TypeScript now use the same `allowed_kinds` config shape. Rust also denies unknown config
fields, so the previous web shape no longer deserializes as an unrestricted permission.

Coverage:

- `custom_permissions::allowed_kinds::rejects_unknown_config_fields`
- `types::authorization::tests::test_allowed_kinds_denies_disallowed_event_kind`

### Fixed: Empty or invalid policies could approve signing

Policy creation now rejects empty permission sets, unknown permission identifiers, and malformed
permission configs. The signer now default-denies sign/encrypt/decrypt requests when a policy has no
custom permissions, and invalid permission config returns an authorization error instead of panicking.

Coverage:

- `types::authorization::tests::test_empty_policy_denies_signing`
- `types::authorization::tests::test_invalid_permission_config_denies_without_panic`

### Fixed: `ALLOWED_PUBKEYS` was frontend-only access control

The API now enforces exact comma-separated pubkey allowlists from `ALLOWED_PUBKEYS`, with
`VITE_ALLOWED_PUBKEYS` as a local fallback. The browser parser now uses the same exact-match behavior.
Docker passes `ALLOWED_PUBKEYS` into the API container.

Coverage:

- `api::http::tests::allowlist_uses_exact_pubkey_matches`

### Fixed: Rust dependencies had active advisories

The lockfile now resolves the active advisory paths to fixed versions, including `bytes`, `ring`,
`rustls-webpki`, `time`, `crossbeam-channel`, `tokio`, and `tracing-subscriber`. The unmaintained
`dotenv` crate was replaced with `dotenvy`.

The SQLx dependency was moved from the aggregate `sqlx` crate to `sqlx-core` plus `sqlx-sqlite` so
unused MySQL/Postgres packages and the unfixed `rsa` advisory are no longer in the lockfile.

The Nostr Rust stack was also moved from the old pinned git revision to current crates.io releases:
`nostr` 0.44.2, `nostr-sdk` 0.44.1, and `nostr-connect` 0.44.0. This required updating the NIP-46
request type name and connect field, and replacing a deprecated timestamp accessor.

`cargo audit` still needs a documented exception for `RUSTSEC-2024-0384` because the latest `nostr`
crate keeps `instant` as a wasm32 target dependency. Native `cargo tree -i instant` is empty, and
Keycast ships native Linux containers.

### Fixed: Web dependencies had active advisories and stale peer resolutions

`bun audit` reported high and moderate advisories through stale peer-resolved copies of Vite, Svelte,
Tailwind 3, Rollup, PostCSS, glob, minimatch, picomatch, yaml, brace-expansion, and cookie.

The web package now uses exact dependency versions, upgrades Vite to the older patched Vite 7 line,
keeps the compatible Svelte plugin line, and adds targeted overrides for `cookie`, `postcss`,
`rollup`, `svelte`, `tailwindcss`, and `vite`. `bun audit` now reports no vulnerabilities.

A later Svelte advisory required an exception to the "about a month old" dependency policy:
`svelte` was moved to 5.55.7 because that is the patched security release. NDK was deduped to 2.15.2,
which removed the stale `websocket-polyfill` path and the old `es5-ext` lifecycle-script warning.
`@biomejs/biome` was removed because it was unused and only contributed a blocked install script.

### Fixed: `bun pm scan` was unavailable

Bun's `pm scan` command requires an npm scanner package plus a `bunfig.toml` setting. The repo now
configures:

```toml
[install.security]
scanner = "@socketsecurity/bun-security-scanner"
```

Both Bun projects install `@socketsecurity/bun-security-scanner@1.1.2` exactly. Scans work without a
token in Socket free mode; set `SOCKET_API_KEY` only when you want organization policy checks.

### Fixed: `nostr-login` dependency and fallback auth path

The web app no longer imports or installs `nostr-login`. Sign-in now uses the raw NIP-07 browser
extension API to fetch the active pubkey and attaches NDK's NIP-07 signer for subsequent event
signing. This keeps the login path on an actively supported browser-extension boundary.

### Fixed: Deployment defaults carried avoidable risk

Deployment hardening changes:

- Docker builds now use digest-pinned Rust, Bun, Debian, and Caddy images instead of moving tags.
- Cargo and Bun install steps use lockfile enforcement.
- `master.key` is no longer copied into the image during the web build or final image assembly.
- The API service is exposed only on the Docker network instead of publishing host port 3000.
- Docker Compose and `scripts/init.sh` now require `ALLOWED_PUBKEYS` for deployment.
- Production signer logging defaults to `info` instead of `debug`.
- The web entrypoint no longer prints full container network diagnostics while waiting for the API.
- `.dockerignore` now excludes Rust and web build output, package installs, local databases, `.env`
  files, and `master.key` from the Docker build context.
- Runtime containers run as UID/GID `10001` by default, use a read-only root filesystem, drop Linux
  capabilities, set `no-new-privileges`, and mount only `/tmp` as writable tmpfs.
- `master.key` and signer config are mounted read-only; the SQLite database directory remains the
  only writeable bind mount.
- The runtime image includes `database/migrations` and creates `/app/signer` so standalone container
  runs and compose file mounts both land on existing paths.
- `scripts/init.sh` writes `KEYCAST_UID` and `KEYCAST_GID`, tightens local `master.key` and
  `database/` permissions, and recursively sets host ownership for the runtime user when possible.
- `scripts/upgrade_preflight.sh` checks existing deployments for required env, host `master.key`,
  required migrations, SQLite integrity, foreign-key issues, invalid permission JSON, legacy
  allowed-kinds config rows, and old root-owned bind mounts before the non-root runtime starts.
- [UPGRADE.md](UPGRADE.md) documents the live upgrade path, backup pair, key recovery path from old
  containers, verification checks, and rollback requirement to restore the database and matching
  `master.key` together.
- The final image installs only essential runtime packages and copies production frontend
  dependencies rather than the full build-stage `node_modules`.
- The Caddy example uses a digest-pinned `caddy-docker-proxy` image and mounts the Docker socket
  read-only.

### Fixed: Remaining warnings were removed

The unused `TeamResponse` type was removed from the API type surface. The Svelte tooltip component now
loads `tippy.js` dynamically on the client, so the SSR build no longer reports an unused external
default import. Vite aliases NDK's `tseep` import to `tseep/lib/ee-safe.js`, removing the production
`eval` warning instead of suppressing it.

The SvelteKit Bun adapter generated a runtime websocket probe that logged a warning because the
current SvelteKit server object does not expose `server.websocket`. The build now patches the generated
handler to call that hook only if it exists.

### Fixed: Web security headers were missing

The SvelteKit server hook now sets a restrictive CSP, `X-Content-Type-Options: nosniff`,
`X-Frame-Options: DENY`, `Referrer-Policy: no-referrer`, and a permissions policy disabling camera,
microphone, and geolocation. HSTS is set when the request is already HTTPS or when a trusted proxy
sends `x-forwarded-proto: https`.

### Fixed: Authorization could use a policy from another team

Authorization creation now checks `policies.id` together with `team_id` before attaching a policy to a
stored key authorization.

### Fixed: Team deletion could leave orphan permissions

Team deletion now captures the permission ids for the team's policies, deletes `policy_permissions`
first, and then deletes now-orphaned permission rows. SQLite connections enable
`PRAGMA foreign_keys=ON`.

Coverage:

- `api::http::teams::tests::delete_team_removes_related_rows_without_orphaning_permissions`

### Fixed: Existing authorization users could be blocked after `max_uses`

Authorization `max_uses` now limits new pubkey redemptions, not reconnects by pubkeys that already
redeemed the authorization. The connect flow validates the remote signer pubkey and secret, then
allows existing redeemed pubkeys before checking whether the authorization is fully redeemed.

Coverage:

- `types::authorization::tests::connect_is_idempotent_for_same_pubkey_and_max_uses_blocks_new_pubkeys`
- `types::authorization::tests::connect_with_wrong_secret_does_not_redeem`

### Fixed: Nested app routes were not protected by the server hook

The SvelteKit server hook now treats `/teams/...` and `/keys/...` as protected paths instead of only
checking exact `/teams` and `/keys` matches. The cookie remains a UI hint only; API authorization is
still the security boundary.

Coverage:

- `web/src/lib/utils/routes.test.ts`

### Fixed: Permission config edge cases were too permissive

`content_filter` and `encrypt_to_self` now reject unknown config fields, matching the stricter
`allowed_kinds` behavior. Empty blocked-word segments are ignored so a trailing comma cannot create an
empty string that blocks every sign/encrypt request.

Coverage:

- `custom_permissions::content_filter::rejects_unknown_config_fields`
- `custom_permissions::content_filter::ignores_empty_blocked_words`
- `custom_permissions::encrypt_to_self::tests::rejects_unknown_config_fields`
- `web/src/lib/utils/permission_config.test.ts`

### Fixed: Several forms did not prevent default submit behavior

Create-team, add-user, add-key, add-authorization, and add-policy forms now call `preventDefault` before
running async signing/API work. Non-submit controls in those forms use `type="button"`.

### Fixed: Test suite was not green

The authorization test fixture now creates the missing `users`, `permissions`, and `policy_permissions`
tables needed by policy-validation tests. `cargo test --workspace` now passes.

## Residual Risks

- NIP-98 URL validation depends on correct reverse-proxy headers. Production should preserve `Host` and
  set `x-forwarded-proto`; only set `x-forwarded-prefix` if the public API prefix differs from `/api`.
- CSP still allows inline styles because Tippy uses inline positioning styles. Removing that would need
  a tooltip implementation change or a nonce-compatible style strategy.
- The Caddy example still needs access to the Docker socket for service discovery. It is mounted
  read-only and `no-new-privileges` is set, but Docker socket exposure remains a privileged deployment
  decision.
- The preflight cannot prove every existing NIP-46 client will reconnect cleanly after the signer
  restart. Test at least one existing authorization during the upgrade window.
- `RUSTSEC-2024-0384` is intentionally ignored because it is a wasm-only upstream `nostr` dependency in
  this native runtime. Re-check this exception whenever the Nostr stack is updated.

## Next Work

The most pressing remaining work is the actual end-to-end deployment rehearsal against the live Caddy
setup: run [UPGRADE.md](UPGRADE.md)'s preflight on the server, verify the exact forwarded headers,
confirm the public NIP-98 `u` URL matches what clients sign, start the hardened containers, and test
sign-in plus one existing NIP-46 authorization before considering the upgrade complete.
