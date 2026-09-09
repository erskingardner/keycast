# Relays

[Documentation](README.md) · [Operations](operations.md) · [Architecture](architecture.md)

Nostr apps exchange encrypted remote-signing requests and replies with Keycast through relays.
The signer shares connections across grants. Operators configure a baseline relay set; imported
keys can also have discovered routes associated with them.

## Baseline routes

Fresh installations use `wss://nos.lol`, `wss://relay.primal.net`, and `wss://relay.damus.io`.
These are configured defaults, not promises about a relay's current availability. Existing configured
sets and client routes are preserved when defaults change.

An admitted instance operator can edit baseline relays and the minimum-connected threshold on
**Instance**. Changes require external signed approval. Team membership does not grant permission
to change global relays. URLs use `wss://`; `ws://` is accepted only for loopback tests. Duplicate
URLs are rejected, and the threshold cannot exceed the number of enabled relays.

Readiness requires accepted signing subscriptions, not just connected WebSockets. With no active
grants, a connected relay may be idle and the signer can be ready without relay connections after
its initial configuration pass. A relay demanding NIP-42 authentication is unavailable for signing;
Keycast does not use managed keys as relay-authentication identities.

## Imported-key discovery

Import queues a background lookup of the key's NIP-65 relay list; import does not wait on the
network. Key details show the cached list, fetch time, social read/write roles, compatibility
result, and whether a route is active for signing. A social relay-list entry alone does not prove
that the relay supports the signing traffic Keycast needs.

The worker queries a public relay-list indexer and up to two operator-configured baseline relays.
It verifies signature, author, kind, size, and timestamp; newest events win, with lowest event ID
breaking equal-timestamp ties. Lookup failure keeps the last known good list. Successful lists
refresh every six hours; unavailable or missing lists retry after thirty minutes. One job runs at
a time with a 90-second deadline and bounded candidates/messages.

## Activate discovered routes

**Activate compatible public relays** is initially off. An operator can enable it on **Instance**.
Turning it off stops new activation while preserving existing client routes. **Disable discovered
relay** records an explicit disabled override so rediscovery cannot silently re-enable that URL.

Automatic connections require WSS on port 443, with no credentials, query, or fragment. Each
connection resolves DNS once, rejects non-public/special addresses, connects to a vetted address,
and verifies TLS against the hostname. No proxies or redirects are used for these connections.
Explicitly configured baseline relays can still use local/private infrastructure.

A candidate must acknowledge an ephemeral throwaway event and deliver it through an accepted
subscription before activation. Imported keys are not used for qualification or relay authentication.
The probe establishes transport behavior at that moment, not continued reliability.

Discovery considers eight safe candidates per event and retains sixteen hints per key. At most
four associated active relays per key and twenty total instance relays, including baseline relays,
are allowed. Capacity exhaustion leaves candidates inactive. Removed hints retain their route for
a 24-hour migration overlap before cleanup. Discovered subscriptions and response publication are
scoped to the associated keys even though connections are shared.

## Keep clients connected during changes

New invitations advertise the key's current baseline and discovered routes. NIP-46 `switch_relays`
returns the same set. Existing clients must actually adopt those updates: adding a listener on the
server cannot redirect a client that continues sending to an old relay.

Keep baseline routes stable for clients that do not implement relay switching. Test a route change
with your intended client before removing working endpoints. Historical browser and protocol tests
are recorded in the [relay investigation](development/history/RELAY_INVESTIGATION_2026-09-08.md)
and [rollout](development/history/RELAY_ROLLOUT_2026-09-08.md); they do not establish compatibility
for every client version.

## Read diagnostics

Expand a relay on **Instance** for connection attempts, retries, latency, network traffic, signing
checkpoints, and recent diagnostic history. Live connection state is sampled every ten seconds;
refresh the status snapshot to see new data.

Connectivity, subscription acceptance, request receipt, publication acknowledgement, and client
receipt are different stages. A first relay acknowledgement completes delivery tracking while
bounded redundant sends continue. If a client times out, check all stages rather than interpreting
the connected count as end-to-end success.

Counters include categorized HTTP/TLS/DNS failures, remote close frames, abrupt transport losses,
and cancelled attempts. A remote close proves peer closure, not fault; normal maintenance can close
a socket too. Abrupt network loss cannot be confidently attributed to a relay. Retries exclude
the first attempt after startup or re-enabling.

Lifetime counters are accompanied by hourly seven-day summaries and recent events grouped by
minute/category. History is capped at 1,000 groups per relay; the UI shows the latest 50. Disabling
a relay preserves history; deleting it removes its diagnostics. Collection starts when the feature
is installed and does not reconstruct older logs.

Observations normally commit every second and flush on graceful shutdown. A crash or sustained
storage failure can lose unflushed data; a bounded pending buffer reports lost observations on
overflow. Persisted categories exclude relay-provided free text, event IDs, payloads, private keys,
and credentials. These diagnostics help investigation but are not an audit-log guarantee.
