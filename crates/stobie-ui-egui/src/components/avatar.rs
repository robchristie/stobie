//! Avatars: a person's initials or glyph, an application-supplied shape with
//! a face, or an image clipped to either.
//!
//! An avatar is decorative, like an icon. It publishes no semantic node and
//! no text observation: the row, heading or control beside it names the
//! person or thing it stands for.

use egui::{
    Color32, FontFamily, FontId, Mesh, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui,
    load::SizedTexture,
};

use crate::{DesignTokens, IconId, PresentationContext, paint_icon};

/// Where an avatar sits, which sets its diameter.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum AvatarSize {
    /// Beside a conversation's or page's title.
    Header,
    /// Leading a list or conversation row.
    List,
    /// In a strip of pinned people or things, where the avatar is the target.
    Strip,
}

/// Avatar diameter in points: the size's token, grown with text up to 150%.
pub fn avatar_size(tokens: &DesignTokens, size: AvatarSize, font_scale: f32) -> f32 {
    let base = match size {
        AvatarSize::Header => tokens.avatar_header_size(),
        AvatarSize::List => tokens.avatar_list_size(),
        AvatarSize::Strip => tokens.avatar_strip_size(),
    };
    let scale = if font_scale.is_finite() {
        font_scale.clamp(1.0, 1.5)
    } else {
        1.0
    };
    base.0 * scale
}

/// One segment of an avatar outline, from the end of the previous one, in
/// the unit square: `[0, 0]` is the top left and `[1, 1]` the bottom right.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OutlineSegment {
    Line([f32; 2]),
    /// A quadratic Bézier: its control point, then its end.
    Quadratic([f32; 2], [f32; 2]),
    /// A cubic Bézier: two control points, then its end.
    Cubic([f32; 2], [f32; 2], [f32; 2]),
}

impl OutlineSegment {
    const fn end(self) -> [f32; 2] {
        match self {
            Self::Line(end) | Self::Quadratic(_, end) | Self::Cubic(_, _, end) => end,
        }
    }
}

/// Why an outline cannot be drawn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AvatarOutlineError {
    /// An outline needs at least two segments to enclose an area.
    TooFewSegments,
    /// The start point is not finite or lies outside the unit square.
    StartOutsideUnitSquare,
    /// A point of this segment is not finite or lies outside the unit square.
    SegmentOutsideUnitSquare { segment: usize },
}

impl std::fmt::Display for AvatarOutlineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooFewSegments => write!(f, "an avatar outline needs at least two segments"),
            Self::StartOutsideUnitSquare => {
                write!(f, "an avatar outline's start lies outside the unit square")
            }
            Self::SegmentOutsideUnitSquare { segment } => write!(
                f,
                "avatar outline segment {segment} has a point outside the unit square"
            ),
        }
    }
}

impl std::error::Error for AvatarOutlineError {}

/// A closed shape for a generated avatar, in the unit square. The outline
/// closes itself from its last point back to its start, and may be concave.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvatarOutline<'a> {
    start: [f32; 2],
    segments: &'a [OutlineSegment],
}

const fn in_unit_square(point: [f32; 2]) -> bool {
    // NaN fails both comparisons, and infinities fail one.
    point[0] >= 0.0 && point[0] <= 1.0 && point[1] >= 0.0 && point[1] <= 1.0
}

const fn validate(start: [f32; 2], segments: &[OutlineSegment]) -> Result<(), AvatarOutlineError> {
    if segments.len() < 2 {
        return Err(AvatarOutlineError::TooFewSegments);
    }
    if !in_unit_square(start) {
        return Err(AvatarOutlineError::StartOutsideUnitSquare);
    }
    let mut index = 0;
    while index < segments.len() {
        let valid = match segments[index] {
            OutlineSegment::Line(end) => in_unit_square(end),
            OutlineSegment::Quadratic(control, end) => {
                in_unit_square(control) && in_unit_square(end)
            }
            OutlineSegment::Cubic(first, second, end) => {
                in_unit_square(first) && in_unit_square(second) && in_unit_square(end)
            }
        };
        if !valid {
            return Err(AvatarOutlineError::SegmentOutsideUnitSquare { segment: index });
        }
        index += 1;
    }
    Ok(())
}

impl<'a> AvatarOutline<'a> {
    /// An outline known when the program is written. Use it in a `const`,
    /// where an invalid outline fails the build.
    ///
    /// # Panics
    ///
    /// Panics when [`Self::try_new`] would return an error.
    pub const fn new(start: [f32; 2], segments: &'a [OutlineSegment]) -> Self {
        match validate(start, segments) {
            Ok(()) => Self { start, segments },
            Err(AvatarOutlineError::TooFewSegments) => {
                panic!("an avatar outline needs at least two segments")
            }
            Err(_) => panic!("avatar outline points must lie within the unit square"),
        }
    }

