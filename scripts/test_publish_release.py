"""Keep publication gates closed for incomplete CI and inconsistent assets."""

import copy
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("publish_release", Path(__file__).with_name("publish-release.py"))
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)
COMMIT = "a" * 40


class PublicationGates(unittest.TestCase):
    def runs(self):
        return [dict(id=index, path=path, head_sha=COMMIT, event="push", head_branch="main",
                     status="completed", conclusion="success")
                for index, path in enumerate(publisher.WORKFLOWS, 1)]

    def test_both_exact_main_workflows_are_required(self):
        runs = self.runs()
        self.assertEqual(set(publisher.selected_runs(runs, COMMIT)), set(publisher.WORKFLOWS))
        self.assertIsNone(publisher.selected_runs(runs[:1], COMMIT))
        self.assertIsNone(publisher.selected_runs([], COMMIT))

    def test_wrong_sha_branch_or_event_cannot_supply_release_evidence(self):
        for field, value in (("head_sha", "b" * 40), ("head_branch", "feature"), ("event", "pull_request")):
            with self.subTest(field=field):
                runs = self.runs()
                runs[0][field] = value
                self.assertIsNone(publisher.selected_runs(runs, COMMIT))

    def test_newer_failed_pending_or_cancelled_run_blocks_older_success(self):
        for status, conclusion in (("completed", "failure"), ("in_progress", None), ("completed", "cancelled")):
            with self.subTest(status=status, conclusion=conclusion):
                runs = self.runs()
                newer = dict(runs[0], id=10, status=status, conclusion=conclusion)
                self.assertIsNone(publisher.selected_runs([newer, *runs], COMMIT))

    def test_every_required_native_and_rust_job_must_succeed(self):
        for path, names in publisher.WORKFLOWS.items():
            jobs = [dict(name=name, status="completed", conclusion="success") for name in names]
            publisher.validate_jobs(path, jobs)
            for changed in (jobs[:-1], jobs + [jobs[0]], jobs + [dict(jobs[0], name="unexpected")]):
                with self.assertRaises(ValueError):
                    publisher.validate_jobs(path, changed)
            for status in ("failure", "cancelled", "skipped"):
                changed = copy.deepcopy(jobs)
                changed[0]["conclusion"] = status
                with self.assertRaises(ValueError):
                    publisher.validate_jobs(path, changed)

    def release_fixture(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        path = Path(temporary.name) / "asset.zip"
        path.write_bytes(b"verified bytes")
        release = dict(draft=True, prerelease=True, assets=[dict(
            name=path.name, state="uploaded", size=path.stat().st_size,
            digest="sha256:" + publisher.digest(path),
        )])
        return release, {path.name: path}

    def test_draft_and_public_asset_records_match_verified_bytes(self):
        release, files = self.release_fixture()
        publisher.check_release(release, files, draft=True)
        release["draft"] = False
        publisher.check_release(release, files, draft=False)

    def test_wrong_state_or_stable_release_cannot_pass_prerelease_gate(self):
        release, files = self.release_fixture()
        for field in ("draft", "prerelease"):
            changed = copy.deepcopy(release)
            changed[field] = False
            with self.assertRaises(ValueError):
                publisher.check_release(changed, files, draft=True)

    def test_missing_duplicate_or_unexpected_assets_are_rejected(self):
        release, files = self.release_fixture()
        for assets in ([], release["assets"] * 2, [dict(release["assets"][0], name="other.zip")]):
            with self.assertRaises(ValueError):
                publisher.check_release(dict(release, assets=assets), files, draft=True)

    def test_wrong_size_hash_or_incomplete_upload_is_rejected(self):
        release, files = self.release_fixture()
        for field, value in (("size", 0), ("digest", "sha256:" + "0" * 64), ("state", "new")):
            changed = copy.deepcopy(release)
            changed["assets"][0][field] = value
            with self.assertRaises(ValueError):
                publisher.check_release(changed, files, draft=True)


if __name__ == "__main__":
    unittest.main()
