# Policies and access

[Documentation](README.md) · [Getting started](getting-started.md) · [Architecture](architecture.md)

A policy defines what an app may do with a key. A grant applies that policy to a particular
connection purpose. Invitations establish the client sessions that use the grant.

## The access model

| Object | Purpose |
|---|---|
| Team | Groups keys, policies, and members under its own administrators. |
| Stored key | The hosted Nostr identity, encrypted at rest. |
| Policy | Explicit signing and cryptographic capabilities; anything omitted is denied. |
| Grant | Binds a key to a policy and a unique NIP-46 communication key, with optional expiry. |
| Invitation | A single-use pairing secret with an expiration, advertised in a bunker URL. |
| Session | Binds a successfully paired client's public key to its grant. |

The communication key in a bunker URL is distinct from the hosted identity an app signs as.
The client also has its own connection key. A session authorizes that client to use the hosted
identity within the grant's policy; none of these identifiers should be treated as interchangeable.

![Expanded key showing its profile, cached relay list, active sessions, and a revoked grant](images/key-access.png)

*Each app or purpose has its own access entry; sessions show which grants have been paired.*

## Management roles

The host's `ALLOWED_PUBKEYS` list controls admission to management. Within that boundary:

- A **team administrator** manages that team's keys, policies, grants, and membership.
- A **team member** can view the team and its activity, but cannot perform administrator actions.
- An **instance operator**, listed in `KEYCAST_OPERATOR_PUBKEYS`, can view instance status and
  manage global relay settings. That role does not bypass team authorization.

Creating a team makes its creator an administrator. A team must retain at least one administrator.
Adding a member does not add them to the host allowlist, and removing management membership should
not be treated as a substitute for revoking client grants. App access follows the grant/session
lifecycle below. All these rules are enforced in the signer, independently of UI controls.

![Team members showing two administrators and one member](images/members.png)

*The demo team has two administrators and a member. [Compare the member view](screenshots.md#teams-and-roles).*

## Choosing permissions

Start with a use case in the policy editor. Focusing the search field opens the catalog; search
by name, NIP, or event kind and check the use cases you need. Only selected use cases appear below
the picker. Expand one to customize its individual capabilities.

**Notes and replies**, **Long-form publishing**, **NIP-17 direct messages**, and **Marmot messaging**
are separate choices. Long-form publishing includes encrypted NIP-37 drafts and revision history;
the older kind 30024 draft format is an optional compatibility setting.

![Searchable use-case picker in the policy editor](images/policy-picker.png)

Permissions combine across selections. If a kind is included by two use cases, removing it from
one leaves the other permission in place. The editor shows these overlaps and provides a
**Disable everywhere** action. **Effective access** shows the combined result. You can also add
an unlisted kind by number.

Encryption and decryption are independent controls. Selected use cases suggest the operations
and peer scopes they need, and the editor identifies those sources. You can narrow or disable a
suggestion; the editor then points out which features may stop working. Removing a use case
removes its suggestions, while explicitly chosen overrides and saved permissions remain.

Policies save explicit permissions, not live references to templates. Reopening a policy shows
its exact signing kinds under **Individual permissions**, along with its saved cryptographic
scopes. Catalog updates cannot silently add access to an existing policy.

### Direct messages, Marmot, and gift wraps

NIP-17 direct messages use identity-signed seals (13), inbox relay metadata (10050), and NIP-44
encryption/decryption. The app signs outer gift wraps (1059) with temporary keys. Inner message
rumors are unsigned. The editor shows these layers under **Handled by the app** rather than
adding unnecessary signing access to the hosted identity.

Marmot messaging has its own preset for key packages (30443), account identity proofs (450),
welcome seals (13), inbox relays (10050), and NIP-44 operations. Gift-wrapped welcomes use 1059;
MLS encryption and temporary-key group messages (445) stay in the client. Optional controls
cover legacy key packages, relay metadata, and push owner proofs. See the
[catalog sources and signing boundaries](development/policy-catalog.md#messaging-layers).

![NIP-17 and Marmot selections with gift-wrap details and separate encryption controls](images/policy-editor.png)

## Policy examples

The UI provides controls for the policy fields; the JSON below also documents the contract for
contributors and integrations. The schema's `version: 1` is a policy format version, not a Keycast
release number.

A policy allowing notes and reactions:

```json
{
  "version": 1,
  "capabilities": {
    "sign_event": { "allowed_kinds": [1, 7] }
  }
}
```

This allows signing those kinds. It does not authorize profile changes, other event kinds,
encryption, or decryption. Event-kind permissions do not filter event content or tags, and do not
require a human approval for each matching request.

A policy for NIP-44 encryption and decryption with the hosted key itself:

```json
{
  "version": 1,
  "capabilities": {
    "nip44_encrypt": { "recipient": "self_only" },
    "nip44_decrypt": { "recipient": "self_only" }
  }
}
```

Each of `nip44_encrypt`, `nip44_decrypt`, `nip04_encrypt`, and `nip04_decrypt` is a separate
capability. The recipient scope is either `self_only` (only the managed public key) or `any`
(any peer public key). For decryption, the peer is the public key supplied with the ciphertext.
Allowing encryption does not also allow decryption, or signing the event that carries a message.

The signer rejects unknown versions, capabilities, fields, invalid recipient scopes, empty
policies, and empty or duplicate signing-kind lists. Event kinds must fit an unsigned 16-bit value.
The private management kinds `27236` and `27237` are always denied through NIP-46, even if listed.
Ordinary NIP-98 signing can be delegated to other services, but it cannot authenticate Keycast
management. The authoritative parser is [policy.rs](../core/src/v2/policy.rs).

Clients may request a narrower capability list when connecting. Such a list can only reduce
what the server policy permits; it cannot widen access. A missing client restriction still leaves
the full server policy in force.

Self-only encryption/decryption covers any data encrypted to that identity, not just drafts or
one application's data. Similarly, kind 31234 grants draft signing for any inner content kind;
the policy does not inspect the encrypted draft to restrict it to articles.

## Expiry, updates, and revocation

| Action | Effect |
|---|---|
| Claim an invitation | Consumes it and establishes a session for that client. A retry by the same client can recover an interrupted successful claim. |
| Expire or revoke an unclaimed invitation | Prevents pairing with it; existing sessions remain active. |
| Edit a policy | Changes subsequent operations for every grant using that policy. |
| Expire or revoke a grant | Prevents new operations through all its sessions and remaining invitations. |
| Client sends `logout` | Ends that client's session; other sessions on the grant remain. |
| Delete a policy | Requires revoking active grants first; retains historical references and cannot reactivate old access. |
| Restart the signer | Preserves live sessions and durable request state. |
| Restore a backup | Revokes all old grants, invitations, and sessions and requires review and fresh pairing. |

An invitation lasts at most seven days and cannot outlive its grant. A grant can deliberately have
no expiration. Use separate grants when different apps need independent revocation or policies.

Revocation blocks new operations. A response already committed before revocation may still be
delivered and cannot be recalled. See the [request lifecycle](architecture.md#request-lifecycle)
for the ordering, and [Backup and recovery](backup-and-recovery.md) for restore behavior.
