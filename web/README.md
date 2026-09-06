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

## Team links

Teams have persistent name-based URLs such as `/teams/personal`. The signer assigns globally
unique slugs, adding `-2`, `-3`, etc. for collisions. Renaming a team preserves its existing URL.
Numeric bookmarks redirect to the slug, including nested key, member, and policy pages. The UI
resolves slugs from the signed-in user's team list; API requests still use numeric IDs and signer
authorization. The additive `0002_team_slugs.sql` migration and transactional startup backfill
assign slugs to existing v2 teams without recreating them.

## Workspace layout

Desktop workspaces use three columns: the team directory, section navigation, and the selected
section's content. Team title and counts sit above the section navigation and content. Keys,
Policies, Members, Activity, and Settings use shareable `?section=` links; unknown sections fall
back to Keys. Team switching retains the selected section, and numeric-to-slug redirects retain
query strings. At narrower widths, section navigation becomes a horizontal strip above the content.

## Relay diagnostics

The operator-only Instance page separates WebSocket connectivity from signing-subscription
acceptance. A connected relay with no active grants is idle, not failed. Expand each relay for
connection counters, latency, network traffic, signing checkpoints, and recent diagnostic history.
The signer wraps the SDK's default WebSocket transport to observe attempts, retries, successful
connections, remote close frames (including standard close codes), abrupt network/transport losses,
and classified HTTP/TLS/DNS errors. It preserves the SDK's TLS and proxy behavior. Local shutdown
is not counted as a remote close. Remote close frames prove peer closure, not fault: normal
maintenance is included, while abrupt network losses cannot be attributed confidently to a relay.
Retries exclude the first attempt after startup or re-enabling a relay. Cancelled attempts are
separate because SDK timeouts and local cancellation cannot always be distinguished.

Migration `0003_relay_reliability.sql` adds lifetime counters, hourly seven-day summaries (the
current hour plus 167 preceding hours), and diagnostic events grouped by minute/category. History
expires after seven days and is capped at 1,000 groups per relay; the UI shows the latest 50.
Trimming detailed history does not reduce counters. Disabling a relay preserves its history;
explicitly deleting the relay deletes its diagnostic records. Collection begins on upgrade: old
in-memory logs cannot be reconstructed.

Counters and history commit together, normally every second, with a final flush on graceful stop.
Unflushed observations can be lost on a crash or prolonged storage failure. The collector caps its
pending buffer at 2,048 groups and records an explicit lost-observation count if it overflows.
Database failures retain the pending transaction batch for retry. Retention runs each minute, and
reads exclude expired data immediately. This is diagnostic telemetry, not an audit-log guarantee.

Only finite, sanitized categories are persisted: no relay-provided text, Nostr payloads, event IDs,
private keys, or credentials. This works independently of `RUST_LOG`. The operator-only status API
returns counters, categorized breakdowns, and recent events. Live connection state is sampled every
ten seconds; refresh the status snapshot to see new data.

## Public profile display

Use `UserIdentity` for user references so avatars, display names, and npubs remain consistent.
Kind-0 metadata is fetched from the shared read relays and its author and signature are verified.
Only sanitized display names and avatar URLs are cached under `keycast:public-profiles:v1` in
browser local storage, indexed by public key. This is display data, never authorization state.

The cache holds at most 256 profiles, reuses entries for six hours across navigation and reloads,
and displays stale entries while refreshing. Failed or missing lookups retry after five minutes;
storage failures fall back to memory. Missing or broken avatars use a local placeholder. Image
bytes use the browser's normal HTTP cache. No managed keys or signing credentials enter this cache.

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
