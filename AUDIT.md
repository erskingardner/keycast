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

## Second review pass, September 9, 2026

A further review covered the signer authorization router, NIP-46 pipeline, envelope and backup
cryptography, the SvelteKit frontend, and the container/CI/operations layer. No critical or
high-severity issue was found in application code. The findings below were fixed in this pass.

| Priority | Finding | Fix |
|---|---|---|
| High | The shipped Caddy example mounted `/var/run/docker.sock`. Read-only applies to the socket file, not the Docker API sent over it, so code execution in the only Internet-facing process was root-equivalent on the host | Replaced with a static `Caddyfile.example` and a hardened proxy service: no socket, dropped capabilities, `read_only`, limits, and body limits for `/api/*`. The dead `caddy=` labels were removed |
| Medium | Relay admission spent the shared per-second budget before verifying signatures or sessions. A remote-signer pubkey is public, so ~128 forged events per second on any shared relay could starve signing for every grant | Split admission into two lanes. Clients with a live session on the target grant draw from the established lane; everything else, including `connect`, draws from a smaller newcomer lane that cannot exhaust it. The live-session index refreshes with the grant configuration |
| Medium | Management write replies were encrypted to a fresh per-reply key, so NIP-44 had nothing to authenticate. Anyone on path, including the untrusted API, could forge an outcome such as a `bunker://` URI pointing at their own signer | The signer derives a stable reply identity from the root credential and publishes its public half through `/config`. The browser pins it, decrypts only with the pinned key, and fails closed on a change. The Status page shows the fingerprint for comparison against `keycast_signer status` |
| Medium | The NIP-46 sign-in popup opened a signer-supplied `auth_url` with an `opener` handle and no scheme check, so a malicious handshake relay could navigate the operator's tab mid sign-in | The URL must be `https:`, the popup is opened with `noopener,noreferrer`, and `Cross-Origin-Opener-Policy: same-origin` is set |
| Medium | One flat bridge network gave the API and web containers unused Internet egress and placed the signer beside the proxy | `keycast` is created `--internal`; the signer has its own egress-only network; the proxy keeps a public network for ACME and ports |
| Medium | `mem_limit` without `memswap_limit` allowed decrypted key material to be paged to host swap, which outlives the container | `memswap_limit` equals `mem_limit`, disabling swap for the cgroup. `/tmp` is also `noexec,nosuid,nodev` |
| Medium | `.dockerignore` patterns were root-anchored, so the `legacy-v1/master.key` that `UPGRADE.md` instructs the operator to create would enter the build context, as would `web/.env` | Rewritten in allowlist form and verified by building the real context with planted decoy secrets. `UPGRADE.md` now archives legacy files outside the checkout |
| Medium | Actions were pinned by mutable tag, images shipped without provenance, the publish step could run from any ref, and the documented "reviewed digest" had no procedure | Every action is pinned to a commit SHA with Dependabot to maintain them; images are built with provenance and an SBOM and attested; publishing is guarded to `master` and serialized; `upgrade_preflight.sh` runs `gh attestation verify` and fails closed |
| Medium | `DELETE /teams/{id}` could never succeed once a team had any grant, including a revoked one: `grants` references `policies` with `ON DELETE RESTRICT` while `policies` cascades from `teams` | The handler deletes the team's grants inside the same immediate transaction. Reproduced against the real migrations before and after |
| Low | `sign_event` forwarded a client-supplied event `id`. The nostr crate signs `unsigned.id` before verifying it, so the signer briefly produced a Schnorr signature over an attacker-chosen 32-byte value even though the mismatch was rejected | The `id` is always stripped and recomputed |
| Low | Imported private keys left several unzeroized copies, and `AddKeyRequest` derived `Debug` over the plaintext | A `Secret` newtype erases its buffer on drop and redacts `Debug`; it is used on the control-socket body, the import request and the lifecycle request, and the lifecycle request is no longer cloned. The approval description now parses only the key name |
| Low | An approval echoing a request body could carry private material to an external signer | Both the signer and the browser refuse to sign or accept an approval whose content contains `secret_key` or `nsec1`, including an operator pasting a key into the name field |
| Low | `backup` accepted the root credential as the backup key, and the manifest carries the root | Rejected: the backup key must differ from the root |
| Low | CSP allowed blanket `https:` in `connect-src` and `http:` in `img-src` | Narrowed to `'self'` and `wss:`; COOP and CORP added |
| Low | Every signed-in page load fetched a contact list that nothing read, announcing the operator's pubkey and IP to five public relays, and the loader followed relay hints from fetched events | The follows feature was removed and `followRelayHints` is off |
| Low | The `[pubkey]` route parameter was interpolated unvalidated into signed request paths | Rejected unless it is 32-byte lowercase hex |
| Low | `upgrade_preflight.sh` opened the live database read-write as root after the ownership repair, which could leave root-owned WAL sidecars the container user cannot open | The check runs first, read-only, and warns about mis-owned sidecars |
| Low | Systemd templates lacked kernel, namespace and syscall restrictions, and the web image let the runtime user own `/app` | Templates hardened with an empty capability bounding set and a `@system-service` filter; `/app` is root-owned |
| Info | `decode_hex` accepted a leading sign, a non-regular credential file blocked startup inside a read, and the Dockerfile frontend was a mutable tag | Strict hex digits, regular-file check, digest-pinned frontend |
| Info | `CREDENTIALS_DIRECTORY` silently won over an explicit `KEYCAST_ROOT_KEY_FILE`, and persistent state had to live inside the checkout | Setting both credential sources is rejected outright; `KEYCAST_STATE_DIR` relocates the database and root credential, defaulting to the checkout |