    /// An outline from data, such as an application's saved shapes.
    pub const fn try_new(
        start: [f32; 2],
        segments: &'a [OutlineSegment],
    ) -> Result<Self, AvatarOutlineError> {
        match validate(start, segments) {
            Ok(()) => Ok(Self { start, segments }),
            Err(error) => Err(error),
        }
    }

    pub const fn start(&self) -> [f32; 2] {
        self.start
    }

    pub const fn segments(&self) -> &'a [OutlineSegment] {
        self.segments
    }

    /// A circle touching all four sides of the square.
    pub const CIRCLE: AvatarOutline<'static> = AvatarOutline::new(
        [0.5, 0.0],
        &[
            OutlineSegment::Cubic([0.776_14, 0.0], [1.0, 0.223_86], [1.0, 0.5]),
            OutlineSegment::Cubic([1.0, 0.776_14], [0.776_14, 1.0], [0.5, 1.0]),
            OutlineSegment::Cubic([0.223_86, 1.0], [0.0, 0.776_14], [0.0, 0.5]),
            OutlineSegment::Cubic([0.0, 0.223_86], [0.223_86, 0.0], [0.5, 0.0]),
        ],
    );

    /// A square with corners rounded to a quarter of its side.
    pub const ROUNDED_SQUARE: AvatarOutline<'static> = AvatarOutline::new(
        [0.25, 0.0],
        &[
            OutlineSegment::Line([0.75, 0.0]),
            OutlineSegment::Cubic([0.888_07, 0.0], [1.0, 0.111_93], [1.0, 0.25]),
            OutlineSegment::Line([1.0, 0.75]),
            OutlineSegment::Cubic([1.0, 0.888_07], [0.888_07, 1.0], [0.75, 1.0]),
            OutlineSegment::Line([0.25, 1.0]),
            OutlineSegment::Cubic([0.111_93, 1.0], [0.0, 0.888_07], [0.0, 0.75]),
            OutlineSegment::Line([0.0, 0.25]),
            OutlineSegment::Cubic([0.0, 0.111_93], [0.111_93, 0.0], [0.25, 0.0]),
        ],
    );

    /// A hexagon with flat top and bottom and softened corners.
    pub const HEXAGON: AvatarOutline<'static> = AvatarOutline::new(
        [0.955, 0.577_9],
        &[
            OutlineSegment::Line([0.795, 0.855_1]),
            OutlineSegment::Quadratic([0.75, 0.933], [0.66, 0.933]),
            OutlineSegment::Line([0.34, 0.933]),
            OutlineSegment::Quadratic([0.25, 0.933], [0.205, 0.855_1]),
            OutlineSegment::Line([0.045, 0.577_9]),
            OutlineSegment::Quadratic([0.0, 0.5], [0.045, 0.422_1]),
            OutlineSegment::Line([0.205, 0.144_9]),
            OutlineSegment::Quadratic([0.25, 0.067], [0.34, 0.067]),
            OutlineSegment::Line([0.66, 0.067]),
            OutlineSegment::Quadratic([0.75, 0.067], [0.795, 0.144_9]),
            OutlineSegment::Line([0.955, 0.422_1]),
            OutlineSegment::Quadratic([1.0, 0.5], [0.955, 0.577_9]),
        ],
    );
}

/// The face drawn on a generated shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AvatarFace {
    None,
    /// Two small upright eyes either side of `centre`, in the unit square:
    /// each about a twelfth of the avatar wide and a sixth tall, with their
    /// centres a quarter of the avatar apart.
    Eyes {
        centre: [f32; 2],
    },
}

impl AvatarFace {
    /// Eyes a little above the middle, which suits the built-in shapes.
    pub const EYES: Self = Self::Eyes {
        centre: [0.5, 0.46],
    };
}

/// What an avatar shows when it has no image, or while its image loads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AvatarFigure<'a> {
    /// A person: their initials on a quiet disc, or a person glyph when the
    /// name has no letters or digits the fonts can draw.
    Person { name: &'a str },
    /// A generated shape in a solid colour, with a face.
    Shape {
        outline: AvatarOutline<'a>,
        fill: Color32,
        face: AvatarFace,
    },
}

impl AvatarFigure<'_> {
    fn outline(&self) -> AvatarOutline<'_> {
        match self {
            Self::Person { .. } => AvatarOutline::CIRCLE,
            Self::Shape { outline, .. } => *outline,
        }
    }
}

/// One avatar. With an image, the image fills the figure's outline, cropped
/// to a centred square; without one, the figure is drawn. Pass `None` while
/// an image loads or after it fails, and the figure stands in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvatarSpec<'a> {
    pub figure: AvatarFigure<'a>,
    pub size: AvatarSize,
    pub image: Option<SizedTexture>,
}

