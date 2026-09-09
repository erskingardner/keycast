# Keycast

Your keys. Your rules. Your server.

**A self-hosted remote signer for Nostr.** Give your apps permission to use your keys
without handing each app your private key.

Keycast keeps your Nostr keys on your server and handles signing requests over NIP-46
(Nostr Connect). You choose what each app can do, pair it with a one-time invitation,
and revoke its access when you need to. It is built for personal accounts, families,
and small teams running one instance on one host.

## Start here

| I want to… | Start with… |
|---|---|
| Connect an app to Keycast | [Getting started](docs/getting-started.md) |
| Run my own instance | [Deployment](docs/deployment.md) |
| Understand permissions and access | [Policies and access](docs/policies-and-access.md) |
| Understand how it works | [Architecture](docs/architecture.md) |
| Work on the code | [Development and contribution](docs/development/README.md) |
| Look up past decisions and test results | [Project history](docs/development/history/README.md) |

Browse the [full documentation](docs/README.md) for operations, backups, relays, and upgrades.

## How it works

```text
  Your Nostr app  <---->  Nostr relays  <---->  Keycast signer
                                                   |
                                            encrypted keys
                                            + access policy
```

Import a key, create a policy, and connect an app. Keycast checks each request against
that app's access before signing an event or performing an allowed encryption operation.
The app receives the result; the private key stays with the signer.

- **Explicit permissions.** Allow specific event kinds and encryption/decryption operations.
  Capabilities you do not grant are denied.
- **Access you can revoke.** Separate grants for different apps or purposes, one-time pairing
  invitations, and persistent client sessions.
- **A shared workspace.** Organize keys and policies into teams with their own administrators.
- **Visibility into your instance.** Inspect signing readiness, relay diagnostics, activity,
  and resource usage.
- **Recovery tools.** Encrypted online backups, a reviewed restore process, and root credential
  rotation through the trusted host CLI.

The web UI manages the instance using approvals from an **external Nostr signer**. Keep that
signer available: Keycast deliberately refuses to sign its own management approvals.
See [sign-in and pairing](docs/getting-started.md) for the distinction.

## Run Keycast

The supported deployment is a Linux host with Docker Compose, a hostname, and an HTTPS reverse
proxy. The stack has three services: a SvelteKit web UI, an Axum API, and a Rust signer.
Only the signer opens the database and root credential.

The [deployment guide](docs/deployment.md) walks through initialization, choosing image digests,
HTTPS routing, and checking the running instance. Then follow [Getting started](docs/getting-started.md)
to connect your first app. Already running Keycast? Read [Upgrading](docs/upgrading.md), including
the required fresh setup when moving from the original release.

Keycast has not received an independent security audit. The host and signer are trusted with
your keys, and browser import also exposes the imported key to the web stack. Read the
[security model](docs/security.md) before choosing which keys to host.

## Build and contribute

The workspace uses Rust and SvelteKit, with Bun for frontend tooling. The
[development guide](docs/development/README.md) covers local setup, the code layout, validation,
and container testing. [TODO.md](TODO.md) tracks open work;
[AGENTS.md](AGENTS.md) records repository rules and security boundaries.

Past architecture decisions, hardening reviews, VM rehearsals, and relay soak results live in
the [development history](docs/development/history/README.md).

## License

[MIT](LICENSE)
