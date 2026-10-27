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
        # One example in each selected consumer.
        for consumer in DOCS.SOURCE_CONSUMERS:
            path = root / consumer
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("//! ```\n//! assert!(true);\n//! ```\n")
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

    def test_the_repository_accounts_for_every_example(self):
        # The real sources, so a new example fails here, in the fast unit
        # tests, rather than at the full documentation check.
        counts = DOCS.coverage(DOCS.ROOT)
        self.assertEqual(set(counts), set(DOCS.CRATES))
        self.assertTrue(all(count > 0 for count in counts.values()), counts)

    def test_counts_are_derived_from_the_examples(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            counts = DOCS.coverage(root)
            self.assertEqual(sum(counts.values()), len(DOCS.SOURCE_CONSUMERS))
            source = root / DOCS.SOURCE_CONSUMERS[0]
            crate = Path(DOCS.SOURCE_CONSUMERS[0]).parts[1]
            # A second example counts, and other languages' fences do not.
            with source.open("a") as stream:
                stream.write("/// ```rust\n/// assert!(true);\n/// ```\n/// ```text\n/// output\n/// ```\n")
            self.assertEqual(DOCS.coverage(root)[crate], counts[crate] + 1)

    def test_zero_ignored_and_unselected_source_examples_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            source = root / DOCS.SOURCE_CONSUMERS[0]
            source.write_text("/// ```text\n/// not rust\n/// ```\n")
            with self.assertRaisesRegex(ValueError, "without a runnable example"):
                DOCS.coverage(root)
            source.write_text("//! ```ignore\n//! impossible();\n//! ```")
            with self.assertRaisesRegex(ValueError, "selected examples must run"):
                DOCS.coverage(root)
            source.write_text("//! ```\n//! assert!(true);\n//! ```\n")
            unselected = root / "crates/stobie-core/src/new.rs"
            unselected.write_text("/// ```\n/// assert!(true);\n/// ```\n")
            with self.assertRaisesRegex(ValueError, "unaccounted rustdoc examples in crates/stobie-core/src/new.rs"):
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
            (root / DOCS.VARIATION_GUIDE).write_text("stale recipe")
            with self.assertRaisesRegex(ValueError, "presenter variation differs"):
                DOCS.coverage(root)


if __name__ == "__main__":
    unittest.main()
