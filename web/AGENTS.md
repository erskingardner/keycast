# AGENTS.md

This file inherits the root guidance and applies to `web/`.

- `VITE_*` config is public. Never put credentials or managed private keys in browser storage.
- The pubkey cookie and config check are UI hints; signer authorization is authoritative.
- Keep NIP-98 payload hashes byte-for-byte aligned with the body sent.
- Keep policy/grant TypeScript shapes synchronized with Rust and reject incomplete forms.
- Keep explicit NIP-07, NIP-55, and NIP-46 sign-in paths; do not restore hidden local-key handling.
- Show one-time bunker URLs only from the creation response and do not persist them.
- Prevent default form submission before asynchronous signing.

Run `bun test`, `bun run check`, `bun run build`, audits, scans, and untrusted-script reporting.
The production adapter is the official SvelteKit Node adapter.
