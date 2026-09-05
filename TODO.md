# V2 follow-up

The locally implementable audit hardening and expanded test suite are tracked in
`docs/V2_AUDIT_CLOSURE.md`. Deployment and independent validation remain, in priority order:

- [ ] Independent review of management kind 27236, signer authorization, NIP-46, envelope encryption,
  and trusted CLI backup/restore/rotation.
- [ ] Rehearse Caddy/TLS and the exact digest-pinned stack on the target disposable VM.
- [ ] Multi-day public-relay soak and real host power-loss/storage-fault rehearsal; local process-kill, relay and SQLite-full tests are implemented.
- [ ] Select off-host backup storage and enable the provided encrypted upload/verification/retention and age-monitoring jobs.
- [ ] Configure host/proxy connection limits and connect the provided capacity/readiness monitor to an external alert receiver.
- [ ] Exercise management approvals with the intended external NIP-07/NIP-55/NIP-46 stores.
- [ ] Evaluate Umbrel/StartOS only after the VM deployment has operating history.

Implemented locally: signer-owned SQLite/ACLs, external same-key approvals, encrypted invitation
replies, bounded durable inbox/outbox, atomic logout, real subscription readiness, corrupt-grant
isolation, first-ACK publication, trusted key import, encrypted backup/restore and root rotation.
See `AUDIT.md`, `docs/V2_OPERATIONS.md`, and `docs/V2_VALIDATION.md` for boundaries and evidence.
