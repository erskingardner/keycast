# V2 hardening contract

Approved September 5, 2026. Personal Docker instance: one active signer, one host, SQLite.
The host and signer are trusted; the API is an untrusted transport. V1 remains incompatible.
Pre-release V2 schema may change; existing local databases are never silently reset.

## Authority

- Only signer opens the authority database or root credential.
- Signer verifies original signed requests and enforces membership and operator roles.
- Reads use NIP-98. Mutations require a Keycast management event, bound to instance identity,
  current authority revision, method, canonical URL, exact body hash, verified command content and unique nonce.
- Management signing is hard-denied through every Keycast NIP-46 grant. The same identity may
  be managed and administer Keycast, but management approvals must come from its external key store.
- Mutations and signing admission are ordered under one authority gate; no network waits under it.
- Management responses containing invitation secrets are encrypted to a fresh browser key whose
  public key is included in the approval. The API cannot redeem a newly created invitation by
  reading its creation response. Browser import remains supported with an explicit frontend trust
  boundary; a local CLI is the safer alternative.

## Durable operation

- Persist incoming encrypted requests, final decisions, audit, responses and lifecycle changes.
- A bounded inbox/outbox recovers after process restart without depending on relay event storage.
- Duplicate delivery reaches application deduplication. First relay acknowledgement completes
  delivery; bounded retry/replication is independent of request admission.
- Logout invalidates admission durably while allowing its cached acknowledgement to be delivered.
- Normal restarts preserve sessions. Explicit restoration of an older backup invalidates sessions
  and invitations and pauses grants pending review of restored authority/policies.

## Operations

- Management remains available while relays are unavailable. Readiness tracks subscriptions and
  publication failures, not only connected sockets. Invalid individual grants are quarantined.
- Bound ingress, tasks, per-client work, persistent backlog and retention. Maintenance is
  incremental. Health probes are cheap; integrity checking is scheduled.
- Trusted CLI: offline import/root rotation/review, online encrypted backup, safe restore.
- Validate unit/contracts, real-runtime relay faults, crash recovery, web, audits and Docker smoke.
- Public multi-day soak, target-host setup and selecting off-host backup storage remain deployment
  work and cannot be claimed from local validation.
