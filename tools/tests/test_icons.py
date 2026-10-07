"""Keep authored artwork and compiled geometry one offline source of truth."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[1] / 'generate-icons.py'
SPEC = importlib.util.spec_from_file_location('icons', SOURCE)
ICONS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ICONS)


class IconCompilerTests(unittest.TestCase):
    def test_checked_in_geometry_matches_all_authored_svg(self):
        self.assertEqual(ICONS.generate(), ICONS.OUTPUT.read_text())

    def test_maintained_toolbar_example_matches_executed_rustdoc(self):
        guide = (ICONS.ROOT / 'docs/ui-guides/components.md').read_text()
        toolbar = guide.split('## Typed icons and action presentations\n', 1)[1]
        example = toolbar.split('```rust\n', 1)[1].split('```', 1)[0].strip()
        source = (ICONS.ROOT / 'crates/polyorama-ui-egui/src/presentation.rs').read_text()
        doc = source.split('    /// ```\n')[1].split('    /// ```')[0]
        self.assertEqual(example, '\n'.join(line.removeprefix('    ///').removeprefix(' ') for line in doc.splitlines()).strip())

    def test_rejects_external_images_styles_and_unsupported_paths(self):
        original = (ICONS.SOURCE / 'check.svg').read_text()
        invalid = [original.replace('stroke="currentColor"', 'stroke="#abcdef"'),
                   original.replace('<path', '<image'),
                   original.replace('M 4', 'Q 4'),
                   original.replace('L 20', 'L 40')]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'check.svg'
            for svg in invalid:
                with self.subTest(svg=svg):
                    path.write_text(svg)
                    with self.assertRaises(ValueError):
                        ICONS.compile_svg(path)
