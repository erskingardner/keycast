# Relay-discovery soak results — September 9, 2026

**Completed with one unresolved end-to-end timeout; not a clean pass.**

The scheduled window ran September 8, 10:37:09 UTC through September 9, 10:37:09 UTC. At review, the deadline had passed, the timer was disabled/inactive, its final service invocation exited successfully after checking the deadline, and all application containers were healthy. Runtime images and harness hash matched the [rollout record](RELAY_ROLLOUT_2026-09-08.md). The latest master CI and image workflows also finished successfully; the VM remained on the intended `a942648` runtime.

## Workload and failure

- **278 runs: 277 passed, one failed** (99.64% run success). There were 5,263 successful checks and one failed check; the failed run stopped at its first ping, so its remaining 18 checks were not attempted.
- All 554 attempted signature checks passed. Every attempted NIP-04/NIP-44 check, including both directions through the discovered routes, passed. Expected policy/malformed-input denials also passed.
- On September 9 at 08:44:35 UTC, the first baseline ping was published; all three baseline relays acknowledged it within 594 ms. The signer audit records `nip46.ping` as allowed for that exact event ID in the same second. That audit row is written atomically with the cached response. The client nevertheless timed out after **30,018 ms**.
- The before/after snapshots stayed ready with valid integrity, no pending work, and unchanged admission/replay counters. No relay diagnostic event was retained in the immediate failure window. The next scheduled run passed.
- The short-lived processed-request record was pruned before this review. Available evidence establishes receipt and response caching, but cannot distinguish reply publication/delivery failure from client subscription/response handling. A relay ACK to the request does not prove reply delivery.

## Admission and replay

Eight fresh-admission rejections occurred together at **13:41:43 UTC on September 8**, all from Ditto and all classified `admission_running_client` (the per-client concurrent-worker limit). They occurred between successful workload samples, roughly 19 hours before the ping timeout. No new audited operation exists in that minute; rejected event IDs/validation reasons were not retained. Their exact cause remains unproven.

The runtime handled **641 cached retries** during the sampled window, with no new replay throttling, storage rejection, parse error, or aggregate relay-publication failure counter increment. The pre-existing 882 replay throttles belonged to preflight and were not new soak failures. The fresh-request path still acquires worker admission before outer-event validation; a bounded regression around stale/malformed input and cache retention is warranted, without treating that code observation as proof of the historical burst.

## Client publication results

These count individual request copies, not distinct outages or signer replies. Shared connection failures may affect several request copies.

| Relay | Acknowledged / attempted | Failed copies |
|---|---:|---|
| nos.lol | 4,126 / 4,156 | 30 timeout |
| relay.primal.net | 4,156 / 4,156 | None |
| relay.ditto.pub | 4,156 / 4,156 | None |
| relay.damus.io | 615 / 1,662 | 932 rate limited, 68 service unavailable, 47 timeout |
| bucket.coracle.social | 1,662 / 1,662 | None |

The 68 Damus service-unavailable copies had HTTP 503 evidence. Coracle delivered every discovered-route request copy, and all discovered-route RPC checks passed despite Damus failures. This makes Coracle the stronger measured transport here; discovery compatibility alone does not make Damus suitable as a sole route. Ditto had no client publication failure in this window, although its signer-side connection still dropped.

## Durable signer relay diagnostics

Full-window retained events (including the short tail after the final workload sample):

| Relay | Reconnect attempts | Peer close frames | Other connection losses | Error observations |
|---|---:|---:|---:|---:|
| nos.lol | 0 | 0 | 0 | 0 |
| Primal | 1 | 1 | 0 | 0 |
| Ditto | 8 | 1 | 7 | 7 |
| Damus | 34 | 1 | 25 | 1,179 |
| Coracle | 0 | 0 | 0 | 0 |

Damus errors comprise 1,146 publication rejections, 32 transport errors and one timeout. These error categories can overlap a connection-loss episode; they must not be added as independent outages. Lifetime and seven-day category snapshots were retained separately and include pre-soak history. No dropped-observation counter increase appeared in workload snapshots.

## Successful-operation latency

Nearest-rank percentiles, in milliseconds. Failed ping latency is reported separately above. Paired crypto checks contain two RPCs; `switch_relays` also exercises two requests.

| Check | Median | p95 | Maximum |
|---|---:|---:|---:|
| ping | 234 | 329 | 427 |
| sign_event | 224 | 268 | 368 |
| nip04_encrypt | 213 | 262 | 417 |
| nip04_decrypt | 206 | 250 | 335 |
| nip44_encrypt | 205 | 249 | 284 |
| nip44_decrypt | 213 | 254 | 299 |
| switch_relays | 420 | 502 | 606 |
| discovered_relay_ping | 841 | 1156 | 1872 |
| discovered_relay_sign | 401 | 481 | 664 |
| discovered_relay_nip04 | 792 | 837 | 1717 |
| discovered_relay_nip44 | 791 | 837 | 1738 |

## Unattended operation and resources

- No container restart during the window. All readiness/integrity snapshots were healthy; post-run SQLite `quick_check` returned `ok`. Pending inputs/responses were zero at every sample and none remained at review. Sampling cannot exclude short transients between checks.
- Completion-to-completion gaps were 304.7–334.3 seconds (median 310.3); none exceeded six minutes. The last workload completed at 10:34:58 UTC, before the next timer invocation stopped at the deadline. There is no unexplained recording gap at the configured five-minute-after-completion cadence.
- Signer memory rose **17.68 → 25.02 MiB**, with a 25.02 MiB maximum. That is small in absolute terms but still an upward trend, not proof of leak-free long-term behavior. API memory ended near 1.04 MiB; web memory ended at 58.26 MiB after a 77.71 MiB peak; Caddy stayed near 19.6 MiB.
- Signer sampled CPU median was 0.58%, p95 2.41%. Database bytes grew 2,711,552 → 3,612,672; WAL remained around 4.2 MB. The VM has 33 GB free. No backup timestamp is recorded; off-host recovery work remains open.

## Prioritized next work

1. Reproduce the initial-ping reply-loss case. Retain bounded, sanitized request/response IDs and per-relay reply outcomes, plus client subscription readiness and receive/decrypt/correlation stages. Verify a safe bounded retry returns the cached result without repeating a key operation. Do not hide the timeout by increasing its deadline.
2. Explain the Ditto admission burst and add a regression for stale/invalid traffic and cache-retention boundaries. Keep validation and execution budgets bounded so the fix does not create a verification DoS path.
3. Prefer the measured reliable routes while preserving existing client access. Treat Damus as optional; retain its errors in the UI. Finish Jumble relay adoption, signing and messaging/encryption UI coverage—the harness results do not establish that UI behavior.
4. Before trusted-key use, complete independent security review, off-host backup/restore/rotation and VM/storage-fault rehearsals. Profile the memory trend on the next run after the two reliability questions above are addressed.

## Evidence and disposition

Completed results, exact harness, runtime provenance and SHA256 manifest are preserved under `/opt/keycast-test/archive/completed-relay-discovery-soak-2026-09-09`. Sanitized analysis and category snapshots are stored with that archive. The app stack is left running; no new soak was started and no runtime or relay setting was changed during this review. The one-time completion reminder is retired after this report.

## Subsequent decision — September 9

Jeff accepted these results as sufficient to end the soak phase and proceed toward
normal personal use. No further timed soak is planned. The timeout and admission
burst remain documented follow-ups. Ditto is excluded from fresh-install signing
defaults. The final selected defaults are nos.lol, Primal and Damus; existing configured routes remain
operator-controlled. This decision does not change the measured results above.
