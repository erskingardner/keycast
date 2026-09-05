# AGENTS.md

This file inherits the root guidance and applies to `core/`.

- Keep the clean v2 migration and cross-team composite foreign keys intact.
- Policy parsing and evaluation must deny empty, unknown, malformed, or unconstrained capabilities.
- Keep Rust policy documents synchronized with TypeScript forms/types.
- Root credentials must be 32-byte base64 or hex private files/systemd credentials.
- Bind every envelope to version, team, record, public key, and purpose; verify derived public keys.
- Use real migration-backed tests for database behavior.
- Never log keys, plaintext, ciphertext, invitation secrets, or complete bunker URLs.

Run `cargo test -p keycast_core`, the workspace suite, and `cargo audit` for security changes.
