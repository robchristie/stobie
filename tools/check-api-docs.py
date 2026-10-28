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
    "stobie-core",
    "stobie-imaging",
    "stobie-runtime",
    "stobie-render-wgpu",
    "stobie-ui-egui",
)
# The source files whose rustdoc examples are selected consumers: exactly the
# crates' files with Rust examples, so a new example's file must be named
# here. Each crate's example count is derived from these files and must equal
# what `cargo test --doc` runs.
SOURCE_CONSUMERS = (
    "crates/stobie-core/src/dock.rs",
    "crates/stobie-core/src/history.rs",
    "crates/stobie-imaging/src/lib.rs",
    "crates/stobie-runtime/src/lib.rs",
    "crates/stobie-render-wgpu/src/lib.rs",
    "crates/stobie-ui-egui/src/lib.rs",
    "crates/stobie-ui-egui/src/actions.rs",
    "crates/stobie-ui-egui/src/presentation.rs",
    "crates/stobie-ui-egui/src/presentation/chat.rs",
    "crates/stobie-ui-egui/src/local_time.rs",
    "crates/stobie-ui-egui/src/components/avatar.rs",
    "crates/stobie-ui-egui/src/presentation/copy.rs",
    "crates/stobie-ui-egui/src/presentation/navigation.rs",
    "crates/stobie-ui-egui/src/presentation/dialog.rs",
    "crates/stobie-ui-egui/src/presentation/scroll.rs",
    "crates/stobie-ui-egui/src/presentation/text_field.rs",
    "crates/stobie-ui-egui/src/presentation/code_copy.rs",
    "crates/stobie-ui-egui/src/presentation/heading.rs",
    "crates/stobie-ui-egui/src/presentation/choice.rs",
    "crates/stobie-ui-egui/src/components/markdown/mod.rs",
)
MARKDOWN_CONSUMERS = (
    "README.md",
    "docs/application-composition.md",
    "docs/image-application-composition.md",
    "docs/start-a-new-app.md",
)
EXAMPLE = "apps/analytical-workspace-lab/examples/minimal-workspace.rs"
VARIATION_GUIDE = "docs/image-application-composition.md"
# Compiled consumers named by the guides: (package, example).
CONSUMERS = (
    ("analytical-workspace-lab", "minimal-workspace"),
    ("stobie-ui-egui", "notes"),
    ("stobie-ui-egui", "library"),
)
VARIATION = ('presentation.badge(ui, "state", "Document annotation", '
             'stobie_ui_egui::StatusTone::Success);')


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


# Rustdoc treats a fence as Rust unless its info string names another
# language; these are the attributes it reads as Rust.
RUST_ATTRIBUTES = {"rust", "ignore", "no_run", "should_panic", "compile_fail", "test_harness"}


def rust_examples(path: Path, name: str) -> int:
    """The runnable Rust examples in one source file's doc comments."""
    fences = re.findall(r"^\s*//[/!]\s*```([^\n]*)", path.read_text(encoding="utf-8"), re.M)
    if len(fences) % 2:
        raise ValueError(f"{name}: unclosed rustdoc example")
    count = 0
    for opening, closing in zip(fences[::2], fences[1::2]):
        if closing.strip():
            raise ValueError(f"{name}: unclosed rustdoc example")
        attributes = {attribute.strip() for attribute in opening.split(",") if attribute.strip()}
        if attributes and not attributes & RUST_ATTRIBUTES and not any(a.startswith("edition") for a in attributes):
            continue  # Another language, such as text or toml.
        if attributes - {"rust"}:
            raise ValueError(f"{name}: selected examples must run; unsupported treatment {opening!r}")
        count += 1
    return count


def coverage(root: Path) -> dict[str, int]:
    """Each crate's rustdoc example count, derived from its sources."""
    expected = {crate: 0 for crate in CRATES}
    found = set()
    for crate in CRATES:
        for path in sorted((root / "crates" / crate / "src").rglob("*.rs")):
            name = path.relative_to(root).as_posix()
            count = rust_examples(path, name)
            if count:
                found.add(name)
                expected[crate] += count
    if unselected := sorted(found - set(SOURCE_CONSUMERS)):
        raise ValueError(f"unaccounted rustdoc examples in {', '.join(unselected)}; select them in SOURCE_CONSUMERS")
    if stale := sorted(set(SOURCE_CONSUMERS) - found):
        raise ValueError(f"selected consumers without a runnable example: {', '.join(stale)}")
    if empty := [crate for crate, count in expected.items() if not count]:
        raise ValueError(f"crates without a selected example: {', '.join(empty)}")
    for consumer in MARKDOWN_CONSUMERS:
        if re.search(r"^```(?:rust|no_run|ignore|compile_fail)\b", (root / consumer).read_text(), re.M):
            raise ValueError(f"{consumer}: unaccounted Markdown Rust example; register an executable consumer")
    example = (root / EXAMPLE).read_text()
    if "// Public variation point: add a measured status badge here." not in example:
        raise ValueError("maintained consumer variation point is absent")
    if VARIATION not in (root / VARIATION_GUIDE).read_text():
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
    print(f"Selected coverage: {sum(expected.values())} runnable rustdoc examples, native/WASM consumer, presenter variation; {count} local links")
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
        for package, example in CONSUMERS:
            run(["cargo", "check", *target, "-p", package, "--example", example])
    print(f"API documentation checks passed: rendered generation, {sum(expected.values())} executed examples, native/WASM consumer compilation and selected local links")


if __name__ == "__main__":
    main()
