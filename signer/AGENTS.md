# AGENTS.md

This file inherits the root guidance and applies to `signer/`.

- One signer multiplexes all grants and relay connections; do not reintroduce child daemons.
- Require a live session before every non-connect NIP-46 operation.
- Claim invitations atomically, keep same-client retries idempotent, and reject cross-client reuse.
- Evaluate current strict policy before decrypting a stored key.
- Verify outer/inner events, exact `p`-tag routing, timestamps, sizes, and method parameters.
- Store encrypted responses before publication and require at least one relay acknowledgement.
- Isolate request errors; terminate the process if a long-lived control/relay worker exits.
- Keep the control socket capability-limited and every status/audit/log surface redacted.

Run signer unit tests, `signer/tests/nip46_roundtrip.rs`, and the full workspace suite.
