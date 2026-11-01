"""Exercise the release verifier with complete and damaged package fixtures."""

import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile
import hashlib
import subprocess

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

    def build(self, *, omit=None, executable=True, version="0.0.2", working_tree="clean", record_version=None, commit=COMMIT, extra_files=None):
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
            if (version.startswith("0.0.3-") or version in ("0.0.4-dev", "0.0.4-alpha.1", "0.0.4-alpha.2", "0.0.4-alpha.3", "0.0.4-alpha.4")):
                for name in ("loops", "repeat", "control_escapes"):
                    for extension in ("srp", "stdout"):
                        member = f"examples/{name}.{extension}"
                        if member != omit:
                            files[member] = b"fixture"
            if version in ("0.0.3-beta.2", "0.0.3-beta.3", "0.0.4-dev", "0.0.4-alpha.1", "0.0.4-alpha.2", "0.0.4-alpha.3", "0.0.4-alpha.4"):
                for member in (
                    "examples/output.srp", "examples/output.stdout",
                    "docs/output.md", "docs/syntax-migration.md",
                ):
                    if member != omit:
                        files[member] = b"fixture"
            if version in ("0.0.3-beta.3", "0.0.4-dev", "0.0.4-alpha.1", "0.0.4-alpha.2", "0.0.4-alpha.3", "0.0.4-alpha.4"):
                for member in (
                    "docs/functions.md", "examples/functions.srp", "examples/functions.stdout",
                    "examples/function_contracts.srp", "examples/function_contracts.stdout",
                    "examples/function_recursion.srp", "examples/function_recursion.stdout",
                    "docs/budgets.md", "docs/reference/cli.md", "docs/embedding.md",
                ):
                    if member != omit:
                        files[member] = b"fixture"
            if version in ("0.0.4-alpha.1", "0.0.4-alpha.2", "0.0.4-alpha.3", "0.0.4-alpha.4"):
                for member in (
                    "docs/floats.md", "examples/float_refinement.srp", "examples/float_refinement.stdout",
                    "examples/float_smoothing.srp", "examples/float_smoothing.stdout",
                ):
                    if member != omit:
                        files[member] = b"fixture"
            if version in ("0.0.4-alpha.2", "0.0.4-alpha.3", "0.0.4-alpha.4"):
                for member in (
                    "docs/vectors.md", "examples/vectors.srp", "examples/vectors.stdout",
                    "examples/vector_probabilities.srp", "examples/vector_probabilities.stdout",
                    "examples/coefficient_fit.srp", "examples/coefficient_fit.stdout",
                ):
                    if member != omit:
                        files[member] = b"fixture"
            if version in ("0.0.4-alpha.3", "0.0.4-alpha.4"):
                for name in ("scaled_norm", "terminal_recursion", "vector_allocation"):
                    for extension in ("srp", "stdout"):
                        member = f"examples/{name}.{extension}"
                        if member != omit:
                            files[member] = b"fixture"
            if version == "0.0.4-alpha.4":
                for member in ("docs/tiles.md", "docs/memory-lifetimes.md", "docs/architecture.md", "docs/testing.md"):
                    if member != omit:
                        files[member] = b"fixture"
                for name in ("matrix_inference", "linear_transform", "matrix_classification", "matrix_probabilities",
                             "matrix_targets", "matrix_forward", "tile_windows", "vector_windows",
                             "classification_training", "logit_loss", "log_space", "score_selection"):
                    for extension in ("srp", "stdout"):
                        member = f"examples/{name}.{extension}"
                        if member != omit:
                            files[member] = b"fixture"
            files.update(extra_files or {})
            files["BUILD-INFO.txt"] = (
                f"version={record_version or version}\ntarget={target}\ncommit={commit}\n"
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

    def test_alpha4_requires_new_and_inherited_package_content(self):
        self.build(version="0.0.4-alpha.4")
        self.assertEqual(len(verifier.verify(self.directory, "0.0.4-alpha.4", COMMIT).splitlines()), 3)
        for member in (
            "docs/tiles.md", "docs/memory-lifetimes.md", "docs/architecture.md", "docs/testing.md",
            "docs/vectors.md", "docs/floats.md", "docs/functions.md", "docs/budgets.md",
            "docs/releases/v0.0.4-alpha.4.md", "examples/repeat.srp", "examples/vector_allocation.stdout",
            "examples/matrix_inference.srp", "examples/matrix_inference.stdout",
            "examples/linear_transform.srp", "examples/linear_transform.stdout",
            "examples/matrix_classification.srp", "examples/matrix_classification.stdout",
            "examples/matrix_probabilities.srp", "examples/matrix_probabilities.stdout",
            "examples/matrix_targets.srp", "examples/matrix_targets.stdout",
            "examples/matrix_forward.srp", "examples/matrix_forward.stdout",
            "examples/tile_windows.srp", "examples/tile_windows.stdout",
            "examples/vector_windows.stdout", "examples/classification_training.srp",
        ):
            with self.subTest(member=member):
                self.build(version="0.0.4-alpha.4", omit=member)
                with self.assertRaisesRegex(ValueError, "missing " + member):
                    verifier.verify(self.directory, "0.0.4-alpha.4", COMMIT)

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

    def test_syntax_release_requires_migration_guides(self):
        for member in ("docs/output.md", "docs/syntax-migration.md"):
            with self.subTest(member=member):
                self.build(version="0.0.3-beta.2", omit=member)
                with self.assertRaisesRegex(ValueError, "missing " + member):
                    verifier.verify(self.directory, "0.0.3-beta.2", COMMIT)

    def test_syntax_release_requires_raw_output_example_and_snapshot(self):
        for member in ("examples/output.srp", "examples/output.stdout"):
            with self.subTest(member=member):
                self.build(version="0.0.3-beta.2", omit=member)
                with self.assertRaisesRegex(ValueError, "missing " + member):
                    verifier.verify(self.directory, "0.0.3-beta.2", COMMIT)

    def test_complete_syntax_release_keeps_sorted_checksums(self):
        self.build(version="0.0.3-beta.2")
        entries = verifier.verify(self.directory, "0.0.3-beta.2", COMMIT).splitlines()
        self.assertEqual(len(entries), 3)
        self.assertEqual(entries, sorted(entries))

    def test_function_candidate_requires_its_guide_and_all_example_pairs(self):
        for member in (
            "docs/functions.md", "examples/functions.srp", "examples/functions.stdout",
            "examples/function_contracts.srp", "examples/function_contracts.stdout",
            "examples/function_recursion.srp", "examples/function_recursion.stdout",
            "examples/repeat.srp", "docs/syntax-migration.md",
            "docs/budgets.md", "docs/reference/cli.md", "docs/embedding.md",
        ):
            with self.subTest(member=member):
                for version in ("0.0.3-beta.3", "0.0.4-dev"):
                    self.build(version=version, omit=member)
                    with self.assertRaisesRegex(ValueError, "missing " + member):
                        verifier.verify(self.directory, version, COMMIT)

    def test_complete_function_candidate_has_three_sorted_checksums(self):
        for version in ("0.0.3-beta.3", "0.0.4-dev"):
            self.build(version=version)
            entries = verifier.verify(self.directory, version, COMMIT).splitlines()
            self.assertEqual(len(entries), 3)
            self.assertEqual(entries, sorted(entries))

    def source_fixture(self):
        source = self.directory / "source"
        source.mkdir()
        files = {
            "README.md": b"fixture", "LICENSE": b"fixture",
            "docs/reference/cli.md": "exact tagged guide\nनमस्ते\n".encode(),
            "docs/extra.md": b"also required from the exact tree\n",
        }
        for name, data in files.items():
            path = source / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        subprocess.run(["git", "init", "--quiet", str(source)], check=True)
        subprocess.run(["git", "-C", str(source), "add", "."], check=True)
        subprocess.run([
            "git", "-C", str(source), "-c", "user.name=Release Fixture",
            "-c", "user.email=fixture@example.invalid", "commit", "--quiet", "-m", "fixture",
        ], check=True)
        commit = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
        return source, commit, files

    def test_vector_candidate_requires_new_and_inherited_guides_and_fixtures(self):
        for member in (
            "docs/vectors.md", "examples/vectors.srp", "examples/vectors.stdout",
            "examples/vector_probabilities.srp", "examples/vector_probabilities.stdout",
            "examples/coefficient_fit.srp", "examples/coefficient_fit.stdout",
            "docs/floats.md", "examples/float_refinement.srp", "docs/functions.md",
            "docs/budgets.md", "docs/embedding.md", "docs/syntax-migration.md",
            "examples/repeat.srp", "examples/output.stdout", "docs/releases/v0.0.4-alpha.2.md",
        ):
            with self.subTest(member=member):
                self.build(version="0.0.4-alpha.2", omit=member)
                with self.assertRaisesRegex(ValueError, "missing " + member):
                    verifier.verify(self.directory, "0.0.4-alpha.2", COMMIT)

    def test_complete_vector_candidate_and_stale_version_record(self):
        self.build(version="0.0.4-alpha.2")
        manifest = verifier.verify(self.directory, "0.0.4-alpha.2", COMMIT)
        self.assertEqual(len(manifest.splitlines()), 3)
        self.assertEqual(manifest.splitlines(), sorted(manifest.splitlines()))
        self.build(version="0.0.4-alpha.2", record_version="0.0.4-alpha.1")
        with self.assertRaisesRegex(ValueError, "unexpected version"):
            verifier.verify(self.directory, "0.0.4-alpha.2", COMMIT)

    def test_float_alpha_requires_numeric_and_inherited_function_contents(self):
        for member in (
            "docs/floats.md", "examples/float_refinement.srp", "examples/float_refinement.stdout",
            "examples/float_smoothing.srp", "examples/float_smoothing.stdout",
            "docs/functions.md", "docs/budgets.md", "docs/reference/cli.md", "docs/embedding.md",
            "docs/syntax-migration.md", "examples/repeat.srp", "examples/output.stdout",
            "examples/function_contracts.srp", "docs/releases/v0.0.4-alpha.1.md",
        ):
            with self.subTest(member=member):
                self.build(version="0.0.4-alpha.1", omit=member)
                with self.assertRaisesRegex(ValueError, "missing " + member):
                    verifier.verify(self.directory, "0.0.4-alpha.1", COMMIT)

    def test_complete_float_alpha_matches_exact_source_and_sorted_checksums(self):
        source, commit, files = self.source_fixture()
        files["docs/floats.md"] = b"finite binary64 guide\n"
        path = source / "docs/floats.md"
        path.write_bytes(files["docs/floats.md"])
        subprocess.run(["git", "-C", str(source), "add", "docs/floats.md"], check=True)
        subprocess.run([
            "git", "-C", str(source), "-c", "user.name=Release Fixture",
            "-c", "user.email=fixture@example.invalid", "commit", "--quiet", "-m", "float guide",
        ], check=True)
        commit = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
        self.build(version="0.0.4-alpha.1", commit=commit, extra_files=files)
        entries = verifier.verify(self.directory, "0.0.4-alpha.1", commit, source).splitlines()
        self.assertEqual(len(entries), 3)
        self.assertEqual(entries, sorted(entries))
        files["docs/floats.md"] = b"stale float guide with a valid archive checksum"
        self.build(version="0.0.4-alpha.1", commit=commit, extra_files=files)
        with self.assertRaisesRegex(ValueError, "source differs.*docs/floats.md"):
            verifier.verify(self.directory, "0.0.4-alpha.1", commit, source)

    def test_packages_match_exact_git_source_in_all_three_archives(self):
        source, commit, files = self.source_fixture()
        self.build(version="0.0.3-beta.3", commit=commit, extra_files=files)
        # A modified checkout must not replace the immutable tagged content.
        (source / "README.md").write_text("uncommitted change")
        manifest = verifier.verify(self.directory, "0.0.3-beta.3", commit, source)
        self.assertEqual(len(manifest.splitlines()), 3)

    def test_valid_hashes_do_not_hide_stale_packaged_documentation(self):
        source, commit, files = self.source_fixture()
        files["docs/reference/cli.md"] = b"stale guide with valid archive checksum"
        self.build(version="0.0.3-beta.3", commit=commit, extra_files=files)
        with self.assertRaisesRegex(ValueError, "source differs.*docs/reference/cli.md"):
            verifier.verify(self.directory, "0.0.3-beta.3", commit, source)

    def test_every_tagged_guide_is_required_even_outside_the_minimum_file_list(self):
        source, commit, files = self.source_fixture()
        del files["docs/extra.md"]
        self.build(version="0.0.3-beta.3", commit=commit, extra_files=files)
        with self.assertRaises(KeyError):
            verifier.verify(self.directory, "0.0.3-beta.3", commit, source)


if __name__ == "__main__":
    unittest.main()
