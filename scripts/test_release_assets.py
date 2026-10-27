"""Exercise the release verifier with complete and damaged package fixtures."""

import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile
import hashlib

spec = importlib.util.spec_from_file_location(
    "verify_release", Path(__file__).with_name("verify-release.py")
)
verifier = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verifier)
COMMIT = "a" * 40


class ReleasedExamples(unittest.TestCase):
    def test_v002_examples_retain_their_published_bytes(self):
        root = Path(__file__).resolve().parents[1]
        manifest = root / "tests/fixtures/v0.0.2-examples.sha256"
        entries = manifest.read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(entries), 22)
        for entry in entries:
            expected, name = entry.split("  ", 1)
            self.assertEqual(hashlib.sha256((root / "tests/fixtures/releases/v0.0.2" / name).read_bytes()).hexdigest(), expected, name)
            if name.endswith(".stdout"):
                self.assertEqual(hashlib.sha256((root / name).read_bytes()).hexdigest(), expected, name)


class ReleaseAssets(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.directory = Path(self.scratch.name)
        self.build()

    def build(self, *, omit=None, executable=True, version="0.0.2", working_tree="clean", record_version=None):
        for target, suffix in (
            ("x86_64-pc-windows-msvc", ".zip"),
            ("x86_64-unknown-linux-gnu", ".tar.gz"),
            ("aarch64-apple-darwin", ".tar.gz"),
        ):
            root = f"scarp-{version}-{target}"
            archive = self.directory / (root + suffix)
            files = {name: b"fixture" for name in (
                "scarp.exe" if suffix == ".zip" else "scarp", "LICENSE",
                "README.md", "CHANGELOG.md", "CONTRIBUTING.md",
                f"docs/releases/v{version}.md", "examples/bindings.srp",
                "examples/mutable.srp", "examples/scopes.srp", "examples/decisions.srp",
            ) if name != omit}
            if version.startswith("0.0.3-"):
                for name in ("loops", "repeat", "control_escapes"):
                    for extension in ("srp", "stdout"):
                        member = f"examples/{name}.{extension}"
                        if member != omit:
                            files[member] = b"fixture"
            files["BUILD-INFO.txt"] = (
                f"version={record_version or version}\ntarget={target}\ncommit={COMMIT}\n"
                f"working_tree={working_tree}\ncompiler=rustc fixture\n"
            ).encode()
            if suffix == ".zip":
                with zipfile.ZipFile(archive, "w") as package:
                    for name, data in files.items():
                        package.writestr(root + "/" + name, data)
            else:
                with tarfile.open(archive, "w:gz") as package:
                    for name, data in files.items():
                        member = tarfile.TarInfo(root + "/" + name)
                        member.size = len(data)
                        member.mode = 0o755 if name == "scarp" and executable else 0o644
                        package.addfile(member, io.BytesIO(data))
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            archive.with_name(archive.name + ".sha256").write_text(
                f"{digest}  {archive.name}\n", encoding="utf-8"
            )

    def test_complete_packages_produce_sorted_manifest(self):
        manifest = verifier.verify(self.directory, "0.0.2", COMMIT)
        self.assertEqual(len(manifest.splitlines()), 3)
        self.assertEqual(manifest.splitlines(), sorted(manifest.splitlines()))

    def test_wrong_source_commit_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "unexpected commit"):
            verifier.verify(self.directory, "0.0.2", "b" * 40)

    def test_corrupt_archive_is_rejected_before_opening(self):
        next(self.directory.glob("*.zip")).write_bytes(b"damaged")
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            verifier.verify(self.directory, "0.0.2", COMMIT)

    def test_missing_release_document_is_rejected(self):
        self.build(omit="docs/releases/v0.0.2.md")
        with self.assertRaisesRegex(ValueError, "missing docs/releases"):
            verifier.verify(self.directory, "0.0.2", COMMIT)

    def test_unix_executable_permissions_are_required(self):
        self.build(executable=False)
        with self.assertRaisesRegex(ValueError, "executable permission"):
            verifier.verify(self.directory, "0.0.2", COMMIT)

    def test_beta_packages_include_iteration_and_control_byte_fixtures(self):
        self.build(version="0.0.3-beta.1")
        self.assertEqual(len(verifier.verify(self.directory, "0.0.3-beta.1", COMMIT).splitlines()), 3)

    def test_beta_packages_cannot_omit_the_repeat_example(self):
        self.build(version="0.0.3-beta.1", omit="examples/repeat.srp")
        with self.assertRaisesRegex(ValueError, "missing examples/repeat"):
            verifier.verify(self.directory, "0.0.3-beta.1", COMMIT)

    def test_beta_packages_reject_old_version_records(self):
        self.build(version="0.0.3-beta.1", record_version="0.0.2")
        with self.assertRaisesRegex(ValueError, "unexpected version"):
            verifier.verify(self.directory, "0.0.3-beta.1", COMMIT)

    def test_modified_source_records_are_not_release_assets(self):
        self.build(working_tree="modified")
        with self.assertRaisesRegex(ValueError, "unexpected working_tree"):
            verifier.verify(self.directory, "0.0.2", COMMIT)


if __name__ == "__main__":
    unittest.main()
