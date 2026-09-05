# Keycast Web

The SvelteKit UI manages teams, imported keys, strict policies, grants, one-time invitations, relay
configuration, and operational status. It asks an external Nostr signer to create every NIP-98
authorization event.

Sign-in supports NIP-07 browser extensions, Amber/NIP-55 clipboard signing, and NIP-46 remote
signers. The browser never stores a managed private key. The `keycastUserPubkey` cookie is only a
route hint; the API is the authorization boundary.

`VITE_DOMAIN` may point local development at a different API origin. Production defaults to the
current origin and Caddy routes `/api/*`. All `VITE_*` values are public and must never contain
secrets.

The production build uses the official `adapter-node` and runs on Node 24.

## Commands

~~~sh
bun install --frozen-lockfile
bun test
bun run check
bun run build
bun audit
bun pm scan
bun pm untrusted
~~~

Keep NIP-98 body bytes synchronized with the signed payload hash, keep Rust/TypeScript policy shapes
identical, and run check plus build for security-sensitive UI changes.

## License

[MIT](../LICENSE)
