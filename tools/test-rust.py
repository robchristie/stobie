#!/usr/bin/env python3
"""Run the canonical Rust selections with separate retained Nextest reports."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
REPORTS = Path(".tools/runtime/verification-evidence/nextest")
SELECTIONS = {
    "workspace": ["--workspace"],
    "ui-no-default": ["-p", "stobie-ui-egui", "--no-default-features"],
    "record-desk": ["--locked"],
}
WORKSPACES = {"record-desk": Path("consumers/record-desk")}
# Tests bind Unix sockets named by a host instance (about 50 bytes) inside
# TMPDIR, and a socket path holds at most 107 bytes on Linux.
TEMPORARY_PATH_BYTES = 48


def clear_reports(root: Path, selections=SELECTIONS) -> None:
    # Remove only owned XML files; other configurations and evidence survive.
    for selection in selections:
        (root / REPORTS / selection / "junit.xml").unlink(missing_ok=True)


def temporary_directory(root: Path):
    """Return a short TMPDIR for the tests and whether the caller removes it.

    The checkout's own `.tools/t` is used while its path is short enough for
    the tests' Unix sockets; a checkout deeper than that, such as a linked
    worktree, gets a private directory under the system's `/tmp` instead.
    """
    temporary = root / ".tools/t"
    if len(os.fsencode(str(temporary))) <= TEMPORARY_PATH_BYTES:
        temporary.mkdir(parents=True, exist_ok=True)
        return temporary, False
    return Path(tempfile.mkdtemp(prefix="stobie-t-", dir="/tmp")), True


def run(selection: str, root: Path = ROOT) -> None:
    arguments = SELECTIONS[selection]
    clear_reports(root, [selection])
    # Keep Unix socket paths below platform limits in longer linked worktrees.
    temporary, owned = temporary_directory(root)
    environment = {**os.environ, "TMPDIR": str(temporary)}
    commands = (
        ["cargo", "nextest", "show-config", "version"],
        ["cargo", "nextest", "run", "--profile", selection, *arguments],
        ["cargo", "test", "--doc", *arguments],
    )
    try:
        for command in commands:
            print("+ " + " ".join(command), flush=True)
            subprocess.run(command, cwd=root / WORKSPACES.get(selection, Path(".")),
                           env=environment, check=True)
    finally:
        if owned:
            shutil.rmtree(temporary, ignore_errors=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selection", choices=["prepare", "prepare-record-desk", *SELECTIONS])
    args = parser.parse_args()
    if args.selection == "prepare":
        clear_reports(ROOT)
    elif args.selection == "prepare-record-desk":
        clear_reports(ROOT, ["record-desk"])
    else:
        run(args.selection)


if __name__ == "__main__":
    main()
