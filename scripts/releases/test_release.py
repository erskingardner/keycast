"""Release boundary tests; no registry or GitHub mutations."""
import copy
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("release", Path(__file__).with_name("release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseTests(unittest.TestCase):
    def test_semver_and_rejected_tags(self):
        self.assertEqual(release.semver("2.0.0-rc.1"), ((2, 0, 0), "rc.1"))
        for value in ("v2.0.0", "2.00.0", "2.0", "2.0.0-rc.01", "2.0.0+build", "2.0.0-", "2.0.0\n"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                release.semver(value)

    def test_stable_aliases_do_not_regress(self):
        def published(tag):
            return {"tag_name": tag, "draft": False, "prerelease": False}
        self.assertEqual(release.stable_aliases("2.0.0-rc.1", []), [])
        self.assertEqual(release.stable_aliases("2.0.0", []), ["v2", "latest"])
        self.assertEqual(release.stable_aliases("2.0.0", [published("v2.0.1")]), [])
        self.assertEqual(release.stable_aliases("2.0.1", [published("v3.0.0")]), ["v2"])
        self.assertEqual(release.stable_aliases("2.10.0", [published("v2.9.0")]), ["v2", "latest"])

    def test_prepare_preserves_dependencies_and_enforces_versions(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in ("Cargo.toml", "Cargo.lock", "package.json", "web/package.json",
                         "api/Cargo.toml", "core/Cargo.toml", "signer/Cargo.toml"):
                (root / name).parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(release.ROOT / name, root / name)
            old_dependencies = json.loads((root / "web/package.json").read_text())["dependencies"]
            with patch.object(release, "ROOT", root):
                release.prepare("2.1.0-rc.2")
                self.assertEqual(release.version_check("v2.1.0-rc.2"), "2.1.0-rc.2")
                self.assertEqual(json.loads((root / "web/package.json").read_text())["dependencies"], old_dependencies)
                with self.assertRaises(ValueError):
                    release.version_check("v2.0.0")
                path = root / "web/package.json"
                path.write_text(path.read_text().replace('"2.1.0-rc.2"', '"1.0.0"'))
                with self.assertRaises(ValueError):
                    release.version_check()

    def manifest(self):
        return {"schema_version": 1, "version": "2.0.0-rc.1", "tag": "v2.0.0-rc.1",
                "source_sha": "a" * 40, "platforms": ["linux/amd64"],
                "images": {c: {"repository": f"ghcr.io/marmot-protocol/keycast-{c}",
                               "digest": "sha256:" + "b" * 64} for c in release.COMPONENTS}}

    def test_candidate_rejects_mixed_source_platform_and_images(self):
        valid = self.manifest()
        release.validate_manifest(valid, "2.0.0-rc.1", "a" * 40)
        for key, value in (("version", "2.0.0"), ("source_sha", "c" * 40),
                           ("platforms", ["linux/arm64"]), ("images", {})):
            invalid = copy.deepcopy(valid)
            invalid[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                release.validate_manifest(invalid, "2.0.0-rc.1", "a" * 40)
        valid["images"]["api"]["repository"] = "ghcr.io/another/keycast-api"
        with self.assertRaises(ValueError):
            release.validate_manifest(valid, "2.0.0-rc.1", "a" * 40)

    def test_checksums_reject_changes_extras_and_unsafe_entries(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            version = "2.0.0-rc.1"
            for name in release.asset_names(version):
                (root / name).write_text("fixture")
            release.checksums(root, version, write=True)
            release.checksums(root, version)
            original = (root / "SHA256SUMS").read_text()
            for bad in (original + original.splitlines()[0] + "\n", original.replace("images.env", "../images.env")):
                (root / "SHA256SUMS").write_text(bad)
                with self.assertRaises(ValueError):
                    release.checksums(root, version)
            (root / "SHA256SUMS").write_text(original)
            (root / "images.env").write_text("tampered")
            with self.assertRaises(ValueError):
                release.checksums(root, version)
            (root / "images.env").write_text("fixture")
            (root / "extra").touch()
            with self.assertRaises(ValueError):
                release.checksums(root, version)

    def test_ci_does_not_fall_back_to_an_older_success(self):
        def run(identifier, conclusion):
            return {"id": identifier, "head_sha": "a" * 40, "head_branch": "master", "event": "push",
                    "status": "completed", "conclusion": conclusion, "html_url": "https://example.test"}
        with patch.object(release, "api", return_value={"workflow_runs": [run(2, "failure"), run(1, "success")]}):
            with self.assertRaises(ValueError):
                release.workflow_run("ci.yml", "a" * 40)

    def test_publish_rc_checks_all_images_and_never_moves_stable_tags(self):
        manifest = self.manifest()
        ci = {"id": 10, "url": "https://example.test/ci"}
        build = {"id": 20, "url": "https://example.test/build"}
        manifest.update(ci=ci, build=build)
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary) / "fixture"
            fixture.mkdir()
            for name in release.asset_names(manifest["version"]):
                (fixture / name).write_text("fixture")
            (fixture / "release.json").write_text(json.dumps(manifest))
            (fixture / "RELEASE_NOTES.md").write_text("notes")
            release.checksums(fixture, manifest["version"], write=True)
            commands = []
            def fake_run(*args):
                commands.append(args)
                if args[:3] == ("git", "cat-file", "-t"):
                    return "tag"
                if args[:2] == ("git", "rev-parse"):
                    return manifest["source_sha"]
                if args[:2] == ("gh", "api"):
                    return "[[]]"
                if args[:3] in (("gh", "run", "download"), ("gh", "release", "download")):
                    shutil.copytree(fixture, Path(args[args.index("--dir") + 1]))
                if args[:3] == ("docker", "image", "inspect"):
                    return json.dumps([{"Architecture": "amd64", "Os": "linux", "Config": {"Labels": {
                        "org.opencontainers.image.version": manifest["version"],
                        "org.opencontainers.image.revision": manifest["source_sha"]}}}])
                return ""
            with patch.object(release, "run", side_effect=fake_run), \
                 patch.object(release, "api", return_value={"immutable": True}), \
                 patch.object(release, "workflow_run", side_effect=[ci, build]), \
                 patch.object(release, "version_check", return_value=manifest["version"]), \
                 patch.object(release, "release_notes", return_value="notes"), \
                 patch.object(release, "image_digest", side_effect=[None] * 3 + ["sha256:" + "b" * 64] * 3):
                release.publish(manifest["tag"])
            attestations = [c for c in commands if c[:3] == ("gh", "attestation", "verify")]
            self.assertEqual(len(attestations), 3)
            self.assertTrue(all("--source-digest" in c for c in attestations))
            promotions = [c for c in commands if c[:4] == ("docker", "buildx", "imagetools", "create")]
            self.assertEqual(len(promotions), 3)
            self.assertTrue(all(c[c.index("--tag") + 1].endswith(":v2.0.0-rc.1") for c in promotions))
            publication = next(c for c in commands if c[:3] == ("gh", "release", "edit"))
            self.assertIn("--latest=false", publication)
            self.assertIn("--prerelease=true", publication)
            self.assertFalse(any("--clobber" in c for c in commands))

    def test_registry_auth_errors_are_not_missing_tags(self):
        with patch.object(release.subprocess, "run", return_value=subprocess.CompletedProcess([], 1, "", "unauthorized")):
            with self.assertRaises(ValueError):
                release.image_digest("example:v2.0.0", allow_missing=True)
        with patch.object(release.subprocess, "run", return_value=subprocess.CompletedProcess([], 1, "", "manifest unknown")):
            self.assertIsNone(release.image_digest("example:v2.0.0", allow_missing=True))


if __name__ == "__main__":
    unittest.main()
