# AGENTS.md

This file inherits the root `AGENTS.md` guidance. It applies to `core/`.

## Core Responsibilities

- Database connection and migration helpers.
- Shared team, user, key, policy, permission, and authorization types.
- Encryption through `KeyManager` implementations.
- Permission evaluation for signer approval.

## Things To Be Careful With

- Permission evaluation must default to deny when a policy is empty, invalid, or cannot be parsed.
- Rust config structs and web TypeScript config types must stay in sync.
- Use explicit validation for permission JSON. Unknown fields should not silently become allow-all.
- Do not log private keys, decrypted key bytes, bunker secrets, or full connection strings with secrets.
- `FileKeyManager` currently reads `master.key` from the repo/runtime root. If you make this
  configurable, update API, signer, Docker, docs, and tests together.
- Tests that exercise database logic should use the real migration schema or a fixture that matches it.

## Validation

```sh
cd core && cargo test
cargo test --workspace
cargo audit
```

Security-sensitive changes should include regression tests for both the allowed and denied paths.
