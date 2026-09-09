# Architecture

[Documentation](README.md) · [Policies and access](policies-and-access.md) · [Security model](security.md)

## Product boundary

Keycast is designed for one self-hosted VM and a small number of personal, family, or team keys.
Its priorities are correct authorization and key isolation, unattended reliability, transparent
status, and simple operation. SQLite and one signer instance are deliberate choices. Horizontal
scaling, multi-region failover, and high-volume multi-tenancy are out of scope.

## At a glance

```text
 External management signer
          | signed approvals
          v
       Browser ---- HTTPS ----> API ---- private Unix socket ----> Signer
                                                                   |  |
 Nostr apps <------------------- shared relays ----------------------+  |
                                                                      v
                                                           SQLite + root credential
```

The browser uses an external signer to approve management actions. Apps use NIP-46 to request
operations under a grant. Both paths meet inside the signer, where authorization and key use
are ordered against the same durable state.

## Runtime boundaries

`keycast-web` is a public SvelteKit/Node service. It contains no server secret and speaks only HTTP
to the API.

`keycast-api` is a public Axum transport. It forwards exact requests and original signatures to the
signer and returns management responses, with write replies encrypted by the signer. It has no database or root credential.

`keycast-signer` owns SQLite, root credentials, authorization, NIP-46, relay supervision and audit.
Its management router verifies instance-bound private kind-27237 reads. Writes require a private Keycast kind-27236 approval
bound to instance, authority revision, nonce, method, canonical URL, body hash, verified command
content and a fresh reply public key. All Keycast grants hard-deny both private management kinds; ordinary NIP-98 remains delegatable for other services. The same Nostr identity
can therefore be hosted and administer the server through an external key store without passkeys.

Only signed HTTP forwarding and redacted status are accepted on the socket. Actor-only lifecycle
operations exist solely inside the trusted signer process. Team administrators control their own
resources; host-configured operators control global relays. Management writes and request preparation
share one authority gate; relay network waits never hold it. The API socket mount is read-only.