## Review follow-up, September 9, 2026

Addressed from the pull request review:

- Publication now pushes to an immutable `sha-` tag, smoke-tests that exact digest, and only then
  promotes it to `v2`/`latest` with `imagetools create`. A rebuild for publication could differ from
  the tested image, because `apt-get install` is time-dependent when the layer cache misses.
- The Compose hardening assertions moved into `scripts/check-compose-hardening.sh`, which inspects
  rendered mounts instead of file text. The previous text search matched the comment documenting the
  socket removal, so the job failed on the very configuration it was meant to accept.
- The reply-key pin is retained in memory when browser storage is unavailable, disabled, or
  throwing. Without that, every request became another first use and an API compromised mid-session
  could substitute its identity without tripping the change warning.
- The reply identity and its re-trust control now load from the unauthenticated `/config` rather
  than the operator-only `/status`, so a team administrator who is not an instance operator can
  still recover after a root-credential rotation.
- `connect-src` accepts the explicitly configured API origin again, so a split-origin HTTPS
  deployment is not blocked by the removal of the blanket `https:` source.
- `KEYCAST_STATE_DIR` is honoured by `scripts/init.sh`, `scripts/generate_key.sh` and
  `scripts/upgrade_preflight.sh`, so setup, validation and permission repair act on the same paths
  Compose mounts rather than a stale copy in the checkout.
- Preflight uses the plain database path with `sqlite3 -readonly`, tries GNU `stat -c` before the
  BSD form, and reports the `gh attestation verify` diagnostic so an authentication or network
  failure is not recorded as missing provenance.
- The starvation regression waits on a new `configuration_reloads` counter instead of sleeping, and
  the admission unit test asserts the constants that actually bind.
- A test compares the Rust and TypeScript secret-marker lists, since the browser refuses first and a
  marker present only in the signer would let content reach an external key store.
- `UPGRADE.md` restarts the Keycast stack after the network is recreated; the previous ordering left
  the signer, API and web containers stopped.

Two findings from an external review were investigated and **not** reproduced as issues. Discovered
relays do not bypass the SSRF vetting: the runtime places every discovered relay in a restricted set
and the custom transport routes those connections through `public_relay::connect`, which re-resolves
and re-validates every DNS answer on each connect and refuses proxies. Unauthenticated read
flooding remains bounded by the existing admission and socket limits.

Known residual, unchanged:

- Browser key import still moves the private key through the API process and through HTTP and JSON
  buffers that cannot be zeroized from application code. The `Secret` wrapper narrows the window;
  the trusted CLI remains the stronger path and the UI says so.
- The API and the signer still share UID 10001. The socket permission fallback for filesystems that
  reject `chmod` on a Unix socket deliberately depends on that, and the API has no writable mount to
  own anything with, so the split was not worth breaking that path.
- The API's per-peer upload budget still sees only the proxy address. `Caddyfile.example` sets body
  limits; a per-IP rate limit needs a Caddy plugin or a host firewall, and is tracked in `TODO.md`.
- Management reads (kind 27237) carry no nonce and stay replayable inside their 60-second window.
  Reads pass through the API in plaintext regardless and expose no secrets.
- The `ca-certificates` layer stays in the Rust runtime image. The signer uses webpki roots, so it
  is unnecessary, but a CA bundle is not meaningful attack surface and removing it risks TLS
  breakage that no local test would catch.

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
