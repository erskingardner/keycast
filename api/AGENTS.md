# AGENTS.md

This file inherits the root guidance and applies to `api/`.

- The API is transport only: no SQLite access, root credential, authorization decisions or actor-only lifecycle commands.
- Forward original signed headers and exact method, path/query and body to the signer.
- Bound admission, body length and timeouts. Return safe transport errors without request contents.
- Preserve encrypted management replies; the API must not decrypt them.
- `/health` is process availability; `/ready` reports signer readiness so management stays reachable during relay loss.
- Authentication, team and operator checks live in `signer/src/management/`.

Run the workspace suite and combined container smoke for transport changes.