Management write replies are NIP-44 encrypted to the fresh recipient in the signed approval,
using a stable root-derived sender identity pinned by the browser. An API-only
attacker cannot recover a newly created invitation bearer from its reply. With the correct
reply identity pinned, it also cannot forge the result. Read replies remain visible to the API.
See [management approvals](security.md#management-approvals) for first-use verification and rotation. A browser-imported key is
still exposed to the frontend/API during import; use the trusted CLI for a stronger import boundary.

SQLite is the durable coordination point. It runs with foreign keys, WAL, FULL synchronization, and a busy timeout.
Composite foreign keys prevent cross-team references. The signer still treats policy documents and
encrypted records as untrusted input.

## Data and authorization model

- A **stored key** is a Nostr key encrypted in an authenticated envelope.
- A **policy** is a versioned, strict capability document. Missing capabilities deny access.
- A **grant** binds one stored key and one policy to a unique NIP-46 communication key.
- An **invitation** contains a one-time random secret; only its hash is stored.
- A **session** binds a grant to the client pubkey that successfully claimed an invitation.

For example, a policy can allow two event kinds and self-encryption:

~~~json
{
  "version": 1,
  "capabilities": {
    "sign_event": {
      "allowed_kinds": [1, 7]
    },
    "nip44_encrypt": {
      "recipient": "self_only"
    }
  }
}
~~~

Unknown versions, capabilities, fields, invalid kind values, duplicate kinds, and unconstrained
capabilities are rejected. A client's requested permission list can only narrow the server policy.
Policy changes take effect on subsequent operations. Policies constrain event kinds and cryptographic
peers, not event content. See [Policies and access](policies-and-access.md) for examples and lifecycle rules.

Every non-`connect` method requires a live session. Invitation claim is one transaction: validate
the constant-time secret comparison and grant/invitation expiry, consume the invitation, establish
the session, and append a redacted audit event. A retry from the same client is idempotent after an
interrupted successful claim; another client cannot reuse the secret. Revoking a grant ends its
sessions and unclaimed invitations. `logout` ends only the caller's session.

![Key detail panel connecting a hosted identity to policies, grants, and client sessions](images/key-access.png)

*The access model in the UI: one hosted identity, separate grants, and paired client sessions.*

## Request lifecycle

For each kind-24133 event, the signer:

1. matches a subscribed recipient, selects the established-session or newcomer admission budget,
   applies global/client/grant limits, then verifies
   event size, timestamp, signature and exactly one well-formed `p` tag;
2. enters the authority gate and resolves the currently live grant by its indexed recipient key;
3. returns a previously committed encrypted response for duplicates, without performing another operation;
4. decrypts and validates the request, checks an invitation preflight or a live session, and persists
   the encrypted input before execution;
5. atomically claims the invitation for `connect`, or evaluates current strict policy before opening
   the stored-key envelope for a signing/cryptographic operation;
6. commits the response, redacted decision and logout lifecycle together; oversized replies become
   durable denials rather than repeatedly failing relay publication;
7. releases the authority gate and immediately wakes outbox publication;
8. completes delivery on the first enabled-relay acknowledgement while bounded, supervised redundant
   sends continue. A response already committed before revocation cannot be recalled.

Malformed requests and failed relay publications are isolated to the request. Concurrency is bounded.
The durable encrypted inbox/outbox is bounded by rows and bytes and pruned after ten minutes.
Audits are bounded to 100,000 rows/30 days; ended sessions and old invitations are pruned incrementally.

## Relays and readiness

Relay configuration is instance-wide: one SDK client and one connection per enabled relay serve all
active grants. Baseline subscriptions cover the active grants; discovered-relay subscriptions and
response publication are scoped to associated keys. Connections remain shared. See
[Relays](relays.md) for discovery, route activation, and client migration.

On startup and configuration changes, the signer subscribes for all active remote-signer pubkeys
with a five-minute replay window. The same five-minute window is enforced on request timestamps, so
older requests require the client to retry with a fresh event. The Nostr SDK owns reconnect behavior.
Persistent per-relay checkpoints are operational evidence, not a cursor that permits old requests.

After the initial configuration pass, when there are no active grants, the signer is ready without relay connections. With active grants,
readiness requires accepted subscriptions for `minimum_connected_relays`, successful delivery when
publication has been attempted, and no integrity/recovery/quarantine failure. The default relay set and threshold are stored in
SQLite and editable through the operator-only Instance page.

## Encryption

The root credential is a 32-byte base64 or hex key loaded from a private file or a systemd credential.
Configuring both credential sources is rejected rather than silently choosing one.
AES-256-GCM envelopes authenticate associated data containing envelope version, team ID, record ID,
public key, and purpose. Decryption also derives the public key from the plaintext and requires it to
match the stored metadata.

The Docker deployment mounts the credential read-only into the signer only. API and web images do
not contain it. Decrypted bytes use zeroizing buffers where practical, but Keycast does not attempt
`mlock` or claim resistance to a fully privileged host compromise.

## Operations and failure semantics

Signer worker tasks are supervised: loss of the control socket or relay supervisor terminates the
process so Docker can restart it. SIGTERM triggers a bounded graceful shutdown. Healthchecks call the
real signer socket. API process health is separate so management remains available during relay loss.

The authenticated status API/UI exposes schema/envelope versions, a non-secret credential
fingerprint, active resource counts, relay checkpoints, last processing time, and redacted error
counters, inbox/outbox age and capacity, database/WAL size and the local-backup timestamp. It never returns key material, ciphertext, invitation secrets, or complete bunker URLs.

The root filesystems are read-only, capabilities are dropped, privilege escalation is disabled,
process/file-descriptor counts are bounded, swap is disabled, scratch space is restricted tmpfs,
and logs rotate. The API/web network is internal; the signer has its own relay egress network and
the static reverse proxy has no Docker socket mount. SQLite and the
root key are the only host-persistent application data; the socket volume is ephemeral coordination
state.

## Storage and recovery

The current database is `database/keycast-v2.db` under `KEYCAST_STATE_DIR` (the checkout by default);
its matching `master.key` lives in that same state directory. Migrations remain tracked source in
[`database/migrations/`](../database/migrations/). These on-disk names are compatibility contracts.
The original release uses incompatible storage and connection credentials; see
[Upgrading](upgrading.md) before replacing an existing installation.

The trusted CLI creates consistent online encrypted backups that include the root credential,
streaming authenticated chunks bound to an encrypted manifest and unique archive identity.
Restoring an older backup revokes all prior grants, invitations and sessions, creates a new instance
identity and holds signing until an explicit review. Offline root rotation atomically rewraps all
active and revoked envelopes. See [Backup and recovery](backup-and-recovery.md) for commands and limits.
Upload-verification, retention and monitoring [job templates](systemd/) are provided. Each deployment
must choose its off-host storage, configure jobs and an alert receiver, and rehearse recovery.
Past implementation decisions and measured test outcomes are in the
[development history](development/history/README.md).
