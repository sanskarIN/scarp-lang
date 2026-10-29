"""Verify native release assets against an exact version and source commit."""

import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import tarfile
import zipfile


def verify_source_members(read_member, root, source_files):
    for name, expected in source_files.items():
        data = read_member(root + "/" + name)
        digest = hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()
        if digest != expected:
            raise ValueError(f"package source differs from release commit: {name}")


def source_files_at(source_directory, commit):
    result = subprocess.run(
        ["git", "-C", str(source_directory), "ls-tree", "-rz", commit, "--",
         "LICENSE", "README.md", "CHANGELOG.md", "CONTRIBUTING.md", "docs", "examples"],
        check=True, stdout=subprocess.PIPE,
    )
    files = {}
    for record in result.stdout.split(b"\0"):
        if record:
            metadata, name = record.decode("utf-8").split("\t", 1)
            mode, kind, digest = metadata.split()
            if kind != "blob" or mode not in ("100644", "100755"):
                raise ValueError(f"unsupported packaged source entry: {name}")
            files[name] = digest
    if "README.md" not in files or "LICENSE" not in files:
        raise ValueError("release source tree is missing root documentation")
    return files


def verify(directory, version, commit, source_directory=None):
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", version):
        raise ValueError("expected a three-part package version")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("expected the full release commit")
    source_files = source_files_at(source_directory, commit) if source_directory is not None else {}
    checksums = []
    for target, extension in (
        ("x86_64-pc-windows-msvc", ".zip"),
        ("x86_64-unknown-linux-gnu", ".tar.gz"),
        ("aarch64-apple-darwin", ".tar.gz"),
    ):
        root = f"scarp-{version}-{target}"
        name = root + extension
        matches = list(directory.rglob(name))
        if len(matches) != 1:
            raise ValueError(f"expected exactly one {name}")
        archive = matches[0]
        with archive.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        checksum = f"{digest}  {name}\n"
        if archive.with_name(name + ".sha256").read_text().strip() != checksum.strip():
            raise ValueError(f"checksum mismatch: {name}")
        if extension == ".zip":
            with zipfile.ZipFile(archive) as package:
                paths = set(package.namelist())
                record = package.read(root + "/BUILD-INFO.txt")
                verify_source_members(package.read, root, source_files)
        else:
            with tarfile.open(archive, "r:gz") as package:
                paths = set(package.getnames())
                stream = package.extractfile(root + "/BUILD-INFO.txt")
                if stream is None:
                    raise ValueError(f"missing build record: {name}")
                record = stream.read()
                if not package.getmember(root + "/scarp").mode & 0o111:
                    raise ValueError(f"missing executable permission: {name}")
                def read_member(name):
                    member = package.getmember(name)
                    if not member.isfile():
                        raise ValueError(f"packaged source is not a regular file: {name}")
                    return package.extractfile(member).read()
                verify_source_members(read_member, root, source_files)
        fields = dict(line.split("=", 1) for line in record.decode("utf-8").splitlines())
        for key, expected in (
            ("version", version), ("target", target),
            ("commit", commit), ("working_tree", "clean"),
        ):
            if fields.get(key) != expected:
                raise ValueError(f"{name}: unexpected {key}: {fields.get(key)}")
        if not fields.get("compiler", "").startswith("rustc "):
            raise ValueError(f"missing compiler record: {name}")
        executable = "scarp.exe" if extension == ".zip" else "scarp"
        iteration_examples = (
            "examples/loops.srp", "examples/loops.stdout",
            "examples/repeat.srp", "examples/repeat.stdout",
            "examples/control_escapes.srp", "examples/control_escapes.stdout",
        ) if (version.startswith("0.0.3-") or version == "0.0.4-dev") else ()
        syntax_migration_files = (
            "examples/output.srp", "examples/output.stdout",
            "docs/output.md", "docs/syntax-migration.md",
        ) if version in ("0.0.3-beta.2", "0.0.3-beta.3", "0.0.4-dev") else ()
        function_files = (
            "docs/functions.md", "examples/functions.srp", "examples/functions.stdout",
            "examples/function_contracts.srp", "examples/function_contracts.stdout",
            "examples/function_recursion.srp", "examples/function_recursion.stdout",
            "docs/budgets.md", "docs/reference/cli.md", "docs/embedding.md",
        ) if version in ("0.0.3-beta.3", "0.0.4-dev") else ()
        for member in (
            executable, "LICENSE", "README.md", "CHANGELOG.md", "CONTRIBUTING.md",
            f"docs/releases/v{version}.md", "examples/bindings.srp",
            "examples/mutable.srp", "examples/scopes.srp", "examples/decisions.srp",
            *iteration_examples, *syntax_migration_files, *function_files,
        ):
            if root + "/" + member not in paths:
                raise ValueError(f"{name}: missing {member}")
        checksums.append(checksum)
        print(f"Verified {name}: {digest}")
    return "".join(sorted(checksums))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--write-manifest", action="store_true")
    parser.add_argument("--source-directory", type=Path, help="compare packaged docs/examples with exact Git blobs")
    args = parser.parse_args()
    try:
        manifest = verify(args.directory, args.version, args.commit, args.source_directory)
        path = args.directory / "SHA256SUMS"
        if args.write_manifest:
            with path.open("x", encoding="utf-8", newline="\n") as output:
                output.write(manifest)
        elif path.read_text(encoding="utf-8") != manifest:
            raise ValueError("SHA256SUMS differs from the verified archives")
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError, tarfile.TarError, zipfile.BadZipFile) as error:
        parser.exit(1, f"release verification failed: {error}\n")


if __name__ == "__main__":
    main()
