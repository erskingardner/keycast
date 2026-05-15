# Keycast Web

The web app is a SvelteKit UI for team management, stored keys, policies, and authorization creation.
It asks the user's Nostr signer to sign NIP-98 HTTP auth events for API requests.

## Local Commands

```sh
bun install
bun run dev
bun test
bun run check
bun run build
bun audit
bun pm scan
bun pm untrusted
```

The repo-level dev command runs this app through `bun run dev:web`.

## Environment

- `VITE_DOMAIN` is optional for local development when the API is not on the same origin. If it has
  no protocol, the client assumes `https://`.
- The repo-level local dev command points the web app at `http://localhost:3100/api` to avoid common
  local services on port `3000`.
- Production images default to the current browser origin and call `/api/*`, so the same image can run
  behind any domain where Caddy routes `/api/*` to the API.
- The browser sign-in gate asks `/api/config?pubkey=<hex>` whether the current pubkey is allowed.
  This does not expose the full API `ALLOWED_PUBKEYS` value; the API remains the security boundary.

## Auth Flow

Sign-in uses the raw NIP-07 browser extension API to fetch the active pubkey. The app then uses
Applesauce's extension signer for NIP-98 event signing. `nostr-login` and NDK are no longer part of
the auth path.

`src/lib/keycast_api.svelte.ts` builds NIP-98 events with `u`, `method`, and optional `payload` tags,
signs them with NIP-07 through Applesauce, and sends the base64-encoded event in the
`Authorization` header. `src/lib/nostr.ts` owns browser signer access and public profile/contact
reads through Applesauce's event store and relay pool.

The cookie named `keycastUserPubkey` is only a UI route hint. It is not proof of authorization. API
authorization must stay server-side.

## Current Audit Notes

- Keep NIP-98 request bodies and payload hashes in exact sync with the API request body.
- The browser allowlist check asks the API for a boolean decision for one pubkey at a time, but API
  enforcement is the security boundary.
- Create forms now prevent default browser submission before running async signing/API work.
- Allowed-kinds policy forms now emit the same `allowed_kinds` config shape that Rust validates.
- Permission parsing helpers are covered by `bun test`; keep route protection, allowlist checks, and
  NIP-98 tag construction in pure helpers where possible.
- Do not restore localStorage private-key sign-in without a fresh security review.
- `bun test`, `bun run check`, `bun run build`, `bun audit`, `bun pm scan`, and `bun pm untrusted`
  are clean.
- Production responses set CSP, frame, content-type, referrer, permissions, and HTTPS HSTS headers.
- The build script patches `svelte-adapter-bun`'s generated websocket probe so a missing websocket
  hook does not create a runtime warning.

See the root [AUDIT.md](../AUDIT.md) before changing auth, permission forms, or key entry pages.

## License

[MIT](../LICENSE)
