#!/usr/bin/env python3
"""Choose guarded prose verification from Git state, or run canonical verification."""

import argparse
import json
import os
from pathlib import Path
import re
import stat
import subprocess

ROOT = Path(__file__).resolve().parent.parent
# Keep this deliberately narrow. Evidence, snapshots, qualification reports,
# design contracts and agent instructions must retain the complete surface.
PROSE = {"README.md", "docs/plan-lifecycle.md"}


def prose_path(path):
    return path in PROSE or re.fullmatch(r"docs/[a-z0-9][a-z0-9.-]*-plan\.md", path) is not None


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args], stderr=subprocess.DEVNULL)


def documentation_only(root, base):
    """Unknown state and every non-allowlisted old/new path select full verification."""
    try:
        if not isinstance(base, str) or not base or base.startswith("-"):
            return False
        base = git(root, "rev-parse", "--verify", base + "^{commit}").decode().strip()
        # Git diffs deliberately trust these index flags and can hide modified
        # tracked bytes. Reject the flags themselves, even on allowlisted prose.
        entries = git(root, "ls-files", "-v", "-z").split(b"\0")
        if entries.pop() != b"" or any(not entry.startswith(b"H ") for entry in entries):
            return False
        try:
            if git(root, "config", "--bool", "core.sparseCheckout").strip() != b"false":
                return False
        except subprocess.CalledProcessError as error:
            if error.returncode != 1:  # An unset option is the normal full checkout.
                return False
        # Inspect committed, staged and unstaged changes independently: a staged
        # code edit hidden by a working-tree reversal must not evade the guard.
        for arguments in ((base, "HEAD"), ("--cached", "HEAD"), ()):
            fields = git(root, "diff", "--raw", "-z", "--no-renames", *arguments, "--").split(b"\0")
            if fields.pop() != b"":
                return False
            if len(fields) % 2:
                return False
            for header, raw_path in zip(fields[::2], fields[1::2]):
                parts = header.decode("ascii").split()
                old_mode, new_mode = parts[0][1:], parts[1]
                if parts[-1] not in {"A", "M", "D"}:
                    return False
                if any(mode not in {"000000", "100644"} for mode in (old_mode, new_mode)):
                    return False
                if not prose_path(raw_path.decode("utf-8")):
                    return False
        untracked = git(root, "ls-files", "--others", "--exclude-standard", "-z")
        for raw_path in untracked.split(b"\0"):
            if raw_path:
                path = raw_path.decode("utf-8")
                mode = (root / path).lstat().st_mode
                if not prose_path(path) or not stat.S_ISREG(mode) or mode & 0o111:
                    return False
        return True
    except (subprocess.CalledProcessError, OSError, UnicodeError, IndexError):
        return False


def event_base(root):
    try:
        event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
        if os.environ.get("GITHUB_EVENT_NAME") == "pull_request":
            return git(root, "merge-base", event["pull_request"]["base"]["sha"], "HEAD").decode().strip()
        if os.environ.get("GITHUB_EVENT_NAME") == "push":
            return event["before"]
    except (KeyError, TypeError, ValueError, OSError, subprocess.CalledProcessError):
        pass
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", help="local comparison commit; missing/invalid means full verification")
    parser.add_argument("--classify", action="store_true", help="report the Git-derived CI route only")
    args = parser.parse_args()
    base = event_base(ROOT) if os.environ.get("GITHUB_ACTIONS") == "true" else args.base
    mode = "docs" if documentation_only(ROOT, base) else "full"
    print(f"Verification route: {mode}", flush=True)
    if args.classify:
        if os.environ.get("GITHUB_OUTPUT"):
            with open(os.environ["GITHUB_OUTPUT"], "a") as output:
                output.write(f"mode={mode}\n")
        return
    if mode == "full":
        subprocess.run(["cargo", "xtask", "verify"], cwd=ROOT, check=True)
        return
    for arguments in ((base, "HEAD"), ("--cached",), ()):
        subprocess.run(["git", "diff", "--check", *arguments, "--"], cwd=ROOT, check=True)
    subprocess.run(["python3", "-m", "unittest", "discover", "-s", "tools/tests", "-p", "test_verify.py"], cwd=ROOT, check=True)
    subprocess.run(["cargo", "test", "-p", "xtask", "plans::"], cwd=ROOT, check=True)
    subprocess.run(["cargo", "xtask", "plans"], cwd=ROOT, check=True)
    print("Documentation verification passed: Git scope guard, whitespace and plan lifecycle", flush=True)


if __name__ == "__main__":
    main()
