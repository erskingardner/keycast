# V2 local validation — September 5, 2026

This records local validation of the V2 implementation, including the follow-through against all ten original
audit findings. Existing application databases and root files were preserved. All fault-test keys,
databases and containers were disposable fixtures. See [the closure matrix](V2_AUDIT_CLOSURE.md).

## Required checks

- `cargo fmt --all --check`, locked workspace check/build/test: passed. **60 Rust tests passed**;
  the explicitly invoked latency/soak test is excluded from the ordinary unit-test run.
- `cargo audit`: no known vulnerabilities in 288 locked dependencies.
- Root and web `bun audit`: no known vulnerabilities. Both Socket scans report no advisories in free
  mode; this does not claim a paid/organization scan.
- Web `bun test`: **31 passed**. `bun run check`: zero errors/warnings. Production build passed.
- Web `bun pm untrusted`: zero untrusted lifecycle-script dependencies.
- `python3 -m unittest discover -s scripts/operations -v`: **three passed**. Tests cover independent
  readiness/capacity/backup-age alerts, preservation after upload failure, and verified-upload ordering
  before success stamps/retention. External storage calls are mocked; no remote account was contacted.
- Development, production and Caddy example Compose files render. Production validation uses dummy
  test digests solely for rendering, not purported published artifacts.
- Docker API, signer and web runtime targets build. The combined `scripts/container-smoke.sh` passes
  with read-only filesystems, signer-only database/root mounts and a read-only API socket mount.

The container smoke rejects an ordinary NIP-98 write and actor-only socket grant creation. It creates a
team with external approval, changes relays as operator, imports a disposable key and obtains an
encrypted invitation through real HTTP/API/socket/signer paths. Browser-side request tests separately
exercise the actual API client with a test external signer, Unicode approvals, exact body hashes,
encrypted status handling, one-use response keys and plaintext-response rejection.

## Security and failure coverage

The suite exercises all team-resource routes as member/outsider, cross-team resource identifiers,
concurrent administrator deletion, policy tombstones and name reuse, ambiguous management tags,
instance/revision mismatch, replay, ordinary delegated NIP-98 versus management authorization, and
private import contents omitted from approval events.

Protocol tests cover all four NIP-04/NIP-44 operations, self-only policy scopes and session narrowing,
reserved management-kind denial, current expiry checks, ended-session rejection, forged signatures,
malformed/duplicate recipient tags, timestamp skew, and durable denial of oversized replies.

Resilience tests use the production relay loop for duplicate deliveries, durable input/outbox restart,
failed-delivery logout, corrupt-grant isolation, rejected subscriptions, relay authentication demands,
relay rotation with a live session, noisy-grant admission isolation and first-ACK publication followed
by bounded second-relay acknowledgement. A real daemon process is SIGKILLed after durable commit and
recovers its response and session; a competing daemon is refused without disturbing its socket.
SQLite's page ceiling produces a real SQLITE_FULL error without filling the host disk, and a separate
write transaction exercises database lock contention and recovery.

The CLI drill performs stdin key import, redacted audit export, online encrypted backup, overwrite
refusal, maintenance-lock exclusion, atomic root rotation, old-root refusal, restore revocation/hold,
and explicit recovery review. Streaming archive tests reject wrong keys, truncation, trailing data,
altered chunks, reordered chunks, and chunks spliced from another archive.

## Local latency and restart soak

Reproduce with:

```sh
cargo test -p keycast_signer --test hardening local_runtime_soak_and_latency_report \
  --locked -- --ignored --nocapture
```

`KEYCAST_SOAK_REQUESTS` controls the sample count (default 1,000). The test uses a disposable file-backed
SQLite database, real daemon binary, local WebSocket relay and a client that verifies every returned
signature. It forces SIGKILL/restart after each 250 requests and paces traffic below admission limits.
The relay's fixture rate limit is raised explicitly; production admission limits remain enabled.

| Measurement | Idle host, debug binary | During concurrent builds |
| --- | ---: | ---: |
| Verified signatures | 1,000 | 1,000 |
| Forced process restarts | 3 | 3 |
| Client round-trip p50 | 10.9 ms | 50.8 ms |
| Client round-trip p95 | 18.1 ms | 247.0 ms |
| Client round-trip p99 | 34.6 ms | 828.5 ms |
| Maximum round trip | 89.9 ms | 3,243.9 ms |
| Elapsed including pacing/restarts | 85.8 s | 171.8 s |
| Final pending inputs/replies | 0 / 0 | 0 / 0 |
| Final database / WAL size | 2.55 / 3.96 MiB | 2.55 / 3.98 MiB |

These are local client-to-client measurements including relay transport, signer decision/durable
commit and response receipt. They are not public-relay latency promises, production release benchmarks,
or separate timing histograms for each internal stage. The idle run is the appropriate local baseline;
the concurrent run demonstrates host contention affects tails. The final response-size denial change
was subsequently covered by its focused regression, full workspace tests and final container rebuild.

## Problems caught during validation

The earlier saturation test caught broad SQLite `INSERT OR IGNORE` suppressing quota-trigger checks;
admission now ignores only event-ID conflicts. Expanded testing caught a transient startup-readiness
race, which now fails closed until initial configuration completes. API client tests exposed header
merging with `Headers` objects; it now uses the native Headers API.

Harness issues were corrected: the container team-response shape, macOS Unix-socket path length,
the local relay's default 60-events/minute limit, a mistaken expectation that members cannot read their
own team's redacted audit, and a module mock that polluted unrelated web tests. The oversized-response
check uses Keycast's own event budget; the locked NIP-44 implementation supports extended payloads and
does not have the older 64 KiB plaintext limit.

## Explicit limits

Not validated here: a production TLS/VM deployment, independent security review, multi-day public-relay
uptime, actual machine power loss, an off-host destination, a rendered browser UI walkthrough, or every
external key store's approval interface. The supplied host jobs are not installed/enabled. Those
operator/deployment steps remain in `TODO.md`; they are not represented as passing local tests.


## PR 24 review follow-up (2026-09-05)

The review follow-up adds regression coverage for rejected-command revision/nonce isolation,
nondelegatable instance-bound management reads, operator-only status with mixed-case keys,
relay replacement at the 20-row limit (including disabled rows and retained checkpoints),
control-request panic isolation, transient pool-pressure recovery, per-grant storage pressure and
cross-team service, session-bound cached/outbox replies, server-side invitation deadlines,
and real slow HTTP uploads that time out while management reads continue.

Local results: **71 Rust tests**, **31 web tests**, and **3 operations tests** passed.
`cargo fmt --all --check`, locked workspace check/build, `cargo clippy --workspace --all-targets
--locked -- -D warnings`, Rust/Bun audits, Socket scans, Svelte type checks/build, and untrusted
script reporting passed. Both source/production Compose and the Caddy example render using
explicitly fictitious digests for the rendering check. All three Docker targets were built, and the combined read-only API/signer/web smoke passed.
Shell fixture checks verified that RNG failure leaves no published root credential, malformed
pubkey lists fail initialization, and only the first allowed key becomes an operator by default.

The 1,000-signature local daemon run passed with three forced SIGKILL/restarts, 1,001 retained
request records, no pending inputs/responses, and approximately 4 MiB WAL. Client round trips
were p50 **15.8 ms**, p95 **23.2 ms**, p99 **26.7 ms** (maximum 42.6 ms), measured in the debug
build with other local build activity. This is a local regression result, not a public-relay
latency or unattended-production guarantee.

The prerelease schema changed again; migration checksum refusal for an older disposable V2
database is intentional. No existing local database or production deployment was modified.
