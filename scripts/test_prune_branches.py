"""Test the conservative pruning decisions without network access."""

import importlib.util
import pathlib
import unittest
from unittest.mock import patch


MODULE = pathlib.Path(__file__).with_name("prune-merged-branches.py")
spec = importlib.util.spec_from_file_location("prune_branches", MODULE)
pruner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pruner)
SHA = "a" * 40
MAIN = "b" * 40


class BranchPruneSafetyTests(unittest.TestCase):
    def test_eligibility_rejects_protected_active_foreign_and_invalid_refs(self):
        sample = dict(name="scarp/completed-feature", protected=False, commit=dict(sha=SHA))
        self.assertTrue(pruner.eligible(sample, set()))
        self.assertFalse(pruner.eligible(sample, {"scarp/completed-feature"}))
        self.assertFalse(pruner.eligible({**sample, "protected": True}, set()))
        self.assertFalse(pruner.eligible({**sample, "name": "development"}, set()))
        self.assertFalse(pruner.eligible({**sample, "name": "release/v0.0.4-alpha.3"}, set()))
        self.assertFalse(pruner.eligible({**sample, "commit": dict(sha="bad")}, set()))

    def test_inspection_preserves_diverged_and_active_pr_heads(self):
        branches = [
            dict(name="main", protected=True, commit=dict(sha=MAIN)),
            dict(name="development", protected=False, commit=dict(sha=SHA)),
            dict(name="scarp/merged", protected=False, commit=dict(sha=SHA)),
            dict(name="scarp/diverged", protected=False, commit=dict(sha="c" * 40)),
            dict(name="scarp/active", protected=False, commit=dict(sha="d" * 40)),
            dict(name="scarp/protected", protected=True, commit=dict(sha="e" * 40)),
        ]
        def fake_api(path, method="GET"):
            if path == "repos/example/scarp":
                return {"default_branch": "main"}
            if path == "repos/example/scarp/branches/main":
                return {"commit": {"sha": MAIN}}
            if "compare/" in path:
                return {"status": "ahead", "behind_by": 1 if "c" * 40 in path else 0}
            raise AssertionError(path)
        with patch.object(pruner, "api", side_effect=fake_api), \
             patch.object(pruner, "collection", return_value=branches), \
             patch.object(pruner, "protected_branch_names", return_value={"scarp/active"}):
            ready, retained = pruner.inspect("example/scarp")
        self.assertEqual(ready, [("scarp/merged", SHA)])
        self.assertEqual(len(retained), 5)

    def test_dry_run_is_read_only(self):
        with patch.object(pruner, "api") as remote:
            pruner.prune("example/scarp", [("scarp/merged", SHA)], apply=False)
        remote.assert_not_called()

    def test_apply_rechecks_updated_sha_before_deleting(self):
        with patch.object(pruner, "api", return_value={"object": {"sha": MAIN}}) as remote:
            pruner.prune("example/scarp", [("scarp/merged", SHA)], apply=True)
        self.assertEqual(remote.call_count, 1)

    def test_apply_rechecks_newly_opened_pr_and_divergence(self):
        def fake_api(path, method="GET"):
            if "git/ref/heads/" in path:
                return {"object": {"sha": SHA}}
            if path == "repos/example/scarp":
                return {"default_branch": "main"}
            if path.endswith("/branches/main"):
                return {"commit": {"sha": MAIN}}
            if "compare/" in path:
                return {"behind_by": 0}
            raise AssertionError(path)
        with patch.object(pruner, "api", side_effect=fake_api) as remote, \
             patch.object(pruner, "protected_branch_names", return_value={"scarp/merged"}):
            pruner.prune("example/scarp", [("scarp/merged", SHA)], apply=True)
        self.assertFalse(any(call.args[1:] == ("DELETE",) for call in remote.call_args_list))


if __name__ == "__main__":
    unittest.main()
