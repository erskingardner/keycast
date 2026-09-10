# Release runbook

[Development](README.md) · [Deployment](../deployment.md) · [Backup and recovery](../backup-and-recovery.md)

Keycast uses **SemVer**, with one product version for API, signer, and web. The first numbered release
is **2.0.0-rc.1**, followed by **2.0.0** after early-user feedback and the stable checks below.
Releases are maintainer decisions; merging an ordinary change does not publish a release.

## Supported artifacts

Official releases ship **Linux AMD64 containers for Docker Compose**. We will add ARM64 or native
binaries only when operators ask for them and we can test and maintain those distributions.
Developers can continue building from source on compatible systems.

Each GitHub release contains:

- `release.json`: product version, source commit, all three image repositories/digests, supported
  platform, and links to the successful CI and image-build runs.
- `images.env`: the six image repository/digest settings to copy into an instance's existing `.env`.
- `keycast-VERSION-deploy.tar.gz`: matching production Compose, the Caddy example, configuration
  example, initialization and preflight scripts, and a deployment-guide link. No instance data.
- `RELEASE_NOTES.md` and `SHA256SUMS` for the downloadable assets.

Production Compose deploys explicit digests. Exact image tags such as `v2.0.0-rc.1` identify a release;
`v2` and `latest` are stable lookup aliases. **Prereleases never change stable aliases.** Until the
first stable numbered release, those aliases may still identify the old unnumbered master build.
Select an RC through its release manifest, not through `latest`.

## Version policy

| Change to a supported contract | Increment | Example |
|---|---|---|
| Compatible bug or dependency fix | Patch | `2.0.0` → `2.0.1` |
| Compatible feature | Minor | `2.0.1` → `2.1.0` |
| Breaking client, policy, configuration, or upgrade behavior | Major | `2.1.0` → `3.0.0` |

The compatibility contract covers the management API, supported NIP-46 behavior, saved policies and
grants, documented deployment configuration, and supported data upgrades. An additive database
migration is not automatically a major release, but requires an upgrade and rollback assessment.
Security fixes that reject previously accepted invalid behavior need a clear release note.

Policy document versions, schema versions, and NIP/Marmot revisions are separate identifiers.
Do not bump them to match a product release. Use SemVer without build metadata (`+...`) because the
version is also a container tag. The root `Cargo.toml` workspace version is authoritative; all three
Rust crates inherit it. Preparation synchronizes the private JavaScript packages and Cargo lock entries
without changing dependency versions. CI rejects version mismatches.

API/signer `--version`, OCI image labels, and the operator status page expose version information.
Official images include their full source revision; source builds show `development` unless
`KEYCAST_BUILD_REVISION` is provided at build time. The status page reports signer and web builds.

## Prepare and validate