impl<'a> AvatarSpec<'a> {
    pub const fn new(figure: AvatarFigure<'a>, size: AvatarSize) -> Self {
        Self {
            figure,
            size,
            image: None,
        }
    }

    pub const fn person(name: &'a str, size: AvatarSize) -> Self {
        Self::new(AvatarFigure::Person { name }, size)
    }

    pub const fn shape(
        outline: AvatarOutline<'a>,
        fill: Color32,
        face: AvatarFace,
        size: AvatarSize,
    ) -> Self {
        Self::new(
            AvatarFigure::Shape {
                outline,
                fill,
                face,
            },
            size,
        )
    }

    pub const fn with_image(mut self, image: SizedTexture) -> Self {
        self.image = Some(image);
        self
    }
}

/// Up to two initials: the first letter or digit of the first word and of
/// the last word, in upper case. Empty when the name has none.
pub fn avatar_initials(name: &str) -> String {
    let initial = |word: &str| word.chars().find(|c| c.is_alphanumeric());
    let mut words = name.split_whitespace().filter_map(initial);
    let first = words.next();
    let last = words.last();
    first
        .into_iter()
        .chain(last)
        .flat_map(char::to_uppercase)
        .collect()
}

/// The eye colour for a fill: near-black or white, whichever contrasts more.
pub fn avatar_eye_colour(fill: Color32) -> Color32 {
    let dark = EYE_DARK;
    let light = Color32::WHITE;
    if contrast_ratio(fill, dark) >= contrast_ratio(fill, light) {
        dark
    } else {
        light
    }
}

const EYE_DARK: Color32 = Color32::from_rgb(24, 24, 28);

