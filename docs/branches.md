# Branch strategy and safe cleanup

Scarp uses a small professional branch model, rather than one permanent
branch for every milestone.

| Role | Name/pattern | Lifetime |
| --- | --- | --- |
| Protected release source | `main` | Permanent |
| Experimental integration | `development` | Permanent until expressly retired |
| Release integration | `release/vX.Y.Z-pre.N` | Temporary, until shipped/archived |
| Scoped features and repairs | `feature/*`, `fix/*`, `scarp/*` | Temporary PR branches |
| Urgent verified repairs | `hotfix/*` | Temporary PR branches |

**Operational target:** around ten or fewer actively used branches. Do not
create placeholder branches merely to reach exactly ten. Historical closed
work belongs in immutable Git history, tags and merged PRs, not 50+ live
branch pointers. The current long-lived `development` branch contains
historical diverged experiments; do not force-reset it onto `main`.

## Automatic safe pruning

On a protected `main` push, `prune-merged-branches.yml` runs unit tests
and calls `scripts/prune-merged-branches.py --apply`, with only contents
write and PR read permissions.

Deletion is **only** allowed for eligible `scarp/*` refs with all checks:

1. GitHub marks the branch unprotected.
2. The name is neither `main` nor `development`.
3. No currently open PR in this repo uses the branch as its head or base.
4. The branch commit is an ancestor of the latest `main` revision.
5. The branch still points to the originally audited SHA just before deletion.
6. The main ancestry and open-PR protections are rechecked immediately
   before GitHub receives the DELETE request.

Unmerged or diverged branches, active PR heads and release branches are
intentionally retained until separately reviewed. The script defaults to
read-only dry-run when invoked locally:

```sh
GITHUB_REPOSITORY=scarp-lang/scarp python3 scripts/prune-merged-branches.py
python3 -m unittest discover -s scripts -p 'test_prune_branches.py'
```

The first command requires a GitHub CLI authenticated for read access.
`--apply` requires an authenticated `main` CI context. Missing access
and failed API calls must fail visibly; they must never be interpreted as
permission to delete extra branches.

## Deferred archive decisions

The old stacked experimental PRs built on `scarp/*` bases may contain
unmerged compiler/backend work. Although their names look historical,
those heads are **not safe to delete** until the code and PR chains are
reviewed, the useful code is merged or preserved in traceable archive
tags, and affected PRs are closed or retargeted. Never reduce branch count
by discarding unique work. Review the surviving branches after safe pruning
and archive them case by case.
