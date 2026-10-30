"""Keep publication gates closed for incomplete CI and inconsistent assets."""

import copy
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("publish_release", Path(__file__).with_name("publish-release.py"))
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)
COMMIT = "a" * 40


class PublicationGates(unittest.TestCase):
    def test_absent_request_never_reads_identity_runs_git_or_contacts_github(self):
        with patch.object(Path, "exists", return_value=False), \
             patch.object(Path, "read_text") as read_request, \
             patch.object(publisher.subprocess, "check_output") as git_call, \
             patch.object(publisher, "api") as api_call, \
             contextlib.redirect_stdout(io.StringIO()) as output:
            publisher.main()
        self.assertIn("nothing to publish", output.getvalue())
        read_request.assert_not_called()
        git_call.assert_not_called()
        api_call.assert_not_called()

    def test_unrequested_version_forms_fail_before_git_or_network(self):
        for version in ("0.0.4", "0.0.4-dev", "v0.0.4-alpha.1", "0.0.4-alpha", "0.0.4-alpha.1.dev", "0.0.4-rc.1"):
            with self.subTest(version=version), \
                 patch.object(Path, "exists", return_value=True), \
                 patch.object(Path, "read_text", return_value=json.dumps(dict(version=version))), \
                 patch.object(publisher.subprocess, "check_output") as git_call, \
                 patch.object(publisher, "api") as api_call:
                with self.assertRaisesRegex(ValueError, "explicitly requested alpha or beta"):
                    publisher.main()
                git_call.assert_not_called()
                api_call.assert_not_called()

    def test_alpha_request_cannot_skip_exact_checkout_gate(self):
        with patch.object(Path, "exists", return_value=True), \
             patch.object(Path, "read_text", return_value=json.dumps(dict(version="0.0.4-alpha.1"))), \
             patch.object(publisher.subprocess, "check_output", return_value="b" * 40 + "\n"), \
             patch.dict(os.environ, SCARP_SOURCE_SHA=COMMIT), \
             patch.object(publisher, "api") as api_call:
            with self.assertRaisesRegex(ValueError, "checkout does not match"):
                publisher.main()
            api_call.assert_not_called()

    def recovery_fixture(self):
        request = dict(version="0.0.3-beta.3", resume_commit=COMMIT, resume_release_id=42,
                       tagger_name="Maintainer", tagger_email="maintainer@example.invalid")
        release = dict(id=42, tag_name="v0.0.3-beta.3", draft=True, prerelease=True)
        tag = dict(tag="v0.0.3-beta.3", object=dict(type="commit", sha=COMMIT),
                   tagger=dict(name=request["tagger_name"], email=request["tagger_email"]))
        return request, release, tag

    def test_authenticated_collection_resolves_drafts_and_rejects_ambiguity(self):
        _, draft, _ = self.recovery_fixture()
        self.assertEqual(publisher.release_from_list([draft], draft["tag_name"]), draft)
        self.assertIsNone(publisher.release_from_list([draft], "v0.0.2"))
        with self.assertRaises(ValueError):
            publisher.release_from_list([draft, draft], draft["tag_name"])

    def test_main_uses_collection_for_release_lookup_instead_of_public_tag_endpoint(self):
        request, release, _ = self.recovery_fixture()
        release.update(draft=False, html_url="https://github.com/example/project/releases/tag/v0.0.3-beta.3")
        with patch.object(Path, "exists", return_value=True), \
             patch.object(Path, "read_text", return_value=json.dumps(request)), \
             patch.object(publisher.subprocess, "check_output", return_value=COMMIT + "\n"), \
             patch.dict(os.environ, SCARP_SOURCE_SHA=COMMIT, GITHUB_REPOSITORY="example/project"), \
             patch.object(publisher, "api", return_value=[release]) as api_call, \
             contextlib.redirect_stdout(io.StringIO()):
            publisher.main()
        api_call.assert_called_once_with("repos/example/project/releases?per_page=100")

    def test_explicit_draft_recovery_retains_exact_tagged_source_and_identity(self):
        request, release, tag = self.recovery_fixture()
        self.assertEqual(publisher.validate_resume(request, release, tag), COMMIT)

    def test_recovery_requires_exact_draft_id_and_full_commit(self):
        request, release, tag = self.recovery_fixture()
        for field, value in (("resume_release_id", 43), ("resume_release_id", "42"),
                             ("resume_commit", "main"), ("resume_commit", "a" * 39)):
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                publisher.validate_resume(dict(request, **{field: value}), release, tag)

    def test_recovery_rejects_changed_tag_target_type_or_maintainer(self):
        request, release, tag = self.recovery_fixture()
        for changed in (
            dict(tag, object=dict(type="commit", sha="b" * 40)),
            dict(tag, object=dict(type="tag", sha=COMMIT)),
            dict(tag, tag="v0.0.3-beta.2"),
            dict(tag, tagger=dict(name="Other", email=request["tagger_email"])),
            dict(tag, tagger=dict(name=request["tagger_name"], email="other@example.invalid")),
        ):
            with self.assertRaises(ValueError):
                publisher.validate_resume(request, release, changed)

    def test_recovery_rejects_published_stable_or_mismatched_draft(self):
        request, release, tag = self.recovery_fixture()
        for changed in (dict(release, draft=False), dict(release, prerelease=False),
                        dict(release, tag_name="v0.0.3-beta.2")):
            with self.assertRaises(ValueError):
                publisher.validate_resume(request, changed, tag)

    def test_publication_patch_uses_release_id_and_explicit_patch_method(self):
        payload = dict(draft=False, prerelease=True, make_latest="false")
        with patch.object(publisher, "gh", return_value="{}") as gh_call:
            publisher.api("repos/example/project/releases/42", payload=payload, method="PATCH")
        gh_call.assert_called_once_with("api", "repos/example/project/releases/42", "--method", "PATCH",
                                        "--input", "-", payload=payload, optional=False)

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
