# AGENTS.md

This file inherits the root `AGENTS.md` guidance. It applies to `signer/`.

## Signer Responsibilities

- `keycast_signer` watches authorization rows and manages signer child processes.
- `signer_daemon` decrypts the bunker key and stored user key for one authorization.
- NIP-46 requests are approved only after `Authorization::validate_policy` says yes.

## Things To Be Careful With

- The daemon handles decrypted private keys in memory. Keep logs clean and avoid adding debug output
  that includes secrets or full bunker URIs.
- Process restarts should not widen authorization. A crashed signer should restart with the same auth
  ID and the same policy checks.
- Do not make expiration, max-use, or connection-secret failures look like generic approval.
- `MASTER_KEY_PATH` is passed to child processes, but the current file key manager reads the root
  `master.key` path directly. Update both sides if changing key lookup.
- If policy parsing fails, fail closed.

## Validation

```sh
cd signer && cargo test
cargo test --workspace
cargo build --workspace
```

Use targeted tests in `core` for policy approval semantics, then run signer builds to catch integration
breakage.
