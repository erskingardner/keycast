# Relay hardening rollout — September 8, 2026

## Runtime and validation

Deployed runtime: `a942648bdfab0b25d062883c328c44d65173d32b` on the disposable VM
at `keycast.ipf.dev`. CI and the Linux image build/combined read-only-container
smoke passed before deployment:

- [CI](https://github.com/marmot-protocol/keycast/actions/runs/34214697570)
- [Image build and smoke](https://github.com/marmot-protocol/keycast/actions/runs/34214697635)

Immutable images:

- API: `sha256:5473eff2c3ce96978135b168348ccb2387560811d4ebc73f49c665ba64899d63`
- Signer: `sha256:91a950b1a01de07f64a71399fbf25006174d0171f557d0184c46c33c9d06a949`
- Web: `sha256:60ab3d9a12bad8290d390a2c2a9e5a9621d26fe0e2c359b432a6f0de94c4b130`

Local checks passed: Rust formatting/check/build/workspace tests/audit, Python
operations tests, root and web JavaScript dependency audits/scans, web tests,
Svelte check/build, both Compose renderings, all three Docker targets and the
combined container smoke. The explicitly ignored local latency benchmark was
not part of the workspace suite. A macOS test helper now allows cold process
launch separately after a process sample showed the new executable waiting at
`_dyld_start` before Rust main. The existing readiness deadline and production
behavior are unchanged; Linux CI also passed the recovery test.

Deployment preserved the existing root credential, consistent SQLite snapshots,
old image digests and Compose/environment files in private rollback directories.
All three application containers became healthy; HTTPS returned 200.

## Discovery and transport evidence

An imported disposable key published a signed NIP-65 event containing a public
relay and a loopback URL. The signer cached the verified event, discarded the
loopback URL, and left the candidate inactive until an external operator approval
enabled activation. The new invitation advertised the activated route.

Damus-only client tests exposed two relay limitations: intermittent **HTTP 503
WebSocket upgrades**, confirmed with Node's HTTP diagnostics channel, and
**publication rate limiting** during rapid sequential RPCs. Keycast's durable
relay history recorded the rejected response publication. A first successful
compatibility probe is therefore not sufficient evidence for making this relay
the only client route. One intermediate test also failed because the one-off
script zeroed its disposable client key too early; that script was corrected.
These failed diagnostics are retained, not counted as successful acceptance.

Coracle passed ten ACK-and-echo probes across two windows. `relay.nsec.app`
timed out in both five-probe windows. `relay.jeffg.fyi` rejected all five ephemeral
publication probes and was not activated. These are observations from this VM
and time window, not universal judgments about those relays.

A newer signed list added Coracle alongside Damus. The signer adopted the new
event, filtered the private address again, and activated Coracle. For this test,
**Coracle and Damus are discovered routes scoped to the disposable key**;
`nos.lol`, Primal and Ditto remain the baseline. This preserves Jumble's original
routes and ensures baseline availability cannot mask failure of the discovered
transport path. Both discovered connections use the restricted public connector.

Using only those discovered routes, acceptance passed identity/ping,
`switch_relays`, signature verification, NIP-04 encryption/decryption, and NIP-44
encryption/decryption against an independent peer implementation. Damus rejected
some publications while Coracle delivered the RPCs. The unrelated key had no
association with these discovered routes; local integration also asserts scoped
response publication and expired-route exclusion.

Jumble successfully logged in with a separate disposable grant and retained the
same displayed identity after browser reload following deployment. Its observed
login did not issue `switch_relays`; current upstream source also lacks an
explicit call in its bunker wrapper. No Jumble social post or DM was sent.
Signing/crypto and switching coverage here comes from the protocol harness;
Jumble's complete UI operation matrix remains open.

## Repeat workload

The versioned harness records connection versus publication failures, HTTP upgrade
status where Node exposes it, latency, admission/replay counters and relay health.
It runs 19 checks when the second fixture is present: the original baseline
operation/denial matrix plus signing and both directions of NIP-04/NIP-44 through
discovered routes only. Multiple published copies are recorded individually;
a rejected relay copy does not imply an RPC failed when another route delivered it.

Sanitized discovery diagnostics are retained at
`/opt/keycast-test/relay-investigation-2026-09-08`; preflight outcomes are in
`/opt/keycast-test/relay-discovery-preflight-2026-09-08`. The earlier completed
279-run soak remains archived separately. The runtime commit and the harness
file hash are recorded in each new soak result.

Preflight completed at 10:34:45 UTC: **12/12 runs passed, 228 checks, zero new
fresh-admission rejections**. Five runs were normal, two ran while Primal's
connection was deliberately reset/blocked inside only the signer's network
namespace, and five followed a signer restart. All test firewall rules were
removed. The client retained sessions across the restart. Individual publication
results included 68 Damus rate-limit rejections and one Damus HTTP 503; redundant
routes delivered every tested RPC. These relay failures remain in the results.

The fresh 24-hour window started **September 8 at 10:37:09 UTC** and ends
**September 9 at 10:37:09 UTC (12:37 Europe/Rome)**. The first systemd workload
passed all 19 checks with valid integrity and no new admission rejection. The
five-minute timer is enabled on the VM and stops scheduling when the deadline
is reached. This is a sampled workload, not continuous maximum-load traffic.

Harness revision: `d0caf3d6535286ae726c1129c6fabeb78ad7acb1`; file SHA256:
`a892317710da8217d3fb9823886a0635aaca8f27b1d744bed6c41eb9f6b8e682`.
Runtime remains pinned to the independently validated `a942648` images above.
The existing completion reminder was updated for September 9 at 12:45 Europe/Rome.
The window is now complete: 277/278 runs passed, with one unresolved ping
timeout. See [the September 9 results](SOAK_RESULTS_2026-09-09.md). This is not
an independent security certification.
