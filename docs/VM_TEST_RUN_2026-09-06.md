# Disposable VM test run — September 6, 2026

The test instance is live at https://keycast.ipf.dev on `91.98.92.28` (Debian 13,
approximately 2 GB RAM). This records initial deployment and protocol evidence,
not completion of the acceptance matrix or approval for real private keys.

**Completed:** the corrected 24-hour workload finished September 8 with all 279
runs passing. See [the completion report](SOAK_RESULTS_2026-09-08.md) for remaining
relay/admission findings, performance, and coverage limits. The application remains running.
The historical deployment and original-run evidence below are retained for comparison.

## Build provenance

Source: `f1a487cb1e66c47a5967d397bdc0b89a4310fdc3` on master.
GitHub CI run `34028247045` and Docker Images run `34028247057` passed.
The image workflow built all three targets and ran the combined read-only-container smoke.

Images deployed by immutable digest:

- API: `ghcr.io/marmot-protocol/keycast-api@sha256:a39dff496ea9604740d35cf63b02e2adb29b11729cd3c9981415bba15290f014`
- Signer: `ghcr.io/marmot-protocol/keycast-signer@sha256:f7e6dd9dbf5ee00a28b9a415b4e747cb94b52bff08bdaab10408abfd15ad32cf`
- Web: `ghcr.io/marmot-protocol/keycast-web@sha256:e3c37b47b9790dd053eb8693ba0eeae9218329c8b14c957952f39b35adcbe5b1`
- Caddy: `lucaslorentz/caddy-docker-proxy:2.13.1-alpine@sha256:3e6cf4a6382da0cd14108a69929d7f8cc59e9a59cae4c9795071fa392ffa8036`

## Deployment

`/opt/keycast` on the VM contains the production Compose file, a static Caddy
override, and private configuration/state. Caddy runs with a static configuration
without a Docker socket mount. Only HTTP/HTTPS and the host's SSH service are
public; API and web application ports are internal. HTTPS certificate validation
and the public landing page passed. Signer, API, and web health checks passed.

The root credential was randomly generated on the VM and is mounted only into
the signer. Registration and operator access are restricted to the owner's
previously supplied public identity and a disposable harness administrator.
All harness private keys are disposable and remain in the VM's private test
directory. The owner is also an administrator of the `VM acceptance` team.

## Initial live results

An independent `nostr-tools` JavaScript client exercised the HTTPS management
API and NIP-46 over public relays. The first bootstrap attempt used an incorrect
import approval description in the harness; the server rejected it. Correcting
the harness to match the verified management description resolved setup.

The first completed run passed 13 checks:

- Invitation claim/connect, ping, and expected managed public key.
- Kind-1 signing with independent event hash/signature verification. The returned
  event was not published as a social post.
- NIP-04 and NIP-44 encryption and decryption, independently verified in both
  directions with a disposable peer and Unicode plaintext.
- NIP-44 self-encryption/decryption.
- Explicit rejection of a disallowed event kind, malformed NIP-44 ciphertext,
  invalid recipient public key, and an unknown method.

The signer reported readiness and three connected relays. Individual single
operations in the first run took 139–274 ms; the self-encryption round trip took
482 ms. These are small-sample public-relay observations, not latency guarantees.

After a controlled signer container restart, two further runs passed all 12
repeatable checks using the persisted client session without claiming another
invitation. The four application/proxy containers used approximately 66 MiB in
one post-run sample; this is not evidence of long-term memory stability.

## Running soak

The VM's `keycast-soak.timer` runs `keycast-soak.service` every five minutes.
Each run opens a client connection using the existing session and repeats the
12 checks above. The workload has a fixed end time of **2026-09-08 11:10 UTC**.
The test was shortened from 72 to 48 hours at the owner's request, retaining the original start time. It runs independently of the developer Mac. Completion leaves the application
stack running. A one-time Codex follow-up is scheduled for September 8 at 13:20 Europe/Rome
to review completion and notify the owner with results. No external alert receiver
has been configured.

Host paths:

- `/opt/keycast-test/acceptance.mjs`: protocol harness.
- `/opt/keycast-test/run-soak.py`: bounded runner and sanitized results recorder.
- `/opt/keycast-test/results/soak-YYYY-MM-DD.jsonl`: outcomes, timings, and Docker
  CPU/memory/network/block-I/O observations. No private keys, plaintext messages,
  invitations, complete bunker URLs, or ciphertext are included.
- `/etc/tmpfiles.d/keycast-soak.conf`: seven-day result retention, serviced by
  the enabled systemd temporary-file cleanup timer.

Harness SHA-256: `6570f35df219e680e939c427ac370fac77b4bee1f8b41c299324d833f99bcb4f`.
Runner SHA-256: `5fb0d947febf853e00ecbf73728e44b4c34bf45bbf32ce4b350d5e0309e5afcd`.

Safe operational commands on the VM:

```sh
systemctl list-timers keycast-soak.timer
systemctl status keycast-soak.service
journalctl -u keycast-soak.service --since today
systemctl stop keycast-soak.timer
```

Do not print files in `/opt/keycast-test/secrets` or the deployment's private
configuration/root credential. Bunker URLs are kept only in private harness state.

## Still pending

### Interim check — September 7, 08:24 UTC

At roughly 21 hours, 245 recorded runs contained 210 passes and 35 failures
(14.3% of runs). There were 2,824 successful operation checks, with no gaps over
seven minutes. Every recorded ping, public-key, signing, and NIP-04 check passed.
Failures occurred during NIP-44, self-encryption, and expected-denial checks; failed
runs lasted approximately 31–33 seconds, consistent with the harness's 30-second
operation deadline. Existing result files do not retain the exact error class,
so these timings alone do not prove every failure is a transport timeout.

