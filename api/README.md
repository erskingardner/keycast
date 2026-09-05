# Keycast API

The Axum API is a bounded public transport for the signer. It forwards original signed method,
path/query, authorization header and exact body over `/run/keycast/signer.sock` and returns the
signer's response. It neither opens SQLite nor loads the root credential. Its socket mount is
read-only; it cannot invoke actor-only lifecycle commands or decrypt management replies.

The signer management router verifies instance-bound kind-27237 reads, external kind-27236 mutation approvals, instance
admission and all team/operator roles. `/health` measures API availability; `/ready` reports signer
readiness. Keeping these distinct allows management during relay outages.

Run the workspace suite and `scripts/container-smoke.sh` for transport changes. See
[the operations runbook](../docs/V2_OPERATIONS.md) and [audit](../AUDIT.md).
