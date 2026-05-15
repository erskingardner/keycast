# AGENTS.md

This file inherits the root `AGENTS.md` guidance. It applies to `api/`.

## API Responsibilities

- Verify NIP-98 auth for every `/api/*` route.
- Extract the caller pubkey from the verified auth event.
- Enforce team admin authorization for team, key, user, policy, and authorization mutations.
- Encrypt incoming private keys before database writes.
- Return structured `ApiError` responses instead of panics for user-controlled input.

## Things To Be Careful With

- NIP-98 `u`, `method`, timestamp, and `payload` checks are security boundaries.
- If an endpoint accepts a body, the signed payload hash must match the exact bytes the handler sees.
- Do not rely only on `VITE_ALLOWED_PUBKEYS`; browser config is public and bypassable. Server allowlist
  enforcement belongs in API middleware.
- Policy IDs must be scoped to the current team.
- Avoid `unwrap` and `expect` in request paths. Malformed tags, JSON, dates, and IDs should return
  client errors.
- When deleting teams, keys, or policies, verify join rows and dependent rows are removed in a safe
  order.

## Validation

```sh
cd api && cargo test
cargo test --workspace
cargo build --workspace
```

Add focused tests for auth and authorization changes. If middleware needs to read the request body,
prove that downstream JSON extraction still works.
