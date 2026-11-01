"""Publish one explicitly requested prerelease after exact-main CI and asset gates."""

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import urllib.request
from datetime import datetime, timezone

spec = importlib.util.spec_from_file_location("verify_release", Path(__file__).with_name("verify-release.py"))
verifier = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verifier)

WORKFLOWS = {
    ".github/workflows/ci.yml": {
        "Minimum Rust / 1.74.0", "Stable / ubuntu-latest",
        "Stable / windows-latest", "Stable / macos-15",
    },
    ".github/workflows/packages.yml": {
        "Package / x86_64-pc-windows-msvc", "Package / x86_64-unknown-linux-gnu",
        "Package / aarch64-apple-darwin",
    },
}


def gh(*args, payload=None, optional=False):
    result = subprocess.run(
        ["gh", *args], input=json.dumps(payload) if payload is not None else None,
        text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    if result.returncode:
        if optional and "HTTP 404" in result.stderr:
            return None
        raise RuntimeError(result.stderr.strip() or "GitHub command failed")
    return result.stdout


def api(path, *, payload=None, optional=False, method="POST"):
    args = ["api", path]
    if payload is not None:
        args += ["--method", method, "--input", "-"]
    result = gh(*args, payload=payload, optional=optional)
    return json.loads(result) if result is not None else None


def selected_runs(runs, commit):
    selected = {}
    for path in WORKFLOWS:
        matches = [run for run in runs if run.get("path") == path
                   and run.get("head_sha") == commit and run.get("event") == "push"
                   and run.get("head_branch") == "main"]
        if not matches:
            return None
        run = max(matches, key=lambda item: item["id"])
        if run.get("status") != "completed" or run.get("conclusion") != "success":
            return None
        selected[path] = run
    return selected


def validate_jobs(path, jobs):
    if {job["name"] for job in jobs} != WORKFLOWS[path] or len(jobs) != len(WORKFLOWS[path]):
        raise ValueError(f"unexpected job matrix for {path}")
    if any(job.get("status") != "completed" or job.get("conclusion") != "success" for job in jobs):
        raise ValueError(f"not all required jobs succeeded for {path}")


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def check_release(release, files, *, draft):
    if release["draft"] != draft or not release["prerelease"]:
        raise ValueError("incorrect draft/prerelease state")
    assets = release["assets"]
    if len(assets) != len(files) or {asset["name"] for asset in assets} != set(files):
        raise ValueError("release asset names/count differ from verified packages")
    for asset in assets:
        path = files[asset["name"]]
        if (asset["state"] != "uploaded" or asset["size"] != path.stat().st_size
                or asset.get("digest") != "sha256:" + digest(path)):
            raise ValueError(f"uploaded asset differs: {asset['name']}")


def release_from_list(releases, tag):
    matches = [release for release in releases if release.get("tag_name") == tag]
    if len(matches) > 1:
        raise ValueError("ambiguous release tag in authenticated collection")
    return matches[0] if matches else None


def validate_resume(request, release, annotated_tag):
    commit = request.get("resume_commit", "")
    release_id = request.get("resume_release_id")
    if (not isinstance(commit, str) or not re.fullmatch(r"[0-9a-f]{40}", commit)
            or type(release_id) is not int or release_id != release["id"]):
        raise ValueError("draft recovery requires its exact release ID and source commit")
    tag = "v" + request["version"]
    if (release["tag_name"] != tag or not release["draft"] or not release["prerelease"]
            or annotated_tag["tag"] != tag or annotated_tag["object"]["type"] != "commit"
            or annotated_tag["object"]["sha"] != commit
            or annotated_tag["tagger"]["name"] != request["tagger_name"]
            or annotated_tag["tagger"]["email"] != request["tagger_email"]):
        raise ValueError("draft, annotated tag, source, or maintainer identity differs from recovery request")
    return commit


def approved_source(request):
    """New publication requests must identify the reviewed source, not a moving ref."""
    commit = request.get("source_commit")
    if not isinstance(commit, str) or not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("publication requires an explicitly approved full source_commit SHA")
    return commit


def verified_workflows(endpoint, commit):
    runs = api(f"{endpoint}/actions/runs?head_sha={commit}&event=push&per_page=100")["workflow_runs"]
    selected = selected_runs(runs, commit)
    if selected is None:
        return None
    for path, run in selected.items():
        jobs = api(f"{endpoint}/actions/runs/{run['id']}/jobs?per_page=100")["jobs"]
        validate_jobs(path, jobs)
        print(f"Verified required jobs: {run['html_url']}")
    return selected


def download_draft_asset(endpoint, asset, destination):
    with destination.open("wb") as output:
        result = subprocess.run(
            ["gh", "api", f"{endpoint}/releases/assets/{asset['id']}",
             "--header", "Accept: application/octet-stream"],
            stdout=output, stderr=subprocess.PIPE,
        )
    if result.returncode:
        raise RuntimeError(result.stderr.decode("utf-8", errors="replace"))


def utc_now():
    return datetime.now(timezone.utc)


def publication_window_open(request):
    """A scheduled request has an inclusive UTC start and exclusive UTC end."""
    fields = ("not_before", "not_after")
    if not any(field in request for field in fields):
        if os.environ.get("GITHUB_EVENT_NAME") == "schedule":
            print("Scheduled runs require an explicit publication window; nothing to publish.")
            return False
        return True
    dates = []
    for field in fields:
        value = request.get(field)
        if not isinstance(value, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", value):
            raise ValueError("scheduled requests require not_before and not_after as UTC timestamps ending in Z")
        dates.append(datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc))
    start, end = dates
    if start >= end:
        raise ValueError("publication window must end after it starts")
    now = utc_now()
    if now < start:
        print(f"Scheduled publication waits until {request['not_before']}; nothing to publish now.")
        return False
    if now >= end:
        print(f"Publication window ended at {request['not_after']}; no release action.")
        return False
    return True


def main():
    request_path = Path(".github/release-request.json")
    if not request_path.exists():
        print("No prerelease request; nothing to publish.")
        return
    request = json.loads(request_path.read_text(encoding="utf-8"))
    version = request["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+-(?:alpha|beta)\.\d+", version):
        raise ValueError("only an explicitly requested alpha or beta version may be published")
    if not publication_window_open(request):
        return
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    if head != os.environ["SCARP_SOURCE_SHA"]:
        raise ValueError("checkout does not match the triggering source commit")
    repository = os.environ["GITHUB_REPOSITORY"]
    endpoint = f"repos/{repository}"
    tag = "v" + version
    # Drafts are visible in the authenticated collection, not the public tag lookup.
    existing = release_from_list(api(f"{endpoint}/releases?per_page=100"), tag)
    if existing is not None and not existing["draft"]:
        if not existing["prerelease"]:
            raise ValueError("existing stable release needs maintainer inspection")
        print(f"Already published: {existing['html_url']}; leaving it unchanged.")
        return
    commit = head
    tag_ref = api(f"{endpoint}/git/ref/tags/{tag}", optional=True)
    if existing is not None:
        if tag_ref is None or tag_ref["object"]["type"] != "tag":
            raise ValueError("draft recovery requires an existing annotated tag")
        annotated = api(f"{endpoint}/git/tags/{tag_ref['object']['sha']}")
        commit = validate_resume(request, existing, annotated)
        subprocess.run(["git", "merge-base", "--is-ancestor", commit, head], check=True)
    elif tag_ref is not None or "resume_commit" in request or "resume_release_id" in request:
        raise ValueError("orphaned tag or unmatched recovery request needs maintainer inspection")
    else:
        commit = approved_source(request)
        # The authorization commit can follow its candidate. Both the current
        # main control revision and approved source need their own green jobs.
        subprocess.run(["git", "merge-base", "--is-ancestor", commit, head], check=True)
    cargo = subprocess.check_output(["git", "show", f"{commit}:Cargo.toml"], text=True)
    if f'version = "{version}"' not in cargo:
        raise ValueError("release source version differs from the explicit request")
    notes_text = subprocess.check_output(["git", "show", f"{commit}:docs/releases/{tag}.md"], text=True)
    if existing is not None and existing["body"].replace("\r\n", "\n").strip() != notes_text.strip():
        raise ValueError("draft release notes differ from the tagged source")
    if api(f"{endpoint}/branches/main")["commit"]["sha"] != head:
        print("Main has advanced; only its current checked publication code can run.")
        return
    control_runs = verified_workflows(endpoint, head)
    if control_runs is None:
        print("Waiting for successful Rust checks and native packages on this exact main commit.")
        return
    selected = control_runs if commit == head else verified_workflows(endpoint, commit)
    if selected is None:
        raise ValueError("requested release source lacks successful exact-main checks")
    with tempfile.TemporaryDirectory(prefix="scarp-publication-") as temporary:
        root = Path(temporary)
        artifacts = root / "artifacts"
        package_run = selected[".github/workflows/packages.yml"]["id"]
        gh("run", "download", str(package_run), "--repo", repository, "--dir", str(artifacts))
        manifest = verifier.verify(artifacts, version, commit, Path.cwd())
        (artifacts / "SHA256SUMS").write_text(manifest, encoding="utf-8", newline="\n")
        publish = root / "publish"
        publish.mkdir()
        files = {}
        names = [line.split("  ", 1)[1] for line in manifest.splitlines()]
        for name in [*names, *(name + ".sha256" for name in names), "SHA256SUMS"]:
            matches = list(artifacts.rglob(name))
            if len(matches) != 1:
                raise ValueError(f"ambiguous release asset: {name}")
            target = publish / name
            shutil.copyfile(matches[0], target)
            files[name] = target
        verifier.verify(publish, version, commit, Path.cwd())
        if api(f"{endpoint}/branches/main")["commit"]["sha"] != head:
            raise ValueError("main moved during asset verification; refusing to publish")
        # A long verification run must not create a tag after its window ends.
        if not publication_window_open(request):
            return
        if existing is None:
            # Create a new annotated tag only after all final-source and asset gates.
            tag_object = api(f"{endpoint}/git/tags", payload={
                "tag": tag, "message": f"Scarp {tag} prerelease\n\nVerified final-main checks and native packages.",
                "object": commit, "type": "commit", "tagger": {
                    "name": request["tagger_name"], "email": request["tagger_email"],
                    "date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
                },
            })
            api(f"{endpoint}/git/refs", payload={"ref": "refs/tags/" + tag, "sha": tag_object["sha"]})
            notes = root / "release-notes.md"
            notes.write_text(notes_text, encoding="utf-8", newline="\n")
            gh("release", "create", tag, "--repo", repository, "--verify-tag", "--draft",
               "--prerelease", "--latest=false", "--title", f"Scarp {tag}",
               "--notes-file", str(notes), *(str(path) for path in files.values()))
            existing = release_from_list(api(f"{endpoint}/releases?per_page=100"), tag)
            if existing is None:
                raise ValueError("created draft is absent from authenticated release collection")
        draft = api(f"{endpoint}/releases/{existing['id']}")
        check_release(draft, files, draft=True)
        downloaded = root / "draft-downloads"
        downloaded.mkdir()
        for asset in draft["assets"]:
            download_draft_asset(endpoint, asset, downloaded / asset["name"])
        for name, original in files.items():
            if digest(downloaded / name) != digest(original):
                raise ValueError(f"draft download differs: {name}")
        if not publication_window_open(request):
            return
        if api(f"{endpoint}/branches/main")["commit"]["sha"] != head:
            raise ValueError("main moved during draft verification; refusing to publish")
        api(f"{endpoint}/releases/{draft['id']}", method="PATCH", payload={
            "draft": False, "prerelease": True, "make_latest": "false",
        })
        release = api(f"{endpoint}/releases/{draft['id']}")
        check_release(release, files, draft=False)
        if release["tag_name"] != tag or not release["published_at"]:
            raise ValueError("GitHub has not confirmed publication")
        for asset in release["assets"]:
            # Public download deliberately carries no authenticated request headers.
            with urllib.request.urlopen(asset["browser_download_url"], timeout=60) as response:
                actual = hashlib.file_digest(response, "sha256").hexdigest()
            if actual != digest(files[asset["name"]]):
                raise ValueError(f"public download differs: {asset['name']}")
            print(f"Verified public asset: {asset['name']} {actual}")
        print(f"Published prerelease: {release['html_url']} at {commit}")
        summary = os.environ.get("GITHUB_STEP_SUMMARY")
        if summary:
            with open(summary, "a", encoding="utf-8") as output:
                output.write(f"Published [{tag}]({release['html_url']}) from `{commit}`. All seven public assets verified.\n")


if __name__ == "__main__":
    main()
