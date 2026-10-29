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
        "Stable / windows-latest", "Stable / macos-latest",
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


def api(path, *, payload=None, optional=False):
    args = ["api", path]
    if payload is not None:
        args += ["--method", "POST", "--input", "-"]
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


def main():
    request_path = Path(".github/release-request.json")
    if not request_path.exists():
        print("No prerelease request; nothing to publish.")
        return
    request = json.loads(request_path.read_text(encoding="utf-8"))
    version = request["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+-beta\.\d+", version):
        raise ValueError("only an explicitly requested beta version may be published")
    cargo = Path("Cargo.toml").read_text(encoding="utf-8")
    if f'version = "{version}"' not in cargo:
        print("Checkout version differs from release request; nothing to publish.")
        return
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    if commit != os.environ["SCARP_SOURCE_SHA"]:
        raise ValueError("checkout does not match the triggering source commit")
    repository = os.environ["GITHUB_REPOSITORY"]
    endpoint = f"repos/{repository}"
    tag = "v" + version
    notes = Path(f"docs/releases/{tag}.md")
    if not notes.is_file():
        raise ValueError("missing versioned release notes")
    existing = api(f"{endpoint}/releases/tags/{tag}", optional=True)
    if existing is not None:
        if existing["draft"] or not existing["prerelease"]:
            raise ValueError("existing release needs maintainer inspection; refusing to overwrite")
        print(f"Already published: {existing['html_url']}; leaving it unchanged.")
        return
    if api(f"{endpoint}/branches/main")["commit"]["sha"] != commit:
        print("Main has advanced; only its current checked commit can be published.")
        return
    runs = api(f"{endpoint}/actions/runs?head_sha={commit}&event=push&per_page=100")["workflow_runs"]
    selected = selected_runs(runs, commit)
    if selected is None:
        print("Waiting for successful Rust checks and native packages on this exact main commit.")
        return
    for path, run in selected.items():
        jobs = api(f"{endpoint}/actions/runs/{run['id']}/jobs?per_page=100")["jobs"]
        validate_jobs(path, jobs)
        print(f"Verified required jobs: {run['html_url']}")
    if api(f"{endpoint}/git/ref/tags/{tag}", optional=True) is not None:
        raise ValueError("tag already exists without a release; refusing to reuse or move it")
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
        if api(f"{endpoint}/branches/main")["commit"]["sha"] != commit:
            raise ValueError("main moved during asset verification; refusing to tag")
        # Create an annotated tag only after all final-source and asset gates.
        tag_object = api(f"{endpoint}/git/tags", payload={
            "tag": tag, "message": f"Scarp {tag} prerelease\n\nVerified final-main checks and native packages.",
            "object": commit, "type": "commit", "tagger": {
                "name": request["tagger_name"], "email": request["tagger_email"],
                "date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            },
        })
        api(f"{endpoint}/git/refs", payload={"ref": "refs/tags/" + tag, "sha": tag_object["sha"]})
        gh("release", "create", tag, "--repo", repository, "--verify-tag", "--draft",
           "--prerelease", "--latest=false", "--title", f"Scarp {tag} — Typed Functions",
           "--notes-file", str(notes), *(str(path) for path in files.values()))
        draft = api(f"{endpoint}/releases/tags/{tag}")
        check_release(draft, files, draft=True)
        downloaded = root / "draft-downloads"
        gh("release", "download", tag, "--repo", repository, "--dir", str(downloaded))
        for name, original in files.items():
            if digest(downloaded / name) != digest(original):
                raise ValueError(f"draft download differs: {name}")
        gh("release", "edit", tag, "--repo", repository, "--draft=false", "--prerelease", "--latest=false")
        release = api(f"{endpoint}/releases/tags/{tag}")
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
