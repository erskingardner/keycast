# Keycast API

The API is an Axum service that owns team, key, policy, and authorization data. It stores data in the
repo-level SQLite database and uses NIP-98 HTTP auth events to identify the caller's Nostr pubkey.

## Responsibilities

- Initialize SQLite and run migrations from `database/migrations/`.
- Load the root `master.key` through `FileKeyManager`.
- Verify NIP-98 auth headers for `/api/*` routes.
- Enforce team admin checks before team, key, user, policy, and authorization changes.
- Encrypt stored Nostr private keys and bunker signing keys before writing them to SQLite.

## Local Commands

From the repo root:

```sh
cargo build --workspace
cargo test --workspace
bun run dev:api
```

Run only this crate:

```sh
cd api
cargo run
cargo test
```

## Current Audit Notes

- NIP-98 validation now binds requests to method, URL, timestamp window, and request-body payload hash.
- API allowlist enforcement reads `ALLOWED_PUBKEYS`. An empty allowlist allows all valid NIP-98
  pubkeys. The unauthenticated `/api/config?pubkey=<hex>` endpoint returns only whether that one
  pubkey is allowed for the browser sign-in gate.
- Authenticated request bodies are buffered for payload validation with a 1 MiB limit.
- Policy creation rejects empty, unknown, or malformed permission configs.
- Authorization creation verifies the selected policy belongs to the same team as the stored key.
- SQLite connections enable foreign-key enforcement. Team deletion deletes join rows first and then
  deletes now-orphaned permission rows for that team.
- Reverse proxies must preserve the original host and scheme. The middleware uses `Host`,
  `x-forwarded-proto`, and `x-forwarded-prefix` to reconstruct the NIP-98 `u` URL.

See the root [AUDIT.md](../AUDIT.md) before changing auth, permissions, or key-handling code.

## License

[MIT](LICENSE)
