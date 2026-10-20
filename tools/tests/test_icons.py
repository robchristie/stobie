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
        source = (ICONS.ROOT / 'crates/stobie-ui-egui/src/presentation.rs').read_text()
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


class CurveCompilerTests(unittest.TestCase):
    def compile(self, data, extra=''):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'probe.svg'
            path.write_text(
                '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" '
                'stroke="currentColor" stroke-width="2" stroke-linecap="round" '
                f'stroke-linejoin="round"><path d="{data}"/>{extra}</svg>')
            return ICONS.compile_svg(path)

    def test_straight_paths_keep_the_polyline_form(self):
        self.assertEqual(self.compile('M 4 4 L 20 4 L 20 20 Z'),
                         ['IconGeometry::Path(&[[4.0, 4.0], [20.0, 4.0], [20.0, 20.0]], true)'])

    def test_a_quarter_circle_becomes_one_standard_cubic(self):
        # The usual 0.5523 handle length for a quarter turn of radius 10.
        self.assertEqual(self.compile('M 2 12 A 10 10 0 0 1 12 2'), [
            'IconGeometry::Curve([2.0, 12.0], &[IconSegment::Cubic('
            '[2.0, 6.477], [6.477, 2.0], [12.0, 2.0])], false)'])

    def test_arcs_split_into_quarter_turns_and_end_exactly(self):
        geometry = self.compile('M 4 12 A 8 8 0 1 0 20 12 L 20 20')[0]
        self.assertEqual(geometry.count('IconSegment::Cubic'), 2)
        self.assertIn('[20.0, 12.0])', geometry)
        self.assertIn('IconSegment::Line([20.0, 20.0])', geometry)
        # The other sweep bends the other way.
        self.assertIn('[12.0, 20.0]', geometry)
        self.assertIn('[12.0, 4.0]', self.compile('M 4 12 A 8 8 0 0 1 20 12')[0])

    def test_small_radii_scale_up_and_ellipses_keep_their_axes(self):
        # A radius too small to reach the end point grows to a half turn.
        self.assertEqual(self.compile('M 4 12 A 1 1 0 0 1 20 12'),
                         self.compile('M 4 12 A 8 8 0 0 1 20 12'))
        self.assertIn('[12.0, 6.0]', self.compile('M 4 12 A 8 6 0 0 1 20 12')[0])

    def test_quadratics_become_exact_cubics_and_curves_close(self):
        self.assertEqual(self.compile('M 3 21 Q 12 3 21 21 Z'), [
            'IconGeometry::Curve([3.0, 21.0], &[IconSegment::Cubic('
            '[9.0, 9.0], [15.0, 9.0], [21.0, 21.0])], true)'])
        self.assertEqual(self.compile('M 3 3 C 3 21 21 21 21 3'), [
            'IconGeometry::Curve([3.0, 3.0], &[IconSegment::Cubic('
            '[3.0, 21.0], [21.0, 21.0], [21.0, 3.0])], false)'])

    def test_rejects_unbounded_or_ambiguous_curves(self):
        for data in ['M 2 12 A 10 10 30 0 1 12 2',  # rotated
                     'M 2 12 A 10 10 0 2 1 12 2',   # flag out of range
                     'M 2 12 A 0 10 0 0 1 12 2',    # zero radius
                     'M 2 12 A 10 10 0 0 1 2 12',   # no movement
                     'M 2 12 a 10 10 0 0 1 12 2',   # relative
                     'M 2 12 q 4 4 8 8',
                     'M 2 12 S 4 4 8 8',            # shorthand
                     'M 2 12 C 4 4 8',              # incomplete
                     'M 2 12 Q 30 4 8 8',           # control point off the grid
                     'M 2 12 A 11 11 0 1 1 2 14',  # a computed control off the grid
                     'A 4 4 0 0 1 8 8']:            # no start
            with self.subTest(data=data), self.assertRaises(ValueError):
                self.compile(data)
