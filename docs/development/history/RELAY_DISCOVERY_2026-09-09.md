# Imported-key relay discovery

Key import queues NIP-65 discovery in the signer. Import does not wait for the
network. Key details show the cached list, fetch time, social read/write roles,
compatibility result and whether a relay is active for signing.

The worker queries a public relay-list indexer and up to two operator-configured
baseline relays. It verifies signatures, author, kind, size and timestamp, chooses
the newest event (lowest event ID breaks equal-timestamp ties), and retains the
last known good list on lookup failure. Successful lists refresh every six hours;
missing/unavailable lists retry after thirty minutes. One discovery job runs at a
time, with a 90-second deadline and bounded messages/candidates. No imported key
is used for relay authentication or qualification.

An instance operator can enable **Activate compatible public relays** on Instance
status using an external signed management approval. It is off initially. Turning
it off stops new activation; existing client routes stay available. The Disable
discovered relay action creates an explicit disabled operator override, preventing
rediscovery from re-enabling that URL. Discovery does
not grant team members permission to change the instance's baseline relays.

Automatic connections require WSS on port 443, with no URL credentials, query or
fragment. Each connection resolves DNS once, rejects non-public/special addresses,
connects to one of the vetted addresses, and verifies TLS against the original
hostname. Proxies and redirects are not used. Operator-configured baseline relays
retain support for explicitly configured local/private infrastructure.

A candidate must acknowledge an ephemeral throwaway event and deliver it through
an accepted subscription before activation. This proves basic transport behavior
at that moment, not continued availability or universal NIP-46 compatibility. A
relay requiring authentication is not automatically granted access to a managed
key for NIP-42 authentication.

Limits: eight safe candidates from an event, sixteen retained hints per key,
four associated active relays per key, and twenty relays for the instance including
baseline relays. Capacity exhaustion leaves candidates inactive. Removed hints
retain their existing route for a 24-hour migration overlap before cleanup.
Connections are shared, but subscriptions and response publication on discovered
relays are scoped to the owning keys. Fresh requests retain their own admission
budget; verified cached retries use a separate bounded verification/delivery lane.

New bunker invitations advertise the key's current baseline and discovered routes.
NIP-46 `switch_relays` returns the same set. Existing clients must adopt updates;
adding listeners alone cannot redirect their requests. Keep baseline routes stable
for clients that do not implement relay switching. The Jumble browser login tested
September 8 succeeded but did not send `switch_relays`; its current upstream bunker
wrapper also does not explicitly call it. This is not a claim that every deployed
Jumble version behaves identically. Do not remove its existing routes on an assumed
migration. Protocol-harness relay switching is tested separately.

Fresh-install signing defaults are `wss://nos.lol`, `wss://relay.primal.net`, and
`wss://relay.damus.io`. The frontend uses the same set for signing bootstrap and
default publication. Ditto and Bucket are excluded. A forward migration updates
the untouched bootstrap set before any grant exists; configured sets and existing
client routes are preserved.

Bucket was useful in the transport tests below, but its advertised 30-second
retention makes it unsuitable as a general metadata fallback. It is not a default.
VM qualification: Coracle and Damus each passed five ACK-and-delivery checks;
relay.nsec.app timed out on all five connection attempts. These short probes do
not establish long-term reliability. Existing clients retain the old endpoints
while replacement routes are tested.
