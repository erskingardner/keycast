# Keycast 24-hour soak results — September 8, 2026

**The exercised protocol workload passed: 279 runs, zero failed runs.** This is
an improvement over the original run's 35 failures in 245 runs (14.3%). It is not
a production-security sign-off or completion of the full acceptance matrix.

## Completion and provenance

- Scheduled window: September 7, 08:55:38 UTC to September 8, 08:55:38 UTC.
- The VM service logged completion at 08:55:54 UTC and disabled its timer.
- The application stack remained running and healthy, with zero automatic
  restarts. HTTPS certificate verification and the landing-page request passed.
- Deployed runtime: `d4df4fe660d03e66c6abefa2804eecff71dd1f36`; image digests match
  the corrected deployment in [the run log](VM_TEST_RUN_2026-09-06.md).
- Every record has the same runtime revision and harness SHA-256:
  `d257e281e4739cce12ff1844aaa4baa04eed66995d48a444d04f5deb88ebec83`.
- First recorded run: September 7, 08:55:43 UTC. Last: September 8, 08:50:48 UTC.
  The longest gap was 315.94 seconds. The final 290 seconds before the deadline
  fall within the normal five-minute sampling cadence; there was no missing-run
  gap over seven minutes. This was periodic sampling, not continuous load.

Results and a SHA-256 manifest were preserved on the VM under
`/opt/keycast-test/archive/completed-24h-results`, separate from rolling retention.
Original results remain under `/opt/keycast-test/archive/original-soak-results`.
These archives contain sanitized outcomes and public request identifiers only.

## Operation results and latency

Each of the 13 checks passed in all 279 runs: 3,627 successful checks, including
279 relay-acknowledgment cleanup checks. There were no failed-operation latency
samples. Percentiles below use the nearest-rank method over successful checks.

| Check | Median ms | p95 ms | p99 ms | Maximum ms |
| --- | ---: | ---: | ---: | ---: |
| Ping | 223 | 322 | 373 | 447 |
| Get public key | 213 | 261 | 399 | 903 |
| Sign event | 216 | 273 | 296 | 314 |
| NIP-04 encrypt | 204 | 260 | 288 | 307 |
| NIP-04 decrypt | 203 | 253 | 269 | 334 |
| NIP-44 encrypt | 210 | 263 | 308 | 374 |
| NIP-44 decrypt | 203 | 251 | 264 | 278 |
| NIP-44 self-encrypt/decrypt, two requests | 403 | 491 | 504 | 534 |
| Deny disallowed event kind | 212 | 251 | 256 | 258 |
| Reject malformed ciphertext | 210 | 250 | 262 | 280 |
| Reject invalid recipient | 211 | 253 | 294 | 315 |
| Reject unknown method | 205 | 252 | 271 | 324 |
| Drain relay acknowledgments | 0 | 437 | 1,265 | 3,685 |

Signatures and crypto results were independently verified by `nostr-tools` with
disposable keys. No returned social event was published as a post. Draining relay
acknowledgments waits for all publication attempts to settle; that check passing
does not mean every relay acknowledged every publication.

## Relay and admission findings

The client observed 10,621 successful publication acknowledgments and 260
publication-attempt timeouts. The current classification does not distinguish
connection establishment from waiting for an acknowledgment. All 260 timeouts
were on the Ditto path: 7.17% of its 3,627 attempts, spread
across 20 runs. Other relays delivered the requests, and every requested operation
still completed successfully. There were zero `client_closed` classifications,
so these were not the harness's earlier connection-teardown artifact.

Signer-side durable counters changed as follows during the window:

| Relay | Retries | Connection losses | Remote close frames | Transport/TLS errors | Publication rejections |
| --- | ---: | ---: | ---: | ---: | ---: |
| nos.lol | 0 | 0 | 0 | 0 | 0 |
| Primal | 2 | 2 | 0 | 2 | 0 |
| Ditto | 12 | 5 | 1 (code 1001) | 5 | 0 |

Ditto also recorded five canceled connection attempts. These are distinct counters,
not necessarily distinct incidents; do not sum them into an incident total.
All three subscriptions were connected and accepted in every before/after sample.
Snapshots do not rule out brief readiness changes between samples.

Admission rejections rose from 0 to **39**: 29 between runs and 10 during runs.
Two between-run increases of 11 followed Primal reconnects. This is consistent
with replayed or delayed relay copies, but the aggregate counter cannot prove
that every rejection was redundant traffic. No tested operation failed. The
counter still needs investigation before declaring admission behavior fully
resolved under reconnect bursts or heavier load.

There were no parse errors or all-relay publication failures. One after-run sample
contained two pending replies aged two seconds; subsequent samples and the final
live database check contained zero pending work. No persistent backlog appeared.

## Resource observations

| Container | First MiB | Final MiB | Peak MiB | Median sampled CPU |
| --- | ---: | ---: | ---: | ---: |
| Signer | 15.04 | 22.43 | 26.05 | 0.47% |
| API | 0.94 | 1.04 | 1.04 | 0.01% |
| Web | 32.35 | 56.14 | 72.36 | 0.01% |
| Caddy | 19.20 | 19.45 | 20.09 | 0.00% |

Memory rose and later fell in the signer and web containers; this run did not
show runaway growth. A 24-hour, single-client sampled workload cannot establish
absence of a leak or capacity at higher concurrency. At review the VM had about
1.37 GB available RAM and 34 GB free disk. The SQLite file grew from 1.68 MB to
2.01 MB, with WAL around 4.17 MB at the final sample. Integrity checks passed in
every snapshot. No backup had been recorded.

## Next steps, in priority order

1. Investigate the remaining 39 admission rejections with a controlled delayed-copy
   and reconnect-burst test. Improve diagnostics to distinguish duplicate traffic
   from rejected new requests. Diagnose Ditto-path connection/acknowledgment timeouts before
   relying on it as the only working relay; keep redundant relays during testing.
2. Perform the Jumble browser walkthrough with disposable keys and exercise the
   outstanding policy, recipient, logout, revocation, expiry, and session-isolation
   cases. These 279 runs reused an existing session; they did not repeatedly test
   invitation claims or logout.
3. Rehearse VM reboot, power-loss/storage faults, trusted import, backup/restore,
   and root rotation against the deployed stack. Establish off-host backups and
   external alerts, and complete the independent security review before valuable
   keys are entrusted to the instance.

The one-time completion follow-up is being paused after this review. The VM's
test timer has already stopped itself; the Keycast application remains available.

## Follow-up investigation

See [the relay investigation](RELAY_INVESTIGATION_2026-09-08.md) for the reproduced
admission mechanism, Ditto connection-timeout evidence, twenty fresh diagnostic
runs, and the proposed NIP-65 discovery boundaries.
