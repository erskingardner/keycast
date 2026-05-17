# AGENTS.md

This file inherits the root `AGENTS.md` guidance. It applies to `web/`.

## Web Responsibilities

- Render team, key, policy, and authorization management views.
- Build NIP-98 events and ask the user's Nostr signer to sign them.
- Send API requests with the signed event in the `Authorization` header.

## Things To Be Careful With

- `VITE_*` values are public browser config. Do not put secrets there.
- The `keycastUserPubkey` cookie is a UI route hint, not proof of API authorization.
- Keep NIP-98 body hashes in exact sync with the JSON sent to the API.
- Permission form data must match the Rust structs exactly.
- Do not store private keys in localStorage or sessionStorage.
- Do not restore `nostr-login`; sign-in should stay on the explicit signer-session paths in
  `src/lib/nostr.ts` unless there is a fresh security review.
- Remote signer session storage may contain NIP-46 client material, but never the user's Nostr
  private key.
- Submit handlers should prevent default browser form submission before doing async signing.
- For security-sensitive UI changes, run both typecheck and build.

## Validation

```sh
cd web && bun run check
cd web && bun run build
cd web && bun audit
cd web && bun pm scan
cd web && bun pm untrusted
```

The current check, build, audit, scan, and untrusted-script gates are clean. The production build runs
`scripts/patch-adapter-websocket.js` after Vite so `svelte-adapter-bun` does not log a missing
websocket hook warning at runtime.
