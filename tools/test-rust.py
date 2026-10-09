#!/usr/bin/env python3
"""Run the canonical Rust selections with separate retained Nextest reports."""

import argparse
import os
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
REPORTS = Path(".tools/runtime/verification-evidence/nextest")
SELECTIONS = {
    "workspace": ["--workspace"],
    "ui-no-default": ["-p", "polyorama-ui-egui", "--no-default-features"],
    "record-desk": ["--locked"],
}
WORKSPACES = {"record-desk": Path("consumers/record-desk")}


def clear_reports(root: Path, selections=SELECTIONS) -> None:
    # Remove only owned XML files; other configurations and evidence survive.
    for selection in selections:
        (root / REPORTS / selection / "junit.xml").unlink(missing_ok=True)


def run(selection: str, root: Path = ROOT) -> None:
    arguments = SELECTIONS[selection]
    clear_reports(root, [selection])
    # Keep Unix socket paths below platform limits in longer linked worktrees.
    temporary = root / ".tools/t"
    temporary.mkdir(parents=True, exist_ok=True)
    environment = {**os.environ, "TMPDIR": str(temporary)}
    commands = (
        ["cargo", "nextest", "show-config", "version"],
        ["cargo", "nextest", "run", "--profile", selection, *arguments],
        ["cargo", "test", "--doc", *arguments],
    )
    for command in commands:
        print("+ " + " ".join(command), flush=True)
        subprocess.run(command, cwd=root / WORKSPACES.get(selection, Path(".")),
                       env=environment, check=True)


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