fn relative_luminance(colour: Color32) -> f32 {
    let channel = |value: u8| {
        let value = f32::from(value) / 255.0;
        if value <= 0.040_45 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(colour.r()) + 0.7152 * channel(colour.g()) + 0.0722 * channel(colour.b())
}

fn contrast_ratio(first: Color32, second: Color32) -> f32 {
    let (a, b) = (relative_luminance(first), relative_luminance(second));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// Paint an avatar into the largest square centred in `rect`. Like
/// [`paint_icon`], this creates no interaction or accessible owner.
pub fn paint_avatar(painter: &Painter, spec: &AvatarSpec<'_>, rect: Rect, tokens: &DesignTokens) {
    if !rect.is_positive() || !rect.is_finite() || !painter.is_visible() {
        return;
    }
    let side = rect.width().min(rect.height());
    let square = Rect::from_center_size(rect.center(), egui::Vec2::splat(side));
    let outline = spec.figure.outline();
    let polygon = flatten(outline, square);
    let triangles = triangulate(&polygon);
    let edge = Stroke::new(1.0, tokens.colours.border_decorative);
    if let Some(image) = spec
        .image
        .filter(|image| image.size.x > 0.0 && image.size.y > 0.0)
    {
        let uv = centred_square_uv(image.size);
        let mut mesh = Mesh::with_texture(image.id);
        for &point in &polygon {
            let t = (point - square.min) / side;
            mesh.vertices.push(egui::epaint::Vertex {
                pos: point,
                uv: egui::pos2(egui::lerp(uv.x_range(), t.x), egui::lerp(uv.y_range(), t.y)),
                color: Color32::WHITE,
            });
        }
        add_triangles(&mut mesh, &triangles);
        painter.add(mesh);
        painter.add(Shape::closed_line(polygon, edge));
        return;
    }
    match spec.figure {
        AvatarFigure::Person { name } => {
            fill_polygon(
                painter,
                &polygon,
                &triangles,
                tokens.colours.surface_raised.into(),
            );
            painter.add(Shape::closed_line(polygon, edge));
            paint_person(painter, name, square, tokens);
        }
        AvatarFigure::Shape { fill, face, .. } => {
            fill_polygon(painter, &polygon, &triangles, fill);
            // A hairline in the fill's own colour smooths the mesh's edge.
            painter.add(Shape::closed_line(polygon, Stroke::new(1.0, fill)));
            if let AvatarFace::Eyes { centre } = face {
                paint_eyes(painter, centre, square, avatar_eye_colour(fill));
            }
        }
    }
}

/// Allocate and paint an avatar at its size. The response senses hover
/// only; an avatar that acts belongs inside a control that names it.
pub fn avatar(
    ui: &mut Ui,
    spec: &AvatarSpec<'_>,
    tokens: &DesignTokens,
    font_scale: f32,
) -> Response {
    let side = avatar_size(tokens, spec.size, font_scale);
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(side), Sense::hover());
    paint_avatar(ui.painter(), spec, rect, tokens);
    response
}

impl PresentationContext {
    /// A decorative avatar at its size. It adds no semantic node or text
    /// observation: name the person or thing in the row, heading or control
    /// beside it.
    ///
    /// ```no_run
    /// # use stobie_ui_egui::{AvatarFace, AvatarOutline, AvatarSize, AvatarSpec, PresentationContext};
    /// # fn header(ui: &mut egui::Ui, p: &mut PresentationContext) {
    /// p.avatar(ui, &AvatarSpec::shape(
    ///     AvatarOutline::HEXAGON,
    ///     egui::Color32::from_rgb(242, 178, 51),
    ///     AvatarFace::EYES,
    ///     AvatarSize::Header,
    /// ));
    /// p.avatar(ui, &AvatarSpec::person("Ada Lovelace", AvatarSize::List));
    /// # }
    /// ```
    pub fn avatar(&mut self, ui: &mut Ui, spec: &AvatarSpec<'_>) -> Response {
        avatar(ui, spec, self.tokens(), self.font_scale())
    }
}

fn paint_person(painter: &Painter, name: &str, square: Rect, tokens: &DesignTokens) {
    let side = square.width();
    let colour: Color32 = tokens.colours.text_primary.into();
    let initials = avatar_initials(name);
    let mut font = FontId::new(
        side * 0.4,
        FontFamily::Name(crate::SEMIBOLD_FONT_FAMILY.into()),
    );
    let drawable =
        !initials.is_empty() && painter.fonts_mut(|fonts| fonts.has_glyphs(&font, &initials));
    if !drawable {
        let glyph = side * 0.6;
        paint_icon(
            painter,
            IconId::User,
            Rect::from_center_size(square.center(), egui::Vec2::splat(glyph)),
            colour,
        );
        return;
    }
    let mut galley = painter.layout_no_wrap(initials.clone(), font.clone(), colour);
    let room = side * 0.72;
    if galley.size().x > room {
        font.size *= room / galley.size().x;
        galley = painter.layout_no_wrap(initials, font, colour);
    }
    // Centre the ink rather than the line box, which carries descender room.
    let ink = galley.mesh_bounds;
    let position = if ink.is_positive() {
        square.center() - ink.center().to_vec2()
    } else {
        square.center() - galley.size() * 0.5
    };
    painter.galley(position, galley, colour);
}

fn paint_eyes(painter: &Painter, centre: [f32; 2], square: Rect, colour: Color32) {
    let side = square.width();
    let centre = square.min + egui::vec2(centre[0], centre[1]) * side;
    let width = (side * 0.085).max(1.5);
    let height = (side * 0.17).max(width);
    for offset in [-0.12, 0.12] {
        let eye = Rect::from_center_size(
            centre + egui::vec2(offset * side, 0.0),
            egui::vec2(width, height),
        );
        painter.rect_filled(eye, width * 0.5, colour);
    }
}

fn centred_square_uv(size: egui::Vec2) -> Rect {
    let aspect = size.x / size.y;
    if aspect > 1.0 {
        let half = 0.5 / aspect;
        Rect::from_min_max(egui::pos2(0.5 - half, 0.0), egui::pos2(0.5 + half, 1.0))
    } else {
        let half = 0.5 * aspect;
        Rect::from_min_max(egui::pos2(0.0, 0.5 - half), egui::pos2(1.0, 0.5 + half))
    }
}

/// The outline as points in `square`, with curves flattened finely enough
/// that no chord strays far from its curve at the avatar's size.
fn flatten(outline: AvatarOutline<'_>, square: Rect) -> Vec<Pos2> {
    let side = square.width();
    let map = |point: [f32; 2]| square.min + egui::vec2(point[0], point[1]) * side;
    let mut points = vec![map(outline.start)];
    let mut current = outline.start;
    for segment in outline.segments {
        let end = segment.end();
        match *segment {
            OutlineSegment::Line(end) => points.push(map(end)),
            OutlineSegment::Quadratic(control, _) => {
                let length = distance(current, control) + distance(control, end);
                let steps = curve_steps(length * side);
                for step in 1..=steps {
                    let t = step as f32 / steps as f32;
                    let u = 1.0 - t;
                    points.push(map(blend(&[
                        (current, u * u),
                        (control, 2.0 * u * t),
                        (end, t * t),
                    ])));
                }
            }
            OutlineSegment::Cubic(first, second, _) => {
                let length =
                    distance(current, first) + distance(first, second) + distance(second, end);
                let steps = curve_steps(length * side);
                for step in 1..=steps {
                    let t = step as f32 / steps as f32;
                    let u = 1.0 - t;
                    points.push(map(blend(&[
                        (current, u * u * u),
                        (first, 3.0 * u * u * t),
                        (second, 3.0 * u * t * t),
                        (end, t * t * t),
                    ])));
                }
            }
        }
        current = end;
    }
    // Drop repeated points, including a final point that returns to the start.
    points.dedup_by(|a, b| a.distance(*b) < 0.01);
    while points.len() > 1 && points[0].distance(points[points.len() - 1]) < 0.01 {
        points.pop();
    }
    points
}

fn curve_steps(length_in_points: f32) -> usize {
    ((length_in_points / 4.0).ceil() as usize).clamp(2, 32)
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn blend(weighted: &[([f32; 2], f32)]) -> [f32; 2] {
    weighted.iter().fold([0.0, 0.0], |sum, (point, weight)| {
        [sum[0] + point[0] * weight, sum[1] + point[1] * weight]
    })
}

fn cross(a: Pos2, b: Pos2, c: Pos2) -> f32 {
    (b - a).x * (c - a).y - (b - a).y * (c - a).x
}

fn signed_area(points: &[Pos2]) -> f32 {
    let n = points.len();
    (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f32>()
        * 0.5
}

/// Ear-clip a simple polygon in either winding into triangles of indices
/// into `points`. A self-crossing outline still yields triangles, though
/// they may overlap; it never panics or loops.
fn triangulate(points: &[Pos2]) -> Vec<[u32; 3]> {
    let count = points.len();
    if count < 3 || u32::try_from(count).is_err() {
        return Vec::new();
    }
    let area = signed_area(points);
    if !(area.abs() > 1.0e-6) {
        return Vec::new();
    }
    let winding = area.signum();
    let mut remaining: Vec<usize> = (0..count).collect();
    let mut triangles = Vec::with_capacity(count - 2);
    while remaining.len() > 3 {
        let length = remaining.len();
        let ear = (0..length).find_map(|i| {
            let (a, b, c) = (
                remaining[(i + length - 1) % length],
                remaining[i],
                remaining[(i + 1) % length],
            );
            let turn = cross(points[a], points[b], points[c]) * winding;
            if turn.abs() < 1.0e-6 {
                // A straight run: drop the middle point without a triangle.
                return Some((i, None));
            }
            if turn < 0.0 {
                return None;
            }
            let blocked = remaining.iter().any(|&other| {
                other != a
                    && other != b
                    && other != c
                    && points[other] != points[a]
                    && points[other] != points[b]
                    && points[other] != points[c]
                    && inside_triangle(points[other], points[a], points[b], points[c])
            });
            (!blocked).then_some((i, Some([a, b, c])))
        });
        // No ear is found only when the outline crosses itself; clip the
        // first corner so the loop always shrinks.
        let (index, triangle) =
            ear.unwrap_or_else(|| (0, Some([remaining[length - 1], remaining[0], remaining[1]])));
        if let Some(triangle) = triangle {
            triangles.push(triangle.map(|index| index as u32));
        }
        remaining.remove(index);
    }
    triangles.push([
        remaining[0] as u32,
        remaining[1] as u32,
        remaining[2] as u32,
    ]);
    triangles
}

fn inside_triangle(point: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
    let first = cross(a, b, point);
    let second = cross(b, c, point);
    let third = cross(c, a, point);
    let negative = first < 0.0 || second < 0.0 || third < 0.0;
    let positive = first > 0.0 || second > 0.0 || third > 0.0;
    !(negative && positive)
}

fn add_triangles(mesh: &mut Mesh, triangles: &[[u32; 3]]) {
    for &[a, b, c] in triangles {
        mesh.add_triangle(a, b, c);
    }
}

fn fill_polygon(painter: &Painter, points: &[Pos2], triangles: &[[u32; 3]], fill: Color32) {
    let mut mesh = Mesh::default();
    for &point in points {
        mesh.colored_vertex(point, fill);
    }
    add_triangles(&mut mesh, triangles);
    painter.add(mesh);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DensityVariant, ThemeVariant};

    fn tokens() -> DesignTokens {
        DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable)
    }

    fn paint(spec: &AvatarSpec<'_>, side: f32) -> Vec<Shape> {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        let mut shapes = Vec::new();
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("avatar-test"));
            let painter = Painter::new(ui.ctx().clone(), layer, Rect::EVERYTHING);
            let rect = Rect::from_min_size(egui::pos2(10.0, 10.0), egui::Vec2::splat(side));
            paint_avatar(&painter, spec, rect, &tokens());
            ui.ctx().graphics(|graphics| {
                if let Some(list) = graphics.get(layer) {
                    shapes = list
                        .all_entries()
                        .map(|clipped| clipped.shape.clone())
                        .collect();
                }
            });
        });
        output.textures_delta.clear();
        shapes
    }

    fn mesh_area(mesh: &Mesh) -> f32 {
        mesh.indices
            .chunks(3)
            .map(|t| {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| mesh.vertices[i as usize].pos);
                cross(a, b, c).abs() * 0.5
            })
            .sum()
    }

    #[test]
    fn sizes_follow_their_tokens_and_grow_with_text_up_to_half_again() {
        let tokens = tokens();
        assert_eq!(avatar_size(&tokens, AvatarSize::Header, 1.0), 32.0);
        assert_eq!(avatar_size(&tokens, AvatarSize::List, 1.0), 40.0);
        assert_eq!(avatar_size(&tokens, AvatarSize::Strip, 1.0), 56.0);
        assert_eq!(avatar_size(&tokens, AvatarSize::List, 1.25), 50.0);
        assert_eq!(avatar_size(&tokens, AvatarSize::List, 3.0), 60.0);
        assert_eq!(avatar_size(&tokens, AvatarSize::List, 0.5), 40.0);
        assert_eq!(avatar_size(&tokens, AvatarSize::List, f32::NAN), 40.0);
    }

    #[test]
    fn initials_take_the_first_and_last_words_first_letters_or_digits() {
        assert_eq!(avatar_initials("Ada Lovelace"), "AL");
        assert_eq!(avatar_initials("  grace   brewster murray hopper "), "GH");
        assert_eq!(avatar_initials("Cher"), "C");
        assert_eq!(avatar_initials("(Rob) \"the builder\" 2"), "R2");
        assert_eq!(avatar_initials("émile zola"), "ÉZ");
        assert_eq!(avatar_initials("straße"), "S");
        assert_eq!(avatar_initials("ßen"), "SS");
        assert_eq!(avatar_initials(""), "");
        assert_eq!(avatar_initials("🙂 🎉"), "");
        assert_eq!(avatar_initials("— · —"), "");
    }

    #[test]
    fn outlines_from_data_are_validated_and_constants_are_valid() {
        for outline in [
            AvatarOutline::CIRCLE,
            AvatarOutline::ROUNDED_SQUARE,
            AvatarOutline::HEXAGON,
        ] {
            assert_eq!(
                AvatarOutline::try_new(outline.start(), outline.segments()),
                Ok(outline)
            );
        }
        let line = [OutlineSegment::Line([1.0, 1.0])];
        assert_eq!(
            AvatarOutline::try_new([0.0, 0.0], &line),
            Err(AvatarOutlineError::TooFewSegments)
        );
        let two = [
            OutlineSegment::Line([1.0, 0.0]),
            OutlineSegment::Line([1.0, 1.0]),
        ];
        assert!(AvatarOutline::try_new([0.0, 0.0], &two).is_ok());
        for start in [
            [-0.1, 0.0],
            [0.0, 1.1],
            [f32::NAN, 0.5],
            [f32::INFINITY, 0.5],
        ] {
            assert_eq!(
                AvatarOutline::try_new(start, &two),
                Err(AvatarOutlineError::StartOutsideUnitSquare)
            );
        }
        let bad = [
            OutlineSegment::Line([1.0, 0.0]),
            OutlineSegment::Cubic([1.0, 0.5], [1.2, 0.5], [1.0, 1.0]),
        ];
        assert_eq!(
            AvatarOutline::try_new([0.0, 0.0], &bad),
            Err(AvatarOutlineError::SegmentOutsideUnitSquare { segment: 1 })
        );
        let bad = [
            OutlineSegment::Quadratic([0.5, f32::NAN], [1.0, 0.0]),
            OutlineSegment::Line([1.0, 1.0]),
        ];
        assert_eq!(
            AvatarOutline::try_new([0.0, 0.0], &bad),
            Err(AvatarOutlineError::SegmentOutsideUnitSquare { segment: 0 })
        );
        assert!(
            AvatarOutlineError::SegmentOutsideUnitSquare { segment: 3 }
                .to_string()
                .contains("segment 3")
        );
    }

    #[test]
    #[should_panic(expected = "unit square")]
    fn new_rejects_an_invalid_outline() {
        let bad = [
            OutlineSegment::Line([2.0, 0.0]),
            OutlineSegment::Line([1.0, 1.0]),
        ];
        let _ = AvatarOutline::new([0.0, 0.0], &bad);
    }

    #[test]
    fn ear_clipping_fills_concave_outlines_exactly_in_either_winding() {
        // A crescent-like notch: an L shape of area 0.75.
        let clockwise = [
            Pos2::new(0.0, 0.0),
            Pos2::new(1.0, 0.0),
            Pos2::new(1.0, 1.0),
            Pos2::new(0.5, 1.0),
            Pos2::new(0.5, 0.5),
            Pos2::new(0.0, 0.5),
        ];
        let mut counter = clockwise;
        counter.reverse();
        for polygon in [clockwise, counter] {
            let triangles = triangulate(&polygon);
            assert!((3..=4).contains(&triangles.len()), "{triangles:?}");
            let area: f32 = triangles
                .iter()
                .map(|t| {
                    cross(
                        polygon[t[0] as usize],
                        polygon[t[1] as usize],
                        polygon[t[2] as usize],
                    )
                    .abs()
                        * 0.5
                })
                .sum();
            assert!((area - 0.75).abs() < 1.0e-5, "{area}");
            // Every triangle lies inside the L: none covers the notch.
            for t in &triangles {
                let centroid = (polygon[t[0] as usize].to_vec2()
                    + polygon[t[1] as usize].to_vec2()
                    + polygon[t[2] as usize].to_vec2())
                    / 3.0;
                assert!(!(centroid.x < 0.5 && centroid.y > 0.5), "{t:?}");
            }
        }
        // Straight runs, degenerate and self-crossing outlines never panic.
        let straight = [
            Pos2::new(0.0, 0.0),
            Pos2::new(0.5, 0.0),
            Pos2::new(1.0, 0.0),
            Pos2::new(0.0, 1.0),
        ];
        let area: f32 = triangulate(&straight)
            .iter()
            .map(|t| {
                cross(
                    straight[t[0] as usize],
                    straight[t[1] as usize],
                    straight[t[2] as usize],
                )
                .abs()
                    * 0.5
            })
            .sum();
        assert!((area - 0.5).abs() < 1.0e-6, "{area}");
        assert!(triangulate(&[Pos2::ZERO, Pos2::new(1.0, 1.0), Pos2::new(2.0, 2.0)]).is_empty());
        let bow_tie = [
            Pos2::new(0.0, 0.0),
            Pos2::new(1.0, 1.0),
            Pos2::new(1.0, 0.0),
            Pos2::new(0.0, 1.0),
            Pos2::new(0.5, 0.2),
        ];
        assert!(triangulate(&bow_tie).len() <= 3);
    }

    #[test]
    fn a_shape_fills_its_outline_with_its_colour_and_contrasting_eyes() {
        let blob = [
            OutlineSegment::Cubic([0.9, 0.0], [1.0, 0.4], [0.8, 0.6]),
            OutlineSegment::Cubic([0.6, 0.8], [0.7, 1.0], [0.4, 1.0]),
            OutlineSegment::Cubic([0.1, 1.0], [0.0, 0.6], [0.2, 0.4]),
            OutlineSegment::Cubic([0.35, 0.25], [0.1, 0.0], [0.5, 0.05]),
        ];
        let outline = AvatarOutline::new([0.5, 0.05], &blob);
        let fill = Color32::from_rgb(250, 210, 60);
        let shapes = paint(
            &AvatarSpec::shape(outline, fill, AvatarFace::EYES, AvatarSize::List),
            40.0,
        );
        let mesh = shapes
            .iter()
            .find_map(|shape| match shape {
                Shape::Mesh(mesh) => Some(mesh.clone()),
                _ => None,
            })
            .expect("filled mesh");
        assert!(mesh.vertices.iter().all(|v| v.color == fill));
        let flattened: Vec<Pos2> = mesh.vertices.iter().map(|v| v.pos).collect();
        let expected = signed_area(&flattened).abs();
        assert!(expected > 300.0, "{expected}");
        assert!(
            (mesh_area(&mesh) - expected).abs() < 0.5,
            "{}",
            mesh_area(&mesh)
        );
        let eyes: Vec<_> = shapes
            .iter()
            .filter_map(|shape| match shape {
                Shape::Rect(rect) => Some(rect),
                _ => None,
            })
            .collect();
        assert_eq!(eyes.len(), 2);
        assert!(eyes.iter().all(|eye| eye.fill == EYE_DARK));
        assert!(eyes[0].rect.center().x < eyes[1].rect.center().x);
        assert!(eyes.iter().all(|eye| eye.rect.height() > eye.rect.width()));

        assert_eq!(
            avatar_eye_colour(Color32::from_rgb(40, 60, 160)),
            Color32::WHITE
        );
        assert_eq!(avatar_eye_colour(Color32::from_rgb(250, 210, 60)), EYE_DARK);
        assert_eq!(avatar_eye_colour(Color32::BLACK), Color32::WHITE);
        let none = paint(
            &AvatarSpec::shape(
                AvatarOutline::HEXAGON,
                fill,
                AvatarFace::None,
                AvatarSize::List,
            ),
            40.0,
        );
        assert!(!none.iter().any(|shape| matches!(shape, Shape::Rect(_))));
    }

    #[test]
    fn a_person_shows_initials_or_falls_back_to_the_person_glyph() {
        let text_of = |shapes: &[Shape]| {
            shapes.iter().find_map(|shape| match shape {
                Shape::Text(text) => Some((text.galley.text().to_owned(), text.clone())),
                _ => None,
            })
        };
        let side = 40.0;
        let shapes = paint(&AvatarSpec::person("Ada Lovelace", AvatarSize::List), side);
        let (text, shape) = text_of(&shapes).expect("initials");
        assert_eq!(text, "AL");
        let ink = shape.visual_bounding_rect();
        let centre = egui::pos2(10.0, 10.0) + egui::Vec2::splat(side * 0.5);
        assert!(ink.center().distance(centre) < 1.5, "{ink:?}");
        assert!(ink.width() <= side * 0.72 + 0.5);
        let disc = shapes
            .iter()
            .find_map(|shape| match shape {
                Shape::Mesh(mesh) => Some(mesh.clone()),
                _ => None,
            })
            .expect("disc");
        let surface: Color32 = tokens().colours.surface_raised.into();
        assert!(disc.vertices.iter().all(|v| v.color == surface));

        // Wide initials shrink to fit.
        let (_, wide) = text_of(&paint(
            &AvatarSpec::person("Wmmm Wmmm", AvatarSize::List),
            side,
        ))
        .expect("wide initials");
        assert!(wide.visual_bounding_rect().width() <= side * 0.72 + 0.5);

        for name in ["", "🙂", "· ·", "王小明"] {
            let shapes = paint(&AvatarSpec::person(name, AvatarSize::List), side);
            assert!(text_of(&shapes).is_none(), "{name:?}");
            // The person glyph's head is a stroked circle.
            assert!(
                shapes
                    .iter()
                    .any(|shape| matches!(shape, Shape::Circle(c) if c.stroke.width > 0.0)),
                "{name:?}"
            );
        }
    }

    #[test]
    fn an_image_fills_the_outline_cropped_to_a_centred_square() {
        let texture = egui::TextureId::User(7);
        let wide = SizedTexture::new(texture, egui::vec2(200.0, 100.0));
        let side = 40.0;
        let spec = AvatarSpec::person("Ada Lovelace", AvatarSize::List).with_image(wide);
        let shapes = paint(&spec, side);
        assert!(!shapes.iter().any(|shape| matches!(shape, Shape::Text(_))));
        let mesh = shapes
            .iter()
            .find_map(|shape| match shape {
                Shape::Mesh(mesh) if mesh.texture_id == texture => Some(mesh.clone()),
                _ => None,
            })
            .expect("image mesh");
        let (min_u, max_u) = mesh
            .vertices
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), v| {
                (lo.min(v.uv.x), hi.max(v.uv.x))
            });
        let (min_v, max_v) = mesh
            .vertices
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), v| {
                (lo.min(v.uv.y), hi.max(v.uv.y))
            });
        assert!((min_u - 0.25).abs() < 1.0e-3 && (max_u - 0.75).abs() < 1.0e-3);
        assert!(min_v.abs() < 1.0e-3 && (max_v - 1.0).abs() < 1.0e-3);
        // The circle clips the image: its area is about πr².
        let area = mesh_area(&mesh);
        assert!((area - std::f32::consts::PI * 400.0).abs() < 8.0, "{area}");

        let tall = centred_square_uv(egui::vec2(50.0, 100.0));
        assert_eq!(
            tall,
            Rect::from_min_max(egui::pos2(0.0, 0.25), egui::pos2(1.0, 0.75))
        );

        // An empty image is ignored and the figure stands in.
        let empty = AvatarSpec::person("Ada", AvatarSize::List)
            .with_image(SizedTexture::new(texture, egui::Vec2::ZERO));
        assert!(
            paint(&empty, side)
                .iter()
                .any(|shape| matches!(shape, Shape::Text(_)))
        );
    }

    #[test]
    fn painting_ignores_empty_or_non_finite_rectangles() {
        let spec = AvatarSpec::person("Ada", AvatarSize::List);
        assert!(paint(&spec, 0.0).is_empty());
        assert!(paint(&spec, f32::NAN).is_empty());
    }

    #[test]
    fn the_presented_avatar_allocates_its_square_and_publishes_no_semantics() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        let tokens = tokens();
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            let mut presentation = PresentationContext::new(
                ui,
                tokens,
                1.25,
                crate::PresentationScope::new("avatar-test"),
                crate::SemanticUiId::root(),
            );
            let response = presentation.avatar(
                ui,
                &AvatarSpec::shape(
                    AvatarOutline::ROUNDED_SQUARE,
                    Color32::from_rgb(90, 160, 240),
                    AvatarFace::EYES,
                    AvatarSize::Strip,
                ),
            );
            assert_eq!(response.rect.size(), egui::Vec2::splat(70.0));
            let published = presentation.finish(ui);
            assert!(published.semantic_nodes.is_empty());
            assert!(published.text_layouts.is_empty());
        });
        output.textures_delta.clear();
    }
}
