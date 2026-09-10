#!/usr/bin/env python3
"""Prepare and publish Keycast's tested, lockstep container release (Python 3.11+)."""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile
import time
import tomllib

ROOT = Path(__file__).resolve().parents[2]
REPO = "marmot-protocol/keycast"
COMPONENTS = ("api", "signer", "web")
SEMVER = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def semver(version):
    match = SEMVER.fullmatch(version)
    require(match is not None, "Expected SemVer without build metadata")
    prerelease = match[4]
    if prerelease:
        require(all(not (p.isdigit() and len(p) > 1 and p[0] == "0")
                    for p in prerelease.split(".")), "Leading zero in prerelease identifier")
    require(len("v" + version) <= 128, "Version exceeds container tag length")
    return tuple(int(match[i]) for i in (1, 2, 3)), prerelease


def run(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def api(endpoint):
    return json.loads(run("gh", "api", f"repos/{REPO}/{endpoint}"))


def version_check(tag=None):
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    semver(version)
    for component in ("api", "core", "signer"):
        package = tomllib.loads((ROOT / component / "Cargo.toml").read_text())["package"]
        require(package["version"] == {"workspace": True}, f"{component} must inherit workspace version")
    for path in ("package.json", "web/package.json"):
        require(json.loads((ROOT / path).read_text())["version"] == version, f"Version mismatch: {path}")
    packages = tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]
    for name in ("keycast_api", "keycast_core", "keycast_signer"):
        require([p["version"] for p in packages if p["name"] == name] == [version], f"Lock mismatch: {name}")
    if tag:
        require(tag == "v" + version, "Tag does not match the checked-out product version")
    return version


def prepare(version):
    semver(version)
    version_check()
    cargo = ROOT / "Cargo.toml"
    content, count = re.subn(r'(\[workspace\.package\]\s*\nversion = ")[^"]+(")',
                             lambda m: m[1] + version + m[2], cargo.read_text(), count=1)
    require(count == 1, "Workspace version not found")
    cargo.write_text(content)
    for name in ("package.json", "web/package.json"):
        path = ROOT / name
        path.write_text(re.sub(r'("version": ")[^"]+(")',
                              lambda m: m[1] + version + m[2], path.read_text(), count=1))
    path = ROOT / "Cargo.lock"
    content = path.read_text()
    for name in ("keycast_api", "keycast_core", "keycast_signer"):
        content, count = re.subn(r'(name = "' + name + r'"\nversion = ")[^"]+(")',
                                 lambda m: m[1] + version + m[2], content)
        require(count == 1, f"Missing/ambiguous lock entry: {name}")
    path.write_text(content)
    print(version_check())


def workflow_run(workflow, sha, timeout=0):
    deadline = time.monotonic() + timeout
    while True:
        runs = api(f"actions/workflows/{workflow}/runs?head_sha={sha}&event=push&branch=master&per_page=100")["workflow_runs"]
        matching = [r for r in runs if r["head_sha"] == sha and r["head_branch"] == "master" and r["event"] == "push"]
        current = max(matching, key=lambda r: r["id"]) if matching else None
        if current and current["status"] == "completed":
            require(current["conclusion"] == "success", f"{workflow} failed: {current['html_url']}")
            return {"id": current["id"], "url": current["html_url"]}
        require(time.monotonic() < deadline, f"No completed successful {workflow} for {sha}")
        print(f"Waiting for {workflow} at {sha[:12]}", flush=True)
        time.sleep(15)


def release_notes(version):
    text = (ROOT / "CHANGELOG.md").read_text()
    match = re.search(r"^## " + re.escape(version) + r"[^\n]*\n(.*?)(?=^## |\Z)", text, re.M | re.S)
    require(match is not None and match[1].strip(), "Missing changelog entry")
    return match[1].strip() + "\n"


def asset_names(version):
    return {"release.json", "images.env", "RELEASE_NOTES.md", f"keycast-{version}-deploy.tar.gz"}


def checksums(directory, version, write=False):
    expected = asset_names(version)
    actual = {p.name for p in directory.iterdir()}
    require(actual == expected | ({"SHA256SUMS"} if not write else set()), "Unexpected/missing release assets")
    require(all(p.is_file() and not p.is_symlink() for p in directory.iterdir()), "Assets must be regular files")
    sums = "".join(f"{hashlib.sha256((directory / name).read_bytes()).hexdigest()}  {name}\n" for name in sorted(expected))
    if write:
        (directory / "SHA256SUMS").write_text(sums)
    else:
        require((directory / "SHA256SUMS").read_text() == sums, "Release checksum mismatch")


