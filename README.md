# Keycast

Secure remote signing and permissions for teams using Nostr.

## Overview

Keycast aims to make remote signing and secure key management easy for teams using Nostr. Previous solutions like [nsec.app](https://nsec.app/), [Knox](https://gitlab.com/soapbox-pub/knox), and [Amber](https://github.com/greenart7c3/Amber) are great for individuals, but Keycast is designed for teams. This means that you can collaboratively manage your keys and create policies and permissions to control who can sign and what they can sign.

Keycast is fully open source and will offer both a hosted version (if you don't want to have to manage your own deployment) and options for running your own sovereign instance via Docker, StartOS, or Umbrel.

## Features

- [x] NIP-98 HTTP Auth based web application and API authentication
- [x] Team management (create teams, manage stored keys, manage users, manage policies). Supports multiple teams per user.
- [x] Secure key management (row-level aes-256 encryption, file or aws kms backed key storage)
- [x] Permissions and policies (flexible, extensible permissions model)
- [x] NIP-46 Remote signing for managed keys
- [x] Docker based deployment
- [ ] StartOS service
- [ ] Umbrel app
- [ ] CLI for managing teams, keys, users, and policies

## Contributing

Contributions are welcome! Please fork or clone the repository and submit a PR with your changes. Small, well-documented changes are appreciated.

### Contributors

<a href="https://github.com/erskingardner/keycast/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=erskingardner/keycast" />
</a>

### The stack

The `api` subdirectory is a Rust application that uses SQLx for database interactions on a local SQLite database. We use `cargo watch` to run the API in watch mode, make sure to install that if you don't already have it.
- [Rust](https://www.rust-lang.org/)
- [SQLx](https://github.com/launchbadge/sqlx)
- [cargo-watch](https://github.com/watchexec/cargo-watch)

The `web` subdirectory contains a SvelteKit app that uses Bun for bundling and Tailwind for styling.
- [SvelteKit](https://kit.svelte.dev/)
- [Bun](https://bun.sh/)
- [Tailwind](https://tailwindcss.com/)

### Getting Started

1. Clone the repository and install workspace dependencies with `bun install`
2. Install the web app dependencies with `cd web && bun install`
3. Config is done with `config.toml` files. You shouldn't have to change anything to get started. The only setting that has any config at the moment is the signing daemon process health check interval. In the future, database location settings will be added here.
4. Then, from the root directory, generate a master encryption key with `bun run key:generate`. This master key is used to encrypt and decrypt Nostr private keys in the database. These are only decrypted when used and remain encrypted at rest. In the future we hope to support other key storage methods, like AWS KMS.

### Running the dev server (API + Web + Signer)
1. You can now run the dev server with `bun run dev`. We use `concurrently` to run the API and web app in parallel. You'll see the web app start up at `https://localhost:5173` and the API will start up at `http://localhost:3000`. Both apps will output logs to the console and will hotreload on code changes. The signer will also start up and will spawn signing processes for each of your authentications. The signing manager will keep an eye on the authentications and pick up new ones as they come in or remove them if they are removed from the database. It will also attempt to restart signing processes if they crash.

### Managing the database

The database is a local SQLite database. There is a helper command to reset the database (drop, create, and run migrations). More can be added as needed.

- `bun run db:reset` - Reset the database (drop, create, and run migrations)

### Custom Permissions

Keycast is built with a flexible permissions model that allows you to define custom permissions. These permissions are defined in the `core/src/custom_permissions` directory. You can define your own custom permissions by implementing the `CustomPermission` trait which has three methods, `can_encrypt`, `can_decrypt`, and `can_sign`. These methods take in the same arguments as the NIP-46 Request objects and return a boolean.

Each request for one of the matching methods (`sign_event`, `nip04_encrypt`, `nip04_decrypt`, `nip44_encrypt`, `nip44_decrypt`) will be checked against all the permissions defined in the policy. If the permission is not granted, the request will be denied.

To make your custom permission usable in the app, you'll also need to reference it in three places:
1. The `AVAILABLE_PERMISSIONS` array in `web/src/lib/types.ts`
1. The `AVAILABLE_PERMISSIONS` array in `core/src/custom_permissions/mod.rs`
1. The `to_custom_permission` method in `core/src/types/permission.rs`

## Deployment

To start, we support Docker. 

1. ssh into your VM or server where you'll want to run Keycast.
1. Install docker following the instructions for your OS here: https://docs.docker.com/engine/install
1. Ensure you have docker compose: https://docs.docker.com/compose/install
1. Clone the repository and navigate to the root directory. `git clone https://github.com/erskingardner/keycast.git && cd keycast`
1. Build the docker image with `sudo docker compose build`
1. Run the docker container with `sudo docker compose up` or `sudo docker compose up -d` to run in detached mode.

### VM requirements

The running app requires very little resources but in order to build the docker images you'll need at least 2GB of RAM (usually helps to have some swap space set up as well). If you're seeing the following errors when building, try to increase the swap space on your VM or use a larger VM.

`failed to solve: process "/bin/sh -c bun run build" did not complete successfully: exit code: 137`

### Reverse proxy

There is also a `Caddyfile` that will attempt to setup a reverse proxy on your server. If you want to use this, you'll need to have a domain name and update the caddyfile with your domain name.

## License

[MIT](LICENSE)
