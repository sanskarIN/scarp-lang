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
            self.assertEqual(hashlib.sha256((root / name).read_bytes()).hexdigest(), expected, name)


class ReleaseAssets(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.directory = Path(self.scratch.name)
        self.build()

    def build(self, *, omit=None, executable=True):
        for target, suffix in (
            ("x86_64-pc-windows-msvc", ".zip"),
            ("x86_64-unknown-linux-gnu", ".tar.gz"),
            ("aarch64-apple-darwin", ".tar.gz"),
        ):
            root = f"scarp-0.0.2-{target}"
            archive = self.directory / (root + suffix)
            files = {name: b"fixture" for name in (
                "scarp.exe" if suffix == ".zip" else "scarp", "LICENSE",
                "README.md", "CHANGELOG.md", "CONTRIBUTING.md",
                "docs/releases/v0.0.2.md", "examples/bindings.srp",
                "examples/mutable.srp", "examples/scopes.srp", "examples/decisions.srp",
            ) if name != omit}
            files["BUILD-INFO.txt"] = (
                f"version=0.0.2\ntarget={target}\ncommit={COMMIT}\n"
                "working_tree=clean\ncompiler=rustc fixture\n"
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


if __name__ == "__main__":
    unittest.main()
