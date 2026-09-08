# Relay investigation — September 8, 2026

Investigation against runtime `d4df4fe660d03e66c6abefa2804eecff71dd1f36`, current
local master, and the preserved completed-soak results. No production source,
relay configuration, images, or timers were changed by this investigation.

## Findings

### Reconnect bursts can consume admission for completed requests

`signer/src/admission.rs` remembers cross-relay copies for two seconds. Copies
from the same relay remain eligible as possible client retries. In
`signer/src/runtime.rs`, incoming events pass admission before
`RequestProcessor::prepare_response` checks the durable response cache. Admission
allows two simultaneous workers per client, sixteen requests per second per
client, and separate grant/global limits. Reconnecting subscriptions request a
five-minute lookback.

A standalone diagnostic compiled the actual admission module and delivered
thirteen requests, completed their workers, then replayed those event IDs with a
synthetic arrival time three seconds later. Keeping admitted replay workers busy
produced **two admitted and eleven rejected**. This reproduces the admission
mechanism and the size of two historical spikes, not the actual historical
network sequence or an end-to-end runtime reconnect. The original soak saw two
increases of eleven following Primal reconnects; seventeen other rejections
remain unassigned. The aggregate metric also includes durable request-capacity
rejections, so it cannot establish the cause of each historical rejection.

The existing cross-relay regression covers copies spaced 150 ms apart, not a
backlog outside the two-second window. This is a real coverage gap. Completed
replays can compete with fresh requests even though replay does not repeat the
cryptographic operation.

Recommended next implementation: distinct bounded handling/budgets for cached
response retries versus new execution, preserving signature/session/revocation
checks and recovery admission. Coalesce repeated response delivery while keeping
legitimate retries possible. Do not simply lengthen the suppression timeout:
that can suppress a client retry after a lost response. Add counters for the
actual rejection limit, cache/replay handling, and relay source without logging
request content. Test reconnect bursts mixed with new requests, late copies,
same-relay retries, revocation, and process restart.

### Ditto failures strongly resemble connection establishment timeouts

All 260 historical Ditto timeouts occurred in twenty runs, with all thirteen
publication attempts failing on that relay in each affected run. The affected
samples cluster on September 7: twelve from 14:51–15:48 UTC and eight from
17:47–18:44 UTC. Other relays carried every operation successfully.

The installed `nostr-tools` pool shares an outstanding connection promise and
uses a 3,000 ms connection deadline; publication acknowledgment has a separate
4,400 ms deadline. Summed protocol and acknowledgment-drain times for affected
runs were 2,980–2,997 ms in eighteen runs, 3,074 ms in one, and 6,029 ms in one.
That strongly supports shared connection timeouts, potentially two attempts in
the six-second run, rather than thirteen separate missing acknowledgments. It
is an inference: the old harness retained only the generic `timeout` class.
A long-lived healthy signer connection does not prove a new client can connect.

A temporary phase-instrumented copy of the versioned harness classified errors
prefixed by `connection failure:` separately from publication errors and recorded
elapsed time. Twenty fresh VM runs passed all checks, with **780 successful
publication attempts, zero timeouts, and zero new admission rejections**. Thus
the intermittent failure did not reproduce in this diagnostic window.

Sanitized diagnostic results are preserved separately at
`/opt/keycast-test/diagnosis-2026-09-08/run-*.json`, alongside the probe source.
The completed soak archive was not modified. The stack remains healthy and the
soak timer remains inactive.

Recommend improving permanent phase/latency diagnostics before another soak.
Ditto was the least reliable of the three paths in this VM test, but this does
not prove universal relay unreliability. Treat it as optional while qualifying
another independent transport relay. Do not remove an advertised endpoint
without testing client relay switching and preserving an overlap period.

## NIP-65 discovery on key import

The current import path seals/stores the key without discovering kind 10002.
Runtime relay configuration is instance-wide and operator-controlled, capped at
twenty relays. Every active grant's transport public key is subscribed on every
configured relay; bunker URLs and the implemented `switch_relays` response both
advertise that shared set. Current defaults include Ditto in the initial database
and frontend relay constants; changing fresh-install defaults alone would not
change existing databases or previously issued bunker URLs.

[NIP-65](https://github.com/nostr-protocol/nips/blob/master/65.md) lists where the
user writes social events and reads mentions. It does not declare that a relay
accepts remote-signing traffic. [NIP-46](https://github.com/nostr-protocol/nips/blob/master/46.md)
uses signer/client transport keys and advertised relays for signing RPCs. The
managed identity can differ from the grant's transport identity. A client must
also learn and adopt any added transport relays; changing the signer alone does
not move its requests. Keycast already implements `switch_relays`, and the
installed client library has a corresponding method, but actual Jumble adoption
and migration behavior still need an end-to-end check.

Recommendation: discover and cache NIP-65 on import, presenting its relays as
candidates with provenance and compatibility status. Keep a small, qualified
baseline of signing relays. Allow automatic activation under an explicit
instance-operator policy, rather than interpreting any imported list as authority
to change the entire instance's network configuration.

Requirements for that implementation:

- Fetch asynchronously using the public key; importing must work when discovery
  is unavailable. Verify event ID/signature/author/kind, bound response sizes and
  tag counts, choose the latest valid replaceable event deterministically, and
  handle future timestamps. Cache the event ID, timestamp, fetch time and expiry;
  periodically refresh with bounded retries and retain the last known good list.
- Normalize/deduplicate URLs. For automatically discovered endpoints, enforce
  public-network destination restrictions at connection time, including DNS
  resolution/rebinding, IPv6, and redirects if supported. Current operator URL
  validation checks scheme and credentials; it is not this untrusted-input
  network boundary. Private/local relays should require explicit operator setup.
- Bound per-key and instance relay counts, connections, discovery work and retry
  rates. Share connections but associate discovered relays with their owning keys;
  scope subscriptions/publication so one imported key does not disclose all
  teams' transport relationships to its relays. Track references for removal.
- Test kind 24133 subscription, delivery and publication compatibility. A NIP-65
  relay might be authenticated, paid, or reject ephemeral events. Discovery alone
  must never authorize signing a NIP-42 challenge with a managed key.
- Advertise approved per-key transport choices in bunker URLs and `switch_relays`;
  retain old endpoints while existing clients migrate. Test this using Jumble.
  Keep discovered social read/write roles distinct from signer transport health.

Adding more relays is not by itself a latency fix. First-success publication
already allows a healthy path to complete quickly, while additional shared
relays increase traffic, duplicate handling and connection work.

## Order of work

1. Harden replay admission and add the missing mixed fresh/replay regression.
2. Preserve diagnostic phase and limit counters; qualify transport defaults and
   verify Jumble relay switching, signing, encryption and decryption.
3. Implement bounded cached NIP-65 discovery and controlled relay activation.
4. Repeat a 24-hour soak on the resulting immutable build, including reconnect
   bursts and individual-relay failure checks rather than only healthy-path RPCs.