def validate_manifest(manifest, version, sha):
    require(manifest["schema_version"] == 1 and manifest["version"] == version
            and manifest["tag"] == "v" + version and manifest["source_sha"] == sha,
            "Candidate version/source mismatch")
    require(re.fullmatch(r"[0-9a-f]{40}", sha), "Invalid source SHA")
    require(manifest["platforms"] == ["linux/amd64"], "Unsupported release platform")
    require(set(manifest["images"]) == set(COMPONENTS), "Incomplete image bundle")
    for component, image in manifest["images"].items():
        require(image["repository"] == f"ghcr.io/marmot-protocol/keycast-{component}", "Unexpected image repository")
        require(re.fullmatch(r"sha256:[0-9a-f]{64}", image["digest"]), "Invalid image digest")


def bundle(args):
    version = version_check()
    sha = run("git", "rev-parse", "HEAD")
    ci = workflow_run("ci.yml", sha)
    directory = Path(args.output).resolve()
    directory.mkdir(parents=True, exist_ok=False)
    manifest = {"schema_version": 1, "version": version, "tag": "v" + version,
                "source_sha": sha, "platforms": ["linux/amd64"], "ci": ci,
                "build": {"id": int(os.environ["GITHUB_RUN_ID"]),
                          "url": f"https://github.com/{REPO}/actions/runs/{os.environ['GITHUB_RUN_ID']}"},
                "images": {c: {"repository": f"ghcr.io/marmot-protocol/keycast-{c}",
                               "digest": getattr(args, c)} for c in COMPONENTS}}
    validate_manifest(manifest, version, sha)
    (directory / "release.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (directory / "images.env").write_text("".join(
        f"KEYCAST_{c.upper()}_IMAGE={manifest['images'][c]['repository']}\n"
        f"KEYCAST_{c.upper()}_DIGEST={manifest['images'][c]['digest']}\n" for c in COMPONENTS))
    (directory / "RELEASE_NOTES.md").write_text(release_notes(version))
    # Only these source files may enter a deployment archive. No instance data.
    files = ["docker-compose.prod.yml", "caddy-docker-compose-example.yml", ".env.example", "Caddyfile.example",
             "scripts/init.sh", "scripts/generate_key.sh", "scripts/upgrade_preflight.sh"]
    with tarfile.open(directory / f"keycast-{version}-deploy.tar.gz", "w:gz") as archive:
        for name in files:
            run("git", "ls-files", "--error-unmatch", name)
            archive.add(ROOT / name, arcname=name, recursive=False)
        readme = (f"Keycast {version} — Linux AMD64\n\n"
                  f"Deployment: https://github.com/{REPO}/blob/v{version}/docs/deployment.md\n"
                  "Use the images.env release asset to fill the six image/digest entries in your .env.\n"
                  "Do not replace an existing .env, database, or root credential.\n").encode()
        info = tarfile.TarInfo("README.txt")
        info.size = len(readme)
        archive.addfile(info, io.BytesIO(readme))
    checksums(directory, version, write=True)


def image_digest(reference, allow_missing=False):
    result = subprocess.run(["docker", "buildx", "imagetools", "inspect", reference,
                             "--format", "{{json .Manifest}}"], cwd=ROOT, text=True, capture_output=True)
    if result.returncode:
        require(allow_missing and ("manifest unknown" in result.stderr.lower()
                                  or "not found" in result.stderr.lower()),
                f"Cannot inspect {reference}: {result.stderr}")
        return None
    return json.loads(result.stdout)["digest"]


def stable_aliases(version, published):
    current, prerelease = semver(version)
    if prerelease:
        return []
    stable = []
    for release in published:
        if release["draft"] or release["prerelease"]:
            continue
        try:
            parsed, pre = semver(release["tag_name"].removeprefix("v"))
        except ValueError:
            continue
        if not pre:
            stable.append(parsed)
    aliases = []
    if not any(v[0] == current[0] and v > current for v in stable):
        aliases.append(f"v{current[0]}")
    if not any(v > current for v in stable):
        aliases.append("latest")
    return aliases


