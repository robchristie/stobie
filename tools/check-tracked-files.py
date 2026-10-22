#!/usr/bin/env python3
"""Keep raw evidence and other bulky generated files out of tracked source.

Raw captures (screenshots, logs, JSON observations) belong in ignored
`.tools/runtime/` output or the evidence archive described in
docs/evidence-archive.md. Tracked `docs/*-evidence/` directories keep only
their Markdown summaries, and `docs/*-evidence.json` captures stay archived.
"""

from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
# Git stores every binary revision whole, so binaries get the tighter limit.
TEXT_LIMIT_BYTES = 1024 * 1024
BINARY_LIMIT_BYTES = 512 * 1024
# Reviewed binary sources. Keep this list short and explicit.
BINARY_ROOTS = (
    "crates/stobie-ui-egui/assets/fonts/",
    "docs/images/",
    "tests/fixtures/",
    "tests/ui-snapshots/expected/",
)
# Reviewed binaries that cannot meet the binary limit, each with its exact
# size, so any replacement is reviewed again. The emoji face is a whole
# monochrome emoji set; assets/fonts/README.md records its provenance.
SIZE_EXCEPTIONS = {
    "crates/stobie-ui-egui/assets/fonts/NotoEmoji-Regular.ttf": 886628,
}
BINARY_SUFFIXES = {
    ".7z", ".avif", ".bin", ".bmp", ".br", ".gif", ".gz", ".ico", ".j2k", ".jp2",
    ".jpeg", ".jpg", ".jph", ".jhc", ".mov", ".mp4", ".ntf", ".nitf", ".otf",
    ".pdf", ".png", ".profraw", ".tar", ".tgz", ".tif", ".tiff", ".ttf", ".wasm",
    ".webm", ".webp", ".woff", ".woff2", ".xz", ".zip", ".zst",
}
# Raw captures: anything but Markdown in docs/*-evidence/, and docs/*-evidence.json.
EVIDENCE = re.compile(r"docs/[^/]+-evidence(/.*(?<!\.md)|\.json)")


def tracked(root):
    output = subprocess.check_output(["git", "-C", str(root), "ls-files", "-z", "--cached"])
    return [path.decode() for path in output.split(b"\0") if path]


def binary(file, path):
    if Path(path).suffix.lower() in BINARY_SUFFIXES:
        return True
    with file.open("rb") as handle:
        return b"\0" in handle.read(8000)


def violations(root, paths):
    found = []
    for path in paths:
        file = root / path
        # Deleted working-tree files and symlinks carry no tracked bytes here.
        if file.is_symlink() or not file.is_file():
            continue
        size = file.stat().st_size
        is_binary = binary(file, path)
        limit = BINARY_LIMIT_BYTES if is_binary else TEXT_LIMIT_BYTES
        if size > limit and SIZE_EXCEPTIONS.get(path) != size:
            kind = "binary" if is_binary else "text"
            found.append(f"{path}: {size} bytes exceeds the {limit}-byte {kind} limit")
        if EVIDENCE.fullmatch(path):
            found.append(f"{path}: raw evidence; archive it in stobie-evidence")
        elif is_binary and not path.startswith(BINARY_ROOTS):
            found.append(f"{path}: binary file outside {', '.join(BINARY_ROOTS)}")
    return found


def main():
    paths = tracked(ROOT)
    found = violations(ROOT, paths)
    if found:
        print("Tracked-file policy failed (see docs/evidence-archive.md):", file=sys.stderr)
        for violation in found:
            print(f"  {violation}", file=sys.stderr)
        return 1
    print(f"Tracked-file policy passed for {len(paths)} files.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
