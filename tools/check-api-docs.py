#!/usr/bin/env python3
"""Check the explicitly selected application-facing documentation consumers."""

import argparse
import os
from pathlib import Path
import re
import subprocess
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1]
# This is a bounded coverage policy, not every export or Markdown file.
CRATES = (
    "polyorama-core",
    "polyorama-runtime",
    "polyorama-render-wgpu",
    "polyorama-ui-egui",
)
EXPECTED_EXAMPLES = {crate: 1 for crate in CRATES}
EXPECTED_EXAMPLES["polyorama-ui-egui"] = 2  # Crate composition and typed icon toolbar.
SOURCE_CONSUMERS = (
    "crates/polyorama-core/src/lib.rs",
    "crates/polyorama-core/src/data.rs",
    "crates/polyorama-core/src/commands.rs",
    "crates/polyorama-core/src/dock.rs",
    "crates/polyorama-runtime/src/lib.rs",
    "crates/polyorama-render-wgpu/src/lib.rs",
    "crates/polyorama-ui-egui/src/lib.rs",
    "crates/polyorama-ui-egui/src/actions.rs",
    "crates/polyorama-ui-egui/src/presentation.rs",
    "crates/polyorama-ui-egui/src/components/action_button.rs",
)
MARKDOWN_CONSUMERS = (
    "README.md",
    "docs/application-composition.md",
)
EXAMPLE = "apps/analytical-workspace-lab/examples/minimal-workspace.rs"
VARIATION = ('presentation.badge(ui, "state", "Document annotation", '
             'polyorama_ui_egui::StatusTone::Success);')


def local_links(root: Path, consumers=MARKDOWN_CONSUMERS) -> int:
    checked = 0
    for consumer in consumers:
        source = root / consumer
        content = source.read_text(encoding="utf-8")
        for target in re.findall(r"!?\[[^\]]*\]\(([^\s)]+)\)", content):
            url = urlsplit(target)
            if url.scheme or url.netloc:
                continue
            destination = source.parent / unquote(url.path) if url.path else source
            if not destination.exists():
                raise ValueError(f"{consumer}: broken local link {target}")
            if url.fragment and destination.suffix == ".md":
                headings = re.findall(r"^#{1,6}\s+(.+?)\s*$", destination.read_text(), re.M)
                anchors = {
                    re.sub(r"[^\w\- ]", "", heading.lower()).replace(" ", "-")
                    for heading in headings
                }
                if unquote(url.fragment) not in anchors:
                    raise ValueError(f"{consumer}: broken local anchor {target}")
            checked += 1
    if checked == 0:
        raise ValueError("selected documentation has zero local links")
    return checked


def coverage(root: Path) -> dict[str, int]:
    expected = {crate: 0 for crate in CRATES}
    for consumer in SOURCE_CONSUMERS:
        text = (root / consumer).read_text(encoding="utf-8")
        fences = re.findall(r"^\s*//[/!]\s*```([^\n]*)", text, re.M)
        if len(fences) % 2:
            raise ValueError(f"{consumer}: unclosed selected rustdoc example")
        for opening, closing in zip(fences[::2], fences[1::2]):
            if opening.strip() not in ("", "rust") or closing.strip():
                raise ValueError(f"{consumer}: selected examples must run; unsupported treatment {opening!r}")
            crate = Path(consumer).parts[1]
            expected[crate] += 1
    if expected != EXPECTED_EXAMPLES:
        raise ValueError(f"selected crate examples must be explicitly accounted for (declared per crate): {expected}")
    for consumer in MARKDOWN_CONSUMERS:
        if re.search(r"^```(?:rust|no_run|ignore|compile_fail)\b", (root / consumer).read_text(), re.M):
            raise ValueError(f"{consumer}: unaccounted Markdown Rust example; register an executable consumer")
    example = (root / EXAMPLE).read_text()
    if "// Public variation point: add a measured status badge here." not in example:
        raise ValueError("maintained consumer variation point is absent")
    if VARIATION not in (root / "docs/application-composition.md").read_text():
        raise ValueError("documented presenter variation differs from selected recipe")
    return expected


def check_doctest_result(output: str, expected: int) -> None:
    results = re.findall(
        r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out",
        output,
    )
    if len(results) != 1 or tuple(map(int, results[0])) != (expected, 0, 0, 0, 0):
        raise ValueError(f"selected doctest coverage is zero, ignored or unaccounted: {results}")


def run(arguments: list[str], *, capture=False, environment=None) -> str:
    print("+ " + " ".join(arguments), flush=True)
    result = subprocess.run(arguments, cwd=ROOT, env=environment, text=True,
                            stdout=subprocess.PIPE if capture else None,
                            stderr=subprocess.STDOUT if capture else None, check=False)
    if capture:
        print(result.stdout, end="", flush=True)
    if result.returncode:
        raise RuntimeError(f"documentation command failed ({result.returncode}): {arguments}")
    return result.stdout or ""


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--links-only", action="store_true", help="lightweight coverage and local-link validation")
    args = parser.parse_args()
    expected = coverage(ROOT)
    count = local_links(ROOT)
    print(f"Selected coverage: five runnable rustdoc examples, native/WASM consumer, presenter variation; {count} local links")
    if args.links_only:
        return
    packages = [argument for crate in CRATES for argument in ("-p", crate)]
    environment = dict(os.environ)
    environment["RUSTDOCFLAGS"] = environment.get("RUSTDOCFLAGS", "") + " -D warnings"
    run(["cargo", "doc", "--no-deps", *packages], environment=environment)
    # Generate documentation and execute examples separately: one never proves the other.
    for crate in CRATES:
        output = run(["cargo", "test", "--doc", "-p", crate], capture=True, environment=environment)
        check_doctest_result(output, expected[crate])
    for target in ([], ["--target", "wasm32-unknown-unknown"]):
        run(["cargo", "check", *target, "-p", "analytical-workspace-lab", "--example", "minimal-workspace"])
    print("API documentation checks passed: rendered generation, five executed examples, native/WASM consumer compilation and selected local links")


if __name__ == "__main__":
    main()
