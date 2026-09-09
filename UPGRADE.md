# Upgrading Keycast

See [the upgrade guide](docs/upgrading.md) for routine updates, rollback considerations, and the
transition from the original release to version 2. Existing deployments using the old bridge
network or Docker-socket proxy must follow [Hardening migration](docs/upgrading.md#hardening-migration).

The original database and bunker credentials are incompatible with version 2. That transition
requires a fresh installation, key re-import, and new client invitations. Preserve the old database
and its matching root credential before replacing a deployment.

For current-generation updates, preserve `database/` and `master.key`, back up first, and use
reviewed image digests. Never regenerate the root credential as an upgrade step.

The [original cutover procedure](docs/development/history/V1_UPGRADE.md) is retained as project history.
