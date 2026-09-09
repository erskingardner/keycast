# Security model

[Documentation](README.md) · [Architecture](architecture.md) · [Backup and recovery](backup-and-recovery.md)

Keycast is a remote signer: it holds private keys and uses them on behalf of authorized clients.
Its security depends on the host, signer, external management signer, and the permissions you grant.
It has not received an independent security audit. Repository hardening and tests are evidence
about specific behavior, not a guarantee against compromise.

## Trust boundaries

| Component or actor | What it can access or influence |
|---|---|
| Host administrator / Docker daemon | Database, root credential, process memory, images, and configuration. Full host access can compromise keys. |
| Signer | Decrypted keys during operations, authorization state, encrypted storage, and relay traffic. This is the trusted application boundary. |
| Public API | Original signed management requests and browser-imported keys in transit. It has no database/root mount and cannot independently grant authority. |
| Served frontend | Management intent, browser-imported private keys, and decrypted management write replies, including newly created invitations. |
| External management signer | The identity that approves administration. Its compromise can authorize changes within that identity's roles. |
| Connected Nostr app | Results of operations allowed by its grant; it does not receive the hosted private key. Broad signing or decryption permissions still carry real authority. |

## Management approvals

Keycast requires external signed approvals for management. It uses private kind `27236` for
writes and instance-bound kind `27237` for reads. It does not accept ordinary NIP-98 as management
authentication, and every hosted grant refuses to sign both private kinds.

Write approvals bind the instance identity, authority revision, method, public URL, body hash,
command description, nonce, and fresh reply-encryption public key. The signer checks the actual
command against the approved contents and enforces admission and team/operator roles.
Write replies are encrypted to the approved recipient using a stable sender identity derived from
the root credential. The browser pins that identity on first use and fails closed if it changes.
Compare the displayed management-reply fingerprint with `keycast_signer status` on the trusted
host: a first-use browser pin by itself does not authenticate the initial identity. With the
correct identity trusted, an API-only attacker cannot read or forge a successful invitation reply.
Management read replies pass through the API in plaintext; their proofs can be replayed within
the one-minute validity window.

Inspect the command and URL in your external signer. A compromised frontend can still mislead
you about what to approve. Import approvals omit the private key itself. Write approvals allow
five minutes for review; read proofs expire after one minute. Approvals are single-use, and
concurrent changes may require a fresh approval. If a reply is lost, inspect state before retrying:
the original command may already have committed.

Root credential rotation changes the reply identity. The **Instance** page exposes the fingerprint
and re-trust control even for team administrators who cannot view operator status. Verify against
the trusted host before accepting a changed identity. Do not clear a pin just to silence a warning.

![Browser warning that the signer reply identity differs from its trusted pin](images/reply-identity-change.png)

*The browser detects the identity change after an actual root rotation in the demo.*

## Key import and storage

Browser import is an explicit trust choice. The private key passes through the page and API during
import even though the field is cleared and not intentionally persisted in browser storage.
HTTPS, CSP, encrypted storage, and external approvals do not remove that exposure. For more
sensitive keys, use [trusted host CLI import](operations.md#key-import), which reads the key from
stdin while the signer is stopped.

Stored keys use authenticated AES-256-GCM envelopes bound to their team, identity, and purpose.
The signer alone loads the root credential. Zeroizing buffers reduce plaintext lifetime where
practical; Keycast does not claim locked memory or protection against a privileged host attacker.

The backup archive contains both a consistent database snapshot and its root credential, protected
by a separate backup encryption key. The backup key must differ from the root credential. Store it separately from the VM and archives.
Root rotation does not undo earlier disclosure of private keys or old backups.

The Compose deployment disables container swap, keeps the API/web on an internal network, and
gives only the signer a relay egress network. The static Caddy proxy has no Docker socket mount.
These reduce exposure but do not remove host trust or the ability to exfiltrate through an existing
client connection. See [Deployment](deployment.md).

## Availability and privacy limits

Policies constrain event kinds and cryptographic peers, not the full meaning or content of an
event. Relay acknowledgements show relay acceptance, not client receipt. Clients need fresh retries
after outages longer than the supported request/reply windows. Resource limits intentionally deny
work when capacity is exhausted. See [Operations](operations.md#availability-and-bounds).

Activity and status omit private keys, invitation secrets, payloads, and ciphertext. Retained public
identifiers and operational metadata can still be sensitive. Never publish database files,
`master.key`, `.env`, complete bunker URLs, or raw imported keys in issues or logs.

## Review status

[TODO.md](../TODO.md) tracks remaining independent review, recovery rehearsals, and operational
follow-ups. The [historical audit](development/history/V2_AUDIT.md),
[validation report](development/history/V2_VALIDATION.md), and
[soak reports](development/history/README.md#deployment-and-relay-evidence) preserve the scope and
limitations of completed checks. They do not establish the configuration or health of your deployment.
