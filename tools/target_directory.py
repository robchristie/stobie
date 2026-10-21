#!/usr/bin/env python3
"""Print the target directory Cargo builds a workspace into.

usage: target_directory.py [--manifest-path PATH]

Cargo resolves it from CARGO_TARGET_DIR, its configuration or the workspace's
own target/, so smokes started against a build elsewhere find its binaries.
"""

import argparse
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def target_directory(manifest_path=None, root: Path = ROOT) -> Path:
    command = ["cargo", "metadata", "--format-version", "1", "--no-deps"]
    if manifest_path is not None:
        command += ["--manifest-path", str(manifest_path)]
    output = subprocess.run(command, cwd=root, check=True, capture_output=True, text=True).stdout
    return Path(json.loads(output)["target_directory"])


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest-path", type=Path)
    print(target_directory(parser.parse_args().manifest_path))


if __name__ == "__main__":
    main()
