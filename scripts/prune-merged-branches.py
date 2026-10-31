#!/usr/bin/env python3
"""Delete only fully merged, inactive Scarp branches after revalidating heads.

Default mode is read-only. Apply mode is for the protected-main GitHub Action.
No unmerged, protected, active-PR, release, development or foreign-named branch
is eligible; this script never rewrites Git history or forces ref updates.
"""

import argparse
import json
import os
import subprocess
import sys
from urllib.parse import quote


def api(path, method="GET"):
    command = ["gh", "api", "-X", method, path]
    result = subprocess.run(command, text=True, capture_output=True, check=True)
    if method == "DELETE":
        return None
    return json.loads(result.stdout)


def collection(repository, endpoint):
    page = 1
    found = []
    while True:
        chunk = api(f"repos/{repository}/{endpoint}{'&' if '?' in endpoint else '?'}per_page=100&page={page}")
        if not isinstance(chunk, list):
            raise RuntimeError("GitHub pagination returned an unexpected object")
        found.extend(chunk)
        if len(chunk) < 100:
            return found
        page += 1


def protected_branch_names(repository):
    prs = collection(repository, "pulls?state=open")
    reserved = set()
    for pr in prs:
        base = pr.get("base", {})
        head = pr.get("head", {})
        for endpoint in (base, head):
            if endpoint.get("repo") and endpoint["repo"].get("full_name") == repository:
                reserved.add(endpoint.get("ref"))
    return reserved


def eligible(branch, protected_refs):
    name = branch.get("name", "")
    return (
        name.startswith("scarp/")
        and not branch.get("protected", True)
        and name not in protected_refs
        and isinstance(branch.get("commit", {}).get("sha"), str)
        and len(branch["commit"]["sha"]) == 40
    )


def inspect(repository):
    descriptor = api(f"repos/{repository}")
    default = descriptor["default_branch"]
    main = api(f"repos/{repository}/branches/{quote(default, safe='')}")
    main_sha = main["commit"]["sha"]
    branches = collection(repository, "branches")
    active_pr_names = protected_branch_names(repository)
    preserved = set(active_pr_names) | {default, "development"}
    ready = []
    retained = []
    for branch in branches:
        name = branch["name"]
        if not eligible(branch, preserved):
            retained.append((name, "protected, active PR, or non-maintenance branch"))
            continue
        commit = branch["commit"]["sha"]
        comparison = api(f"repos/{repository}/compare/{commit}...{main_sha}")
        if comparison.get("behind_by") != 0 or comparison.get("status") not in ("ahead", "identical"):
            retained.append((name, "contains commits not in main"))
            continue
        ready.append((name, commit))
    return ready, retained


def prune(repository, candidates, apply):
    for name, sha in candidates:
        if not apply:
            print(f"DRY-RUN: safely merged branch: {name} at {sha}")
            continue
        # A fresh lease-like GET prevents deletion if anyone updated the
        # branch while the ancestry audit was running.
        current = api(f"repos/{repository}/git/ref/heads/{quote(name, safe='/')}")
        if current.get("object", {}).get("sha") != sha:
            print(f"SKIPPED moved branch {name}", file=sys.stderr)
            continue
        # Verify there are no new PR references and no new divergence.
        protected = protected_branch_names(repository)
        default = api(f"repos/{repository}")["default_branch"]
        fresh = api(f"repos/{repository}/branches/{quote(default, safe='')}")
        comparison = api(f"repos/{repository}/compare/{sha}...{fresh['commit']['sha']}")
        if name in protected or comparison.get("behind_by") != 0:
            print(f"SKIPPED newly protected or unmerged branch {name}", file=sys.stderr)
            continue
        api(f"repos/{repository}/git/refs/heads/{quote(name, safe='/')}", "DELETE")
        print(f"PRUNED safely merged branch {name} at {sha}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apply", action="store_true", help="Delete eligible branches; default prints a dry-run")
    arguments = parser.parse_args()
    repository = os.environ.get("GITHUB_REPOSITORY", "")
    if "/" not in repository or not all(repository.split("/", 1)):
        parser.error("GITHUB_REPOSITORY must be an owner/repository identifier")
    if arguments.apply and os.environ.get("GITHUB_REF") != "refs/heads/main":
        parser.error("apply mode requires the protected main branch workflow")
    ready, retained = inspect(repository)
    print(f"Audited {len(ready) + len(retained)} branches: {len(ready)} safely merged; {len(retained)} retained")
    for name, reason in retained:
        print(f"RETAINED {name}: {reason}")
    prune(repository, ready, arguments.apply)


if __name__ == "__main__":
    main()
