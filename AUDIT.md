# Keycast V2 audit and hardening

Reviewed and updated September 5, 2026. Scope: current V2 work compared with the original master
architecture. This is a local implementation review and validation, not an independent audit.

## Architecture decision

Keep V2. One multiplexed signer, one host, SQLite, shared relays and explicit grants/sessions are a
better fit for a personal server than V1's per-authorization processes and reusable credentials.
The important correction is authority: the signer now owns both keys and authorization state.
Separating root decryption from an API that can freely change grants was insufficient.

The supported trust boundary is a trusted host/signer and external approval key store. The API is
untrusted transport and has no SQLite/root access. The frontend remains a management interface.
A single Nostr key may be both managed and an administrator; management approvals must come from
its external signer. Passkeys are not part of this design.

## Findings addressed

| Priority | Finding | Current behavior |
|---|---|---|
| Critical | API could manufacture grants or widen permissions through shared SQLite and actor-only socket commands | Only signer opens SQLite; socket forwards original signed HTTP requests; signer checks signatures and all team/operator rules |
| Critical | Delegated signing of HTTP-auth events could promote the managed identity's authority | Writes require private management kind 27236; reads require instance-bound kind 27237. Every NIP-46 grant hard-denies both even when a policy explicitly lists them |
| High | Replay and stale approvals could apply after authority changed | Management approvals bind instance identity, global authority revision, exact method/URL/body, nonce and reply recipient; nonces are single-use and successful mutations advance the revision in the same transaction |
| High | API could read an invitation bearer secret from its creation response | Write replies use NIP-44 encryption to a fresh browser public key included in the external approval |
| High | Global relay changes lacked operator authorization | Signer checks a separate host-configured operator allowlist and parses exact URLs; malformed loopback-prefix URLs are rejected |
| High | Browser import exposed key material to the web stack | Retained as an explicit trust choice; hardened field/cache/clearing behavior, private material omitted from approval events, trusted host CLI available |
| High | Logout depended on successful publication | Session invalidation, response cache and audit commit together before network delivery |
| High | SDK event deduplication prevented retries from reaching the response cache | Runtime uses every relay Message event, validates subscription ID and performs application deduplication |
| High | No autonomous delivery/restart recovery | Encrypted inputs and response outbox persist; bounded workers recover and retry independently of new inbound events |
| High | One malformed grant killed all signing | Invalid grants are quarantined and readiness is degraded; valid grants continue |
| High | Readiness treated connected sockets as successful subscriptions | EOSE/closed subscription state, integrity, recovery hold, quarantine and publication failure affect readiness |
| High | Untracked tasks and unbounded socket reads/waits threatened unattended operation | Tracked worker/publisher/control tasks, admission bounds, deadlines, runtime/worker progress detection and process-exit supervision |
| High | No recoverable backup/rotation workflow | Online authenticated backup, fresh-directory restore with old authority revoked, review hold and transactional offline root rewrapping |
| Medium | Expiry/last-admin/revocation races | Signer authority gate orders mutations and signing preparation; invitation expiry is rechecked inside a cancellation-safe immediate transaction |
| Medium | Requests scanned/decrypted all grants and waited for the slowest relay | Recipient lookup uses its index; network publication is outside admission and completes on first acknowledgement |
| Medium | Unbounded persistence and expensive health probing | Per-grant/client/team byte/row quotas, reserved response space, table capacities, incremental retention, hourly integrity checks and cheap health paths |
| Medium | Hex root credentials failed decoding; plaintext lifetime and core dumps | Hex/base64 regression coverage, zeroizing root/decoded buffers and cipher state, container core dumps disabled |
| Medium | Mutable production image selection and API sharing writable state | Reviewed per-image digests required; API socket mount read-only; no API database mount; memory/PID/log/privilege bounds |

Management approval content is checked against the actual command so the external signer can show
what is being approved. Import approvals show the key name, not its private material. A concurrent
management operation can cause 409; obtain a fresh approval rather than replaying the old request.
An accepted command whose reply is lost may have committed: inspect state before retrying creation.

The authority gate orders preparation, not physical network delivery. A signature/response committed
before revocation may already be in flight and cannot be recalled. Revocation blocks new operations.

## Validation

The local validation includes workspace formatting/check/build/tests, Rust advisory scanning, root
and web Bun audits/Socket scans, web tests/type checking/build, Compose rendering, all three Docker
targets, and the combined read-only container smoke. Detailed evidence is recorded in
[docs/V2_VALIDATION.md](docs/V2_VALIDATION.md).

The regression suite exercises the production relay loop for duplicate delivery and restart outbox
recovery, subscription rejection, corrupt-grant isolation, logout with failed delivery, reserved
management-kind denial, stale grant claiming, unknown-client inbox protection, first-ack latency,
and a real CLI backup/rotation/restore drill. The container smoke exercises signed HTTP management
through the API and socket, browser import, and encrypted invitation creation.

## Remaining deployment work and limits

1. Obtain independent security review of the new management protocol, authorization transactions,
   cryptographic use and host recovery commands before substantial key custody.
2. Run a multi-day public-relay and target-VM soak, including disk-full, abrupt power loss, proxy/TLS
   configuration and external client behavior. Local tests are not months of uptime evidence.
3. Select an off-host backup destination, schedule/upload encrypted backups and alert on failures,
   age, disk space and signing readiness. No remote storage account or host timer has been configured.
4. Host root, Docker daemon access, the signer process or the external key store can compromise keys.
   Process separation does not defend against a compromised host. Browser/API compromise during
   browser import can steal that imported key. A compromised frontend can also mislead approvals;
   inspect the command and URL in the external signer and use CLI import for sensitive keys.
5. Relay acknowledgement proves acceptance by a relay, not client receipt. Requests have a five-minute
   age window and outbox retries a ten-minute lifetime. NIP-46 ephemeral relay messages do not provide
   durable replay; clients must retry with fresh events after longer outages.
6. Capacity and retention are deliberate for a personal instance. Saturation fails closed and may
   require operator cleanup/tuning. The streaming backup CLI supports compact databases up to 256 MiB. Resource
   bounds reduce abuse, but do not promise availability against arbitrary network/host exhaustion.

V1 remains incompatible. The prerelease V2 schema was changed intentionally; an existing development
database fails the migration checksum rather than being silently reset. No existing database was
reset. Production deployment remains outside this local validation.


## Additional findings closed during completion testing

- Grant admission now has independent rate/concurrency limits, including recovery. Tickets release
  capacity on normal completion, cancellation and panic; forged client churn cannot fill the maps.
- Startup readiness is gated on the initial configuration/subscription pass. Rejected subscriptions
  back off. Shutdown and database close also have explicit deadlines.
- Policy removal uses tombstones; current membership and cross-team resource checks have a route
  matrix regression. Concurrent administrator removal retains an administrator.
- SQLite page limits and backlog/space indicators supplement row/byte quotas. Inbox pruning can
  keep up with sustained personal-instance traffic. Streaming authenticated backups cover the
  database capacity without whole-archive allocations.
- Actual SIGKILL, SQLite-full, lock contention, relay authentication demand, relay rotation, four
  crypto-operation policy checks, external approval response handling, and a 1,000-request restart
  soak supplement the earlier baseline. See the validation report for measured limits.
