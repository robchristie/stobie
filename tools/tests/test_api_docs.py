"""Regression coverage for selected documentation qualification failures."""

import importlib.util
from pathlib import Path
import tempfile
import unittest


SPEC = importlib.util.spec_from_file_location(
    "api_docs", Path(__file__).resolve().parents[1] / "check-api-docs.py"
)
DOCS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DOCS)


class ApiDocsTests(unittest.TestCase):
    def fixture(self, root):
        for consumer in DOCS.SOURCE_CONSUMERS:
            path = root / consumer
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("//! ```\n//! assert!(true);\n//! ```\n" if path.name in ("lib.rs", "presentation.rs") else "")
        for consumer in DOCS.MARKDOWN_CONSUMERS:
            path = root / consumer
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(DOCS.VARIATION)
        path = root / DOCS.EXAMPLE
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("// Public variation point: add a measured status badge here.")

    def test_broken_selected_link_and_anchor_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "README.md"
            source.write_text("[guide](missing.md)")
            with self.assertRaisesRegex(ValueError, "broken local link missing.md"):
                DOCS.local_links(root, ("README.md",))
            source.write_text("# Existing\n[section](#absent)")
            with self.assertRaisesRegex(ValueError, "broken local anchor"):
                DOCS.local_links(root, ("README.md",))

    def test_zero_and_ignored_source_examples_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            self.assertEqual(sum(DOCS.coverage(root).values()), 5)
            source = root / DOCS.SOURCE_CONSUMERS[0]
            source.write_text("")
            with self.assertRaisesRegex(ValueError, "explicitly accounted"):
                DOCS.coverage(root)
            source.write_text("//! ```ignore\n//! impossible();\n//! ```")
            with self.assertRaisesRegex(ValueError, "selected examples must run"):
                DOCS.coverage(root)

    def test_new_markdown_rust_example_needs_a_consumer(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            with (root / "README.md").open("a") as stream:
                stream.write("\n```rust\nimpossible();\n```\n")
            with self.assertRaisesRegex(ValueError, "unaccounted Markdown Rust example"):
                DOCS.coverage(root)

    def test_execution_cannot_hide_zero_ignored_or_extra_examples(self):
        template = "test result: ok. {} passed; 0 failed; {} ignored; 0 measured; 0 filtered out"
        DOCS.check_doctest_result(template.format(1, 0), 1)
        for passed, ignored in ((0, 0), (0, 1), (2, 0)):
            with self.subTest(passed=passed, ignored=ignored):
                with self.assertRaisesRegex(ValueError, "zero, ignored or unaccounted"):
                    DOCS.check_doctest_result(template.format(passed, ignored), 1)

    def test_documented_variation_cannot_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            (root / "docs/application-composition.md").write_text("stale recipe")
            with self.assertRaisesRegex(ValueError, "presenter variation differs"):
                DOCS.coverage(root)


if __name__ == "__main__":
    unittest.main()