def publish(tag):
    version = version_check(tag)
    _, prerelease = semver(version)
    sha = run("git", "rev-parse", "HEAD")
    require(run("git", "cat-file", "-t", f"refs/tags/{tag}") == "tag", "Use an annotated tag")
    require(run("git", "rev-parse", f"refs/tags/{tag}^{{commit}}") == sha, "Tag/checkout mismatch")
    run("git", "merge-base", "--is-ancestor", sha, "origin/master")
    ci = workflow_run("ci.yml", sha)
    build = workflow_run("docker.yml", sha)
    releases = json.loads(run("gh", "api", "--paginate", "--slurp", f"repos/{REPO}/releases?per_page=100"))
    releases = [item for page in releases for item in page]
    existing = next((r for r in releases if r["tag_name"] == tag), None)
    aliases = stable_aliases(version, releases)
    with tempfile.TemporaryDirectory(prefix="keycast-release-") as temporary:
        directory = Path(temporary) / "candidate"
        run("gh", "run", "download", str(build["id"]), "--repo", REPO,
            "--name", "release-candidate", "--dir", str(directory))
        checksums(directory, version)
        manifest = json.loads((directory / "release.json").read_text())
        validate_manifest(manifest, version, sha)
        require(manifest["ci"] == ci and manifest["build"] == build, "Candidate workflow mismatch")
        require((directory / "RELEASE_NOTES.md").read_text() == release_notes(version), "Release notes mismatch")
        # Validate all three images and detect conflicts before creating any tags.
        missing = []
        for image in manifest["images"].values():
            reference = f"{image['repository']}@{image['digest']}"
            run("gh", "attestation", "verify", "oci://" + reference, "--repo", REPO,
                "--source-digest", sha, "--source-ref", "refs/heads/master",
                "--signer-workflow", f"{REPO}/.github/workflows/docker.yml", "--deny-self-hosted-runners")
            run("docker", "pull", "--platform", "linux/amd64", reference)
            config = json.loads(run("docker", "image", "inspect", reference))[0]
            require(config["Architecture"] == "amd64" and config["Os"] == "linux", "Image platform mismatch")
            labels = config["Config"]["Labels"]
            require(labels["org.opencontainers.image.version"] == version
                    and labels["org.opencontainers.image.revision"] == sha, "Image build metadata mismatch")
            exact = f"{image['repository']}:{tag}"
            digest = image_digest(exact, allow_missing=True)
            require(digest in (None, image["digest"]), f"Refusing to replace {exact}")
            if digest is None:
                missing.append(image)
        # Existing assets are immutable inputs, even when resuming a draft.
        present = set()
        if existing:
            require(existing["prerelease"] == bool(prerelease), "Existing release channel mismatch")
            present = {a["name"] for a in existing["assets"]}
            require(present <= asset_names(version) | {"SHA256SUMS"}, "Unexpected existing release assets")
            if present:
                old = Path(temporary) / "existing"
                run("gh", "release", "download", tag, "--repo", REPO, "--dir", str(old))
                for name in present:
                    require((old / name).read_bytes() == (directory / name).read_bytes(),
                            f"Refusing to replace release asset {name}")
            if not existing["draft"]:
                require(present == asset_names(version) | {"SHA256SUMS"}, "Published release is incomplete")
        else:
            command = ["gh", "release", "create", tag, "--repo", REPO, "--verify-tag", "--draft",
                       "--title", f"Keycast {tag}", "--notes-file", str(directory / "RELEASE_NOTES.md")]
            if prerelease:
                command.append("--prerelease")
            run(*command)
        for path in sorted(directory.iterdir()):
            if path.name not in present:
                run("gh", "release", "upload", tag, str(path), "--repo", REPO)
        uploaded = Path(temporary) / "uploaded"
        run("gh", "release", "download", tag, "--repo", REPO, "--dir", str(uploaded))
        checksums(uploaded, version)
        for path in directory.iterdir():
            require((uploaded / path.name).read_bytes() == path.read_bytes(), "Uploaded asset mismatch")
        for image in missing:
            run("docker", "buildx", "imagetools", "create", "--prefer-index=false",
                "--tag", f"{image['repository']}:{tag}", f"{image['repository']}@{image['digest']}")
        for image in manifest["images"].values():
            require(image_digest(f"{image['repository']}:{tag}") == image["digest"], "Version tag digest mismatch")
        if not existing or existing["draft"]:
            run("gh", "release", "edit", tag, "--repo", REPO, "--draft=false",
                f"--prerelease={'true' if prerelease else 'false'}", f"--latest={'true' if 'latest' in aliases else 'false'}")
        require(api(f"releases/tags/{tag}")["immutable"],
                "Published release is not immutable; check repository release settings")
        for alias in aliases:
            for image in manifest["images"].values():
                run("docker", "buildx", "imagetools", "create", "--prefer-index=false", "--tag",
                    f"{image['repository']}:{alias}", f"{image['repository']}@{image['digest']}")
                require(image_digest(f"{image['repository']}:{alias}") == image["digest"], "Stable alias digest mismatch")
        print(f"Published https://github.com/{REPO}/releases/tag/{tag}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("prepare").add_argument("version")
    check = sub.add_parser("check-version")
    check.add_argument("--tag")
    wait = sub.add_parser("wait-ci")
    wait.add_argument("--sha", required=True)
    wait.add_argument("--timeout", type=int, default=1800)
    candidate = sub.add_parser("bundle")
    for component in COMPONENTS:
        candidate.add_argument("--" + component, required=True)
    candidate.add_argument("--output", required=True)
    sub.add_parser("publish").add_argument("tag")
    args = parser.parse_args()
    if args.command == "prepare":
        prepare(args.version)
    elif args.command == "check-version":
        print(version_check(args.tag))
    elif args.command == "wait-ci":
        print(json.dumps(workflow_run("ci.yml", args.sha, args.timeout)))
    elif args.command == "bundle":
        bundle(args)
    elif args.command == "publish":
        publish(args.tag)


if __name__ == "__main__":
    main()
