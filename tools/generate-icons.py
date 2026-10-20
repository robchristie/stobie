#!/usr/bin/env python3
"""Compile Stobie's closed SVG subset into dependency-free vector geometry."""
import argparse
import math
from pathlib import Path
import re
import subprocess
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'crates/stobie-ui-egui/assets/icons'
OUTPUT = ROOT / 'crates/stobie-ui-egui/src/generated_icons.rs'
NS = '{http://www.w3.org/2000/svg}'


def coordinate(value):
    result = float(value)
    if not 0 <= result <= 24:
        raise ValueError(f'coordinate outside icon grid: {value}')
    return result


def number(value):
    return repr(coordinate(value))


def literal(point):
    # Computed control points are rounded so generated geometry stays stable.
    return '[' + ', '.join(repr(round(coordinate(value), 3) + 0.0) for value in point) + ']'


def arc_to_cubics(start, rx, ry, rotation, large, sweep, end):
    """Elliptical arc (SVG implementation notes F.6.5, no rotation) as cubic Béziers
    of at most a quarter turn each."""
    if rotation != 0:
        raise ValueError('rotated arcs are unsupported')
    if large not in (0, 1) or sweep not in (0, 1):
        raise ValueError('arc flags must be 0 or 1')
    if rx <= 0 or ry <= 0 or start == end:
        raise ValueError('degenerate arc')
    (x1, y1), (x2, y2) = start, end
    hx, hy = (x1 - x2) / 2, (y1 - y2) / 2
    scale = hx * hx / (rx * rx) + hy * hy / (ry * ry)
    if scale > 1:
        rx, ry = rx * math.sqrt(scale), ry * math.sqrt(scale)
    numerator = rx * rx * ry * ry - rx * rx * hy * hy - ry * ry * hx * hx
    denominator = rx * rx * hy * hy + ry * ry * hx * hx
    factor = math.sqrt(max(0.0, numerator / denominator))
    if large == sweep:
        factor = -factor
    cx = factor * rx * hy / ry + (x1 + x2) / 2
    cy = -factor * ry * hx / rx + (y1 + y2) / 2
    first = math.atan2((y1 - cy) / ry, (x1 - cx) / rx)
    turn = math.atan2((y2 - cy) / ry, (x2 - cx) / rx) - first
    if sweep and turn <= 0:
        turn += 2 * math.pi
    elif not sweep and turn >= 0:
        turn -= 2 * math.pi
    count = max(1, math.ceil(abs(turn) / (math.pi / 2) - 1e-9))
    step = turn / count
    handle = 4 / 3 * math.tan(step / 4)
    cubics = []
    for index in range(count):
        a, b = first + step * index, first + step * (index + 1)
        p0 = (cx + rx * math.cos(a), cy + ry * math.sin(a))
        p3 = end if index == count - 1 else (cx + rx * math.cos(b), cy + ry * math.sin(b))
        c1 = (p0[0] - handle * rx * math.sin(a), p0[1] + handle * ry * math.cos(a))
        c2 = (p3[0] + handle * rx * math.sin(b), p3[1] - handle * ry * math.cos(b))
        cubics.append((c1, c2, p3))
    return cubics


