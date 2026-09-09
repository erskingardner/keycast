# Keycast documentation

Keycast lets Nostr apps request signatures and permitted encryption operations from keys hosted
on your server. Start with the guide that matches what you are doing.

[Take the screenshot tour](screenshots.md) to see signing access, team administration, mobile layouts,
and instance operations with a populated demo.

![Lighthouse Studio demo workspace with three keys](images/workspace.png)

*Screenshots throughout these guides use fictional profiles in an isolated instance.*

## Use Keycast

- [Getting started](getting-started.md): sign in, import a key, and pair your first app.
- [Policies and access](policies-and-access.md): teams, permissions, grants, invitations, and sessions.
- [Security model](security.md): what Keycast protects, what it trusts, and how management approvals work.

## Run an instance

- [Deployment](deployment.md): Docker Compose, configuration, HTTPS, and first-run checks.
- [Operations](operations.md): trusted CLI commands, availability, limits, monitoring, and audit export.
- [Backup and recovery](backup-and-recovery.md): encrypted archives, restore review, and root rotation.
- [Relays](relays.md): baseline routes, imported-key discovery, client compatibility, and diagnostics.
- [Upgrading](upgrading.md): routine updates and the transition from the original release.

## Understand and contribute

- [Architecture](architecture.md): process boundaries, storage, authorization, and the request lifecycle.
- [Development](development/README.md): local setup, repository layout, and validation.
- [Project history](development/history/README.md): dated decisions, audit evidence, and deployment experiments.
- [Open work](../TODO.md): remaining engineering and deployment follow-ups.

The guides above describe the current implementation. Historical reports preserve the scope and
results of particular builds; they are not current deployment instructions or availability promises.

[Back to Keycast](../README.md)
