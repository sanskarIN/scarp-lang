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
from datetime import datetime, timedelta, timezone

spec = importlib.util.spec_from_file_location("publish_release", Path(__file__).with_name("publish-release.py"))
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)
COMMIT = "a" * 40


class PublicationGates(unittest.TestCase):
    def test_new_requests_require_the_exact_approved_source(self):
        for value in (None, "main", "v0.0.4-alpha.4", "a" * 39, "A" * 40, 1):
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, "approved full source_commit"):
                publisher.approved_source(dict(source_commit=value))
        self.assertEqual(publisher.approved_source(dict(source_commit=COMMIT)), COMMIT)

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


class PublicationWindow(unittest.TestCase):
    start = datetime(2026, 10, 31, 4, 30, tzinfo=timezone.utc)
    end = datetime(2026, 10, 31, 18, 30, tzinfo=timezone.utc)

    def request(self):
        return dict(version="0.0.4-alpha.2", source_commit=COMMIT, tagger_name="Sanskar", tagger_email="sanskarin@outlook.in",
                    not_before="2026-10-31T04:30:00Z", not_after="2026-10-31T18:30:00Z")

    def test_future_request_exits_before_git_environment_or_api(self):
        with patch.object(Path, "exists", return_value=True), \
             patch.object(Path, "read_text", return_value=json.dumps(self.request())), \
             patch.object(publisher, "utc_now", return_value=self.start - timedelta(days=1)), \
             patch.object(publisher.subprocess, "check_output") as git_call, \
             patch.object(publisher, "api") as api_call, \
             patch.dict(os.environ, {}, clear=True), \
             contextlib.redirect_stdout(io.StringIO()) as output:
            publisher.main()
        self.assertIn("waits until 2026-10-31T04:30:00Z", output.getvalue())
        git_call.assert_not_called()
        api_call.assert_not_called()

    def test_start_is_inclusive_end_exclusive_and_later_years_are_closed(self):
        for now, expected in (
            (self.start - timedelta(seconds=1), False), (self.start, True),
            (self.end - timedelta(seconds=1), True), (self.end, False),
            (self.start.replace(year=2027), False),
        ):
            with self.subTest(now=now), patch.object(publisher, "utc_now", return_value=now), \
                 contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(publisher.publication_window_open(self.request()), expected)
        # Fixed UTC conversion also checks the requested 10 AM India time.
        self.assertEqual(self.start.astimezone(timezone(timedelta(hours=5, minutes=30))).hour, 10)

    def test_malformed_partial_or_reversed_windows_fail_before_git_or_api(self):
        changes = (
            dict(not_before=None), dict(not_after=None), dict(not_after="2026-10-31"),
            dict(not_before="2026-10-31T10:00:00+05:30"), dict(not_before="2026-10-32T04:30:00Z"),
            dict(not_before="2026-10-31T4:30:00Z"), dict(not_after="2026-10-31T04:30:00Z"),
            dict(not_after="2026-10-30T18:30:00Z"),
        )
        for change in changes:
            request = dict(self.request(), **change)
            with self.subTest(change=change), patch.object(Path, "exists", return_value=True), \
                 patch.object(Path, "read_text", return_value=json.dumps(request)), \
                 patch.object(publisher.subprocess, "check_output") as git_call, \
                 patch.object(publisher, "api") as api_call:
                with self.assertRaises(ValueError):
                    publisher.main()
                git_call.assert_not_called()
                api_call.assert_not_called()
        for missing in ("not_before", "not_after"):
            request = self.request()
            del request[missing]
            with self.assertRaises(ValueError):
                publisher.publication_window_open(request)

    def test_legacy_immediate_requests_keep_manual_and_workflow_behavior(self):
        for event in ("workflow_run", "workflow_dispatch", "pull_request"):
            with patch.dict(os.environ, GITHUB_EVENT_NAME=event):
                self.assertTrue(publisher.publication_window_open(dict(version="0.0.3-beta.3")))

    def test_cron_cannot_activate_an_immediate_request_without_dates(self):
        with patch.dict(os.environ, GITHUB_EVENT_NAME="schedule"), \
             patch.object(Path, "exists", return_value=True), \
             patch.object(Path, "read_text", return_value=json.dumps(dict(version="0.0.4-alpha.2"))), \
             patch.object(publisher.subprocess, "check_output") as git_call, \
             patch.object(publisher, "api") as api_call, \
             contextlib.redirect_stdout(io.StringIO()):
            publisher.main()
        git_call.assert_not_called()
        api_call.assert_not_called()

    def test_due_schedule_still_requires_the_exact_checkout(self):
        with patch.object(Path, "exists", return_value=True), \
             patch.object(Path, "read_text", return_value=json.dumps(self.request())), \
             patch.object(publisher, "utc_now", return_value=self.start), \
             patch.object(publisher.subprocess, "check_output", return_value="b" * 40 + "\n"), \
             patch.dict(os.environ, SCARP_SOURCE_SHA=COMMIT), \
             patch.object(publisher, "api") as api_call:
            with self.assertRaisesRegex(ValueError, "checkout does not match"):
                publisher.main()
            api_call.assert_not_called()

    def drive_publication(self, clocks, move_after_draft=False, allow_publish=False, approved_commit=COMMIT):
        """Exercise real stage ordering; external CI/archive behavior has separate tests."""
        state = dict(created=False, mutations=[], gh_calls=[], files={}, public_downloads=[], source_reads=[])
        names = [f"archive-{i}.bin" for i in range(3)]
        manifest = "".join("a" * 64 + "  " + name + "\n" for name in names)
        draft = dict(id=42, tag_name="v0.0.4-alpha.2", draft=True, prerelease=True)

        def fake_git(args, **kwargs):
            if args[1] == "rev-parse":
                return COMMIT + "\n"
            if args[-1].endswith(":Cargo.toml"):
                state["source_reads"].append(args[-1])
                return 'version = "0.0.4-alpha.2"\n'
            if ":docs/releases/" in args[-1]:
                state["source_reads"].append(args[-1])
                return "verified release notes\n"
            raise AssertionError(args)

        def fake_api(path, *, payload=None, optional=False, method="POST"):
            if payload is not None:
                state["mutations"].append((path, method))
            if method == "PATCH":
                if not allow_publish:
                    raise AssertionError("unexpected publication")
                self.assertEqual(payload, dict(draft=False, prerelease=True, make_latest="false"))
                draft.update(draft=False, published_at="2026-10-31T04:31:00Z",
                             html_url="https://example.invalid/releases/v0.0.4-alpha.2")
                return draft
            if path.endswith("/releases?per_page=100"):
                return [draft] if state["created"] else []
            if "/git/ref/tags/" in path:
                return None
            if path.endswith("/branches/main"):
                return dict(commit=dict(sha="b" * 40 if move_after_draft and state["created"] else COMMIT))
            if path.endswith("/git/tags"):
                state["tag_source"] = payload["object"]
                return dict(sha="b" * 40)
            if path.endswith("/git/refs"):
                return {}
            if path.endswith("/releases/42"):
                return draft
            raise AssertionError(path)

        def fake_gh(*args):
            state["gh_calls"].append(args[:2])
            if args[:2] == ("run", "download"):
                directory = Path(args[args.index("--dir") + 1])
                directory.mkdir(parents=True)
                for name in [*names, *(name + ".sha256" for name in names)]:
                    (directory / name).write_bytes(name.encode())
            elif args[:2] == ("release", "create"):
                state["files"] = {Path(path).name: Path(path).read_bytes() for path in args[-7:]}
                draft["assets"] = [dict(id=i, name=name, browser_download_url="https://example.invalid/" + name)
                                   for i, name in enumerate(state["files"])]
                state["created"] = True
            else:
                raise AssertionError(args)

        def download(endpoint, asset, destination):
            destination.write_bytes(state["files"][asset["name"]])

        def public_download(url, **kwargs):
            name = url.rsplit("/", 1)[1]
            state["public_downloads"].append(name)
            return io.BytesIO(state["files"][name])

        selected = {path: dict(id=i + 1) for i, path in enumerate(publisher.WORKFLOWS)}
        with patch.object(Path, "exists", return_value=True), \
             patch.object(Path, "read_text", return_value=json.dumps(dict(self.request(), source_commit=approved_commit))), \
             patch.object(publisher, "utc_now", side_effect=clocks), \
             patch.object(publisher.subprocess, "check_output", side_effect=fake_git), \
             patch.object(publisher.subprocess, "run") as ancestry_check, \
             patch.object(publisher, "api", side_effect=fake_api), \
             patch.object(publisher, "gh", side_effect=fake_gh), \
             patch.object(publisher, "verified_workflows", return_value=selected) as workflows, \
             patch.object(publisher.verifier, "verify", return_value=manifest), \
             patch.object(publisher, "check_release"), \
             patch.object(publisher, "download_draft_asset", side_effect=download), \
             patch.object(publisher.urllib.request, "urlopen", side_effect=public_download), \
             patch.dict(os.environ, dict(SCARP_SOURCE_SHA=COMMIT, GITHUB_REPOSITORY="example/project",
                                        GITHUB_EVENT_NAME="schedule"), clear=True), \
             contextlib.redirect_stdout(io.StringIO()):
            publisher.main()
        ancestry_check.assert_called_once_with(["git", "merge-base", "--is-ancestor", approved_commit, COMMIT], check=True)
        state["workflow_sources"] = [call.args[1] for call in workflows.call_args_list]
        return state

    def test_authorization_can_follow_source_without_changing_tagged_candidate(self):
        approved = "c" * 40
        state = self.drive_publication([self.start, self.start, self.start], allow_publish=True, approved_commit=approved)
        self.assertEqual(state["workflow_sources"], [COMMIT, approved])
        self.assertEqual(state["tag_source"], approved)
        self.assertTrue(all(source.startswith(approved + ":") for source in state["source_reads"]))

    def test_expiry_during_artifact_verification_prevents_tag_creation(self):
        state = self.drive_publication([self.start, self.end])
        self.assertEqual(state["mutations"], [])
        self.assertNotIn(("release", "create"), state["gh_calls"])

    def test_expiry_during_draft_downloads_prevents_publication(self):
        state = self.drive_publication([self.start, self.start, self.end])
        self.assertTrue(state["created"])
        self.assertEqual([path.rsplit("/", 1)[1] for path, _ in state["mutations"]], ["tags", "refs"])
        self.assertFalse(any(method == "PATCH" for _, method in state["mutations"]))

    def test_main_change_or_cancellation_during_draft_checks_prevents_publication(self):
        with self.assertRaisesRegex(ValueError, "main moved during draft verification"):
            self.drive_publication([self.start, self.start, self.start], move_after_draft=True)

    def test_in_window_schedule_reaches_publication_and_verifies_all_downloads(self):
        state = self.drive_publication([self.start, self.start, self.start], allow_publish=True)
        self.assertTrue(state["created"])
        self.assertEqual(len(state["files"]), 7)
        self.assertEqual(set(state["public_downloads"]), set(state["files"]))
        self.assertEqual([method for _, method in state["mutations"]], ["POST", "POST", "PATCH"])


if __name__ == "__main__":
    unittest.main()