def compile_path(path, data):
    """Absolute M, L, Q, C, A and Z commands, with explicit command letters and
    whitespace. Straight paths keep the polyline form; curves become cubics."""
    words = data.split()
    geometry = []
    start = None
    segments = []
    closed = False

    def take(count):
        if len(words) < count:
            raise ValueError(f'{path}: incomplete path command')
        return [words.pop(0) for _ in range(count)]

    def point():
        x, y = take(2)
        return (coordinate(x), coordinate(y))

    def emit():
        if not segments:
            raise ValueError(f'{path}: path needs at least two points')
        if all(kind == 'L' for kind, *_ in segments):
            points = [start] + [end for _, end in segments]
            coords = ', '.join(f'[{x!r}, {y!r}]' for x, y in points)
            geometry.append(f'IconGeometry::Path(&[{coords}], {str(closed).lower()})')
            return
        parts = []
        for kind, *values in segments:
            if kind == 'L':
                parts.append(f'IconSegment::Line({literal(values[0])})')
            else:
                parts.append('IconSegment::Cubic(' + ', '.join(literal(v) for v in values) + ')')
        geometry.append(f'IconGeometry::Curve({literal(start)}, &[{", ".join(parts)}], '
                        f'{str(closed).lower()})')

    while words:
        command = words.pop(0)
        if command == 'M':
            if segments:
                emit()
            start, segments, closed = point(), [], False
            continue
        if start is None:
            raise ValueError(f'{path}: unsupported path command {command}')
        current = segments[-1][-1] if segments else start
        if command == 'Z':
            closed = True
            emit()
            start, segments, closed = None, [], False
        elif command == 'L':
            segments.append(('L', point()))
        elif command == 'Q':
            control, end = point(), point()
            # The exact cubic form of a quadratic Bézier.
            c1 = tuple(current[i] + 2 / 3 * (control[i] - current[i]) for i in range(2))
            c2 = tuple(end[i] + 2 / 3 * (control[i] - end[i]) for i in range(2))
            segments.append(('C', c1, c2, end))
        elif command == 'C':
            segments.append(('C', point(), point(), point()))
        elif command == 'A':
            rx, ry, rotation, large, sweep = (float(word) for word in take(5))
            end = point()
            try:
                cubics = arc_to_cubics(current, rx, ry, rotation, large, sweep, end)
            except ValueError as error:
                raise ValueError(f'{path}: {error}') from None
            segments += [('C', *cubic) for cubic in cubics]
        else:
            raise ValueError(f'{path}: unsupported path command {command}')
    if segments:
        emit()
    return geometry


def compile_svg(path):
    svg = ET.fromstring(path.read_text())
    expected = {'viewBox': '0 0 24 24', 'fill': 'none', 'stroke': 'currentColor',
                'stroke-width': '2', 'stroke-linecap': 'round', 'stroke-linejoin': 'round'}
    if svg.tag != NS + 'svg' or svg.attrib != expected:
        raise ValueError(f'{path}: unsupported icon style')
    geometry = []
    for shape in svg:
        if shape.tag == NS + 'path' and set(shape.attrib) == {'d'}:
            geometry += compile_path(path, shape.attrib['d'])
        elif shape.tag == NS + 'circle':
            filled = shape.attrib.get('fill') == 'currentColor'
            expected_keys = {'cx', 'cy', 'r'} | ({'fill', 'stroke'} if filled else set())
            if set(shape.attrib) != expected_keys or (filled and shape.attrib['stroke'] != 'none'):
                raise ValueError(f'{path}: unsupported circle')
            x, y, radius = [number(shape.attrib[key]) for key in ('cx', 'cy', 'r')]
            geometry.append(f'IconGeometry::Circle([{x}, {y}], {radius}, {str(filled).lower()})')
        else:
            raise ValueError(f'{path}: unsupported SVG element')
    if not geometry:
        raise ValueError(f'{path}: empty artwork')
    return geometry


def generate(source=SOURCE):
    paths = sorted(source.glob('*.svg'))
    variants = []
    for path in paths:
        if not re.fullmatch('[a-z]+(-[a-z]+)*', path.stem):
            raise ValueError(f'invalid icon filename: {path}')
        variants.append((''.join(word.title() for word in path.stem.split('-')), compile_svg(path)))
    lines = ['// @generated by python3 tools/generate-icons.py; do not edit.',
             '/// Bounded, project-authored artwork. Icons never supply action names.',
             '#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]',
             '#[non_exhaustive]', 'pub enum IconId {']
    lines += [f'    {name},' for name, _ in variants]
    lines += ['}', 'impl IconId {', '    /// Complete initial vocabulary in a stable display order.',
              '    pub const ALL: &\'static [Self] = &[']
    lines += [f'        Self::{name},' for name, _ in variants]
    lines += ['    ];', '    fn geometry(self) -> &\'static [IconGeometry] {', '        match self {']
    lines += [f'            Self::{name} => &[{", ".join(shapes)}],' for name, shapes in variants]
    lines += ['        }', '    }', '}']
    result = subprocess.run(['rustfmt', '--edition', '2024'], input='\n'.join(lines)+'\n',
                            text=True, capture_output=True, check=True)
    return result.stdout


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    generated = generate()
    if args.check:
        if OUTPUT.read_text() != generated:
            raise SystemExit('icon geometry drift; run python3 tools/generate-icons.py')
    else:
        OUTPUT.write_text(generated)
