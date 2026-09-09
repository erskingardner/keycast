# V2 operator runbook

This release uses one active signer on one host. Only the signer opens SQLite and the root credential.
The API forwards signed requests over a read-only socket mount. Keep the same Nostr identity in an
external NIP-07/NIP-55/NIP-46 signer for management approvals; it may also be imported into Keycast.
Keycast itself refuses to sign management kinds 27236 and 27237 regardless of policy. No passkeys are required.

Management writes show the command contents to the external signer and bind method, public URL,
body hash, instance identity, authority revision, nonce, and a fresh reply-encryption public key.
Inspect the URL and command in the external signer. Approvals are single use; a failed or interrupted
write may consume an approval and requires a fresh one. Concurrent management screens can receive
409 and must request a new approval. Write approvals allow up to five minutes for human review; read proofs expire after one minute. Reads use instance-bound kind 27237. Ordinary NIP-98 remains delegatable for other services, but is never accepted as Keycast management authentication. Each identity may consume at most 128 approval nonces per ten minutes; rejected commands cannot consume another identity’s budget or advance the authority revision.

`ALLOWED_PUBKEYS` controls instance admission. `KEYCAST_OPERATOR_PUBKEYS` independently controls global
relay changes and instance status. Initialization defaults operators to the first allowed key; use `--operator-pubkeys` to specify operators separately. Each team still checks
its own administrators. Changing relays requires external approval and operator membership.
`KEYCAST_PUBLIC_URL` is required on the signer and must be `https://HOST/api`; loopback HTTP is allowed
for development. The API must not have a database or root-key mount.

Invitation lifetimes are capped at seven days in the signer and cannot exceed their grant. Grants may deliberately have no expiration for unattended personal use. Open invitation IDs and expiration times can be listed and revoked from the key page without ending sessions.

## Key import

Browser import is supported over HTTPS. It uses a password field, avoids storage/caching, clears the
form after attempting import, and excludes the private key from the external approval event.
It still exposes the supplied key to the browser, served frontend, and API during import. A
compromised frontend/API can steal it or mislead a user into approving another command. CSP and
external approvals cannot remove that trust boundary. Use the local CLI for more sensitive keys.

For host commands set `KEYCAST_DATABASE_PATH`, `KEYCAST_MIGRATIONS_PATH` and `KEYCAST_ROOT_KEY_FILE`.
Run as the trusted account owning those files. With Docker, run commands in the signer image using
its existing database/root mounts; give backup commands a separate private backup-directory mount.
Only public identifiers and file paths belong in command arguments. Never pass an nsec in argv.

```sh
# Stop the signer first. A maintenance lock refuses import while it is running.
keycast_signer import TEAM_ID ADMIN_HEX 'Personal key' < /secure/private-key.txt
```

The actor must already be a team administrator. The CLI reads the private key from stdin and prints
only the imported public key. Protect the source file; do not paste a key into shell command history.

## Backup

Generate a dedicated backup encryption key once. The backup key protects a complete bundle containing
both a consistent SQLite snapshot and its root credential. Losing the backup key makes the bundle
unrecoverable. Store a recovery copy separately from the backup files and from the VM.

```sh
keycast_signer generate-key /secure/keycast-backup.key
keycast_signer backup /secure/keycast-backup.key /backups/keycast-YYYYMMDD-HHMMSS.kcb
```

Backup can run while the signer is active. `VACUUM INTO` includes committed WAL contents and produces
a compact snapshot; the maintenance lock prevents root rotation during the snapshot. Output files
are created with mode 0600, synced, and never overwritten. The binary authenticated format contains
no JSON expansion of the database. The KCB3 format streams independently authenticated 1 MiB chunks, with an authenticated manifest
binding the root credential, database length, archive identity and chunk positions. Truncation,
reordering and mixing archives fail authentication. The supported compact database bound is 256 MiB,
matching the clean database page ceiling; memory use does not scale with the whole backup. Temporary snapshots are
private files in the database directory, removed after the command. An abrupt host failure may leave
a `snapshot-*` file there; it contains the encrypted database, not the root credential.

