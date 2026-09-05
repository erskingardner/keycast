# V2 audit closure matrix

This maps the original ten architecture findings to current code and local evidence. It does not
claim independent review, a production deployment, or years of operating history. V2 remains the
chosen architecture: a single trusted signer and SQLite authority behind an untrusted API transport.

| Original finding | Implemented resolution | Local evidence |
| --- | --- | --- |
| API can manufacture authority | Signer-only DB/root, original externally signed commands verified in signer, narrow socket wire protocol | Complete HTTP route/role/cross-team matrix and combined container API/socket smoke |
| Delegated NIP-98 can administer its own key | Private management kind 27236, always denied through NIP-46; same identity approved from its external store; bound instance/revision/body/nonce/reply key | Delegated-write denial, replay and context ambiguity tests, Unicode browser-client signing/encryption tests |
| Any admitted user can replace relays | Explicit operator allowlist, parsed URL validation, signer enforcement and audit | Nonoperator rejection and exact-host/URL tests |
| Cached requests are not durable recovery | Encrypted durable inbox/outbox, transactional response/audit/logout, Message-level duplicates, current authority checked before preparation | Production runtime duplicate/restart tests; actual SIGKILL before delivery followed by recovery; logout failed-publication test |
| Connected sockets falsely imply readiness | Initial-load gate, EOSE/CLOSED handling, quarantine/integrity/recovery hold, publication failures, bounded subscription backoff, watchdog/shutdown deadlines | Rejected and authentication-required relays, actual process startup, rotation onto a new subscription |
| Resource growth and starvation | Global/client/grant admission, RAII capacity release, fair recovery/outbox selection, persistent byte/row/page caps, incremental retention, redacted status and monitor | Rotating-client noisy-grant test, budget saturation and SQLite-full recovery, bounded map tests, host-monitor tests and measured WAL/backlog after soak |
| One malformed grant or worker stops service | Per-grant validation/quarantine, incremental relay reload, tracked request/publisher/replication tasks, bounded shutdown | Healthy grant continues beside corrupt envelope; changing relays preserves session; cancellation capacity tests |
| Restore revives old authority | Online encrypted streaming snapshots; authenticated manifest and ordered chunks; atomic root rewrap; new-instance restore hold/revocation; single-signer locks | CLI import/export/backup/rotation/restore/overwrite drill; chunk truncation/reorder/splice rejection; second daemon refused |
| Signing path and publication latency | Indexed recipient query, coalesced session heartbeat, immediate outbox wakeup after decision, first-ACK completion with bounded replication | Real signed-event round trip; delayed second ACK does not delay first ACK and is subsequently recorded; reproducible latency/restart soak |
| Smaller correctness gaps | Correct hex decoder, key-use/claim expiry checks, authority gate and live-session check, last-admin serialization, exact p tags, policy tombstones, pinned images | Envelope/schema/unit tests, timestamp/forged-event and oversized-response denial tests, concurrent administrator removal, policy removal/reuse regression, Docker build and read-only smoke |

The approved browser-import tradeoff remains explicit: the browser, frontend and API receive the key
at import time. The trusted local CLI avoids that path. Passkeys were not added. An API transport
cannot manufacture a valid management approval or decrypt a legitimate creation reply, but it can
interrupt delivery; a compromised frontend can mislead an approval screen. Host root and the external
key store remain trusted.

The host-operation templates provide upload verification, local retention, audit export and alertable
status. Selecting a remote account/retention policy, enabling timers, configuring the alert receiver,
public TLS/VM rehearsal and multi-day public-relay validation require the operator's actual deployment.
