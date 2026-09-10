# Changelog

All components use one [SemVer](docs/development/releases.md) version.

## 2.0.0-rc.1 — 2026-09-10

The first numbered release of the rebuilt Keycast remote signer. This is a release candidate for
self-hosters and early users; stable `2.0.0` follows feedback and the stable release checks.

### Highlights

- A single multiplexed NIP-46 signer with shared relays, per-key relay routing, durable request/reply
  handling, bounded resource admission, and readiness reporting.
- Team workspaces, encrypted key storage, invitations, sessions, and strict policies enforced by
  the signer. The public API communicates over a private Unix socket and cannot load the root key.
- A searchable policy editor with customizable use cases for notes and replies, long form publishing,
  NIP-17 direct messages, Marmot messaging, and many other Nostr capabilities. NIP-04 and NIP-44
  encryption/decryption permissions are separate controls. Existing saved policies retain their meaning.
- Sign-in/sign-out updates immediately, plus operator status, audit history, encrypted backup,
  recovery, and root-key rotation tools.
- Reorganized user, deployment, architecture, and contributor guides with real application screenshots.
- Numbered releases with version/commit metadata, provenance-verified image digests, and a matching
  deployment bundle. API, signer, and web are released together.

### Installation and upgrades

Official artifacts are **Linux AMD64 containers** for Docker Compose. No native binaries or additional
platform builds are distributed. Use all three digests from this release's `release.json`/`images.env`.
Exact image tags are `v2.0.0-rc.1`; this prerelease does not change `v2` or `latest`.

A fresh installation is recommended for evaluating the release candidate. Original Keycast/V1
storage, encrypted keys, authorization rows, credentials, and bunker URLs are incompatible; there
is no V1 migration. Existing deployments of the rebuilt V2 code should back up before upgrading,
review the additive migrations and [hardening steps](https://github.com/marmot-protocol/keycast/blob/v2.0.0-rc.1/docs/upgrading.md),
and retain their previous three-image manifest. Release metadata adds no database migration or
policy format change relative to the immediately preceding master build.

An image rollback is safe only when the older software supports the resulting database and configuration.
[Backup restoration](https://github.com/marmot-protocol/keycast/blob/v2.0.0-rc.1/docs/backup-and-recovery.md#recovery-after-loss-or-rollback)
revokes old grants, invitations, and sessions and requires review and re-pairing. Never regenerate
an existing root credential to resolve an upgrade problem.

### Known limitations

Keycast targets a small, single-host instance. It has not received an independent security audit.
Relay timeout/admission findings, real host fault recovery, and broader external-signer/client
compatibility checks remain tracked in [TODO.md](https://github.com/marmot-protocol/keycast/blob/v2.0.0-rc.1/TODO.md).
A release and its automated checks do not establish the health of a deployed instance.