Schedule the command with the host's service manager, upload the resulting encrypted file to the
chosen off-host store, and alert on job failure/backup age. A local file is not disaster protection
against loss of the VM. No upload destination or host timer is enabled by this repository change.

## Recovery after loss or rollback

An ordinary restart preserves sessions. Restoring an older backup is different: it can resurrect
revoked permissions, invitations, and administrators. Always use the restore command, rather than
copying a historical database into a running instance.

```sh
keycast_signer restore /secure/keycast-backup.key /backups/keycast-YYYYMMDD-HHMMSS.kcb /secure/new-keycast
```

The destination must not exist. Restore checks authentication, schema, root-key pairing and database
integrity. It creates a new instance identity, clears old request caches/approvals, revokes all old
grants/invitations/sessions, and sets a recovery hold. The directory contains `keycast-v2.db` and
`root.key`. A failed restore retains `RESTORE_INCOMPLETE`; the daemon refuses to start it.

Review restored team administrators, policy documents, relay settings, and the host allowlists.
Management remains available during the hold so external administrators can repair state. Stop the
signer, point the CLI at the restored database/root, and explicitly record the review:

```sh
keycast_signer review-restore
```

Restart, create fresh grants and invitations, and reconnect clients. Recording the review never
reactivates the old grants. Rehearse this procedure with a disposable copy before relying on backups.

## Root credential rotation

```sh
keycast_signer generate-key /secure/keycast-next-root.key
# Stop the signer; leave KEYCAST_ROOT_KEY_FILE pointing to the old root for this command.
keycast_signer rotate-root /secure/keycast-next-root.key
```

Rotation rewraps every active and revoked key/grant envelope in one SQLite transaction. Both key
files remain intact. After success, configure the signer to load the new root and restart. The old
root fails the database fingerprint check. If interrupted, the database uses either the old or new
root; both files are retained for recovery. Take and verify a new backup before retiring the old key.
Rotation after compromise does not undo disclosure of plaintext keys or old backups.

## Availability and bounds

`/health` measures API process availability; `/ready` asks whether signing is ready. The management
interface remains reachable during relay loss. Readiness requires accepted subscriptions for the
configured minimum, a healthy database, and no quarantined grants/recovery hold. A connected socket
alone is insufficient. First publication acknowledgement proves relay acceptance, not client receipt. Remaining sends
continue in a bounded, supervised replication group; saturation can skip redundant replication.
Rejected or silent subscriptions retry with exponential backoff and jitter (up to 305 seconds).
Relays requiring NIP-42 authentication are reported as unavailable; the signer does not use managed
user keys as a relay-authentication identity.

The daemon supervises control/relay tasks, detects stalled runtime progress, and exits for the service
manager to restart it. Docker's restart policy handles process exits; it does not restart a live
container merely because its healthcheck is unhealthy. Relay failures are retried in process.

There are 32 request workers, two concurrent requests per client and eight per grant; ingress is
limited to 128 events/second globally, 16/client/second and 32/grant/second. Recovery uses the same
admission limits. Fair selection interleaves grants in inbox recovery and the outbox. There are
16 outbox publishers, up to 16 bounded background replication batches, and eight deadlined control
connections (the public API permits 32 transport requests). Unknown clients
cannot fill the durable inbox. The inbox is capped at 10,000 rows and 128 MiB including encrypted payloads and reserved reply space;
retries expire after ten minutes and new requests must be no older than five minutes. Expired inbox records are pruned in batches of 1,000 every five seconds. Audits retain
at most 100,000 rows/30 days. Ended sessions and unreferenced old invitations are pruned incrementally.
Administrative table limits are explicit in the initial schema (including 1,000 grants/keys/policies
and 20 relays). Responses that exceed the same 256 KiB content budget as ingress are durably denied before
publication. Each grant is limited to 2,000 rows/4 MiB, each client public key to 2,500 rows/8 MiB across grants, and each team to 4,000 rows/16 MiB. An admitted input reserves 264 KiB for its encrypted reply before execution; completion releases unused space. Capacity exhaustion returns a bounded, encrypted retry error without executing or persisting the rejected operation. Those overload errors are best effort and still subject to publisher admission. Sustained traffic above these ten-minute retention budgets is throttled intentionally.