Use Python 3.11+, Git, GitHub CLI, Docker/Buildx, and the toolchain in the
[development guide](README.md#validation). GitHub Actions performs the official Linux AMD64 build.
The repository must have **immutable releases enabled** in Settings → General → Releases before
publication. An administrator can check that setting with:

```sh
gh api repos/marmot-protocol/keycast/immutable-releases
```

The release workflow uses ordinary contents/package permissions; it does not receive administration
access to change that setting. It verifies the published release is immutable.

1. Start from current `master`, choose the version, and run:

   ```sh
   python3 scripts/releases/release.py prepare 2.0.0-rc.1
   python3 scripts/releases/release.py check-version
   ```

2. Add the version to [CHANGELOG.md](../../CHANGELOG.md). Describe user-visible changes, supported
   upgrade sources, migrations, changed defaults, policy/session behavior, known limitations, and
   rollback. Review the complete diff. Run the [validation commands](README.md#validation), plus:

   ```sh
   python3 -m unittest discover -s scripts/releases -v
   ```

3. Merge the reviewed release preparation into `master`. Wait for **CI and Docker Images to finish
   successfully on that exact commit**. Docker Images waits for full CI, builds all three Linux AMD64
   targets, runs the combined read-only-container smoke, pushes uniquely tagged candidate images,
   pulls and smokes their exact published digests, then attests and records the bundle. The resulting
   `release-candidate` workflow artifact is retained for 90 days. Nothing moves stable aliases.

4. Download and review that candidate before tagging:

   ```sh
   gh run list --branch master --commit COMMIT_SHA
   gh run download DOCKER_RUN_ID --name release-candidate --dir /tmp/keycast-release-review
   (cd /tmp/keycast-release-review && shasum -a 256 -c SHA256SUMS)
   cat /tmp/keycast-release-review/release.json
   ```

Use a fresh review directory each time. Confirm the manifest version/SHA, all three digests, platform,
validation run links, release notes, and archive contents. Never infer success from a local build alone.

## Publish

Check out the validated commit and use an **annotated** tag:

```sh
git fetch origin master --tags
git switch --detach COMMIT_SHA
python3 scripts/releases/release.py check-version --tag v2.0.0-rc.1
git tag -a v2.0.0-rc.1 -m "Keycast v2.0.0-rc.1"
git push origin refs/tags/v2.0.0-rc.1
```

[Release](../../.github/workflows/release.yml) verifies that the tag matches the product version and
belongs to `master`, requires successful CI and Docker Images runs for the same commit, and downloads
that build's candidate. It checks asset hashes, all three source-bound image attestations, platform,
and version/revision labels. It refuses conflicting exact-version image tags or existing release assets.

The workflow assembles a draft GitHub release, attaches the complete bundle, assigns exact image tags
to the recorded digests without rebuilding, verifies those tags, and publishes the release. Stable
versions then advance their major alias and `latest` only when no newer applicable stable release
exists. Registry publication is serialized across image builds and releases.

Verify the published result:

```sh
gh release view v2.0.0-rc.1
gh release verify v2.0.0-rc.1
gh release download v2.0.0-rc.1 --dir /tmp/keycast-published-release
(cd /tmp/keycast-published-release && shasum -a 256 -c SHA256SUMS)
gh release verify-asset v2.0.0-rc.1 /tmp/keycast-published-release/release.json
```

Compare each exact image tag's digest with `release.json`, and verify each image's provenance using
the manifest's full source SHA:

```sh
docker buildx imagetools inspect ghcr.io/marmot-protocol/keycast-signer:v2.0.0-rc.1
gh attestation verify oci://ghcr.io/marmot-protocol/keycast-signer@sha256:CHOSEN_DIGEST \
  --repo marmot-protocol/keycast --source-digest COMMIT_SHA \
  --source-ref refs/heads/master \
  --signer-workflow marmot-protocol/keycast/.github/workflows/docker.yml
```

Repeat for API and web. A complete immutable release manifest is the authoritative three-component
bundle: three registry repositories cannot be updated atomically.

## Failed publication and retries

Read the failed job before retrying. For a transient failure, rerun the **Release** workflow; it reuses
the recorded candidate and permits only matching existing tags/assets. A partially assembled draft or
partial image tagging is recoverable this way. Do not rerun Docker Images to replace a candidate after
version tags or release assets have been created. A conflicting asset or image is a stop condition,
not a reason to use `--clobber`, force-push a tag, or delete an immutable release.

Never move a published version tag or replace its artifacts. Fixes use a new version. If release
source or artifacts need changes, prepare and validate a new candidate/version. Keep the original
release and its provenance available for operators. A published RC and stable release normally have
different source commits and digests because they embed different versions.

## Stable acceptance, deployment, and rollback

Before `2.0.0`, collect RC feedback and exercise a representative NIP-46 client, permitted and denied
signing/encryption operations, a populated upgrade, and backup/recovery using disposable data. Review
[TODO.md](../../TODO.md) and [AUDIT.md](../../AUDIT.md); record the exact candidate and remaining limits.
Then prepare `2.0.0` and follow the same validation and publication process.

Deployment is a separate operation. Follow [upgrading](../upgrading.md): back up and verify the target,
retain its old three-image manifest, verify the new artifacts, deploy all three together, and check
readiness plus a signing flow. Publication does not establish that an instance is deployed or healthy.

An image-only rollback requires compatibility with the resulting database and configuration. Otherwise,
recover from a compatible backup and account for data written afterward. Restoration deliberately revokes
old grants, invitations, and sessions, requiring review and re-pairing. Use the concrete
[recovery procedure](../backup-and-recovery.md#recovery-after-loss-or-rollback). Original Keycast/V1 data,
credentials, and bunker URLs are incompatible; there is no automatic V1 migration.