Successful signing had a 229 ms median and 273 ms p95. Successful individual
crypto operations had medians of 194–209 ms and p95 values of 246–258 ms. These
latencies exclude failed operations; they must not be presented as all-request
latencies. One successful unknown-method rejection took 29.3 seconds.

HTTPS and all application health checks passed; there were no automatic container
restarts. Live signed status reported readiness, database integrity, three accepted
relay subscriptions, no pending inputs/responses, and no quarantined grants. The
host had about 1.4 GB available RAM and 34 GB free disk. Signer memory grew from
10.8 to 20.0 MiB, including a step up overnight; web memory settled near 53 MiB.
This is modest usage but does not yet establish absence of a memory leak.

The signer reported 624 ingress rejections. Its admission path can silently drop
requests before durable processing when concurrency/rate bounds are hit. Audit
records for three recent failed runs ended before the operation at which the
client failed, making admission/delivery a priority for investigation. This is
evidence to investigate, not a confirmed root cause. Relay telemetry separately
recorded 221 publication rejections each on nos.lol and Primal, classified as
message-format rejection. Primal recovered from a disconnect and three HTTP 502
failures; Ditto recovered from one connection loss. These observations do not
establish that the relays caused the failed runs.

At the interim check the deployed build and workload were unchanged. The completion
review must investigate these failures rather than call the soak successful based
only on healthy containers.

### Early stop and fixes — September 7

At the owner's request, the original workload was stopped after approximately
21 hours. The VM timer was disabled, its completion reminder paused, and the
application stack left running. Original results, harness, runner, and deadline
were preserved under `/opt/keycast-test/archive/original-*` on the VM.

A three-relay regression reproduced one request causing three response
publications when other relays delivered their copies after the first response.
The runtime now coalesces these copies within a bounded two-second window before
they consume admission capacity. Same-relay retransmissions remain eligible;
other-relay retries become eligible after the short window. Failed work is removed
from this cache, and the existing durable recovery path remains in place.

A separate relay probe reproduced nos.lol and Primal rejecting a 510-second-old
reply as an expired ephemeral event, while Ditto accepted it. Cached NIP-46
responses now refresh their outer signed envelope once it is 30 seconds old,
preserving the encrypted RPC result and independently rechecking grant/session
authority. A real-relay regression rejects the old envelope, accepts its refreshed
replacement, verifies one operation execution, and verifies that revocation stops
publication. Diagnostics now identify expired ephemeral replies instead of
incorrectly calling every `invalid:` rejection a message-format problem.

The versioned `scripts/operations/nip46-soak.mjs` harness retains exact failure
classes, per-relay publication acknowledgments with safe reason categories,
request event IDs for audit correlation, and signer status before/after each run.
It requires `KEYCAST_TEST_API`, `KEYCAST_TEST_SOURCE`, and existing disposable
`/secrets/keys.json` and `/secrets/state.json` fixtures inside the web container.
The test matrix and 30-second operation deadlines are retained. A fresh 24-hour
run will use separate results after the corrected build passes deployment gates.

### Corrected deployment and 24-hour restart

Runtime source `d4df4fe660d03e66c6abefa2804eecff71dd1f36` was deployed after CI
`34102271610` and Docker Images `34102271609` passed, including all three image
builds and the combined read-only-container smoke. Both Compose files rendered
successfully. Local Rust/web/security/operations checks passed. An additional
local run completed 1,000 requests and three process restarts without failures,
with zero pending work (17.7 ms median and 21.9 ms p95 on local relays).

Corrected immutable image digests:

- API: `sha256:1c8477f4ac292d34ac06210e6e7d3b3e03936551b40511b232be69180c4e870a`
- Signer: `sha256:d07b99a42dc4a9e5ee612721e62ab9ffcde1c729a95adffe786ff1ee848a7c0d`
- Web: `sha256:7b7511ec3342ac9730184eb09514e3fab9e86bcf43b9fadc9d2b723972c69c20`

The image repositories and Caddy digest remain as listed above. HTTPS and all
application health checks passed after deployment. Twenty consecutive public-relay
runs passed 240 operation checks with zero admission rejections and no increase
in signer-side relay error counters. Their detailed results are retained in
`/opt/keycast-test/archive/preflight-after-fix.jsonl`. This batch alone does not
establish that all intermittent failures are eliminated.

The harness now drains outstanding relay acknowledgments before closing the
client, rather than counting its own connection teardown as a relay error. This
adds a thirteenth check to the existing twelve checks. Any forced cleanup during
a failed run is classified separately as `client_closed`. The first actual new
soak run passed all 13 checks with zero ingress or client publication failures.

The new VM timer runs every five minutes, ends after 24 hours, and leaves the
application stack running. Its deadline termination was exercised successfully
before starting the new run. The runner records the runtime source and harness
hash on every run. Current script hashes:

- Harness: `d257e281e4739cce12ff1844aaa4baa04eed66995d48a444d04f5deb88ebec83`
- Runner: `453caedfc34f8f34d63ed18d7dc20c61d67277a7a603bccb7b7c221213b6e370`

`/opt/keycast-test/soak-started` and `soak-until` are authoritative for the new
window. The old completion reminder has been replaced with a one-time review on
September 8 at 11:06 Europe/Rome, comparing the new results against the original
run. The acceptance and recovery items below remain open.

Jumble browser flows have not been tested. The full matrix in `TODO.md` remains
open: logout/invitation reuse/expiry, individual crypto permission and recipient
restrictions, isolation, payload boundaries and malformed templates, browser and
trusted CLI import, backup/restore/rotation, relay outages, VM reboot/power loss,
storage faults, policy edits and revocation during active sessions, and completed
multi-day resource/latency analysis. A successful initial run does not close those
items. Off-host backup and external alert delivery are also outstanding.