Publication failures affect readiness for at most sixty seconds after the latest failed attempt. Retries continue while the response remains eligible. Transient pool pressure, SQLite busy/locked and full-disk errors are retried without restarting the signer; corruption remains fail-stop. Individual request/control/publisher task panics are isolated, while a stalled supervisor still triggers the watchdog.

The API gives uploads a separate global 64-request/64 MiB budget, at most four uploads per socket peer, and a three-second body deadline. Signer calls retain their independent 32-request/ten-second bound. Proxied clients share the proxy’s socket-peer budget: forwarded IP headers are deliberately not trusted. Set per-client limits at the public proxy to control abusive peers behind it. Saturation fails closed. Configure reverse-proxy connection/rate limits as well.

## Release and local validation

The prerelease schema has changed deliberately. Existing prerelease V2 databases fail the migration
checksum check instead of being silently reset. Select a fresh disposable database for development.
V1 remains deliberately incompatible.

Production Compose requires reviewed `KEYCAST_API_DIGEST`, `KEYCAST_SIGNER_DIGEST`, and
`KEYCAST_WEB_DIGEST` values. Source Compose builds locally and does not require published digests.
Run `scripts/container-smoke.sh` after building images tagged `keycast-hardening-{api,signer,web}:local`.
It creates only disposable containers/volumes and exercises real HTTP management through the private
socket with read-only containers. It does not deploy the stack or touch existing databases.

## Host monitoring and off-host jobs

`scripts/operations/keycast_ops.py` provides explicit `backup` and `monitor` commands. The reviewed
configuration is an argument array, never shell text. `docs/systemd/` contains example service/timer
units and configuration. These are templates; nothing has been installed or enabled on this host.
Choose the rclone destination and a versioned/immutable remote retention policy, supply its credential
configuration, and configure the service's `OnFailure=` notification before enabling timers.

The backup job uploads an encrypted archive, verifies its bytes using
[rclone check --download](https://rclone.org/commands/rclone_check/), then records off-host success.
Only after that success does it prune owned local archives, retaining at least two (default seven).
A failed upload or verification leaves prior archives and the previous success timestamp intact.
The job never deletes remote archives. Limit the remote credential to the chosen backup prefix and
use provider-side retention to survive accidental deletion or compromise of the VM.

`keycast_signer status` emits redacted JSON through the private socket. The monitor checks readiness,
inbox capacity, oldest pending response, database/WAL growth, filesystem free space, and verified
off-host backup age. It exits nonzero and emits stable alert codes; connect these failures to the
operator's external alert receiver. The daemon's local-backup timestamp is deliberately distinct from
the off-host success marker. Default thresholds warn at 8,000 inbox rows/100 MiB, 60 seconds outbox
age, 200 MiB database, 64 MiB WAL, less than 1 GiB free, or no verified backup within 25 hours.

Set `KEYCAST_DATABASE_PATH`, `KEYCAST_ROOT_KEY_FILE`, `KEYCAST_MIGRATIONS_PATH`, `KEYCAST_SIGNER_SOCKET`
and `RCLONE_CONFIG` in the private host environment file. With Docker, set `signer_command` to an
argument array for `docker compose run --rm --no-deps -T` with the signer service and explicit backup
key/directory mounts. Those paths must resolve identically to the paths in the job configuration.
Set `status_command` to `docker compose exec -T keycast-signer /app/keycast_signer`. Note that this
requires the `keycast` user to be in the `docker` group, which is root-equivalent on the host and
undoes the systemd sandboxing; prefer a host binary. The example service assumes
a host binary; adapt its account and filesystem permissions to the actual deployment. Docker access
is trusted host administration, not an additional security boundary.

Export retained audit metadata with `keycast_signer audit-export /secure/audit-YYYYMMDD.jsonl`.
The online export is a consistent SELECT, mode 0600, and refuses overwrite. It omits payloads,
ciphertext and free-form details. Export before retention removes old rows if longer history is needed.

Removing a policy requires revoking its active grants first. Removal hides it from management and
prevents new grants or updates, while retaining a tombstone for historical grants. Reusing its name
creates a new policy identity and cannot reactivate the old authority.
