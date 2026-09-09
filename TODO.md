# Keycast follow-up

The locally implementable audit hardening and expanded test suite are tracked in
[V2_AUDIT_CLOSURE.md](docs/development/history/V2_AUDIT_CLOSURE.md).

September 9 decision: finish the soak phase and move toward normal personal use.
No further timed soaks are planned. The remaining reliability findings stay tracked
as follow-ups; completed soak evidence does not close independent review or recovery work.
Fresh-install signing defaults are nos.lol, Primal and Damus; Ditto and Bucket are excluded.

September 9 second review pass: the high and medium findings from the signer, web, crypto and
deployment reviews are fixed and covered by tests. See the [second-pass table](docs/development/history/V2_AUDIT.md#second-review-pass-september-9-2026).
The container network is now internal and the reverse proxy no longer takes the Docker socket, so
an existing deployment needs the one-time steps in
[Hardening migration](docs/upgrading.md#hardening-migration).

Remaining deployment and independent validation work:

- [ ] Independent review of management kind 27236, signer authorization, NIP-46, envelope encryption,
  and trusted CLI backup/restore/rotation.
- [x] Rehearse Caddy/TLS and the exact digest-pinned stack on the target disposable VM.
  Deployed and verified September 6, 2026; see [VM_TEST_RUN_2026-09-06.md](docs/development/history/VM_TEST_RUN_2026-09-06.md).
- [x] Complete the planned public-relay soak runs and review their outcomes.
  Further soaks are not planned following the September 9 decision.
  The corrected 24-hour sampled workload passed all 279 runs; remaining relay/admission
  findings and coverage limits are in [SOAK_RESULTS_2026-09-08.md](docs/development/history/SOAK_RESULTS_2026-09-08.md).
  The subsequent relay-discovery soak completed with 277/278 runs passing, one
  reply timeout and eight Ditto admission rejections; see [SOAK_RESULTS_2026-09-09.md](docs/development/history/SOAK_RESULTS_2026-09-09.md).
- [ ] Rehearse real host power-loss/storage-fault recovery; local process-kill, relay and SQLite-full tests are implemented.
- [ ] Track the initial-ping reply timeout and Ditto `admission_running_client` burst as
  reliability follow-ups; add bounded delivery tracing if needed during normal usage.
- [x] Separate bounded cached-response retry handling from fresh-request admission and test
  reconnect bursts with concurrent fresh work; retain timeout-phase and rejection-limit counters.
- [x] Add cached NIP-65 discovery on import with operator-controlled activation, network-destination
  restrictions and per-key routing. Protocol relay switching is tested; Jumble automatic
  adoption remains a client compatibility limitation (keep baseline routes available). Investigation and proposed
  ordering: [RELAY_INVESTIGATION_2026-09-08.md](docs/development/history/RELAY_INVESTIGATION_2026-09-08.md). VM rollout and the new scoped-relay
  workload: [RELAY_ROLLOUT_2026-09-08.md](docs/development/history/RELAY_ROLLOUT_2026-09-08.md).
- [ ] Complete the VM key-operation matrix below, including encryption/decryption rather than signing alone.
- [ ] Select off-host backup storage and enable the provided encrypted upload/verification/retention and age-monitoring jobs.
- [ ] Connect the provided capacity/readiness monitor to an external alert receiver. Proxy body and
  request limits now ship in `Caddyfile.example`; per-IP rate limits still need a Caddy plugin or
  host firewall, because the API sees only the proxy's address.
- [ ] Exercise management approvals with the intended external NIP-07/NIP-55/NIP-46 stores.
- [ ] Evaluate Umbrel/StartOS only after the VM deployment has operating history.

Implemented locally: signer-owned SQLite/ACLs, external same-key approvals, encrypted invitation
replies, bounded durable inbox/outbox, atomic logout, real subscription readiness, corrupt-grant
isolation, first-ACK publication, trusted key import, encrypted backup/restore and root rotation.
See [AUDIT.md](AUDIT.md), [operations.md](docs/operations.md), and [V2_VALIDATION.md](docs/development/history/V2_VALIDATION.md) for boundaries and evidence.

## VM key-operation acceptance and soak coverage

Use disposable keys and the exact deployed build. Exercise Jumble's available client flows and
use an independent NIP-46 protocol harness for operations Jumble does not expose. Record results
per operation, policy, and recovery scenario; a successful post is not full key-operation coverage.

- [ ] Verify `connect`, `get_public_key`, `ping`, and `logout`, including invitation reuse,
  reconnects, expired/revoked sessions, and rejection of unsupported methods.
- [ ] Exercise `sign_event` across permitted and denied event kinds; independently verify the
  returned public key, event hash, and signature, and reject mismatched or malformed templates.
- [ ] Exercise all four crypto methods: `nip04_encrypt`, `nip04_decrypt`, `nip44_encrypt`, and
  `nip44_decrypt`. Verify both directions against an independent client with disposable peer keys,
  including self-encryption, Unicode, empty input, and payload-size boundaries.
- [ ] Check each encryption/decryption permission independently, requested session capabilities,
  self-only versus other-recipient restrictions, and isolation between keys, grants, and teams.
  Include malformed ciphertext, NIP-44 authentication failures, and invalid recipient keys.
- [ ] Exercise browser and trusted CLI key import, external-signer management approvals, encrypted
  management requests/replies, and backup/restore/root-credential rotation. Verify the same public
  identities and permitted signing/crypto operations afterward.
- [ ] Repeat signing and all four crypto methods during the multi-day soak and after relay loss,
  reconnect, signer restart, VM reboot, and recovery rehearsals. Check policy edits, revocation,
  and logout take effect while clients remain connected.
- [ ] Report success/failure counts and latency by operation, alongside relay reliability and
  resource growth. Store sanitized outcomes only, never private keys, plaintext messages,
  invitation secrets, or complete bunker URLs in test logs.
