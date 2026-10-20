use egui::{Color32, Painter, Rect, Stroke, epaint::CubicBezierShape};

use crate::DesignTokens;

#[derive(Debug)]
enum IconGeometry {
    /// A straight polyline, closed or open.
    Path(&'static [[f32; 2]], bool),
    /// A path with curves: its start, then segments, closed or open.
    Curve([f32; 2], &'static [IconSegment], bool),
    Circle([f32; 2], f32, bool),
}

/// One segment of a curved path, from the end of the previous one. The
/// compiler turns quadratic Béziers and elliptical arcs into cubics.
#[derive(Debug)]
enum IconSegment {
    Line([f32; 2]),
    Cubic([f32; 2], [f32; 2], [f32; 2]),
}

include!("../generated_icons.rs");

/// Theme-derived icon dimensions, independent of any interactive hit target.
pub fn icon_size(tokens: &DesignTokens, font_scale: f32) -> f32 {
    tokens.icon_size().0 * font_scale.clamp(1.0, 1.5)
}

/// Paint decorative vector artwork centred in `rect`, preserving its square aspect.
///
/// `rect` is artwork geometry, not a control allocation. This painter creates no
/// interaction or accessible owner. A named component owns those contracts.
/// Foreground can be the owning action's resolved colour or a theme status colour.
/// Geometry is compiled from project-authored SVG; there is no image/font loader,
/// runtime SVG parsing, network access or renderer-specific implementation.
pub fn paint_icon(painter: &Painter, icon: IconId, rect: Rect, foreground: Color32) {
    if !rect.is_positive() || !rect.is_finite() || !painter.is_visible() {
        return;
    }
    let scale = rect.width().min(rect.height()) / 24.0;
    let origin = rect.center() - egui::vec2(12.0, 12.0) * scale;
    let point = |xy: [f32; 2]| origin + egui::vec2(xy[0], xy[1]) * scale;
    let stroke = Stroke::new(2.0 * scale, foreground);
    for geometry in icon.geometry() {
        match geometry {
            IconGeometry::Path(points, closed) => {
                for segment in points.windows(2) {
                    painter.line_segment([point(segment[0]), point(segment[1])], stroke);
                }
                if *closed {
                    painter
                        .line_segment([point(points[points.len() - 1]), point(points[0])], stroke);
                }
                // Round caps and joins match the authored SVG style, including
                // diagonal strokes. They scale with artwork, never with hit size.
                for &centre in *points {
                    painter.circle_filled(point(centre), stroke.width * 0.5, foreground);
                }
            }
            IconGeometry::Curve(start, segments, closed) => {
                let mut current = *start;
                for segment in *segments {
                    match segment {
                        IconSegment::Line(end) => {
                            painter.line_segment([point(current), point(*end)], stroke);
                            current = *end;
                        }
                        IconSegment::Cubic(first, second, end) => {
                            painter.add(CubicBezierShape::from_points_stroke(
                                [point(current), point(*first), point(*second), point(*end)],
                                false,
                                Color32::TRANSPARENT,
                                stroke,
                            ));
                            current = *end;
                        }
                    }
                }
                if *closed && current != *start {
                    painter.line_segment([point(current), point(*start)], stroke);
                }
                // Round caps and joins at every segment's ends, as for polylines.
                painter.circle_filled(point(*start), stroke.width * 0.5, foreground);
                for segment in *segments {
                    let (IconSegment::Line(end) | IconSegment::Cubic(_, _, end)) = segment;
                    painter.circle_filled(point(*end), stroke.width * 0.5, foreground);
                }
            }
            IconGeometry::Circle(centre, radius, filled) => {
                if *filled {
                    painter.circle_filled(point(*centre), radius * scale, foreground);
                } else {
                    painter.circle_stroke(point(*centre), radius * scale, stroke);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Shape;

    #[test]
    fn vocabulary_geometry_is_bounded_monochrome_and_scales_uniformly() {
        for side in [16.0, 20.0, 24.0, 32.0] {
            let rect = Rect::from_center_size(egui::pos2(60.0, 50.0), egui::vec2(side * 2.0, side));
            let colour = Color32::from_rgb(50, 100, 150);
            for &icon in IconId::ALL {
                let context = egui::Context::default();
                let mut output = context.run_ui(Default::default(), |ui| {
                    paint_icon(ui.painter(), icon, rect, colour);
                });
                output.textures_delta.clear();
                assert!(!output.shapes.is_empty(), "{icon:?}");
                let square = Rect::from_center_size(rect.center(), egui::Vec2::splat(side));
                for shape in output.shapes {
                    assert!(
                        square
                            .expand(0.01)
                            .contains_rect(shape.shape.visual_bounding_rect()),
                        "{icon:?}: {:?}",
                        shape.shape
                    );
                    match shape.shape {
                        Shape::LineSegment { stroke, .. } => {
                            assert_eq!(stroke.width, side / 12.0);
                            assert_eq!(stroke.color, colour);
                        }
                        Shape::CubicBezier(curve) => {
                            assert_eq!(curve.stroke.width, side / 12.0);
                            assert_eq!(curve.stroke.color, egui::epaint::ColorMode::Solid(colour));
                            assert_eq!(curve.fill, Color32::TRANSPARENT);
                            assert!(!curve.closed);
                        }
                        Shape::Circle(circle) => {
                            assert!(circle.fill == colour || circle.stroke.color == colour);
                            if circle.stroke.width > 0.0 {
                                assert_eq!(circle.stroke.width, side / 12.0);
                            }
                        }
                        other => panic!("unexpected icon primitive: {other:?}"),
                    }
                }
            }
        }
    }

    /// The shapes `icon` paints at 24 points.
    fn shapes(icon: IconId) -> Vec<Shape> {
        let context = egui::Context::default();
        let rect = Rect::from_center_size(egui::pos2(20.0, 20.0), egui::Vec2::splat(24.0));
        let mut output = context.run_ui(Default::default(), |ui| {
            paint_icon(ui.painter(), icon, rect, Color32::WHITE);
        });
        output.textures_delta.clear();
        output.shapes.into_iter().map(|shape| shape.shape).collect()
    }

    #[test]
    fn chat_icons_draw_their_rounded_parts_as_curves() {
        let curves = |icon| {
            shapes(icon)
                .iter()
                .filter(|shape| matches!(shape, Shape::CubicBezier(_)))
                .count()
        };
        // The paperclip's three bends, the microphone's capsule and cradle,
        // and the person's shoulders are curves; the head is a circle.
        assert_eq!(curves(IconId::Attach), 6);
        assert_eq!(curves(IconId::Mic), 6);
        assert_eq!(curves(IconId::User), 2);
        assert!(shapes(IconId::User).iter().any(
            |shape| matches!(shape, Shape::Circle(circle) if circle.fill == Color32::TRANSPARENT)
        ));
        // Send and Copy are straight, like the earlier vocabulary.
        assert_eq!(curves(IconId::Send), 0);
        assert_eq!(curves(IconId::Copy), 0);
        // A curve's ends meet the next segment: the capsule closes on itself.
        let mic = shapes(IconId::Mic);
        let Shape::CubicBezier(first) = mic
            .iter()
            .find(|shape| matches!(shape, Shape::CubicBezier(_)))
            .unwrap()
        else {
            unreachable!()
        };
        let Shape::CubicBezier(second) = mic
            .iter()
            .filter(|shape| matches!(shape, Shape::CubicBezier(_)))
            .nth(1)
            .unwrap()
        else {
            unreachable!()
        };
        assert_eq!(first.points[3], second.points[0]);
    }
}
